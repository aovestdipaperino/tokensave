use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::{
    AgentIntegration, ClaudeIntegration, DoctorCounters, HealthcheckContext, InstallContext,
    EXPECTED_TOOL_PERMS,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_install_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: EXPECTED_TOOL_PERMS,
    }
}

/// Creates a fake tokensave binary in a temp dir so healthcheck binary-exists
/// checks pass.
fn make_install_ctx_with_real_bin(home: &Path) -> InstallContext {
    let bin_dir = home.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let bin_path = bin_dir.join("tokensave");
    std::fs::write(&bin_path, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: bin_path.to_string_lossy().to_string(),
        tool_permissions: EXPECTED_TOOL_PERMS,
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_claude_json_with_mcp_server() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let claude_json = read_json(&home.join(".claude.json"));
    let ts = &claude_json["mcpServers"]["tokensave"];
    assert!(ts.is_object(), "mcpServers.tokensave should be an object");
    assert_eq!(
        ts["command"].as_str().unwrap(),
        "/usr/local/bin/tokensave",
        "command should match the bin path"
    );
    let args: Vec<&str> = ts["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(args, vec!["serve"], "args should be [\"serve\"]");
}

#[test]
fn test_install_creates_settings_with_hook() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let settings = read_json(&home.join(".claude/settings.json"));
    let hooks = settings["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse should be an array");

    let tokensave_hook = hooks.iter().find(|h| {
        h.get("matcher").and_then(|m| m.as_str()) == Some("Agent")
            && h.get("hooks")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter().any(|entry| {
                        entry
                            .get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(|c| c.contains("tokensave"))
                    })
                })
                .unwrap_or(false)
    });
    assert!(
        tokensave_hook.is_some(),
        "PreToolUse should contain a hook with matcher=Agent and command containing tokensave"
    );

    // Verify the hook command format
    let hook = tokensave_hook.unwrap();
    let cmd = hook["hooks"][0]["command"].as_str().unwrap();
    assert!(
        cmd.contains("tokensave hook-pre-tool-use"),
        "hook command should contain 'tokensave hook-pre-tool-use', got: {cmd}"
    );
}

#[test]
fn test_install_creates_settings_with_permissions() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let settings = read_json(&home.join(".claude/settings.json"));
    let allow = settings["permissions"]["allow"]
        .as_array()
        .expect("permissions.allow should be an array");
    let allow_strs: Vec<&str> = allow.iter().filter_map(|v| v.as_str()).collect();

    for perm in EXPECTED_TOOL_PERMS {
        assert!(
            allow_strs.contains(perm),
            "permissions.allow should contain {perm}"
        );
    }
}

#[test]
fn test_install_creates_claude_md_with_rules() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let claude_md = std::fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap();
    assert!(
        claude_md.contains("## MANDATORY: No Explore Agents When Tokensave Is Available"),
        "CLAUDE.md should contain the mandatory rules marker"
    );
    assert!(
        claude_md.contains("tokensave_context"),
        "CLAUDE.md should mention tokensave tools"
    );
    assert!(
        claude_md.contains("NEVER use Agent(subagent_type=Explore)"),
        "CLAUDE.md should contain the no-explore-agent rule"
    );
    assert!(
        claude_md.contains("When you spawn an Explore agent"),
        "CLAUDE.md should contain the explore agent guidance paragraph"
    );
    assert!(
        claude_md.contains("exclude_node_ids"),
        "CLAUDE.md should mention exclude_node_ids for dedup"
    );
}

#[test]
fn test_claude_md_contains_explore_agent_paragraph() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate CLAUDE.md with existing content
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("CLAUDE.md"), "# Existing content\n").unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(
        content.contains("When you spawn an Explore agent"),
        "should contain explore agent paragraph"
    );
    assert!(
        content.contains("tokensave_context"),
        "should reference tokensave_context as the tool"
    );
    assert!(
        content.contains("exclude_node_ids"),
        "should mention exclude_node_ids for dedup"
    );
}

