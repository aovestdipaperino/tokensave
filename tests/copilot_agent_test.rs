use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::{AgentIntegration, CopilotIntegration, DoctorCounters, HealthcheckContext};

mod common;
use common::{make_install_ctx as make_ctx, read_json};

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

/// Platform-specific path for VS Code's dedicated `mcp.json` (1.102+, GA MCP)
/// under the temp home.
fn vscode_mcp_json_path(home: &Path) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code/User/mcp.json")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".config/Code/User/mcp.json")
    }
    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Roaming/Code/User/mcp.json")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        home.join(".config/Code/User/mcp.json")
    }
}

/// Platform-specific path for VS Code Insiders' dedicated `mcp.json`.
fn vscode_insiders_mcp_json_path(home: &Path) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code - Insiders/User/mcp.json")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".config/Code - Insiders/User/mcp.json")
    }
    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Roaming/Code - Insiders/User/mcp.json")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        home.join(".config/Code - Insiders/User/mcp.json")
    }
}

/// Platform-specific path for VS Code Insiders' `User` dir (used to gate
/// Insiders install/detection, mirroring the "User" dir existence check).
fn vscode_insiders_user_dir(home: &Path) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code - Insiders/User")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".config/Code - Insiders/User")
    }
    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Roaming/Code - Insiders/User")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        home.join(".config/Code - Insiders/User")
    }
}

fn cli_config_path(home: &Path) -> std::path::PathBuf {
    home.join(".copilot/mcp-config.json")
}

/// Platform-specific path for the JetBrains plugin's mcp.json under the temp home.
fn jetbrains_config_path(home: &Path) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        home.join("AppData/Local/github-copilot/intellij/mcp.json")
    }
    #[cfg(not(target_os = "windows"))]
    {
        home.join(".config/github-copilot/intellij/mcp.json")
    }
}

/// Create the `github-copilot` plugin dir that gates the JetBrains install.
fn create_jetbrains_plugin_dir(home: &Path) {
    let plugin_dir = jetbrains_config_path(home)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::create_dir_all(plugin_dir).unwrap();
}

fn assert_mcp_json_has_tokensave(mcp_json_path: &Path) {
    let config = read_json(mcp_json_path);
    let ts = &config["servers"]["tokensave"];
    assert!(ts.is_object(), "servers.tokensave should be an object");
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
    assert_eq!(args, vec!["serve"], "args should be just [\"serve\"]");
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_vscode_mcp_json_with_mcp_server() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let mcp_json_path = vscode_mcp_json_path(home);
    assert!(mcp_json_path.exists(), "VS Code mcp.json should be created");
    assert_mcp_json_has_tokensave(&mcp_json_path);

    let ts = read_json(&mcp_json_path)["servers"]["tokensave"].clone();
    assert!(ts.get("cwd").is_none(), "cwd should not be set (issue #66)");
}

#[test]
fn test_install_does_not_write_settings_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    assert!(
        !vscode_settings_path(home).exists(),
        "install should not create settings.json (issue #266 — only mcp.json is written)"
    );
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
    assert_eq!(args, vec!["serve"], "args should be just [\"serve\"]");
    assert!(ts.get("cwd").is_none(), "cwd should not be set (issue #66)");
}

#[test]
fn test_install_leaves_unrelated_settings_json_untouched() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate VS Code settings with other, unrelated content.
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    let original = r#"{"editor.fontSize": 14, "workbench.colorTheme": "One Dark Pro"}"#;
    std::fs::write(&settings_path, original).unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let settings = read_json(&settings_path);
    assert_eq!(
        settings["editor.fontSize"], 14,
        "existing VS Code setting should be preserved"
    );
    assert!(
        settings.get("mcp").is_none(),
        "install should not add an mcp key to settings.json"
    );

    // tokensave should land only in mcp.json.
    assert_mcp_json_has_tokensave(&vscode_mcp_json_path(home));
}

#[test]
fn test_install_leaves_stale_legacy_settings_json_registration_untouched() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Simulate a pre-#266 install: tokensave registered in legacy
    // settings.json, alongside unrelated hand-written settings and a
    // comment — settings.json is hand-maintained and may be JSONC.
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    let original = r#"{
            // user's font preference
            "editor.fontSize": 14,
            "mcp": {
                "servers": {
                    "tokensave": {"command": "/usr/local/bin/tokensave", "args": ["serve"]},
                    "other-server": {"command": "foo", "args": []}
                }
            }
        }"#;
    std::fs::write(&settings_path, original).unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    // install must never write or strip settings.json — it is read-only for
    // install, even to migrate away a stale legacy entry. Rewriting it would
    // destroy comments in a hand-maintained file (review round 2, finding #1).
    let after = std::fs::read_to_string(&settings_path).unwrap();
    assert_eq!(
        after, original,
        "install must leave settings.json byte-for-byte unchanged"
    );

    assert_mcp_json_has_tokensave(&vscode_mcp_json_path(home));
}

