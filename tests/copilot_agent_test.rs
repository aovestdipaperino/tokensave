use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::{
    AgentIntegration, CopilotIntegration, DoctorCounters, HealthcheckContext, InstallContext,
    EXPECTED_TOOL_PERMS,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: EXPECTED_TOOL_PERMS,
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}

/// Platform-specific path for the VS Code settings.json under the temp home.
fn vscode_settings_path(home: &Path) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code/User/settings.json")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".config/Code/User/settings.json")
    }
    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Roaming/Code/User/settings.json")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        home.join(".config/Code/User/settings.json")
    }
}

fn cli_config_path(home: &Path) -> std::path::PathBuf {
    home.join(".copilot/mcp-config.json")
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_vscode_settings_with_mcp_server() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let settings_path = vscode_settings_path(home);
    assert!(
        settings_path.exists(),
        "VS Code settings.json should be created"
    );

    let settings = read_json(&settings_path);
    let ts = &settings["mcp"]["servers"]["tokensave"];
    assert!(ts.is_object(), "mcp.servers.tokensave should be an object");
    assert_eq!(
        ts["type"].as_str().unwrap(),
        "stdio",
        "type should be stdio"
    );
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
fn test_install_creates_cli_config_with_mcp_server() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let cli_path = cli_config_path(home);
    assert!(
        cli_path.exists(),
        "Copilot CLI mcp-config.json should be created"
    );

    let config = read_json(&cli_path);
    let ts = &config["mcpServers"]["tokensave"];
    assert!(
        ts.is_object(),
        "mcpServers.tokensave should be an object in CLI config"
    );
    assert_eq!(ts["type"].as_str().unwrap(), "stdio");
    assert_eq!(ts["command"].as_str().unwrap(), "/usr/local/bin/tokensave");
    let args: Vec<&str> = ts["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(args, vec!["serve"]);
}

#[test]
fn test_install_preserves_existing_vscode_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate VS Code settings with other content
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"editor.fontSize": 14, "workbench.colorTheme": "One Dark Pro"}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let settings = read_json(&settings_path);
    assert_eq!(
        settings["editor.fontSize"], 14,
        "existing VS Code setting should be preserved"
    );
    assert!(
        settings["mcp"]["servers"]["tokensave"].is_object(),
        "tokensave MCP server should be added"
    );
}

#[test]
fn test_install_preserves_existing_cli_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate CLI config with another MCP server
    let cli_path = cli_config_path(home);
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(
        &cli_path,
        r#"{"mcpServers": {"other-server": {"command": "foo", "args": []}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let config = read_json(&cli_path);
    assert!(
        config["mcpServers"]["other-server"].is_object(),
        "existing server should be preserved in CLI config"
    );
    assert!(
        config["mcpServers"]["tokensave"].is_object(),
        "tokensave should be added alongside existing servers"
    );
}

#[test]
fn test_install_idempotent_vscode() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.install(&ctx).unwrap();

    let settings = read_json(&vscode_settings_path(home));
    assert!(
        settings["mcp"]["servers"]["tokensave"].is_object(),
        "tokensave should still be registered after double install"
    );
    // Ensure there's exactly one "tokensave" key (no duplication)
    let servers = settings["mcp"]["servers"].as_object().unwrap();
    let ts_count = servers.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(ts_count, 1, "tokensave should appear exactly once");
}

#[test]
fn test_install_idempotent_cli() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.install(&ctx).unwrap();

    let config = read_json(&cli_config_path(home));
    let servers = config["mcpServers"].as_object().unwrap();
    let ts_count = servers.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(ts_count, 1, "tokensave should appear exactly once in CLI config");
}

// ===========================================================================
// Uninstall verification
// ===========================================================================

#[test]
fn test_uninstall_removes_vscode_mcp_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    let settings_path = vscode_settings_path(home);
    // settings.json is always written back (never deleted)
    assert!(settings_path.exists(), "settings.json should still exist after uninstall");
    let settings = read_json(&settings_path);
    let has_tokensave = settings
        .get("mcp")
        .and_then(|v| v.get("servers"))
        .and_then(|v| v.get("tokensave"))
        .is_some();
    assert!(
        !has_tokensave,
        "mcp.servers.tokensave should be removed after uninstall"
    );
}

#[test]
fn test_uninstall_cleans_empty_mcp_objects() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    let settings_path = vscode_settings_path(home);
    let settings = read_json(&settings_path);
    // After removing tokensave (the only server), both "servers" and "mcp"
    // should be cleaned up.
    assert!(
        settings.get("mcp").is_none()
            || settings["mcp"]
                .as_object()
                .is_some_and(|o| o.is_empty()),
        "empty mcp object should be cleaned up"
    );
}

#[test]
fn test_uninstall_removes_cli_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    let cli_path = cli_config_path(home);
    // When tokensave was the only entry, the file should be removed entirely
    if cli_path.exists() {
        let config = read_json(&cli_path);
        let has_tokensave = config
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(
            !has_tokensave,
            "mcpServers.tokensave should be removed from CLI config"
        );
    }
}

