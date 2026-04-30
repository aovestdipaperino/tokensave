//! Tests that the hidden `extract-worker` subcommand cannot be invoked
//! by users in a way that would let them turn the tokensave binary into
//! an arbitrary code-execution vector.

use std::io::Write;
use std::process::{Command, Stdio};

fn worker_bin() -> &'static str {
    env!("CARGO_BIN_EXE_tokensave")
}

#[test]
fn worker_without_token_env_var_exits_nonzero() {
    let mut child = Command::new(worker_bin())
        .arg("extract-worker")
        .env_remove("TOKENSAVE_WORKER_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn worker");
    drop(child.stdin.take());
    let status = child.wait().expect("wait");
    assert!(
        !status.success(),
        "worker must reject invocation without TOKENSAVE_WORKER_TOKEN env var"
    );
}

#[test]
fn worker_with_malformed_token_env_var_exits_nonzero() {
    let mut child = Command::new(worker_bin())
        .arg("extract-worker")
        .env("TOKENSAVE_WORKER_TOKEN", "not-hex-at-all")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn worker");
    drop(child.stdin.take());
    let status = child.wait().expect("wait");
    assert!(!status.success(), "malformed token must be rejected");
}

#[test]
fn worker_with_wrong_length_token_exits_nonzero() {
    // 16 hex chars = 8 bytes, but TOKEN_LEN is 32.
    let mut child = Command::new(worker_bin())
        .arg("extract-worker")
        .env("TOKENSAVE_WORKER_TOKEN", "deadbeefdeadbeef")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn worker");
    drop(child.stdin.take());
    let status = child.wait().expect("wait");
    assert!(!status.success(), "short token must be rejected");
}

#[test]
fn worker_with_correct_env_but_wrong_stdin_token_exits_nonzero() {
    // Set a valid-format env var, then send wrong bytes on stdin.
    // Worker should detect the mismatch and exit non-zero.
    let token_hex = "0".repeat(64); // 32 bytes of zeros, hex-encoded
    let mut child = Command::new(worker_bin())
        .arg("extract-worker")
        .env("TOKENSAVE_WORKER_TOKEN", &token_hex)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn worker");
    {
        let stdin = child.stdin.as_mut().expect("piped");
        // 32 bytes of 0xFF — definitely not the expected zeros.
        stdin.write_all(&[0xFFu8; 32]).expect("write stdin");
    }
    drop(child.stdin.take());
    let status = child.wait().expect("wait");
    assert!(!status.success(), "wrong stdin token must be rejected");
}

#[test]
fn extract_worker_subcommand_is_hidden_from_help() {
    let output = Command::new(worker_bin())
        .arg("--help")
        .output()
        .expect("run --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        !combined.contains("extract-worker"),
        "extract-worker must be hidden from --help output, got:\n{combined}"
    );
}
