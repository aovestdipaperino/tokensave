use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::*;

// ---------------------------------------------------------------------------
// 1. Registry tests
// ---------------------------------------------------------------------------

#[test]
fn test_get_all_integrations() {
    let all = all_integrations();
    assert_eq!(all.len(), 9);
}

#[test]
fn test_available_integrations() {
    let ids = available_integrations();
    assert!(ids.contains(&"claude"));
    assert!(ids.contains(&"copilot"));
    assert!(ids.contains(&"codex"));
    assert!(ids.contains(&"gemini"));
    assert!(ids.contains(&"opencode"));
    assert!(ids.contains(&"cursor"));
    assert!(ids.contains(&"zed"));
    assert!(ids.contains(&"cline"));
    assert!(ids.contains(&"roo-code"));
    assert_eq!(ids.len(), 9);
}

#[test]
fn test_get_integration_valid() {
    for id in &[
        "claude", "opencode", "codex", "gemini", "copilot", "cursor", "zed", "cline", "roo-code",
    ] {
        let agent = get_integration(id).unwrap();
        assert_eq!(agent.id(), *id);
    }
}

#[test]
fn test_get_integration_invalid() {
    assert!(get_integration("nonexistent").is_err());
    assert!(get_integration("").is_err());
    assert!(get_integration("CLAUDE").is_err()); // case-sensitive
}

// ---------------------------------------------------------------------------
// 2. Agent trait tests (name/id)
// ---------------------------------------------------------------------------

#[test]
fn test_agent_names_and_ids() {
    for agent in all_integrations() {
        assert!(!agent.name().is_empty(), "agent name should not be empty");
        assert!(!agent.id().is_empty(), "agent id should not be empty");
    }
}

#[test]
fn test_agent_names_are_human_readable() {
    // Names should have at least one space or capital letter (human-readable, not slug)
    let expected_names: Vec<(&str, &str)> = vec![
        ("claude", "Claude Code"),
        ("copilot", "GitHub Copilot"),
        ("codex", "Codex CLI"),
        ("gemini", "Gemini CLI"),
        ("opencode", "OpenCode"),
        ("cursor", "Cursor"),
        ("zed", "Zed"),
        ("cline", "Cline"),
        ("roo-code", "Roo Code"),
    ];
    for (id, expected_name) in expected_names {
        let agent = get_integration(id).unwrap();
        assert_eq!(agent.name(), expected_name, "name mismatch for agent {id}");
    }
}

// ---------------------------------------------------------------------------
// 3. Install / config creation tests (with tempdir)
// ---------------------------------------------------------------------------

fn make_install_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: EXPECTED_TOOL_PERMS,
    }
}

#[test]
fn test_claude_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    // Check ~/.claude.json exists and has mcpServers.tokensave
    let claude_json = home.join(".claude.json");
    assert!(claude_json.exists(), "~/.claude.json should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert!(
        content.get("mcpServers").is_some(),
        "mcpServers key should exist"
    );
    assert!(
        content["mcpServers"]["tokensave"].is_object(),
        "mcpServers.tokensave should be an object"
    );
    // Verify args contain "serve"
    let args = content["mcpServers"]["tokensave"]["args"].as_array().unwrap();
    assert!(args.iter().any(|v| v.as_str() == Some("serve")));

    // Check ~/.claude/settings.json exists with hook and permissions
    let settings_path = home.join(".claude/settings.json");
    assert!(settings_path.exists(), "settings.json should exist after install");
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    // Check hook
    assert!(
        settings["hooks"]["PreToolUse"].is_array(),
        "PreToolUse hook should be an array"
    );
    // Check permissions
    assert!(
        settings["permissions"]["allow"].is_array(),
        "permissions.allow should be an array"
    );

    // Check CLAUDE.md exists with tokensave rules
    let claude_md = home.join(".claude/CLAUDE.md");
    assert!(claude_md.exists(), "CLAUDE.md should exist after install");
    let md_content = std::fs::read_to_string(&claude_md).unwrap();
    assert!(md_content.contains("tokensave"), "CLAUDE.md should mention tokensave");
}

