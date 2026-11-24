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

fn state_dir(home: &Path) -> std::path::PathBuf {
    home.join(".tokensave")
}

/// A home whose upload is due right now: tokens pending, never uploaded, no
/// recent failure, uploads enabled. Written in the current split layout
/// (preferences in `config.toml`, volatile counters in `state.toml`), because
/// any save during the run migrates legacy state keys out of `config.toml`.
fn home_with_due_upload() -> TempDir {
    let home = TempDir::new().expect("temp home");
    let dir = state_dir(home.path());
    std::fs::create_dir_all(&dir).expect("state dir");
    std::fs::write(dir.join("config.toml"), "upload_enabled = true\n").expect("config");
    std::fs::write(
        dir.join("state.toml"),
        "pending_upload = 100\n\
         last_upload_at = 0\n\
         last_flush_attempt_at = 0\n\
         last_pricing_fetch_at = 4102444800\n",
    )
    .expect("state");
    home
}

fn read_state(home: &Path) -> toml::Table {
    let text = std::fs::read_to_string(state_dir(home).join("state.toml")).expect("read state");
    text.parse().expect("state is TOML")
}

fn assert_no_upload_attempt(home: &Path, var: &str) {
    let state = read_state(home);
    assert_eq!(
        state
            .get("pending_upload")
            .and_then(toml::Value::as_integer),
        Some(100),
        "{var}: the pending count must be left alone"
    );
    let attempted = state
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