#[test]
fn test_uninstall_preserves_other_cli_servers() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate CLI config with another server
    let cli_path = cli_config_path(home);
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(
        &cli_path,
        r#"{"mcpServers": {"other-tool": {"command": "other-tool", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    assert!(cli_path.exists(), "CLI config should still exist when other servers remain");
    let config = read_json(&cli_path);
    assert!(
        config["mcpServers"]["other-tool"].is_object(),
        "other server should be preserved"
    );
    let has_tokensave = config
        .get("mcpServers")
        .and_then(|v| v.get("tokensave"))
        .is_some();
    assert!(!has_tokensave, "tokensave should be removed");
}

#[test]
fn test_uninstall_preserves_other_vscode_settings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate VS Code settings
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"editor.fontSize": 14}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    let settings = read_json(&settings_path);
    assert_eq!(
        settings["editor.fontSize"], 14,
        "existing VS Code settings should be preserved after uninstall"
    );
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    // Should not panic or error
    CopilotIntegration.uninstall(&ctx).unwrap();
}

#[test]
fn test_uninstall_cli_with_no_tokensave_is_noop() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create a CLI config without tokensave
    let cli_path = cli_config_path(home);
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(
        &cli_path,
        r#"{"mcpServers": {"something-else": {"command": "x"}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.uninstall(&ctx).unwrap();

    // File should remain unchanged
    let config = read_json(&cli_path);
    assert!(config["mcpServers"]["something-else"].is_object());
}

// ===========================================================================
// Healthcheck verification
// ===========================================================================

#[test]
fn test_healthcheck_clean_install_no_issues() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Copilot install should have no issues");
}

#[test]
fn test_healthcheck_missing_config_produces_warnings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0 || dc.issues > 0,
        "healthcheck on empty dir should report warnings or issues"
    );
}

#[test]
fn test_healthcheck_detects_missing_serve_arg_vscode() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code settings with tokensave but missing "serve" in args
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "/usr/local/bin/tokensave", "args": []}}}}"#,
    )
    .unwrap();

    // Also create CLI config so that check passes
    let cli_path = cli_config_path(home);
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(
        &cli_path,
        r#"{"mcpServers": {"tokensave": {"type": "stdio", "command": "/usr/local/bin/tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should detect missing 'serve' arg in VS Code settings"
    );
}

#[test]
fn test_healthcheck_detects_missing_serve_arg_cli() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code settings with correct config
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "/usr/local/bin/tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    // CLI config with tokensave but no "serve" in args
    let cli_path = cli_config_path(home);
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(
        &cli_path,
        r#"{"mcpServers": {"tokensave": {"type": "stdio", "command": "/usr/local/bin/tokensave", "args": []}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should detect missing 'serve' arg in CLI config"
    );
}

#[test]
fn test_healthcheck_detects_no_tokensave_in_existing_vscode() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code settings without tokensave
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(&settings_path, r#"{"editor.fontSize": 14}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should report issue when tokensave is not in VS Code settings"
    );
}

#[test]
fn test_healthcheck_detects_no_tokensave_in_existing_cli() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code settings with proper tokensave (so that check passes)
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    // Create CLI config without tokensave
    let cli_path = cli_config_path(home);
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(&cli_path, r#"{"mcpServers": {}}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should report issue when tokensave is not in CLI config"
    );
}

// ===========================================================================
// is_detected / has_tokensave
// ===========================================================================

#[test]
fn test_is_detected_empty_home() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !CopilotIntegration.is_detected(home),
        "should not be detected on empty home"
    );
}

#[test]
fn test_is_detected_with_copilot_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".copilot")).unwrap();
    assert!(
        CopilotIntegration.is_detected(home),
        "should be detected when .copilot dir exists"
    );
}

#[test]
fn test_is_detected_with_vscode_user_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    // Create the VS Code User dir
    #[cfg(target_os = "macos")]
    let user_dir = home.join("Library/Application Support/Code/User");
    #[cfg(target_os = "linux")]
    let user_dir = home.join(".config/Code/User");
    #[cfg(target_os = "windows")]
    let user_dir = home.join("AppData/Roaming/Code/User");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let user_dir = home.join(".config/Code/User");

    std::fs::create_dir_all(&user_dir).unwrap();
    assert!(
        CopilotIntegration.is_detected(home),
        "should be detected when VS Code User dir exists"
    );
}

#[test]
fn test_has_tokensave_before_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !CopilotIntegration.has_tokensave(home),
        "has_tokensave should be false before install"
    );
}

#[test]
fn test_has_tokensave_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    assert!(
        CopilotIntegration.has_tokensave(home),
        "has_tokensave should be true after install"
    );
}

#[test]
fn test_has_tokensave_after_uninstall() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();
    assert!(
        !CopilotIntegration.has_tokensave(home),
        "has_tokensave should be false after uninstall"
    );
}

#[test]
fn test_has_tokensave_vscode_only() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code settings with tokensave but no CLI config
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    assert!(
        CopilotIntegration.has_tokensave(home),
        "has_tokensave should be true with only VS Code config"
    );
}

#[test]
fn test_has_tokensave_cli_only() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create CLI config with tokensave but no VS Code settings
    let cli_path = cli_config_path(home);
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(
        &cli_path,
        r#"{"mcpServers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    assert!(
        CopilotIntegration.has_tokensave(home),
        "has_tokensave should be true with only CLI config"
    );
}

// ===========================================================================
// Name / ID
// ===========================================================================

#[test]
fn test_name_and_id() {
    assert_eq!(CopilotIntegration.name(), "GitHub Copilot");
    assert_eq!(CopilotIntegration.id(), "copilot");
}
