use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokensave::agents::{
    AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext, InstallScope,
    PlankIntegration,
};

mod common;
use common::{make_install_ctx as make_ctx, read_json};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Plank's user-scope config path under a fake home.
fn plank_global_config_path(home: &Path) -> PathBuf {
    home.join(".plank/.mcp.json")
}

/// Plank's project-scope config path under a fake project root.
fn plank_local_config_path(project: &Path) -> PathBuf {
    project.join(".mcp.json")
}

/// A `--local` install context rooted at `project`.
fn make_local_ctx(home: &Path, project: &Path) -> InstallContext {
    let mut ctx = make_ctx(home);
    ctx.scope = InstallScope::Local {
        project_path: project.to_path_buf(),
    };
    ctx
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_mcp_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PlankIntegration.install(&ctx).unwrap();

    let mcp_path = plank_global_config_path(home);
    assert!(mcp_path.exists(), ".mcp.json should be created");

    let config = read_json(&mcp_path);
    let ts = &config["mcpServers"]["tokensave"];
    assert!(ts.is_object(), "mcpServers.tokensave should be an object");
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
fn test_local_install_writes_project_mcp_json() {
    let home_dir = TempDir::new().unwrap();
    let project_dir = TempDir::new().unwrap();
    let home = home_dir.path();
    let project = project_dir.path();

    assert!(
        PlankIntegration.supports_local(),
        "Plank has a project-scoped config"
    );

    let ctx = make_local_ctx(home, project);
    PlankIntegration.install(&ctx).unwrap();

    let local_path = plank_local_config_path(project);
    assert!(
        local_path.exists(),
        "--local install should write <project>/.mcp.json"
    );
    assert!(
        !plank_global_config_path(home).exists(),
        "--local install should not touch the user-scope config"
    );

    let config = read_json(&local_path);
    assert!(config["mcpServers"]["tokensave"].is_object());

    // And uninstall targets the same file.
    PlankIntegration.uninstall(&ctx).unwrap();
    assert!(
        !local_path.exists(),
        "local .mcp.json should be removed when tokensave was the only entry"
    );
}

#[test]
fn test_install_preserves_existing_server() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate .mcp.json with an unrelated server and a top-level key.
    let mcp_path = plank_global_config_path(home);
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"someSetting": true, "mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    PlankIntegration.install(&ctx).unwrap();

    let config = read_json(&mcp_path);
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

    PlankIntegration.install(&ctx).unwrap();
    PlankIntegration.install(&ctx).unwrap();

    let config = read_json(&plank_global_config_path(home));
    let servers = config["mcpServers"].as_object().unwrap();
    let ts_count = servers.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(ts_count, 1, "tokensave should appear exactly once");
}

#[test]
fn test_primary_config_path_matches_install_target() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PlankIntegration.install(&ctx).unwrap();

    let primary = PlankIntegration.primary_config_path(home).unwrap();
    assert!(
        primary.exists(),
        "primary_config_path should exist after install"
    );
    assert_eq!(
        primary,
        plank_global_config_path(home),
        "primary_config_path should match where install wrote"
    );
}

// ===========================================================================
// Uninstall verification
// ===========================================================================

#[test]
fn test_uninstall_removes_only_tokensave() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_path = plank_global_config_path(home);
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    PlankIntegration.install(&ctx).unwrap();
    PlankIntegration.uninstall(&ctx).unwrap();

    assert!(
        mcp_path.exists(),
        "config should still exist because another server remains"
    );
    let config = read_json(&mcp_path);
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

    PlankIntegration.install(&ctx).unwrap();
    PlankIntegration.uninstall(&ctx).unwrap();

    assert!(
        !plank_global_config_path(home).exists(),
        ".mcp.json should be deleted when tokensave was the only entry"
    );
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PlankIntegration.uninstall(&ctx).unwrap();
}

// ===========================================================================
// Healthcheck verification
// ===========================================================================

#[test]
fn test_healthcheck_clean_install_no_issues() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PlankIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    PlankIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Plank install should have no issues");
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
    PlankIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0 || dc.issues > 0,
        "healthcheck on empty dir should report warnings or issues"
    );
}

#[test]
fn test_healthcheck_detects_missing_mcp_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_path = plank_global_config_path(home);
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(&mcp_path, r#"{"mcpServers": {}}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    PlankIntegration.healthcheck(&mut dc, &hctx);
    assert!(dc.issues > 0, "healthcheck should detect missing MCP entry");
}

// ===========================================================================
// is_detected / has_tokensave
// ===========================================================================

#[test]
fn test_is_detected_empty_home() {
    let dir = TempDir::new().unwrap();
    assert!(
        !PlankIntegration.is_detected(dir.path()),
        "should not be detected on empty home"
    );
}

#[test]
fn test_is_detected_with_plank_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".plank")).unwrap();
    assert!(
        PlankIntegration.is_detected(home),
        "should be detected when .plank exists"
    );
}

#[test]
fn test_has_tokensave_before_and_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !PlankIntegration.has_tokensave(home),
        "has_tokensave should be false before install"
    );

    let ctx = make_ctx(home);
    PlankIntegration.install(&ctx).unwrap();
    assert!(
        PlankIntegration.has_tokensave(home),
        "has_tokensave should be true after install"
    );

    PlankIntegration.uninstall(&ctx).unwrap();
    assert!(
        !PlankIntegration.has_tokensave(home),
        "has_tokensave should be false after uninstall"
    );
}

// ===========================================================================
// Name / ID
// ===========================================================================

#[test]
fn test_name_and_id() {
    assert_eq!(PlankIntegration.name(), "Plank");
    assert_eq!(PlankIntegration.id(), "plank");
}