#[test]
fn test_uninstall_removes_explore_agent_paragraph() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate CLAUDE.md with existing content
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("CLAUDE.md"), "# My Rules\n\nKeep it clean.\n").unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Verify install added the explore agent paragraph
    let content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(content.contains("When you spawn an Explore agent"));

    // Now uninstall
    ClaudeIntegration.uninstall(&ctx).unwrap();

    let content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(
        !content.contains("When you spawn an Explore agent"),
        "explore agent paragraph should be removed after uninstall"
    );
    assert!(
        content.contains("My Rules"),
        "existing content should be preserved after uninstall"
    );
}

#[test]
fn test_install_idempotent_claude_md() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    ClaudeIntegration.install(&ctx).unwrap();
    ClaudeIntegration.install(&ctx).unwrap();

    let claude_md = std::fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap();
    let marker = "## MANDATORY: No Explore Agents When Tokensave Is Available";
    let count = claude_md.matches(marker).count();
    assert_eq!(
        count, 1,
        "marker should appear exactly once after double install, found {count}"
    );
}

#[test]
fn test_install_preserves_existing_claude_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate .claude.json with an extra key
    std::fs::write(
        home.join(".claude.json"),
        r#"{"foo": "bar"}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let claude_json = read_json(&home.join(".claude.json"));
    assert_eq!(
        claude_json["foo"].as_str().unwrap(),
        "bar",
        "existing key 'foo' should be preserved"
    );
    assert!(
        claude_json["mcpServers"]["tokensave"].is_object(),
        "mcpServers.tokensave should be added alongside existing keys"
    );
}

#[test]
fn test_install_preserves_existing_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate settings.json with an existing hook
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "echo hello"}]
      }
    ]
  }
}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let settings = read_json(&claude_dir.join("settings.json"));
    let hooks = settings["hooks"]["PreToolUse"].as_array().unwrap();

    // Should have both the existing Bash hook and the new Agent hook
    let has_bash = hooks
        .iter()
        .any(|h| h.get("matcher").and_then(|m| m.as_str()) == Some("Bash"));
    let has_agent = hooks
        .iter()
        .any(|h| h.get("matcher").and_then(|m| m.as_str()) == Some("Agent"));
    assert!(has_bash, "existing Bash hook should be preserved");
    assert!(has_agent, "new Agent hook should be added");
}

#[test]
fn test_install_migrates_old_mcp_from_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate settings.json with old-location MCP server
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{
  "mcpServers": {
    "tokensave": {
      "command": "/old/path/tokensave",
      "args": ["serve"]
    }
  }
}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // settings.json should NOT have mcpServers.tokensave anymore
    let settings = read_json(&claude_dir.join("settings.json"));
    let has_stale = settings
        .get("mcpServers")
        .and_then(|v| v.get("tokensave"))
        .is_some();
    assert!(
        !has_stale,
        "tokensave MCP server should be removed from settings.json (old location)"
    );

    // .claude.json should have it in the new location
    let claude_json = read_json(&home.join(".claude.json"));
    assert!(
        claude_json["mcpServers"]["tokensave"].is_object(),
        "tokensave MCP server should exist in .claude.json (new location)"
    );
    assert_eq!(
        claude_json["mcpServers"]["tokensave"]["command"]
            .as_str()
            .unwrap(),
        "/usr/local/bin/tokensave",
        "MCP command should use the new bin path, not the old one"
    );
}

// ===========================================================================
// Uninstall content verification
// ===========================================================================

#[test]
fn test_uninstall_removes_mcp_from_claude_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();
    ClaudeIntegration.uninstall(&ctx).unwrap();

    // File may be deleted (empty) or exist without tokensave
    let path = home.join(".claude.json");
    if path.exists() {
        let claude_json = read_json(&path);
        let has_tokensave = claude_json
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(
            !has_tokensave,
            "mcpServers.tokensave should be gone after uninstall"
        );
    }
}

#[test]
fn test_uninstall_removes_empty_claude_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    // Install (creates .claude.json with only mcpServers.tokensave)
    ClaudeIntegration.install(&ctx).unwrap();
    assert!(home.join(".claude.json").exists());

    ClaudeIntegration.uninstall(&ctx).unwrap();

    // Since the only content was tokensave, file should be deleted
    assert!(
        !home.join(".claude.json").exists(),
        ".claude.json should be deleted when it becomes empty after uninstall"
    );
}

