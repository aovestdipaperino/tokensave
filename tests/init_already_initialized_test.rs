//! `tokensave init` on an already-initialized project must fail unambiguously.
//!
//! A caller that treats `init`'s exit code as proof a rebuild happened will sit
//! on a stale index if the "already initialized, nothing done" case ever reports
//! success. #372 §1 reported observing exit 0 with a silent-looking message in an
//! interactive session; that could not be reproduced — the current binary exits 1
//! both with and without a TTY — so these tests exist to keep it that way rather
//! than to change behaviour. The invocation mode must not enter into it: no-op
//! means non-zero, in every mode.
//!
//! Driven through the real binary because the behaviour lives in the CLI path.

use std::process::{Command, Stdio};

use tempfile::TempDir;

/// A project with a real index in place, so a second `init` takes the
/// already-initialized branch.
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

/// Runs `tokensave init <args>` with stdin closed and returns (code, stderr).
fn run_init(dir: &std::path::Path, args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("init")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn tokensave init");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn re_init_with_an_explicit_path_exits_non_zero() {
    let dir = initialized_project();
    let path = dir.path().to_string_lossy().into_owned();
    let (code, stderr) = run_init(dir.path(), &[&path]);

    assert_eq!(
        code,
        Some(1),
        "re-init must fail so a caller cannot mistake it for a completed rebuild; stderr: {stderr}"
    );
    assert!(
        stderr.contains("already initialized"),
        "the error must say why it failed; stderr: {stderr}"
    );
}

#[test]
fn re_init_with_no_path_argument_exits_non_zero() {
    // The argument-less form resolves the project from the working directory;
    // it must reach the same refusal as the explicit-path form.
    let dir = initialized_project();
    let (code, stderr) = run_init(dir.path(), &[]);

    assert_eq!(
        code,
        Some(1),
        "argument-less re-init must fail too; stderr: {stderr}"
    );
    assert!(stderr.contains("already initialized"), "stderr: {stderr}");
}

#[test]
fn re_init_with_a_relative_path_exits_non_zero() {
    // `.` resolves to the same project, so it must be refused the same way
    // rather than falling through to a second index (#372).
    let dir = initialized_project();
    let (code, stderr) = run_init(dir.path(), &["."]);

    assert_eq!(
        code,
        Some(1),
        "re-init via `.` must fail too; stderr: {stderr}"
    );
    assert!(stderr.contains("already initialized"), "stderr: {stderr}");
}

#[test]
fn the_refusal_names_the_command_that_would_actually_rebuild() {
    // The message is the only thing telling a stuck caller what to run next;
    // #372 §1 spent real debugging time on exactly this gap.
    let dir = initialized_project();
    let path = dir.path().to_string_lossy().into_owned();
    let (_, stderr) = run_init(dir.path(), &[&path]);

    assert!(
        stderr.contains("sync --force"),
        "the refusal must point at the rebuild command; stderr: {stderr}"
    );
}