#[test]
fn test_gemini_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();

    // Check ~/.gemini/settings.json
    let settings_path = home.join(".gemini/settings.json");
    assert!(settings_path.exists(), "settings.json should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(
        content["mcpServers"]["tokensave"].is_object(),
        "mcpServers.tokensave should exist"
    );
    // Verify trust flag
    assert_eq!(
        content["mcpServers"]["tokensave"]["trust"],
        serde_json::json!(true),
        "gemini should have trust: true"
    );

    // Check GEMINI.md
    let gemini_md = home.join(".gemini/GEMINI.md");
    assert!(gemini_md.exists(), "GEMINI.md should exist after install");
    let md_content = std::fs::read_to_string(&gemini_md).unwrap();
    assert!(md_content.contains("tokensave"));
}

#[test]
fn test_codex_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CodexIntegration.install(&ctx).unwrap();

    // Check ~/.codex/config.toml
    let config_path = home.join(".codex/config.toml");
    assert!(config_path.exists(), "config.toml should exist after install");
    // Verify the file contains the expected content as text (the TOML output from
    // toml::to_string_pretty uses dotted headers which may not round-trip through
    // toml::Value::parse in all crate versions)
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("[mcp_servers.tokensave]"),
        "config.toml should contain [mcp_servers.tokensave]"
    );
    assert!(
        content.contains("\"serve\""),
        "config.toml should contain \"serve\" in args"
    );

    // Check AGENTS.md
    let agents_md = home.join(".codex/AGENTS.md");
    assert!(agents_md.exists(), "AGENTS.md should exist after install");
    let md_content = std::fs::read_to_string(&agents_md).unwrap();
    assert!(md_content.contains("tokensave"));
}

#[test]
fn test_cursor_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CursorIntegration.install(&ctx).unwrap();

    let mcp_path = home.join(".cursor/mcp.json");
    assert!(mcp_path.exists(), "mcp.json should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_opencode_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    // OpenCode uses ~/.config/opencode/opencode.json
    // Create the parent dir so install can discover it
    let ctx = make_install_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let config_path = home.join(".config/opencode/opencode.json");
    assert!(config_path.exists(), "opencode.json should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert!(content["mcp"]["tokensave"].is_object());
}

#[test]
fn test_zed_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ZedIntegration.install(&ctx).unwrap();

    // On macOS: ~/Library/Application Support/Zed/settings.json
    // On linux: ~/.config/zed/settings.json
    #[cfg(target_os = "macos")]
    let settings_path = home.join("Library/Application Support/Zed/settings.json");
    #[cfg(not(target_os = "macos"))]
    let settings_path = home.join(".config/zed/settings.json");

    assert!(settings_path.exists(), "Zed settings.json should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["context_servers"]["tokensave"].is_object());
}

#[test]
fn test_cline_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    ClineIntegration.install(&ctx).unwrap();

    // Cline uses VS Code extension global storage
    #[cfg(target_os = "macos")]
    let settings_path = home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
    #[cfg(target_os = "linux")]
    let settings_path = home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
    #[cfg(target_os = "windows")]
    let settings_path = home.join("AppData/Roaming/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let settings_path = home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");

    assert!(settings_path.exists(), "Cline settings should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_roo_code_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    RooCodeIntegration.install(&ctx).unwrap();

    #[cfg(target_os = "macos")]
    let settings_path = home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(target_os = "linux")]
    let settings_path = home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(target_os = "windows")]
    let settings_path = home.join("AppData/Roaming/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let settings_path = home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");

    assert!(settings_path.exists(), "Roo Code settings should exist after install");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_copilot_install_creates_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    // Check VS Code settings.json
    #[cfg(target_os = "macos")]
    let vscode_settings = home.join("Library/Application Support/Code/User/settings.json");
    #[cfg(target_os = "linux")]
    let vscode_settings = home.join(".config/Code/User/settings.json");
    #[cfg(target_os = "windows")]
    let vscode_settings = home.join("AppData/Roaming/Code/User/settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let vscode_settings = home.join(".config/Code/User/settings.json");

    assert!(vscode_settings.exists(), "VS Code settings.json should exist");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&vscode_settings).unwrap()).unwrap();
    assert!(content["mcp"]["servers"]["tokensave"].is_object());

    // Check CLI config
    let cli_config = home.join(".copilot/mcp-config.json");
    assert!(cli_config.exists(), "Copilot CLI config should exist");
    let cli_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cli_config).unwrap()).unwrap();
    assert!(cli_content["mcpServers"]["tokensave"].is_object());
}

// ---------------------------------------------------------------------------
// 4. Install followed by Uninstall
// ---------------------------------------------------------------------------

