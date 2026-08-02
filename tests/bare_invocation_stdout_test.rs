//! A bare `tokensave` (no subcommand) must keep stdout empty. Agent permission
//! hooks (Claude Code, Cursor) invoke a bare `tokensave` and parse its stdout
//! as JSON; any help text there is a fatal parse error that fail-closes the
//! wrapped command. Help belongs on stderr, and an empty stdout with exit 0
//! reads as "no opinion". (#347, #348, #351)
//!
//! Driven through the real binary because the behaviour lives in the CLI path.

use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// A project with a real index in place, so a bare invocation takes the
/// "already initialized → show help" branch of `handle_no_command`.
fn initialized_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("init")
        .arg(dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("failed to spawn tokensave init");
    assert!(status.success(), "init must succeed to set up the fixture");
    dir
}

/// Runs a bare `tokensave` in `dir` with `stdin_data` piped (never a TTY under
/// `cargo test`) and returns stdout and stderr captured separately.
fn run_bare(dir: &std::path::Path, stdin_data: &str) -> (String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn tokensave");
    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(stdin_data.as_bytes())
        .expect("failed to write stdin");
    let output = child
        .wait_with_output()
        .expect("failed to wait for tokensave");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn bare_invocation_in_initialized_project_keeps_stdout_empty() {
    let dir = initialized_project();
    // A permission hook pipes its JSON payload into a bare `tokensave`.
    let (stdout, stderr) = run_bare(dir.path(), "{\"tool\":\"Bash\"}\n");

    assert!(
        stdout.is_empty(),
        "bare invocation must write nothing to stdout so hooks can parse it as JSON, got: {stdout:?}"
    );
    // Help still belongs somewhere the user can see it — stderr.
    assert!(
        stderr.contains("Usage"),
        "help should be rendered to stderr instead: {stderr:?}"
    );
}

#[test]
fn bare_invocation_uninitialized_with_piped_stdin_does_not_prompt_or_init() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

    // A piped "y" must not be taken for an interactive answer, so no index is
    // created and stdout stays clean.
    let (stdout, stderr) = run_bare(dir.path(), "y\n");

    assert!(
        stdout.is_empty(),
        "bare invocation must write nothing to stdout, got: {stdout:?}"
    );
    assert!(
        !stderr.contains("Create one now?"),
        "a non-interactive bare invocation must not prompt: {stderr:?}"
    );
    assert!(
        !dir.path().join(".tokensave").exists(),
        "the piped 'y' must not be taken as consent to initialize"
    );
}
