use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tempfile::TempDir;
use tokensave::agents::{AgentIntegration, DoctorCounters, HealthcheckContext, PiIntegration};

mod common;
use common::{make_install_ctx as make_ctx, read_json};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// `PI_CODING_AGENT_DIR` is a process-global env var that `pi_config_path`
// reads. Cargo runs integration tests in parallel within a binary, so every
// test in this file must hold this lock to avoid one test's env-var mutation
// leaking into another's config-path resolution.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Default (no env override) Pi config path under a fake home.
fn pi_config_path(home: &Path) -> PathBuf {
    home.join(".pi/agent/mcp.json")
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_mcp_json() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PiIntegration.install(&ctx).unwrap();

    let mcp_path = pi_config_path(home);
    assert!(mcp_path.exists(), "mcp.json should be created");

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
fn test_install_preserves_existing_server() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate mcp.json with an unrelated server and a top-level key.
    let mcp_path = pi_config_path(home);
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"someSetting": true, "mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    PiIntegration.install(&ctx).unwrap();

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
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    PiIntegration.install(&ctx).unwrap();
    PiIntegration.install(&ctx).unwrap();

    let config = read_json(&pi_config_path(home));
    let servers = config["mcpServers"].as_object().unwrap();
    let ts_count = servers.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(ts_count, 1, "tokensave should appear exactly once");
}

#[test]
fn test_primary_config_path_matches_install_target() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PiIntegration.install(&ctx).unwrap();

    let primary = PiIntegration.primary_config_path(home).unwrap();
    assert!(
        primary.exists(),
        "primary_config_path should exist after install"
    );
    assert_eq!(
        primary,
        pi_config_path(home),
        "primary_config_path should match where install wrote"
    );
}

#[test]
fn test_pi_coding_agent_dir_override_is_honored() {
    let _guard = ENV_LOCK.lock().unwrap();

    let home_dir = TempDir::new().unwrap();
    let override_dir = TempDir::new().unwrap();
    let home = home_dir.path();

    std::env::set_var("PI_CODING_AGENT_DIR", override_dir.path());

    let ctx = make_ctx(home);
    PiIntegration.install(&ctx).unwrap();

    let override_path = override_dir.path().join("mcp.json");
    assert!(
        override_path.exists(),
        "install should write to $PI_CODING_AGENT_DIR/mcp.json"
    );
    assert!(
        !pi_config_path(home).exists(),
        "default ~/.pi/agent/mcp.json should NOT be written when env override is set"
    );

    // primary_config_path must follow the override too.
    assert_eq!(
        PiIntegration.primary_config_path(home).unwrap(),
        override_path,
        "primary_config_path should honor $PI_CODING_AGENT_DIR"
    );

    let config = read_json(&override_path);
    assert!(config["mcpServers"]["tokensave"].is_object());

    std::env::remove_var("PI_CODING_AGENT_DIR");
}

// ===========================================================================
// Uninstall verification
// ===========================================================================

#[test]
fn test_uninstall_removes_only_tokensave() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // Pre-populate with another server, then install tokensave alongside it.
    let mcp_path = pi_config_path(home);
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    PiIntegration.install(&ctx).unwrap();
    PiIntegration.uninstall(&ctx).unwrap();

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
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    PiIntegration.install(&ctx).unwrap();
    PiIntegration.uninstall(&ctx).unwrap();

    assert!(
        !pi_config_path(home).exists(),
        "mcp.json should be deleted when tokensave was the only entry"
    );
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PiIntegration.uninstall(&ctx).unwrap();
}

// ===========================================================================
// Healthcheck verification
// ===========================================================================

#[test]
fn test_healthcheck_clean_install_no_issues() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    PiIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    PiIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean Pi install should have no issues");
}

#[test]
fn test_healthcheck_missing_config_produces_warning() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    PiIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0 || dc.issues > 0,
        "healthcheck on empty dir should report warnings or issues"
    );
}

#[test]
fn test_healthcheck_detects_missing_mcp_entry() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mcp_path = pi_config_path(home);
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(&mcp_path, r#"{"mcpServers": {}}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    PiIntegration.healthcheck(&mut dc, &hctx);
    assert!(dc.issues > 0, "healthcheck should detect missing MCP entry");
}

// ===========================================================================
// is_detected / has_tokensave
// ===========================================================================

#[test]
fn test_is_detected_empty_home() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !PiIntegration.is_detected(home),
        "should not be detected on empty home"
    );
}

#[test]
fn test_is_detected_with_pi_dir() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".pi/agent")).unwrap();
    assert!(
        PiIntegration.is_detected(home),
        "should be detected when .pi/agent exists"
    );
}

#[test]
fn test_has_tokensave_before_and_after_install() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("PI_CODING_AGENT_DIR");

    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !PiIntegration.has_tokensave(home),
        "has_tokensave should be false before install"
    );

    let ctx = make_ctx(home);
    PiIntegration.install(&ctx).unwrap();
    assert!(
        PiIntegration.has_tokensave(home),
        "has_tokensave should be true after install"
    );

    PiIntegration.uninstall(&ctx).unwrap();
    assert!(
        !PiIntegration.has_tokensave(home),
        "has_tokensave should be false after uninstall"
    );
}

// ===========================================================================
// Name / ID
// ===========================================================================

#[test]
fn test_name_and_id() {
    assert_eq!(PiIntegration.name(), "Pi");
    assert_eq!(PiIntegration.id(), "pi");
}
