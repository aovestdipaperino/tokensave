//! #455: git hooks that do not force one hook directory on every repository.
//!
//! The global path works by claiming `core.hooksPath`, which is a single
//! machine-wide setting. It overrides the default of a separate `.git/hooks`
//! per checkout, so someone whose projects need different tooling cannot have
//! different hooks per project — and git stops reading each repository's own
//! hook directory entirely.
//!
//! Per-repository hooks are the git-native answer and need no global config,
//! so nothing here touches git config at all.

use std::path::Path;
use std::process::Command;
use tokensave::agents::{
    install_local_git_hooks, local_git_hooks_present, remove_local_git_hooks, repo_hooks_dir,
};

fn git(repo: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git")
        .status
        .success();
    assert!(ok, "git {args:?} failed");
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("ts455")
        .tempdir()
        .expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    dir
}

#[test]
fn hooks_land_in_the_repositorys_own_directory() {
    let dir = repo();
    let out = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    assert_eq!(
        out.installed,
        vec!["post-commit", "post-checkout", "post-merge"]
    );
    assert_eq!(out.hooks_dir, dir.path().join(".git").join("hooks"));
    assert!(local_git_hooks_present(dir.path()));
}

/// The whole point of the issue: no machine-wide setting is claimed, so other
/// repositories keep whatever hooks they had.
#[test]
fn installing_never_writes_git_config() {
    let dir = repo();
    let before = std::fs::read_to_string(dir.path().join(".git").join("config")).expect("config");
    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    let after = std::fs::read_to_string(dir.path().join(".git").join("config")).expect("config");

    assert_eq!(
        before, after,
        "the repository's git config must be untouched"
    );
    assert!(
        !after.contains("hooksPath"),
        "local hooks must never claim core.hooksPath"
    );
}

/// A repository with husky, pre-commit, or a hand-written hook keeps it.
#[test]
fn an_existing_hook_keeps_its_content_and_gains_a_section() {
    let dir = repo();
    let hook = dir.path().join(".git").join("hooks").join("post-commit");
    std::fs::write(&hook, "#!/bin/sh\necho mine\n").expect("write hook");

    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    let contents = std::fs::read_to_string(&hook).expect("read");

    assert!(contents.contains("echo mine"), "got: {contents}");
    assert!(contents.contains("tokensave"), "got: {contents}");
}

/// Removal is the same conservative rule as the global path: keep anything
/// tokensave did not write, delete a file that is nothing but ours.
#[test]
fn removal_keeps_foreign_content_and_deletes_a_pure_tokensave_hook() {
    let dir = repo();
    let hooks = dir.path().join(".git").join("hooks");
    std::fs::write(hooks.join("post-commit"), "#!/bin/sh\necho mine\n").expect("write hook");
    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    remove_local_git_hooks(dir.path());

    let kept = std::fs::read_to_string(hooks.join("post-commit")).expect("post-commit survives");
    assert!(kept.contains("echo mine"));
    assert!(!kept.contains("tokensave"));
    assert!(
        !hooks.join("post-merge").exists(),
        "a hook holding only tokensave's section is deleted outright"
    );
    assert!(!local_git_hooks_present(dir.path()));
}

#[test]
fn installing_twice_reports_the_second_run_as_already_present() {
    let dir = repo();
    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    let second = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    assert!(second.installed.is_empty());
    assert_eq!(
        second.already_present,
        vec!["post-commit", "post-checkout", "post-merge"]
    );
}

/// Linked worktrees share one hook directory with the main checkout, so
/// resolving from `--git-dir` would write to a per-worktree directory git
/// never reads.
#[test]
fn a_worktree_resolves_to_the_shared_hook_directory() {
    let dir = repo();
    std::fs::write(dir.path().join("f.txt"), "x").expect("write");
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "init",
        ],
    );
    let wt = dir.path().join("wt");
    git(
        dir.path(),
        &["worktree", "add", "-q", &wt.to_string_lossy(), "-b", "side"],
    );

    let from_main = repo_hooks_dir(dir.path()).expect("main hooks dir");
    let from_worktree = repo_hooks_dir(&wt).expect("worktree hooks dir");
    assert_eq!(
        from_main.canonicalize().ok(),
        from_worktree.canonicalize().ok(),
        "a worktree must resolve to the checkout's shared hook directory"
    );
}

#[test]
fn a_directory_that_is_not_a_repository_is_an_error_not_a_silent_success() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(install_local_git_hooks(dir.path(), "/usr/bin/tokensave").is_err());
    assert!(!local_git_hooks_present(dir.path()));
}

#[test]
fn a_hook_that_cannot_be_written_is_reported_as_failed() {
    let dir = repo();
    let hooks = repo_hooks_dir(dir.path()).expect("hooks dir");
    // A directory where the hook file belongs: the path exists, so the append
    // branch is taken, and opening a directory for writing fails on both Unix
    // and Windows.
    std::fs::create_dir_all(hooks.join("post-commit")).expect("occupy hook path");

    let out = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    assert_eq!(
        out.failed,
        vec!["post-commit"],
        "a hook that could not be written must be reported, not silently dropped"
    );
    assert!(
        !out.installed.contains(&"post-commit".to_string()),
        "a failed hook must not be listed as installed, got: {:?}",
        out.installed
    );
    assert_eq!(
        out.installed,
        vec!["post-checkout", "post-merge"],
        "the other two hooks must still install"
    );
}
