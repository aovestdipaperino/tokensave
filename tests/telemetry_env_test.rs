//! `TOKENSAVE_TELEMETRY=off` and `DO_NOT_TRACK=1` turn the worldwide counter
//! off for a run, whatever `config.toml` says (#1066).
//!
//! The variables are passed to a child process with `Command::env`, so this
//! file never mutates the test process's environment.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

/// Runs a cheap command that still goes through the startup flush, with the
/// given extra environment.
fn run_gain(home: &Path, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    cmd.env("HOME", home)
        .env("USERPROFILE", home)
        .env("TOKENSAVE_UPDATE_CHECK", "off")
        .env_remove("TOKENSAVE_SKIP_AGENT_MAINTENANCE")
        .env_remove("TOKENSAVE_TELEMETRY")
        .env_remove("DO_NOT_TRACK")
        .args(["gain", "--all", "--range", "all", "--json"]);
    for (key, value) in env {
        cmd.env(key, value);
    }
    let output = cmd.output().expect("run tokensave gain");
    assert!(
        output.status.success(),
        "gain failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn config_path(home: &Path) -> std::path::PathBuf {
    home.join(".tokensave").join("config.toml")
}

/// A home whose config has an upload that is due right now: tokens pending,
/// never uploaded, no recent failure, uploads enabled.
fn home_with_due_upload() -> TempDir {
    let home = TempDir::new().expect("temp home");
    std::fs::create_dir_all(home.path().join(".tokensave")).expect("state dir");
    std::fs::write(
        config_path(home.path()),
        "upload_enabled = true\n\
         pending_upload = 100\n\
         last_upload_at = 0\n\
         last_flush_attempt_at = 0\n\
         last_pricing_fetch_at = 4102444800\n",
    )
    .expect("config");
    home
}

fn read_config(home: &Path) -> toml::Table {
    let text = std::fs::read_to_string(config_path(home)).expect("read config");
    text.parse().expect("config is TOML")
}

fn assert_no_upload_attempt(home: &Path, var: &str) {
    let config = read_config(home);
    assert_eq!(
        config
            .get("pending_upload")
            .and_then(toml::Value::as_integer),
        Some(100),
        "{var}: the pending count must be left alone"
    );
    let attempted = config
        .get("last_flush_attempt_at")
        .and_then(toml::Value::as_integer)
        .unwrap_or(0);
    assert_eq!(attempted, 0, "{var}: no upload may be attempted");
}

#[test]
fn tokensave_telemetry_off_skips_a_due_upload() {
    let home = home_with_due_upload();
    run_gain(home.path(), &[("TOKENSAVE_TELEMETRY", "off")]);
    assert_no_upload_attempt(home.path(), "TOKENSAVE_TELEMETRY=off");
}

#[test]
fn do_not_track_skips_a_due_upload() {
    let home = home_with_due_upload();
    run_gain(home.path(), &[("DO_NOT_TRACK", "1")]);
    assert_no_upload_attempt(home.path(), "DO_NOT_TRACK=1");
}

#[test]
fn telemetry_off_suppresses_the_first_run_counter_notice() {
    let home = TempDir::new().expect("temp home");
    let output = run_gain(home.path(), &[("TOKENSAVE_TELEMETRY", "off")]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("worldwide counter"),
        "the opt-out notice is moot when telemetry is off: {stderr}"
    );
}