#[test]
fn test_install_preserves_existing_mcp_json_servers() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"other-server": {"type": "stdio", "command": "foo", "args": []}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let config = read_json(&mcp_json_path);
    assert!(
        config["servers"]["other-server"].is_object(),
        "existing server should be preserved in mcp.json"
    );
    assert!(
        config["servers"]["tokensave"].is_object(),
        "tokensave should be added alongside existing servers"
    );
}

#[test]
fn test_install_accepts_jsonc_mcp_json_with_comments_and_trailing_commas() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // VS Code's mcp.json, like settings.json, is JSONC: comments and
    // trailing commas are valid and commonly present.
    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{
            // a hand-written comment above an existing server
            "servers": {
                "other-server": { "type": "stdio", "command": "foo", "args": [], },
            },
        }"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap_or_else(|e| {
        panic!("install should tolerate JSONC comments/trailing commas in mcp.json: {e}")
    });

    let config = read_json(&mcp_json_path);
    assert!(
        config["servers"]["other-server"].is_object(),
        "existing server should be preserved"
    );
    assert!(
        config["servers"]["tokensave"].is_object(),
        "tokensave should be added alongside the existing JSONC-authored server"
    );
}

#[test]
fn test_has_tokensave_and_doctor_detect_jsonc_mcp_json_registration() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{
            // tokensave was registered by hand, with comments left in place
            "servers": {
                "tokensave": { "type": "stdio", "command": "/usr/local/bin/tokensave", "args": ["serve"], },
            },
        }"#,
    )
    .unwrap();

    assert!(
        CopilotIntegration.has_tokensave(home),
        "has_tokensave should parse mcp.json as JSONC, not plain JSON"
    );

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(
        dc.issues, 0,
        "doctor should pass when mcp.json is valid JSONC with tokensave registered"
    );
}

#[test]
fn test_uninstall_removes_tokensave_from_jsonc_mcp_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{
            // comment before servers
            "servers": {
                "other-server": { "type": "stdio", "command": "foo", "args": [], },
                "tokensave": { "type": "stdio", "command": "/usr/local/bin/tokensave", "args": ["serve"], },
            },
        }"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.uninstall(&ctx).unwrap();

    let config = read_json(&mcp_json_path);
    assert!(
        config["servers"].get("tokensave").is_none(),
        "tokensave should be removed even though the file had comments/trailing commas"
    );
    assert!(
        config["servers"]["other-server"].is_object(),
        "unrelated server should be preserved"
    );
}

#[test]
fn test_install_creates_cli_config_and_mcp_json_when_both_stable_and_insiders_present() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(vscode_insiders_user_dir(home)).unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    assert_mcp_json_has_tokensave(&vscode_mcp_json_path(home));
    assert_mcp_json_has_tokensave(&vscode_insiders_mcp_json_path(home));
}

#[test]
fn test_install_skips_insiders_mcp_json_without_insiders_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    assert!(
        !vscode_insiders_mcp_json_path(home).exists(),
        "Insiders mcp.json should not be created when the Insiders User dir is absent"
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

    let config = read_json(&vscode_mcp_json_path(home));
    assert!(
        config["servers"]["tokensave"].is_object(),
        "tokensave should still be registered after double install"
    );
    // Ensure there's exactly one "tokensave" key (no duplication)
    let servers = config["servers"].as_object().unwrap();
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
    assert_eq!(
        ts_count, 1,
        "tokensave should appear exactly once in CLI config"
    );
}

#[test]
fn test_install_creates_jetbrains_config_when_plugin_dir_exists() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    create_jetbrains_plugin_dir(home);

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let jetbrains_path = jetbrains_config_path(home);
    assert!(
        jetbrains_path.exists(),
        "JetBrains mcp.json should be created when the plugin dir exists"
    );

    let config = read_json(&jetbrains_path);
    let ts = &config["servers"]["tokensave"];
    assert!(
        ts.is_object(),
        "servers.tokensave should be an object in JetBrains config"
    );
    assert_eq!(ts["command"].as_str().unwrap(), "/usr/local/bin/tokensave");
    let args: Vec<&str> = ts["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(args, vec!["serve"], "args should be just [\"serve\"]");

    let instructions = jetbrains_path
        .parent()
        .unwrap()
        .join("global-copilot-instructions.md");
    assert!(
        instructions.exists(),
        "JetBrains global instructions should be created"
    );
}

#[test]
fn test_install_skips_jetbrains_config_without_plugin_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    assert!(
        !jetbrains_config_path(home).exists(),
        "JetBrains mcp.json should not be created when the plugin dir is absent"
    );
}

