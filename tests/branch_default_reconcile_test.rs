//! The recorded default branch follows the repository — #874.
//!
//! `branch-meta.json` recorded `default_branch` once, at init, and nothing
//! checked it again. After `git branch -m master main` the index kept serving
//! `main` as an untracked branch borrowing from a `master` that no longer
//! existed, and auto-track duplicated the database. Opening a project now
//! renames the entry when that is unambiguous, reports it when it is not, and
//! `tokensave branch set-default` resolves the rest.

use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use tokensave::branch_meta::{load_branch_meta, BranchMeta};

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn tokensave(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_COUNT", "0")
        .env("HOME", root)
        .env_remove("TOKENSAVE_AUTO_TRACK")
        .output()
        .expect("run tokensave")
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// A committed repository on `branch`, indexed with `tokensave init`.
fn indexed_repo(root: &Path, branch: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src").join("lib.rs"),
        "pub fn base() -> i32 { 1 }\n",
    )
    .unwrap();
    git(root, &["init", "-b", branch]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);
    let out = tokensave(root, &["init", "."]);
    assert!(out.status.success(), "init failed: {}", stderr(&out));
}

fn meta(root: &Path) -> BranchMeta {
    load_branch_meta(&tokensave::config::get_tokensave_dir(root)).expect("branch metadata")
}

#[test]
fn renaming_the_default_branch_after_init_renames_the_entry() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    indexed_repo(root, "master");
    assert_eq!(meta(root).default_branch, "master");

    git(root, &["branch", "-m", "master", "main"]);
    let out = tokensave(root, &["sync"]);
    assert!(out.status.success(), "sync failed: {}", stderr(&out));
    assert!(
        stderr(&out).contains("default branch updated: master → main"),
        "the rename is logged: {}",
        stderr(&out)
    );
    assert!(
        !stderr(&out).contains("not tracked"),
        "main must not be served as an untracked branch: {}",
        stderr(&out)
    );

    let meta = meta(root);
    assert_eq!(meta.default_branch, "main");
    assert!(!meta.is_tracked("master"));
    assert_eq!(meta.branches["main"].db_file, "tokensave.db");
    assert!(
        !tokensave::config::get_tokensave_dir(root)
            .join("branches")
            .exists(),
        "no database is copied for the renamed default"
    );
}

#[test]
fn a_stale_origin_head_is_ignored() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("README"), "x\n").unwrap();
    git(root, &["init", "-b", "main"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);
    // `origin/HEAD` still names the branch upstream renamed away.
    git(
        root,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/master",
        ],
    );
    assert_eq!(
        tokensave::branch::detect_default_branch(root).as_deref(),
        Some("main")
    );

    // A live `origin/HEAD` target is still preferred over the heuristics.
    git(root, &["branch", "trunk"]);
    git(root, &["update-ref", "refs/remotes/origin/trunk", "trunk"]);
    git(
        root,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );
    assert_eq!(
        tokensave::branch::detect_default_branch(root).as_deref(),
        Some("trunk")
    );
}

#[test]
fn branch_add_bootstraps_the_detected_default_in_a_master_repo() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    indexed_repo(root, "master");
    // A project indexed before branch metadata existed.
    std::fs::remove_file(tokensave::config::get_tokensave_dir(root).join("branch-meta.json"))
        .unwrap();

    git(root, &["branch", "feature"]);
    let out = tokensave(root, &["branch", "add", "feature"]);
    assert!(out.status.success(), "branch add failed: {}", stderr(&out));

    let meta = meta(root);
    assert_eq!(meta.default_branch, "master");
    assert!(!meta.is_tracked("main"));
    assert_eq!(meta.branches["feature"].parent.as_deref(), Some("master"));
}

#[test]
fn a_separately_tracked_default_is_reported_not_merged() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    indexed_repo(root, "master");
    git(root, &["checkout", "-b", "main"]);
    let out = tokensave(root, &["branch", "add", "main"]);
    assert!(out.status.success(), "branch add failed: {}", stderr(&out));
    git(root, &["branch", "-D", "master"]);

    let out = tokensave(root, &["sync"]);
    assert!(out.status.success(), "sync failed: {}", stderr(&out));
    assert!(
        stderr(&out).contains("tokensave branch set-default main"),
        "open points at the fix: {}",
        stderr(&out)
    );
    let unchanged = meta(root);
    assert_eq!(unchanged.default_branch, "master");
    assert_eq!(unchanged.branches["master"].db_file, "tokensave.db");
    assert_eq!(unchanged.branches["main"].db_file, "branches/main.db");

    let listing = tokensave(root, &["branch", "list"]);
    assert!(stderr(&listing).contains("no longer exists"));

    let verify = tokensave(root, &["verify", ".", "--json"]);
    assert!(
        String::from_utf8_lossy(&verify.stdout).contains("BRANCH_DEFAULT_STALE"),
        "verify reports the stale default: {}",
        String::from_utf8_lossy(&verify.stdout)
    );

    // The explicit command promotes the tracked entry; the old default is
    // then an ordinary branch that `branch remove` can delete.
    let out = tokensave(root, &["branch", "set-default", "main"]);
    assert!(out.status.success(), "set-default failed: {}", stderr(&out));
    let promoted = meta(root);
    assert_eq!(promoted.default_branch, "main");
    assert!(promoted.branches["main"].parent.is_none());
    assert!(promoted.is_tracked("master"));

    let out = tokensave(root, &["branch", "remove", "master"]);
    assert!(out.status.success(), "remove failed: {}", stderr(&out));
    assert!(!meta(root).is_tracked("master"));
    let verify = tokensave(root, &["verify", ".", "--json"]);
    assert!(!String::from_utf8_lossy(&verify.stdout).contains("BRANCH_DEFAULT_STALE"));
}

#[test]
fn nothing_changes_while_both_branches_exist() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    indexed_repo(root, "master");
    git(root, &["branch", "main"]);

    let out = tokensave(root, &["sync"]);
    assert!(out.status.success(), "sync failed: {}", stderr(&out));
    assert!(!stderr(&out).contains("default branch"), "{}", stderr(&out));
    let meta = meta(root);
    assert_eq!(meta.default_branch, "master");
    assert!(!meta.is_tracked("main"));
}

#[test]
fn set_default_renames_an_untracked_branch_and_rejects_unknown_ones() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    indexed_repo(root, "master");
    git(root, &["branch", "trunk"]);

    let out = tokensave(root, &["branch", "set-default", "nonexistent"]);
    assert!(!out.status.success(), "an unknown branch is rejected");
    assert_eq!(meta(root).default_branch, "master");

    let out = tokensave(root, &["branch", "set-default", "trunk"]);
    assert!(out.status.success(), "set-default failed: {}", stderr(&out));
    let meta = meta(root);
    assert_eq!(meta.default_branch, "trunk");
    assert_eq!(meta.branches["trunk"].db_file, "tokensave.db");
    assert!(!meta.is_tracked("master"));
}