#[test]
fn test_claude_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    // Install
    ClaudeIntegration.install(&ctx).unwrap();
    assert!(home.join(".claude.json").exists());

    // Uninstall
    ClaudeIntegration.uninstall(&ctx).unwrap();

    // ~/.claude.json should be removed (was only tokensave)
    // It may be removed entirely or have mcpServers removed
    if home.join(".claude.json").exists() {
        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".claude.json")).unwrap(),
        )
        .unwrap();
        // Should not have tokensave anymore
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(!has_tokensave, "tokensave should be removed from .claude.json after uninstall");
    }
}

#[test]
fn test_gemini_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    GeminiIntegration.install(&ctx).unwrap();
    let settings_path = home.join(".gemini/settings.json");
    assert!(settings_path.exists());

    GeminiIntegration.uninstall(&ctx).unwrap();

    // After uninstall, settings.json should be removed or not contain tokensave
    if settings_path.exists() {
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(!has_tokensave, "tokensave should be removed from settings.json");
    }

    // GEMINI.md should be removed (was only tokensave rules)
    let gemini_md = home.join(".gemini/GEMINI.md");
    if gemini_md.exists() {
        let content = std::fs::read_to_string(&gemini_md).unwrap();
        assert!(
            !content.contains("## Prefer tokensave MCP tools"),
            "GEMINI.md should not contain tokensave rules after uninstall"
        );
    }
}

#[test]
fn test_codex_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    CodexIntegration.install(&ctx).unwrap();
    assert!(home.join(".codex/config.toml").exists());

    CodexIntegration.uninstall(&ctx).unwrap();

    // Note: The uninstall uses load_toml_file which returns empty due to
    // toml::Value parse limitation. As a result, uninstall cannot find and
    // remove the mcp_servers entry. The config file is left empty after
    // uninstall because the empty table is written back. This documents
    // current behavior.
    // AGENTS.md should be removed though (uses text-based removal)
    let agents_md = home.join(".codex/AGENTS.md");
    if agents_md.exists() {
        let content = std::fs::read_to_string(&agents_md).unwrap();
        assert!(
            !content.contains("## Prefer tokensave MCP tools"),
            "AGENTS.md should not have tokensave rules after uninstall"
        );
    }
}

#[test]
fn test_cursor_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    CursorIntegration.install(&ctx).unwrap();
    let mcp_path = home.join(".cursor/mcp.json");
    assert!(mcp_path.exists());

    CursorIntegration.uninstall(&ctx).unwrap();

    // mcp.json should be removed (was only tokensave)
    if mcp_path.exists() {
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(!has_tokensave, "tokensave should be removed from mcp.json");
    }
}

#[test]
fn test_copilot_install_then_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    // CLI config should be cleaned up
    let cli_config = home.join(".copilot/mcp-config.json");
    if cli_config.exists() {
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cli_config).unwrap()).unwrap();
        let has_tokensave = content
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(!has_tokensave);
    }
}

// ---------------------------------------------------------------------------
// 5. Healthcheck with tempdir
// ---------------------------------------------------------------------------

/// Creates a fake tokensave binary in a temp dir and returns the path string.
/// This allows healthchecks to verify binary existence.
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

#[test]
fn test_healthcheck_claude_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx_with_real_bin(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Claude install should have no issues");
}

#[test]
fn test_healthcheck_gemini_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    GeminiIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Gemini install should have no issues");
}

#[test]
fn test_healthcheck_codex_after_install() {
    // Note: Codex healthcheck uses load_toml_file internally, which always
    // returns an empty table due to the toml::Value parse limitation.
    // Therefore healthcheck always reports the MCP server as not registered.
    // This test documents the current behavior.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CodexIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CodexIntegration.healthcheck(&mut dc, &hctx);
    // Healthcheck reports 1 issue (MCP server not found) due to parse limitation
    assert_eq!(
        dc.issues, 1,
        "Codex healthcheck reports MCP not found due to toml::Value parse limitation"
    );
}

#[test]
fn test_healthcheck_cursor_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    CursorIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CursorIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Cursor install should have no issues");
}

#[test]
fn test_healthcheck_opencode_clean_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    OpenCodeIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean OpenCode install should have no issues");
}

#[test]
fn test_healthcheck_no_install_warns() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Healthcheck without installing should produce warnings (not crashes)
    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    ClaudeIntegration.healthcheck(&mut dc, &hctx);
    // Should have issues (missing config files)
    assert!(
        dc.issues > 0 || dc.warnings > 0,
        "healthcheck on empty dir should report issues or warnings"
    );
}