#[test]
fn test_install_preserves_existing_jetbrains_servers() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let jetbrains_path = jetbrains_config_path(home);
    std::fs::create_dir_all(jetbrains_path.parent().unwrap()).unwrap();
    std::fs::write(
        &jetbrains_path,
        r#"{"servers": {"other-server": {"command": "foo", "args": []}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();

    let config = read_json(&jetbrains_path);
    assert!(
        config["servers"]["other-server"].is_object(),
        "existing server should be preserved in JetBrains config"
    );
    assert!(
        config["servers"]["tokensave"].is_object(),
        "tokensave should be added alongside existing servers"
    );
}

#[test]
fn test_install_idempotent_jetbrains() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    create_jetbrains_plugin_dir(home);
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.install(&ctx).unwrap();

    let config = read_json(&jetbrains_config_path(home));
    let servers = config["servers"].as_object().unwrap();
    let ts_count = servers.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(
        ts_count, 1,
        "tokensave should appear exactly once in JetBrains config"
    );
}

// ===========================================================================
// Uninstall verification
// ===========================================================================

#[test]
fn test_uninstall_removes_vscode_mcp_json_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    let mcp_json_path = vscode_mcp_json_path(home);
    // tokensave was the only server, so the (otherwise-empty) file is removed.
    if mcp_json_path.exists() {
        let config = read_json(&mcp_json_path);
        let has_tokensave = config
            .get("servers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(
            !has_tokensave,
            "servers.tokensave should be removed from mcp.json"
        );
    }
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

    assert!(
        cli_path.exists(),
        "CLI config should still exist when other servers remain"
    );
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
fn test_uninstall_preserves_other_mcp_json_servers() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"other-tool": {"type": "stdio", "command": "other-tool", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    assert!(
        mcp_json_path.exists(),
        "mcp.json should still exist when other servers remain"
    );
    let config = read_json(&mcp_json_path);
    assert!(
        config["servers"]["other-tool"].is_object(),
        "other server should be preserved"
    );
    let has_tokensave = config
        .get("servers")
        .and_then(|v| v.get("tokensave"))
        .is_some();
    assert!(!has_tokensave, "tokensave should be removed");
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

#[test]
fn test_uninstall_removes_jetbrains_config() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    create_jetbrains_plugin_dir(home);
    let ctx = make_ctx(home);

    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    let jetbrains_path = jetbrains_config_path(home);
    // When tokensave was the only entry, the file should be removed entirely
    if jetbrains_path.exists() {
        let config = read_json(&jetbrains_path);
        let has_tokensave = config
            .get("servers")
            .and_then(|v| v.get("tokensave"))
            .is_some();
        assert!(
            !has_tokensave,
            "servers.tokensave should be removed from JetBrains config"
        );
    }
    let instructions = jetbrains_path
        .parent()
        .unwrap()
        .join("global-copilot-instructions.md");
    assert!(
        !instructions.exists()
            || !std::fs::read_to_string(&instructions)
                .unwrap()
                .contains("tokensave"),
        "tokensave rules should be removed from JetBrains global instructions"
    );
}

#[test]
fn test_uninstall_preserves_other_jetbrains_servers() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let jetbrains_path = jetbrains_config_path(home);
    std::fs::create_dir_all(jetbrains_path.parent().unwrap()).unwrap();
    std::fs::write(
        &jetbrains_path,
        r#"{"servers": {"other-tool": {"command": "other-tool", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.install(&ctx).unwrap();
    CopilotIntegration.uninstall(&ctx).unwrap();

    assert!(
        jetbrains_path.exists(),
        "JetBrains config should still exist when other servers remain"
    );
    let config = read_json(&jetbrains_path);
    assert!(
        config["servers"]["other-tool"].is_object(),
        "other server should be preserved"
    );
    let has_tokensave = config
        .get("servers")
        .and_then(|v| v.get("tokensave"))
        .is_some();
    assert!(!has_tokensave, "tokensave should be removed");
}

#[test]
fn test_uninstall_cleans_legacy_settings_json_registration() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Simulate a pre-#266 install that only ever wrote settings.json.
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.uninstall(&ctx).unwrap();

    let settings = read_json(&settings_path);
    let has_tokensave = settings
        .get("mcp")
        .and_then(|v| v.get("servers"))
        .and_then(|v| v.get("tokensave"))
        .is_some();
    assert!(
        !has_tokensave,
        "legacy settings.json registration should be cleaned up by uninstall"
    );
}