#[test]
fn test_uninstall_removes_hook_from_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();
    ClaudeIntegration.uninstall(&ctx).unwrap();

    let settings_path = home.join(".claude/settings.json");
    if settings_path.exists() {
        let settings = read_json(&settings_path);
        let has_hook = settings["hooks"]["PreToolUse"]
            .as_array()
            .map(|arr| {
                arr.iter().any(|h| {
                    h.get("hooks")
                        .and_then(|a| a.as_array())
                        .map(|arr| {
                            arr.iter().any(|entry| {
                                entry
                                    .get("command")
                                    .and_then(|c| c.as_str())
                                    .is_some_and(|c| c.contains("tokensave"))
                            })
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        assert!(
            !has_hook,
            "PreToolUse should not contain tokensave hook after uninstall"
        );
    }
}

#[test]
fn test_uninstall_removes_permissions_from_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();
    ClaudeIntegration.uninstall(&ctx).unwrap();

    let settings_path = home.join(".claude/settings.json");
    if settings_path.exists() {
        let settings = read_json(&settings_path);
        let has_ts_perm = settings["permissions"]["allow"]
            .as_array()
            .map(|arr| {
                arr.iter().any(|v| {
                    v.as_str()
                        .is_some_and(|s| s.starts_with("mcp__tokensave__"))
                })
            })
            .unwrap_or(false);
        assert!(
            !has_ts_perm,
            "permissions.allow should not contain mcp__tokensave__* after uninstall"
        );
    }
}

#[test]
fn test_uninstall_preserves_other_permissions() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Install first so all files are set up
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Now add a non-tokensave permission to settings.json
    let settings_path = home.join(".claude/settings.json");
    let mut settings = read_json(&settings_path);
    let allow = settings["permissions"]["allow"]
        .as_array_mut()
        .unwrap();
    allow.push(serde_json::json!("Bash(*)"));
    let pretty = serde_json::to_string_pretty(&settings).unwrap();
    std::fs::write(&settings_path, format!("{pretty}\n")).unwrap();

    ClaudeIntegration.uninstall(&ctx).unwrap();

    let settings = read_json(&settings_path);
    let allow = settings["permissions"]["allow"]
        .as_array()
        .expect("permissions.allow should still exist");
    let strs: Vec<&str> = allow.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        strs.contains(&"Bash(*)"),
        "non-tokensave permission 'Bash(*)' should be preserved, got: {strs:?}"
    );
    assert!(
        !strs.iter().any(|s| s.starts_with("mcp__tokensave__")),
        "tokensave permissions should be removed"
    );
}

#[test]
fn test_uninstall_removes_claude_md_rules() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let claude_md_path = home.join(".claude/CLAUDE.md");
    assert!(claude_md_path.exists());

    ClaudeIntegration.uninstall(&ctx).unwrap();

    // CLAUDE.md had only tokensave rules, should be removed
    if claude_md_path.exists() {
        let content = std::fs::read_to_string(&claude_md_path).unwrap();
        assert!(
            !content.contains("MANDATORY: No Explore Agents When Tokensave Is Available"),
            "CLAUDE.md should not contain tokensave marker after uninstall"
        );
    }
}

#[test]
fn test_uninstall_preserves_other_claude_md_content() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create CLAUDE.md with pre-existing content
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("CLAUDE.md"),
        "## My Custom Rules\n\nAlways write tests.\n",
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Verify install appended rules
    let md_content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(md_content.contains("My Custom Rules"));
    assert!(md_content.contains("MANDATORY: No Explore Agents"));

    ClaudeIntegration.uninstall(&ctx).unwrap();

    // After uninstall, custom content should remain
    let md_content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(
        md_content.contains("My Custom Rules"),
        "custom content should be preserved after uninstall"
    );
    assert!(
        md_content.contains("Always write tests"),
        "custom content body should be preserved"
    );
    assert!(
        !md_content.contains("MANDATORY: No Explore Agents"),
        "tokensave marker should be removed after uninstall"
    );
}

// ===========================================================================
// Healthcheck verification
// ===========================================================================

#[test]
fn test_healthcheck_detects_missing_claude_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should detect missing .claude.json"
    );
}