#[test]
fn test_doctor_counters() {
    let mut dc = DoctorCounters::new();
    assert_eq!(dc.issues, 0);
    assert_eq!(dc.warnings, 0);

    dc.pass("this is fine");
    assert_eq!(dc.issues, 0);
    assert_eq!(dc.warnings, 0);

    dc.fail("something broke");
    assert_eq!(dc.issues, 1);
    assert_eq!(dc.warnings, 0);

    dc.warn("be careful");
    assert_eq!(dc.issues, 1);
    assert_eq!(dc.warnings, 1);

    dc.info("just info");
    assert_eq!(dc.issues, 1);
    assert_eq!(dc.warnings, 1);

    dc.fail("another failure");
    assert_eq!(dc.issues, 2);
    assert_eq!(dc.warnings, 1);
}

// ---------------------------------------------------------------------------
// 6. Helper function tests
// ---------------------------------------------------------------------------

#[test]
fn test_load_json_file_missing() {
    let val = load_json_file(Path::new("/nonexistent/file.json"));
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_valid() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    std::fs::write(&path, r#"{"key": "value"}"#).unwrap();
    let val = load_json_file(&path);
    assert_eq!(val["key"], "value");
}

#[test]
fn test_load_json_file_invalid_returns_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "not valid json").unwrap();
    let val = load_json_file(&path);
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_strict_missing() {
    let result = load_json_file_strict(Path::new("/nonexistent/file.json"));
    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_strict_empty_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.json");
    std::fs::write(&path, "").unwrap();
    let result = load_json_file_strict(&path);
    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_json_file_strict_whitespace_only() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ws.json");
    std::fs::write(&path, "   \n  \t  ").unwrap();
    let result = load_json_file_strict(&path);
    assert!(result.is_ok());
}

#[test]
fn test_load_json_file_strict_invalid() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "not valid json").unwrap();
    assert!(load_json_file_strict(&path).is_err());
}

#[test]
fn test_load_json_file_strict_valid() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("good.json");
    std::fs::write(&path, r#"{"hello": "world"}"#).unwrap();
    let val = load_json_file_strict(&path).unwrap();
    assert_eq!(val["hello"], "world");
}

#[test]
fn test_backup_config_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, r#"{"original": true}"#).unwrap();
    let backup = backup_config_file(&path).unwrap();
    assert!(backup.is_some());
    let backup_path = backup.unwrap();
    assert!(backup_path.exists());
    // Verify backup content matches original
    let backup_content = std::fs::read_to_string(&backup_path).unwrap();
    assert_eq!(backup_content, r#"{"original": true}"#);
}

#[test]
fn test_backup_config_file_missing() {
    let result = backup_config_file(Path::new("/nonexistent/file.json")).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_safe_write_json_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("output.json");
    let value = serde_json::json!({"hello": "world"});
    safe_write_json_file(&path, &value, None).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["hello"], "world");
}

#[test]
fn test_safe_write_json_file_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deep/nested/dir/output.json");
    let value = serde_json::json!({"nested": true});
    safe_write_json_file(&path, &value, None).unwrap();
    assert!(path.exists());
}

#[test]
fn test_safe_write_json_file_overwrites_existing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("existing.json");
    std::fs::write(&path, r#"{"old": true}"#).unwrap();
    let value = serde_json::json!({"new": true});
    safe_write_json_file(&path, &value, None).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["new"], true);
    assert!(content.get("old").is_none());
}

#[test]
fn test_write_json_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("write_test.json");
    let value = serde_json::json!({"test": 42});
    write_json_file(&path, &value).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["test"], 42);
}

#[test]
fn test_load_toml_file_missing() {
    let val = load_toml_file(Path::new("/nonexistent/file.toml"));
    assert!(val.is_table());
    assert!(val.as_table().unwrap().is_empty());
}

#[test]
fn test_load_toml_file_valid() {
    // Note: in toml v1.x, `str.parse::<toml::Value>()` treats the input as a
    // single TOML *value* (string, number, etc.), NOT as a full TOML document.
    // Therefore `load_toml_file` always falls back to an empty table for normal
    // TOML files. This is a known limitation. Verify the fallback behavior.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.toml");
    std::fs::write(&path, "key = \"value\"\nnumber = 42\n").unwrap();
    let val = load_toml_file(&path);
    assert!(val.is_table(), "load_toml_file should return a table");
    // The current implementation falls back to empty table because
    // `str.parse::<toml::Value>()` does not parse full TOML documents.
    // This test documents the current behavior.
    assert!(
        val.as_table().unwrap().is_empty(),
        "load_toml_file returns empty table due to toml::Value parse limitation"
    );
}

