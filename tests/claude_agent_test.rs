use tempfile::TempDir;
use tokensave::agents::{
    expected_tool_perms, AgentIntegration, ClaudeIntegration, DoctorCounters, HealthcheckContext,
};

mod common;
use common::{make_install_ctx, make_install_ctx_with_real_bin, read_json};

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
fn test_reinstall_preserves_existing_resolvable_command() {
    // Issue #161: a user-chosen MCP command that still resolves to a
    // tokensave binary must survive reinstall instead of being overwritten
    // with this install's absolute path.
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // A fake tokensave binary at a user-chosen location.
    let user_bin = home.join("mybin").join("tokensave");
    std::fs::create_dir_all(user_bin.parent().unwrap()).unwrap();
    std::fs::write(&user_bin, "").unwrap();
    let user_bin = user_bin.to_string_lossy().to_string();

    // Pre-seed .claude.json with the user's command. Built with serde so
    // Windows backslash paths are escaped correctly.
    let seeded = serde_json::json!({
        "mcpServers": {"tokensave": {"command": user_bin, "args": ["serve"]}}
    });
    std::fs::write(home.join(".claude.json"), seeded.to_string()).unwrap();

    let ctx = make_install_ctx(home); // installs with /usr/local/bin/tokensave
    ClaudeIntegration.install(&ctx).unwrap();

    let claude_json = read_json(&home.join(".claude.json"));
    assert_eq!(
        claude_json["mcpServers"]["tokensave"]["command"]
            .as_str()
            .unwrap(),
        user_bin,
        "existing resolvable command should be preserved on reinstall"
    );
}

#[test]
fn test_reinstall_replaces_stale_command() {
    // A previous command that no longer exists must be replaced.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::write(
        home.join(".claude.json"),
        r#"{"mcpServers": {"tokensave": {"command": "/gone/tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let claude_json = read_json(&home.join(".claude.json"));
    assert_eq!(
        claude_json["mcpServers"]["tokensave"]["command"]
            .as_str()
            .unwrap(),
        "/usr/local/bin/tokensave",
        "stale command should be replaced with the new bin path"
    );
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
        h.get("matcher").and_then(|m| m.as_str()) == Some("Agent|Grep|Bash")
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
        "PreToolUse should contain a hook with matcher=Agent|Grep|Bash and command containing tokensave"
    );

    // Verify the hook command format (issue #81: modern args[] shape).
    let hook = tokensave_hook.unwrap();
    let inner = &hook["hooks"][0];
    let cmd = inner["command"].as_str().unwrap();
    assert!(
        cmd.contains("tokensave"),
        "hook command should be the tokensave exe path, got: {cmd}"
    );
    let args: Vec<&str> = inner["args"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        args,
        vec!["hook-pre-tool-use"],
        "subcommand must live in args[], not concatenated into command"
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

    for perm in expected_tool_perms() {
        assert!(
            allow_strs.contains(&perm.as_str()),
            "permissions.allow should contain {perm}"
        );
    }
}

#[test]
fn test_install_writes_single_wildcard_entry_when_requested() {
    // Opt-in compact install (`--wildcard-permissions` / the
    // `wildcard_permissions` config field): a single "mcp__tokensave__*"
    // entry should be written instead of every tool individually.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let mut ctx = make_install_ctx(home);
    ctx.tool_permissions = tokensave::agents::install_tool_perms(true);
    ctx.force_permission_style = true; // represents `--wildcard-permissions`
    ClaudeIntegration.install(&ctx).unwrap();

    let settings = read_json(&home.join(".claude/settings.json"));
    let allow: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .expect("permissions.allow should be an array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        allow,
        vec!["mcp__tokensave__*"],
        "wildcard install should write exactly one compact entry"
    );
}

#[test]
fn test_reinstall_switching_from_explicit_to_wildcard_prunes_old_entries() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home); // explicit per-tool list
    ClaudeIntegration.install(&ctx).unwrap();

    let mut wildcard_ctx = make_install_ctx(home);
    wildcard_ctx.tool_permissions = tokensave::agents::install_tool_perms(true);
    wildcard_ctx.force_permission_style = true; // represents `--wildcard-permissions`
    ClaudeIntegration.install(&wildcard_ctx).unwrap();

    let settings = read_json(&home.join(".claude/settings.json"));
    let allow: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        allow,
        vec!["mcp__tokensave__*"],
        "reinstalling with wildcard enabled should leave no explicit leftovers"
    );
}

#[test]
fn test_silent_reinstall_preserves_user_wildcard_grant() {
    // Regression test: a silent reinstall on upgrade (or any flagless
    // install/reinstall) must NOT clobber a user's hand-authored compact
    // grant back into the 80+ explicit entries, even though
    // `install_tool_perms`/`ctx.tool_permissions` carries the explicit list
    // by default for configs written before the wildcard feature existed.
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Seed settings.json as if the user hand-wrote a compact grant.
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        serde_json::json!({ "permissions": { "allow": ["mcp__tokensave__*"] } }).to_string(),
    )
    .unwrap();

    // Shape of the silent-reinstall-on-upgrade context: the explicit list,
    // force_permission_style = false (no flag was passed this run).
    let mut ctx = make_install_ctx(home);
    ctx.tool_permissions = expected_tool_perms();
    assert!(!ctx.force_permission_style);
    ClaudeIntegration.install(&ctx).unwrap();

    let settings = read_json(&home.join(".claude/settings.json"));
    let allow: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        allow,
        vec!["mcp__tokensave__*"],
        "a flagless reinstall must preserve an existing covering grant, not inflate it \
         back to the explicit per-tool list"
    );
}

