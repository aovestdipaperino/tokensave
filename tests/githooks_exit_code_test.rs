//! `githooks on` must exit non-zero when it could not write the hooks.
//!
//! The library-level tests pin the failure *reporting*; these run the binary,
//! because the bug in #488 was the wiring: the reason was printed in red and
//! the command still exited 0, so any script gating on it saw a success.

#![cfg(not(windows))]

use std::fs;
use std::process::Command;

/// Put a regular file where the hooks directory has to go, so creating it
/// cannot succeed. This is the shape the bug was reported in.
fn occupy(path: &std::path::Path) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, "not a directory").expect("occupy hooks path");
}

#[test]
fn a_local_install_that_cannot_write_exits_non_zero() {
    let repo = tempfile::tempdir().expect("temp repo");
    let home = tempfile::tempdir().expect("temp home");
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init")
        .success());
    // The hooks *directory* is fine; the hook file itself cannot be written,
    // which is the case the old code printed and then swallowed.
    fs::create_dir_all(repo.path().join(".git/hooks/post-commit")).expect("occupy hook path");

    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["githooks", "on", "--local"])
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .output()
        .expect("run tokensave githooks on --local");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a failed local install must not exit 0. stderr: {stderr}"
    );
    assert!(
        stderr.contains("could not install git hooks"),
        "the failure has to say which hooks. stderr: {stderr}"
    );
}

#[test]
fn a_global_install_that_cannot_write_exits_non_zero() {
    let home = tempfile::tempdir().expect("temp home");
    let cwd = tempfile::tempdir().expect("temp cwd");
    occupy(&home.path().join(".config/git/hooks"));

    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["githooks", "on"])
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("run tokensave githooks on");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a failed global install must not exit 0. stderr: {stderr}"
    );
}