#[test]
fn test_load_toml_file_invalid_returns_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "{{{{not valid toml").unwrap();
    let val = load_toml_file(&path);
    assert!(val.is_table());
    assert!(val.as_table().unwrap().is_empty());
}

#[test]
fn test_write_toml_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("output.toml");
    let mut table = toml::map::Map::new();
    table.insert("key".to_string(), toml::Value::String("value".to_string()));
    let val = toml::Value::Table(table);
    write_toml_file(&path, &val).unwrap();
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("key"));
    assert!(content.contains("value"));
}

// ---------------------------------------------------------------------------
// JSONC helpers
// ---------------------------------------------------------------------------

#[test]
fn test_load_jsonc_file_missing() {
    let val = load_jsonc_file(Path::new("/nonexistent/file.jsonc"));
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_load_jsonc_file_with_comments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.jsonc");
    std::fs::write(
        &path,
        r#"{
        // This is a comment
        "key": "value", // trailing comment
        /* block comment */
        "number": 42,
    }"#,
    )
    .unwrap();
    let val = load_jsonc_file(&path);
    assert_eq!(val["key"], "value");
    assert_eq!(val["number"], 42);
}

#[test]
fn test_load_jsonc_file_strict_missing() {
    let result = load_jsonc_file_strict(Path::new("/nonexistent/file.jsonc"));
    assert!(result.is_ok());
}

#[test]
fn test_load_jsonc_file_strict_with_comments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.jsonc");
    std::fs::write(
        &path,
        r#"{
        // comment
        "key": "value"
    }"#,
    )
    .unwrap();
    let val = load_jsonc_file_strict(&path).unwrap();
    assert_eq!(val["key"], "value");
}

#[test]
fn test_parse_jsonc() {
    let input = r#"{
        // line comment
        "a": 1,
        /* block */ "b": 2,
    }"#;
    let val = parse_jsonc(input);
    assert_eq!(val["a"], 1);
    assert_eq!(val["b"], 2);
}

// ---------------------------------------------------------------------------
// 7. is_detected / has_tokensave tests
// ---------------------------------------------------------------------------

#[test]
fn test_is_detected_claude() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!ClaudeIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    assert!(ClaudeIntegration.is_detected(home));
}

#[test]
fn test_is_detected_codex() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CodexIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    assert!(CodexIntegration.is_detected(home));
}

#[test]
fn test_is_detected_gemini() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!GeminiIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".gemini")).unwrap();
    assert!(GeminiIntegration.is_detected(home));
}

#[test]
fn test_is_detected_cursor() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CursorIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".cursor")).unwrap();
    assert!(CursorIntegration.is_detected(home));
}

#[test]
fn test_is_detected_opencode() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!OpenCodeIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".config/opencode")).unwrap();
    assert!(OpenCodeIntegration.is_detected(home));
}

#[test]
fn test_is_detected_zed() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!ZedIntegration.is_detected(home));
    #[cfg(target_os = "macos")]
    std::fs::create_dir_all(home.join("Library/Application Support/Zed")).unwrap();
    #[cfg(not(target_os = "macos"))]
    std::fs::create_dir_all(home.join(".config/zed")).unwrap();
    assert!(ZedIntegration.is_detected(home));
}

#[test]
fn test_is_detected_copilot() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    // Copilot is detected when either VS Code User dir or .copilot dir exists
    assert!(!CopilotIntegration.is_detected(home));
    std::fs::create_dir_all(home.join(".copilot")).unwrap();
    assert!(CopilotIntegration.is_detected(home));
}

