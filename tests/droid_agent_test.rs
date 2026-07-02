use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokensave::agents::{
    expected_tool_perms, AgentIntegration, DoctorCounters, DroidIntegration, HealthcheckContext,
    InstallContext, InstallScope,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: expected_tool_perms(),
        scope: InstallScope::Global,
    }
}

fn make_local_ctx(home: &Path, project: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: expected_tool_perms(),
        scope: InstallScope::Local {
            project_path: project.to_path_buf(),
        },
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}

fn mcp_path(home: &Path) -> PathBuf {
    home.join(".factory/mcp.json")
}

fn agents_md_path(home: &Path) -> PathBuf {
    home.join(".factory/AGENTS.md")
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_mcp_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let path = mcp_path(home);
    assert!(path.exists(), "mcp.json should be created");

    let config = read_json(&path);
    let ts = &config["mcpServers"]["tokensave"];
    assert!(ts.is_object(), "mcpServers.tokensave should be an object");
    assert_eq!(
        ts["type"].as_str().unwrap(),
        "stdio",
        "type should be stdio"
    );
    assert_eq!(
        ts["command"].as_str().unwrap(),
        "/usr/local/bin/tokensave",
        "command should be the tokensave binary path"
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
fn test_install_appends_prompt_rules() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let rules = std::fs::read_to_string(agents_md_path(home)).unwrap();
    assert!(
        rules.contains("## Prefer tokensave MCP tools"),
        "AGENTS.md should contain the tokensave rules marker"
    );
}

#[test]
fn test_install_preserves_existing_server() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = mcp_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"someSetting": true, "mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let config = read_json(&path);
    assert!(
        config["someSetting"].as_bool().unwrap(),
        "unrelated top-level key should be preserved"
    );
    assert!(
        config["mcpServers"]["other-tool"].is_object(),
        "existing MCP server should be preserved"
    );
    assert!(
        config["mcpServers"]["tokensave"].is_object(),
        "tokensave should be added"
    );
}

#[test]
fn test_install_idempotent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.install(&ctx).unwrap();

    let config = read_json(&mcp_path(home));
    let servers = config["mcpServers"].as_object().unwrap();
    let ts_count = servers.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(ts_count, 1, "tokensave should appear exactly once");

    let rules = std::fs::read_to_string(agents_md_path(home)).unwrap();
    assert_eq!(
        rules.matches("## Prefer tokensave MCP tools").count(),
        1,
        "rules block should appear exactly once"
    );
}

#[test]
fn test_primary_config_path_matches_install_target() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let primary = DroidIntegration.primary_config_path(home).unwrap();
    assert!(
        primary.exists(),
        "primary_config_path should exist after install"
    );
    assert_eq!(
        primary,
        mcp_path(home),
        "primary_config_path should match where install wrote"
    );
}

#[test]
fn test_local_install_targets_project() {
    let home_dir = TempDir::new().unwrap();
    let proj_dir = TempDir::new().unwrap();
    let home = home_dir.path();
    let project = proj_dir.path();

    let ctx = make_local_ctx(home, project);
    DroidIntegration.install(&ctx).unwrap();

    assert!(
        project.join(".factory/mcp.json").exists(),
        "--local should write <project>/.factory/mcp.json"
    );
    assert!(
        project.join("AGENTS.md").exists(),
        "--local should write <project>/AGENTS.md"
    );
    assert!(
        !mcp_path(home).exists(),
        "global ~/.factory/mcp.json should not be written for --local"
    );
}

// ===========================================================================
// Uninstall verification
// ===========================================================================

#[test]
fn test_uninstall_removes_only_tokensave() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = mcp_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    assert!(
        path.exists(),
        "config should still exist because another server remains"
    );
    let config = read_json(&path);
    assert!(
        config["mcpServers"]["other-tool"].is_object(),
        "other server should be preserved"
    );
    assert!(
        config
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_none(),
        "tokensave should be removed"
    );
}

#[test]
fn test_uninstall_removes_empty_config_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    assert!(
        !mcp_path(home).exists(),
        "mcp.json should be deleted when tokensave was the only entry"
    );
}

#[test]
fn test_uninstall_removes_prompt_rules() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    let path = agents_md_path(home);
    if path.exists() {
        let rules = std::fs::read_to_string(&path).unwrap();
        assert!(
            !rules.contains("tokensave"),
            "tokensave rules should be removed from AGENTS.md"
        );
    }
}

#[test]
fn test_uninstall_preserves_other_agents_md_content() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = agents_md_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "# My rules\n\nKeep this.\n").unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    let rules = std::fs::read_to_string(&path).unwrap();
    assert!(rules.contains("Keep this."), "user content should survive");
    assert!(
        !rules.contains("tokensave"),
        "tokensave rules should be removed"
    );
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.uninstall(&ctx).unwrap();
}

// ===========================================================================
// Healthcheck verification
// ===========================================================================

#[test]
fn test_healthcheck_clean_install_no_issues() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean droid install should have no issues");
}

#[test]
fn test_healthcheck_missing_config_produces_warning() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0 || dc.issues > 0,
        "healthcheck on empty dir should report warnings or issues"
    );
}

#[test]
fn test_healthcheck_detects_missing_mcp_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = mcp_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert!(dc.issues > 0, "healthcheck should detect missing MCP entry");
}

// ===========================================================================
// is_detected / has_tokensave
// ===========================================================================

#[test]
fn test_is_detected_empty_home() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !DroidIntegration.is_detected(home),
        "should not be detected on empty home"
    );
}

#[test]
fn test_is_detected_with_factory_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".factory")).unwrap();
    assert!(
        DroidIntegration.is_detected(home),
        "should be detected when ~/.factory exists"
    );
}

#[test]
fn test_has_tokensave_before_and_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !DroidIntegration.has_tokensave(home),
        "has_tokensave should be false before install"
    );

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();
    assert!(
        DroidIntegration.has_tokensave(home),
        "has_tokensave should be true after install"
    );

    DroidIntegration.uninstall(&ctx).unwrap();
    assert!(
        !DroidIntegration.has_tokensave(home),
        "has_tokensave should be false after uninstall"
    );
}

// ===========================================================================
// Name / ID
// ===========================================================================

#[test]
fn test_name_and_id() {
    assert_eq!(DroidIntegration.name(), "Factory Droid");
    assert_eq!(DroidIntegration.id(), "droid");
}
