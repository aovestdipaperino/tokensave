//! `branch add` writes to the branch it names, and `branch gc` keeps live
//! branches — #501.
//!
//! Two bugs shared one root: both commands assumed a layout instead of asking
//! git. `branch add` copied the parent DB for the named branch and then synced
//! through `TokenSave::open`, which resolves the DB for *HEAD* — so the new
//! branch's DB stayed a bare copy while the working tree, untracked files
//! included, was written into the current branch's DB. `branch gc` decided a
//! branch was gone by looking for `.git/refs/heads/<name>` on disk, which is
//! never there inside a linked worktree (`.git` is a file) nor in a
//! `reftable` repository (no loose refs), so it deleted live branch DBs.

use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

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
        .output()
        .expect("run tokensave")
}

/// Bug 1: adding a branch that is not checked out must not write the working
/// tree into the current branch's database.
#[test]
fn adding_an_unchecked_out_branch_leaves_the_current_db_alone() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn base() -> i32 { 1 }\n").unwrap();

    git(root, &["init", "-b", "main"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);
    assert!(tokensave(root, &["init", "."]).status.success());

    git(root, &["checkout", "-b", "other"]);
    std::fs::write(root.join("src/other.rs"), "pub fn only_on_other() {}\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "other"]);
    git(root, &["checkout", "main"]);

    // A file that exists on no branch at all — the clearest marker of a sync
    // that walked the working directory when it should not have.
    std::fs::write(root.join("src/stray.rs"), "pub fn stray() {}\n").unwrap();

    let out = tokensave(root, &["branch", "add", "other"]);
    assert!(out.status.success(), "branch add should succeed");

    let listing = tokensave(root, &["files", "--format", "flat"]);
    let text = String::from_utf8_lossy(&listing.stdout);
    assert!(
        !text.contains("stray.rs"),
        "the working tree must not be indexed into the current branch's DB: {text}"
    );
}

/// Bug 2: `gc` must not delete a branch that git still has, and inside a
/// linked worktree it used to delete every one of them.
#[test]
fn gc_keeps_live_branches_inside_a_worktree() {
    let tmp = tempdir().unwrap();
    let repo = tmp.path().join("repo");
    let wt = tmp.path().join("wt");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/lib.rs"), "pub fn base() -> i32 { 1 }\n").unwrap();

    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-m", "base"]);
    git(
        &repo,
        &["worktree", "add", "-b", "test-branch", wt.to_str().unwrap()],
    );

    assert!(tokensave(&wt, &["init", "."]).status.success());
    assert!(tokensave(&wt, &["branch", "add", "test-branch"])
        .status
        .success());

    let db = wt.join(".tokensave/branches/test-branch.db");
    assert!(db.exists(), "branch DB should exist before gc");

    let out = tokensave(&wt, &["branch", "gc"]);
    assert!(out.status.success(), "gc should succeed");
    assert!(
        db.exists(),
        "gc deleted a live branch DB inside a worktree: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The counterpart: a branch git really has deleted is still collected, so the
/// fix did not simply disable `gc`.
#[test]
fn gc_still_removes_a_branch_git_no_longer_has() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn base() -> i32 { 1 }\n").unwrap();

    git(root, &["init", "-b", "main"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);
    assert!(tokensave(root, &["init", "."]).status.success());

    git(root, &["checkout", "-b", "doomed"]);
    assert!(tokensave(root, &["branch", "add", "doomed"])
        .status
        .success());
    let db = root.join(".tokensave/branches/doomed.db");
    assert!(db.exists());

    git(root, &["checkout", "main"]);
    git(root, &["branch", "-D", "doomed"]);

    assert!(tokensave(root, &["branch", "gc"]).status.success());
    assert!(
        !db.exists(),
        "a genuinely deleted branch should be collected"
    );
}