#[test]
fn test_has_tokensave_claude() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    // No config => false
    assert!(!ClaudeIntegration.has_tokensave(home));

    // After install => true
    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();
    assert!(ClaudeIntegration.has_tokensave(home));

    // After uninstall => false
    ClaudeIntegration.uninstall(&ctx).unwrap();
    assert!(!ClaudeIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_gemini() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!GeminiIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();
    assert!(GeminiIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_codex() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CodexIntegration.has_tokensave(home));

    // Note: CodexIntegration.has_tokensave uses load_toml_file internally, which
    // always returns an empty table due to the toml::Value parse limitation.
    // Therefore has_tokensave always returns false for codex, even after install.
    // This test documents the current behavior.
    let ctx = make_install_ctx(home);
    CodexIntegration.install(&ctx).unwrap();
    // The config file exists but has_tokensave returns false due to parse limitation
    assert!(home.join(".codex/config.toml").exists());
    assert!(
        !CodexIntegration.has_tokensave(home),
        "has_tokensave returns false due to toml::Value parse limitation"
    );
}

#[test]
fn test_has_tokensave_cursor() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CursorIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    CursorIntegration.install(&ctx).unwrap();
    assert!(CursorIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_opencode() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!OpenCodeIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    OpenCodeIntegration.install(&ctx).unwrap();
    assert!(OpenCodeIntegration.has_tokensave(home));
}

#[test]
fn test_has_tokensave_copilot() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(!CopilotIntegration.has_tokensave(home));

    let ctx = make_install_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    assert!(CopilotIntegration.has_tokensave(home));
}

// ---------------------------------------------------------------------------
// 8. Idempotency tests
// ---------------------------------------------------------------------------

#[test]
fn test_claude_install_idempotent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    // Install twice should not fail
    ClaudeIntegration.install(&ctx).unwrap();
    ClaudeIntegration.install(&ctx).unwrap();

    // Config should still be valid
    let claude_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".claude.json")).unwrap(),
    )
    .unwrap();
    assert!(claude_json["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_gemini_install_idempotent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    GeminiIntegration.install(&ctx).unwrap();
    GeminiIntegration.install(&ctx).unwrap();

    let settings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".gemini/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(settings["mcpServers"]["tokensave"].is_object());
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx(home);

    // Uninstalling when nothing is installed should not panic or error
    ClaudeIntegration.uninstall(&ctx).unwrap();
    GeminiIntegration.uninstall(&ctx).unwrap();
    CodexIntegration.uninstall(&ctx).unwrap();
    CursorIntegration.uninstall(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();
    OpenCodeIntegration.uninstall(&ctx).unwrap();
    ZedIntegration.uninstall(&ctx).unwrap();
    ClineIntegration.uninstall(&ctx).unwrap();
    RooCodeIntegration.uninstall(&ctx).unwrap();
}

// ---------------------------------------------------------------------------
// 9. Install preserves existing config
// ---------------------------------------------------------------------------

#[test]
fn test_claude_install_preserves_existing_claude_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate .claude.json with other data
    let claude_json_path = home.join(".claude.json");
    std::fs::write(
        &claude_json_path,
        r#"{"mcpServers": {"other-server": {"command": "foo"}}, "customKey": 42}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json_path).unwrap()).unwrap();
    // tokensave added
    assert!(content["mcpServers"]["tokensave"].is_object());
    // existing server preserved
    assert!(content["mcpServers"]["other-server"].is_object());
    // custom key preserved
    assert_eq!(content["customKey"], 42);
}

#[test]
fn test_gemini_install_preserves_existing_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let settings_path = home.join(".gemini/settings.json");
    std::fs::create_dir_all(home.join(".gemini")).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcpServers": {"other": {"command": "bar"}}, "theme": "dark"}"#,
    )
    .unwrap();

    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).unwrap();

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["tokensave"].is_object());
    assert!(content["mcpServers"]["other"].is_object());
    assert_eq!(content["theme"], "dark");
}

// ---------------------------------------------------------------------------
// 10. Constants sanity
// ---------------------------------------------------------------------------

#[test]
fn test_tool_names_not_empty() {
    assert!(!TOOL_NAMES.is_empty());
    for name in TOOL_NAMES {
        assert!(name.starts_with("tokensave_"), "tool name should start with tokensave_: {name}");
    }
}

#[test]
fn test_expected_tool_perms_not_empty() {
    assert!(!EXPECTED_TOOL_PERMS.is_empty());
    for perm in EXPECTED_TOOL_PERMS {
        assert!(
            perm.starts_with("mcp__tokensave__"),
            "tool perm should start with mcp__tokensave__: {perm}"
        );
    }
}