#[test]
fn test_healthcheck_detects_missing_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create .claude.json with MCP server but no settings.json
    std::fs::write(
        home.join(".claude.json"),
        r#"{"mcpServers":{"tokensave":{"command":"/usr/local/bin/tokensave","args":["serve"]}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);

    // Should detect missing settings.json (hooks/permissions) and missing CLAUDE.md
    assert!(
        dc.issues > 0 || dc.warnings > 0,
        "healthcheck should detect missing settings.json"
    );
}

#[test]
fn test_healthcheck_detects_missing_permissions() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create .claude.json with MCP server
    std::fs::write(
        home.join(".claude.json"),
        r#"{"mcpServers":{"tokensave":{"command":"/usr/local/bin/tokensave","args":["serve"]}}}"#,
    )
    .unwrap();

    // Create settings.json with hook but NO permissions
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Agent",
        "hooks": [{"type": "command", "command": "/usr/local/bin/tokensave hook-pre-tool-use"}]
      }
    ]
  }
}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should detect missing permissions"
    );
}

#[test]
fn test_healthcheck_detects_stale_permissions() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx_with_real_bin(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Add a stale permission that is not in EXPECTED_TOOL_PERMS
    let settings_path = home.join(".claude/settings.json");
    let mut settings = read_json(&settings_path);
    let allow = settings["permissions"]["allow"].as_array_mut().unwrap();
    allow.push(serde_json::json!("mcp__tokensave__fake_tool"));
    let pretty = serde_json::to_string_pretty(&settings).unwrap();
    std::fs::write(&settings_path, format!("{pretty}\n")).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0,
        "healthcheck should warn about stale permissions"
    );
}

#[test]
fn test_healthcheck_detects_missing_claude_md() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx_with_real_bin(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Delete CLAUDE.md
    std::fs::remove_file(home.join(".claude/CLAUDE.md")).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0,
        "healthcheck should warn about missing CLAUDE.md"
    );
}

#[test]
fn test_healthcheck_clean_local_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let project = dir.path().join("myproject");
    std::fs::create_dir_all(&project).unwrap();

    // Create a local .mcp.json with tokensave
    std::fs::write(
        project.join(".mcp.json"),
        r#"{"mcpServers":{"tokensave":{"command":"/usr/local/bin/tokensave","args":["serve"]}}}"#,
    )
    .unwrap();

    // Install in home so healthcheck doesn't fail on missing global config
    let ctx = make_install_ctx_with_real_bin(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: project.clone(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);

    // The local .mcp.json should be cleaned up (removed entirely since tokensave
    // was the only entry)
    assert!(
        !project.join(".mcp.json").exists(),
        "local .mcp.json should be removed after healthcheck cleanup"
    );
}

#[test]
fn test_healthcheck_local_settings_cleanup() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let project = dir.path().join("myproject");
    let local_claude = project.join(".claude");
    std::fs::create_dir_all(&local_claude).unwrap();

    // Create local settings.local.json with tokensave entries
    std::fs::write(
        local_claude.join("settings.local.json"),
        r#"{
  "enableAllProjectMcpServers": false,
  "enabledMcpjsonServers": ["tokensave"],
  "mcpServers": {
    "tokensave": {
      "command": "/usr/local/bin/tokensave",
      "args": ["serve"]
    }
  }
}"#,
    )
    .unwrap();

    // Install in home so healthcheck doesn't fail on missing global config
    let ctx = make_install_ctx_with_real_bin(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: project.clone(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);

    // The local settings.local.json should be cleaned up
    // (removed entirely since tokensave was the only content that mattered)
    assert!(
        !local_claude.join("settings.local.json").exists(),
        "settings.local.json should be removed after healthcheck cleanup"
    );
}

// ===========================================================================
// is_detected / has_tokensave
// ===========================================================================

#[test]
fn test_has_tokensave_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    assert!(
        ClaudeIntegration.has_tokensave(home),
        "has_tokensave should return true after install"
    );
}

#[test]
fn test_has_tokensave_without_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    assert!(
        !ClaudeIntegration.has_tokensave(home),
        "has_tokensave should return false without install"
    );
}