#[test]
fn test_install_creates_managed_rules_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Issue #256: rules live in a tokensave-owned managed file, not appended
    // to the user's CLAUDE.md.
    assert!(
        !home.join(".claude/CLAUDE.md").exists(),
        "install should not create/touch CLAUDE.md"
    );

    let rules = std::fs::read_to_string(home.join(".claude/rules/tokensave.md")).unwrap();
    assert!(
        rules.contains("## MANDATORY: No Explore Agents When Tokensave Is Available"),
        "managed rules file should contain the mandatory rules marker"
    );
    assert!(
        rules.contains("tokensave_context"),
        "managed rules file should mention tokensave tools"
    );
    assert!(
        rules.contains("NEVER use Agent(subagent_type=Explore)"),
        "managed rules file should contain the no-explore-agent rule"
    );
    assert!(
        rules.contains("When you spawn an Explore agent"),
        "managed rules file should contain the explore agent guidance paragraph"
    );
    assert!(
        rules.contains("exclude_node_ids"),
        "managed rules file should mention exclude_node_ids for dedup"
    );
}

#[test]
fn test_install_does_not_touch_existing_claude_md() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate CLAUDE.md with existing content
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("CLAUDE.md"), "# Existing content\n").unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert_eq!(
        content, "# Existing content\n",
        "CLAUDE.md should be left untouched by install"
    );

    let rules = std::fs::read_to_string(claude_dir.join("rules/tokensave.md")).unwrap();
    assert!(
        rules.contains("When you spawn an Explore agent"),
        "managed rules file should contain explore agent paragraph"
    );
    assert!(
        rules.contains("tokensave_context"),
        "managed rules file should reference tokensave_context as the tool"
    );
    assert!(
        rules.contains("exclude_node_ids"),
        "managed rules file should mention exclude_node_ids for dedup"
    );
}

#[test]
fn test_install_migrates_legacy_claude_md_block() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate CLAUDE.md with the pre-#256 inline block.
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("CLAUDE.md"),
        "# My Rules\n\nKeep it clean.\n\n\
        ## MANDATORY: No Explore Agents When Tokensave Is Available\n\n\
        **NEVER use Agent(subagent_type=Explore)** ...\n\n\
        ## When you spawn an Explore agent in a tokensave-enabled project\n\n\
        Some legacy guidance.\n",
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(
        !content.contains("MANDATORY: No Explore Agents When Tokensave Is Available"),
        "install should strip the legacy inline block from CLAUDE.md"
    );
    assert!(
        content.contains("My Rules"),
        "install should preserve unrelated CLAUDE.md content"
    );

    let rules = std::fs::read_to_string(claude_dir.join("rules/tokensave.md")).unwrap();
    assert!(
        rules.contains("MANDATORY: No Explore Agents When Tokensave Is Available"),
        "managed rules file should contain the rules (no duplication, no loss)"
    );
}

#[test]
fn test_install_idempotent_managed_rules_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    ClaudeIntegration.install(&ctx).unwrap();
    ClaudeIntegration.install(&ctx).unwrap();

    let rules = std::fs::read_to_string(home.join(".claude/rules/tokensave.md")).unwrap();
    let marker = "## MANDATORY: No Explore Agents When Tokensave Is Available";
    let count = rules.matches(marker).count();
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
    std::fs::write(home.join(".claude.json"), r#"{"foo": "bar"}"#).unwrap();

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

    // Should have both the existing Bash hook and the new tokensave hook
    let has_bash = hooks
        .iter()
        .any(|h| h.get("matcher").and_then(|m| m.as_str()) == Some("Bash"));
    let has_tokensave = hooks
        .iter()
        .any(|h| h.get("matcher").and_then(|m| m.as_str()) == Some("Agent|Grep|Bash"));
    assert!(has_bash, "existing Bash hook should be preserved");
    assert!(
        has_tokensave,
        "new tokensave PreToolUse hook should be added with matcher=Agent|Grep|Bash"
    );
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
    let allow = settings["permissions"]["allow"].as_array_mut().unwrap();
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
fn test_uninstall_removes_managed_rules_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let rules_path = home.join(".claude/rules/tokensave.md");
    assert!(rules_path.exists());

    ClaudeIntegration.uninstall(&ctx).unwrap();

    assert!(
        !rules_path.exists(),
        "managed rules file should be removed after uninstall"
    );
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

    // Install should leave CLAUDE.md untouched and put rules in the managed file.
    let md_content = std::fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(md_content.contains("My Custom Rules"));
    assert!(!md_content.contains("MANDATORY: No Explore Agents"));

    let rules_content = std::fs::read_to_string(claude_dir.join("rules/tokensave.md")).unwrap();
    assert!(rules_content.contains("MANDATORY: No Explore Agents"));

    ClaudeIntegration.uninstall(&ctx).unwrap();

    // After uninstall, custom content should remain untouched
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
        !claude_dir.join("rules/tokensave.md").exists(),
        "managed rules file should be removed after uninstall"
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

    // Delete the managed rules file
    std::fs::remove_file(home.join(".claude/rules/tokensave.md")).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should flag a missing managed rules file"
    );
}

#[test]
fn test_healthcheck_preserves_local_mcp_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let project = dir.path().join("myproject");
    std::fs::create_dir_all(&project).unwrap();

    // Create a local .mcp.json with tokensave (a valid `--local` install)
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

    // A project-local install is now a supported mode — doctor must report it
    // as valid and must NOT delete it.
    assert!(
        project.join(".mcp.json").exists(),
        "doctor must preserve a valid project-local .mcp.json (--local is supported)"
    );
}

#[test]
fn test_healthcheck_preserves_local_settings() {
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

    // Doctor reports project-local config but must not delete it.
    assert!(
        local_claude.join("settings.local.json").exists(),
        "doctor must preserve a valid project-local settings.local.json (--local is supported)"
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