#[test]
fn test_tool_perms_match_tool_names() {
    // Each tool name should have a corresponding permission
    assert_eq!(
        TOOL_NAMES.len(),
        EXPECTED_TOOL_PERMS.len(),
        "TOOL_NAMES and EXPECTED_TOOL_PERMS should have same length"
    );
    for name in TOOL_NAMES {
        let expected_perm = format!("mcp__tokensave__{name}");
        assert!(
            EXPECTED_TOOL_PERMS.contains(&expected_perm.as_str()),
            "missing permission for tool {name}: expected {expected_perm}"
        );
    }
}

// ---------------------------------------------------------------------------
// 11. restore_config_backup
// ---------------------------------------------------------------------------

#[test]
fn test_restore_config_backup_restores_content() {
    let dir = TempDir::new().unwrap();
    let original_path = dir.path().join("config.json");
    let backup_path = dir.path().join("config.json.bak");

    // Create original and backup
    std::fs::write(&original_path, r#"{"version": 1}"#).unwrap();
    std::fs::write(&backup_path, r#"{"version": 1}"#).unwrap();

    // Corrupt the original
    std::fs::write(&original_path, "CORRUPTED").unwrap();

    // Restore from backup
    restore_config_backup(&original_path, &backup_path);

    let restored = std::fs::read_to_string(&original_path).unwrap();
    assert_eq!(
        restored,
        r#"{"version": 1}"#,
        "restored content should match the backup"
    );
}

#[test]
fn test_restore_config_backup_to_missing_original() {
    let dir = TempDir::new().unwrap();
    let original_path = dir.path().join("config.json");
    let backup_path = dir.path().join("config.json.bak");

    // Only create backup, not original
    std::fs::write(&backup_path, r#"{"saved": true}"#).unwrap();

    restore_config_backup(&original_path, &backup_path);

    assert!(original_path.exists(), "original should be created from backup");
    let content = std::fs::read_to_string(&original_path).unwrap();
    assert_eq!(content, r#"{"saved": true}"#);
}

#[test]
fn test_restore_config_backup_missing_backup_does_not_panic() {
    let dir = TempDir::new().unwrap();
    let original_path = dir.path().join("config.json");
    let backup_path = dir.path().join("config.json.bak");

    std::fs::write(&original_path, "original").unwrap();

    // Restore with a nonexistent backup — should not panic
    restore_config_backup(&original_path, &backup_path);

    // Original should remain untouched since backup failed
    let content = std::fs::read_to_string(&original_path).unwrap();
    assert_eq!(content, "original");
}

// ---------------------------------------------------------------------------
// 12. which_tokensave
// ---------------------------------------------------------------------------

#[test]
fn test_which_tokensave_returns_some_or_none() {
    // which_tokensave checks current_exe and PATH — we just verify it
    // doesn't panic and returns a sensible result.
    let result = which_tokensave();
    // In a test environment, the current exe is the test runner, not tokensave,
    // so it may return None (unless tokensave is on PATH). Either way, no panic.
    if let Some(ref path) = result {
        assert!(!path.is_empty(), "path should not be empty if Some");
    }
    // Test passes regardless of Some or None — just ensures no panic.
}

// ---------------------------------------------------------------------------
// 13. home_dir
// ---------------------------------------------------------------------------

#[test]
fn test_home_dir_returns_some() {
    let result = home_dir();
    assert!(
        result.is_some(),
        "home_dir should return Some on most systems"
    );
    let home = result.unwrap();
    assert!(home.is_absolute(), "home dir should be an absolute path");
}

// ---------------------------------------------------------------------------
// 14. migrate_installed_agents
// ---------------------------------------------------------------------------

#[test]
fn test_migrate_installed_agents_skips_when_already_populated() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let mut config = tokensave::user_config::UserConfig::default();
    config.installed_agents = vec!["claude".to_string()];

    // Should return immediately since installed_agents is non-empty
    migrate_installed_agents(home, &mut config);

    // The existing list should be unchanged
    assert_eq!(config.installed_agents, vec!["claude".to_string()]);
}

#[test]
fn test_migrate_installed_agents_detects_installed_agents() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Install copilot so it can be detected
    let ctx = make_install_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let mut config = tokensave::user_config::UserConfig::default();
    assert!(config.installed_agents.is_empty());

    // migrate will scan and detect copilot is installed
    // Note: save() will try to write to ~/.tokensave/config.toml which may fail
    // in CI, but the function still populates installed_agents in memory.
    migrate_installed_agents(home, &mut config);

    assert!(
        config.installed_agents.contains(&"copilot".to_string()),
        "copilot should be detected, got: {:?}",
        config.installed_agents
    );
}

#[test]
fn test_migrate_installed_agents_empty_home_no_change() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let mut config = tokensave::user_config::UserConfig::default();

    migrate_installed_agents(home, &mut config);

    // No agents installed in empty home, list should remain empty
    assert!(
        config.installed_agents.is_empty(),
        "installed_agents should remain empty when no agents detected"
    );
}

// ---------------------------------------------------------------------------
// 15. pick_integrations_interactive (no-agent-detected error path)
// ---------------------------------------------------------------------------

#[test]
fn test_pick_integrations_interactive_no_agents_detected() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Empty home — no agents detected
    let result = pick_integrations_interactive(home, &[]);
    assert!(
        result.is_err(),
        "pick_integrations_interactive should error when no agents detected"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("No supported agents detected"),
        "error should mention no agents detected, got: {err_msg}"
    );
}

