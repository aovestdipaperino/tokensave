//! Issue #605: read-only hook status must not hide per-repository hooks.

#![cfg(not(windows))]

use std::fs;
use std::process::Command;

fn git_init(path: &std::path::Path) {
    let template = tempfile::tempdir().expect("git template");
    assert!(Command::new("git")
        .args([
            "init",
            "-q",
            "--template",
            template.path().to_str().expect("template path"),
        ])
        .current_dir(path)
        .status()
        .expect("git init")
        .success());
}

#[test]
fn bare_githooks_reports_hooks_in_the_current_repository() {
    let repo = tempfile::tempdir().expect("temp repo");
    let home = tempfile::tempdir().expect("temp home");
    git_init(repo.path());
    fs::create_dir_all(repo.path().join(".git/hooks")).expect("create hooks directory");
    fs::write(
        repo.path().join(".git/hooks/post-commit"),
        "#!/bin/sh\n# tokensave: auto-sync\n/usr/bin/tokensave sync >/dev/null 2>&1 &\n",
    )
    .expect("write local hook");

    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("githooks")
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .output()
        .expect("run tokensave githooks");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains("repository hooks:"),
        "bare status must include the current repository. stderr: {stderr}"
    );
    assert!(
        stderr.contains("post-commit: runs tokensave"),
        "bare status must identify the local hook. stderr: {stderr}"
    );
    assert!(
        stderr.find("repository hooks:") < stderr.find("install them with `tokensave githooks on`"),
        "local status must be shown before the global install suggestion. stderr: {stderr}"
    );
}

#[test]
fn doctor_recommends_scoped_repair_for_a_stale_repository_hook() {
    let repo = tempfile::tempdir().expect("temp repo");
    let home = tempfile::tempdir().expect("temp home");
    git_init(repo.path());

    let init = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["init", "--no-git-hook"])
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .output()
        .expect("initialize project");
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    fs::create_dir_all(repo.path().join(".git/hooks")).expect("create hooks directory");
    fs::write(
        repo.path().join(".git/hooks/post-checkout"),
        "#!/bin/sh\n# tokensave: auto-init\nold hook body\n# tokensave: end auto-init\n",
    )
    .expect("write stale hook");

    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("doctor")
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .output()
        .expect("run tokensave doctor");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("tokensave githooks on --local --path"),
        "doctor must recommend the scoped local repair. stderr: {stderr}"
    );
    assert!(
        !stderr.contains("run `tokensave reinstall` to update it"),
        "doctor must not recommend global reinstall for a local hook. stderr: {stderr}"
    );
}
