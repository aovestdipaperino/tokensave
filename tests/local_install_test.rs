use serde_json::Value;
use tempfile::TempDir;
use tokensave::agents::get_integration;
use tokensave::agents::{DoctorCounters, HealthcheckContext, InstallContext, InstallScope};

fn read_json(p: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn codex_does_not_support_local() {
    let ag = get_integration("codex").unwrap();
    assert!(!ag.supports_local(), "codex must remain global-only");
}

#[test]
fn claude_supports_local() {
    assert!(get_integration("claude").unwrap().supports_local());
}

#[test]
fn claude_local_writes_project_files_only() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();

    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec!["mcp__tokensave__search".to_string()],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
    };
    get_integration("claude").unwrap().install(&ctx).unwrap();

    // Project files exist with tokensave registered.
    let mcp = read_json(&proj.path().join(".mcp.json"));
    assert!(mcp["mcpServers"]["tokensave"].is_object());
    let settings = read_json(&proj.path().join(".claude/settings.json"));
    assert!(settings["hooks"]["PreToolUse"].is_array());
    assert!(proj.path().join("CLAUDE.md").exists());

    // Global config under home was NOT touched.
    assert!(
        !home.path().join(".claude.json").exists(),
        "must not write global ~/.claude.json"
    );
    assert!(
        !home.path().join(".claude/settings.json").exists(),
        "must not write global settings"
    );
}

#[test]
fn doctor_preserves_valid_local_mcp_json() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let mcp_path = proj.path().join(".mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"tokensave":{"command":"/usr/bin/tokensave","args":["serve"]}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let ctx = HealthcheckContext {
        home: home.path().to_path_buf(),
        project_path: proj.path().to_path_buf(),
    };
    get_integration("claude")
        .unwrap()
        .healthcheck(&mut dc, &ctx);

    assert!(
        mcp_path.exists(),
        "doctor must NOT delete a valid local .mcp.json"
    );
}

#[test]
fn cursor_local_writes_project_mcp_only() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
    };
    get_integration("cursor").unwrap().install(&ctx).unwrap();
    let mcp = read_json(&proj.path().join(".cursor/mcp.json"));
    assert!(mcp["mcpServers"]["tokensave"].is_object());
    assert!(
        !home.path().join(".cursor/mcp.json").exists(),
        "must not write global cursor config"
    );
}

#[test]
fn gemini_local_writes_project_settings() {
    let home = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let ctx = InstallContext {
        home: home.path().to_path_buf(),
        tokensave_bin: "/usr/bin/tokensave".to_string(),
        tool_permissions: vec![],
        scope: InstallScope::Local {
            project_path: proj.path().to_path_buf(),
        },
    };
    get_integration("gemini").unwrap().install(&ctx).unwrap();
    let settings = read_json(&proj.path().join(".gemini/settings.json"));
    assert!(settings["mcpServers"]["tokensave"].is_object());
    assert!(!home.path().join(".gemini/settings.json").exists());
}