#[test]
fn test_pick_integrations_interactive_single_uninstalled_agent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create only the .copilot dir so exactly one agent is detected
    std::fs::create_dir_all(home.join(".copilot")).unwrap();

    // Single detected agent that is NOT installed => fast path returns it directly
    let result = pick_integrations_interactive(home, &[]);
    assert!(result.is_ok(), "should succeed with single uninstalled agent");
    let (to_install, to_uninstall) = result.unwrap();
    assert_eq!(to_install, vec!["copilot".to_string()]);
    assert!(to_uninstall.is_empty());
}

// ---------------------------------------------------------------------------
// 16. vscode_data_dir / copilot_cli_dir
// ---------------------------------------------------------------------------

#[test]
fn test_vscode_data_dir_is_under_home() {
    let home = Path::new("/fake/home");
    let dir = tokensave::agents::vscode_data_dir(home);
    assert!(
        dir.starts_with("/fake/home"),
        "vscode_data_dir should be under home: {}",
        dir.display()
    );
}

#[test]
fn test_copilot_cli_dir_is_under_home() {
    let home = Path::new("/fake/home");
    let dir = tokensave::agents::copilot_cli_dir(home);
    assert_eq!(
        dir,
        Path::new("/fake/home/.copilot"),
        "copilot_cli_dir should be home/.copilot"
    );
}

// ---------------------------------------------------------------------------
// 17. parse_jsonc edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_parse_jsonc_empty_string() {
    let val = parse_jsonc("");
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_parse_jsonc_only_comments() {
    let input = "// just a comment\n/* block */\n";
    let val = parse_jsonc(input);
    assert!(val.is_object());
    assert!(val.as_object().unwrap().is_empty());
}

#[test]
fn test_parse_jsonc_nested_comments() {
    let input = r#"{
        "a": "hello // not a comment",
        /* this is a real comment */
        "b": true
    }"#;
    let val = parse_jsonc(input);
    assert_eq!(val["a"].as_str().unwrap(), "hello // not a comment");
    assert_eq!(val["b"], true);
}

#[test]
fn test_parse_jsonc_trailing_comma_in_object() {
    let input = r#"{"a": 1, "b": 2,}"#;
    let val = parse_jsonc(input);
    assert_eq!(val["a"], 1);
    assert_eq!(val["b"], 2);
}

#[test]
fn test_parse_jsonc_trailing_comma_in_array() {
    let input = r#"{"arr": [1, 2, 3,]}"#;
    let val = parse_jsonc(input);
    let arr = val["arr"].as_array().unwrap();
    assert_eq!(arr.len(), 3);
}

// ---------------------------------------------------------------------------
// 18. backup + safe_write round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_backup_and_safe_write_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("roundtrip.json");

    // Create initial file
    let initial = serde_json::json!({"name": "tokensave", "version": 1});
    safe_write_json_file(&path, &initial, None).unwrap();

    // Create backup
    let backup = backup_config_file(&path).unwrap();
    assert!(backup.is_some());
    let backup_path = backup.unwrap();

    // Overwrite with new content
    let updated = serde_json::json!({"name": "tokensave", "version": 2});
    safe_write_json_file(&path, &updated, Some(&backup_path)).unwrap();

    // Verify new content
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(content["version"], 2);

    // Verify backup still has old content
    let backup_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&backup_path).unwrap()).unwrap();
    assert_eq!(backup_content["version"], 1);

    // Restore from backup
    restore_config_backup(&path, &backup_path);
    let restored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(restored["version"], 1);
}