#[test]
fn test_uninstall_cleans_both_mcp_json_and_legacy_settings_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let settings_path = vscode_settings_path(home);
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    CopilotIntegration.uninstall(&ctx).unwrap();

    if mcp_json_path.exists() {
        let config = read_json(&mcp_json_path);
        assert!(
            config
                .get("servers")
                .and_then(|v| v.get("tokensave"))
                .is_none(),
            "mcp.json registration should be removed"
        );
    }
    let settings = read_json(&settings_path);
    assert!(
        settings
            .get("mcp")
            .and_then(|v| v.get("servers"))
            .and_then(|v| v.get("tokensave"))
            .is_none(),
        "legacy settings.json registration should also be removed"
    );
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

    // Create VS Code mcp.json with tokensave but missing "serve" in args
    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"tokensave": {"type": "stdio", "command": "/usr/local/bin/tokensave", "args": []}}}"#,
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
        "healthcheck should detect missing 'serve' arg in VS Code mcp.json"
    );
}

#[test]
fn test_healthcheck_detects_missing_serve_arg_cli() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code mcp.json with correct config
    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"tokensave": {"type": "stdio", "command": "/usr/local/bin/tokensave", "args": ["serve"]}}}"#,
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

    // Create VS Code settings.json (legacy location) without tokensave and no mcp.json
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
        "healthcheck should report issue when tokensave is not registered anywhere for VS Code"
    );
}

#[test]
fn test_healthcheck_detects_no_tokensave_in_existing_cli() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code mcp.json with proper tokensave (so that check passes)
    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}"#,
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

#[test]
fn test_healthcheck_detects_missing_serve_arg_jetbrains() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // JetBrains config with tokensave but no "serve" in args
    let jetbrains_path = jetbrains_config_path(home);
    std::fs::create_dir_all(jetbrains_path.parent().unwrap()).unwrap();
    std::fs::write(
        &jetbrains_path,
        r#"{"servers": {"tokensave": {"command": "/usr/local/bin/tokensave", "args": []}}}"#,
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
        "healthcheck should detect missing 'serve' arg in JetBrains config"
    );
}

#[test]
fn test_healthcheck_passes_with_modern_mcp_json_only() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(
        dc.issues, 0,
        "a valid mcp.json-only registration should not fail the VS Code check"
    );
}

#[test]
fn test_healthcheck_passes_with_legacy_settings_json_only() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Only a legacy settings.json registration exists, no mcp.json at all.
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(
        dc.issues, 0,
        "a valid legacy settings.json-only registration should still pass (back-compat)"
    );
}

#[test]
fn test_healthcheck_warns_on_duplicate_registration_in_both_locations() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    let settings_path = vscode_settings_path(home);
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    CopilotIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(
        dc.issues, 0,
        "having both a valid mcp.json and legacy settings.json registration should still pass"
    );
    assert!(
        dc.warnings > 0,
        "a duplicate registration across mcp.json and settings.json should warn"
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
fn test_is_detected_with_jetbrains_plugin_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    create_jetbrains_plugin_dir(home);
    assert!(
        CopilotIntegration.is_detected(home),
        "should be detected when the github-copilot plugin dir exists"
    );
}

#[test]
fn test_has_tokensave_jetbrains_only() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let jetbrains_path = jetbrains_config_path(home);
    std::fs::create_dir_all(jetbrains_path.parent().unwrap()).unwrap();
    std::fs::write(
        &jetbrains_path,
        r#"{"servers": {"tokensave": {"command": "tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    assert!(
        CopilotIntegration.has_tokensave(home),
        "has_tokensave should be true with only JetBrains config"
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
fn test_has_tokensave_vscode_mcp_json_only() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create VS Code mcp.json with tokensave but no CLI config
    let mcp_json_path = vscode_mcp_json_path(home);
    std::fs::create_dir_all(mcp_json_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_json_path,
        r#"{"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}"#,
    )
    .unwrap();

    assert!(
        CopilotIntegration.has_tokensave(home),
        "has_tokensave should be true with only VS Code mcp.json config"
    );
}

#[test]
fn test_has_tokensave_legacy_vscode_settings_only() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Create legacy VS Code settings.json with tokensave, no mcp.json, no CLI config.
    let settings_path = vscode_settings_path(home);
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        r#"{"mcp": {"servers": {"tokensave": {"type": "stdio", "command": "tokensave", "args": ["serve"]}}}}"#,
    )
    .unwrap();

    assert!(
        CopilotIntegration.has_tokensave(home),
        "has_tokensave should be true with only legacy VS Code settings.json config (back-compat)"
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
// primary_config_path
// ===========================================================================

#[test]
fn test_primary_config_path_is_mcp_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert_eq!(
        CopilotIntegration.primary_config_path(home),
        Some(vscode_mcp_json_path(home)),
        "primary_config_path should point at the modern mcp.json, not settings.json"
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
