//! A sync must not write the working tree into another branch's index (#865).
//!
//! In multi-branch mode each tracked branch owns a database. A branch that is
//! not tracked, and a detached HEAD, are served from the database of another
//! branch: the nearest tracked ancestor, or the default branch. That database
//! is borrowed. A sync through such a handle used to write the working tree
//! into it, so the owner's index held files that exist only on some other
//! branch. That is the #400 failure by another route.
//!
//! A project that tracks only its default branch has one database for every
//! branch by design. The last test pins that such a project keeps syncing.

use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};
use tokensave::branch_meta;
use tokensave::config::{get_tokensave_dir, load_config, save_config};
use tokensave::tokensave::TokenSave;

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

fn commit_file(root: &Path, name: &str, body: &str) {
    std::fs::write(root.join(name), body).unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", name]);
}

/// A project indexed on `master`, with automatic branch tracking off, so that
/// a branch stays untracked until a test tracks it.
async fn single_database_project() -> TempDir {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
    commit_file(root, "base.rs", "fn base() {}\n");

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    drop(cg);

    let mut config = load_config(root).unwrap();
    config.auto_track = false;
    save_config(root, &config).unwrap();
    tmp
}

/// [`single_database_project`] with `feature` tracked and synced, which puts
/// the project in multi-branch mode. `feature` is left checked out.
async fn multi_branch_project() -> TempDir {
    let tmp = single_database_project().await;
    let root = tmp.path();
    git(root, &["checkout", "-b", "feature"]);
    assert!(
        tokensave::branch::track_branch_copy(root, &get_tokensave_dir(root), "feature")
            .await
            .unwrap(),
        "precondition: feature must become tracked"
    );
    commit_file(root, "on_feature.rs", "fn on_feature() {}\n");
    let cg = TokenSave::open(root).await.unwrap();
    assert_eq!(cg.serving_branch(), Some("feature"), "precondition");
    cg.sync().await.unwrap();
    tmp
}

async fn indexed_files(cg: &TokenSave) -> Vec<String> {
    cg.get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect()
}

/// The tracked branch whose database serves the untracked branch `cg` is on.
/// Which tracked branch that is depends on the ancestor search, and the rule
/// under test is the same for each of them.
fn borrowed_from(cg: &TokenSave) -> String {
    let owner = cg.serving_branch().expect("a serving branch").to_string();
    assert_ne!(Some(owner.as_str()), cg.active_branch(), "precondition");
    assert!(cg.is_fallback(), "precondition");
    owner
}

fn last_synced_at(root: &Path, branch: &str) -> String {
    branch_meta::load_branch_meta(&get_tokensave_dir(root))
        .expect("branch metadata")
        .branches[branch]
        .last_synced_at
        .clone()
}

fn backdate(root: &Path, branch: &str) {
    let dir = get_tokensave_dir(root);
    let mut meta = branch_meta::load_branch_meta(&dir).expect("branch metadata");
    meta.branches
        .get_mut(branch)
        .expect("branch present")
        .last_synced_at = "1000000000".to_string();
    branch_meta::save_branch_meta(&dir, &meta).expect("save branch metadata");
}

/// `third` is not tracked, so the database of a tracked branch serves it. A
/// sync on `third` must leave that database, and its sync time, as they were.
#[tokio::test]
async fn a_sync_does_not_write_an_untracked_branch_into_its_ancestor() {
    let tmp = multi_branch_project().await;
    let root = tmp.path();
    git(root, &["checkout", "-b", "third"]);
    std::fs::write(root.join("third_only.rs"), "fn third_only() {}\n").unwrap();
    backdate(root, "master");
    backdate(root, "feature");

    let cg = TokenSave::open(root).await.unwrap();
    assert_eq!(cg.active_branch(), Some("third"), "precondition");
    let owner = borrowed_from(&cg);

    let error = cg
        .sync()
        .await
        .expect_err("a sync through a borrowed index must be refused");
    let message = error.to_string();
    assert!(
        message.contains("tokensave branch add") && message.contains("third"),
        "the refusal must name the branch and the remedy: {message}"
    );

    let files = indexed_files(&cg).await;
    assert!(
        !files.iter().any(|p| p.contains("third_only")),
        "{owner}'s index must not hold a file that exists only on third: {files:?}"
    );
    assert_eq!(
        last_synced_at(root, &owner),
        "1000000000",
        "a refused sync on third must not report {owner} as synced"
    );
}

/// Automatic syncs decide through this scope, and report the refusal.
#[tokio::test]
async fn an_automatic_sync_refuses_a_borrowed_index() {
    let tmp = multi_branch_project().await;
    let root = tmp.path();
    git(root, &["checkout", "-b", "third"]);
    std::fs::write(root.join("third_only.rs"), "fn third_only() {}\n").unwrap();

    let cg = TokenSave::open(root).await.unwrap();
    let owner = borrowed_from(&cg);
    assert_eq!(
        cg.find_stale_files_bounded().await,
        tokensave::tokensave::AutoSyncScope::BorrowedIndex { owner }
    );
}

/// The MCP server refreshes the files a response names through this path.
#[tokio::test]
async fn a_refresh_of_stale_files_does_not_write_into_the_ancestor() {
    let tmp = multi_branch_project().await;
    let root = tmp.path();
    git(root, &["checkout", "-b", "third"]);
    std::fs::write(root.join("third_only.rs"), "fn third_only() {}\n").unwrap();

    let cg = TokenSave::open(root).await.unwrap();
    let owner = borrowed_from(&cg);
    let _ = cg
        .sync_if_stale_silent(&["third_only.rs".to_string()])
        .await;

    let files = indexed_files(&cg).await;
    assert!(
        !files.iter().any(|p| p.contains("third_only")),
        "{owner}'s index must not hold a file that exists only on third: {files:?}"
    );
}

/// A detached HEAD is served from the default branch's database.
#[tokio::test]
async fn a_sync_on_a_detached_head_does_not_write_into_the_default_branch() {
    let tmp = multi_branch_project().await;
    let root = tmp.path();
    git(root, &["checkout", "--detach", "master"]);
    std::fs::write(root.join("detached_only.rs"), "fn detached_only() {}\n").unwrap();
    backdate(root, "master");

    let cg = TokenSave::open(root).await.unwrap();
    assert_eq!(cg.active_branch(), None, "precondition");
    assert_eq!(cg.serving_branch(), Some("master"), "precondition");

    cg.sync()
        .await
        .expect_err("a sync through a borrowed index must be refused");

    let files = indexed_files(&cg).await;
    assert!(
        !files.iter().any(|p| p.contains("detached_only")),
        "master's index must not hold a file from a detached HEAD: {files:?}"
    );
    assert_eq!(last_synced_at(root, "master"), "1000000000");
}

/// A project that tracks only its default branch has one index for every
/// branch. It must keep syncing on an untracked branch and on a detached HEAD.
#[tokio::test]
async fn a_single_database_project_syncs_on_any_branch() {
    let tmp = single_database_project().await;
    let root = tmp.path();

    git(root, &["checkout", "-b", "other"]);
    std::fs::write(root.join("other_only.rs"), "fn other_only() {}\n").unwrap();
    let cg = TokenSave::open(root).await.unwrap();
    cg.sync().await.unwrap();
    let files = indexed_files(&cg).await;
    assert!(files.iter().any(|p| p.contains("other_only")), "{files:?}");
    drop(cg);

    git(root, &["checkout", "--detach"]);
    std::fs::write(root.join("detached_only.rs"), "fn detached_only() {}\n").unwrap();
    let cg = TokenSave::open(root).await.unwrap();
    cg.sync().await.unwrap();
    let files = indexed_files(&cg).await;
    assert!(
        files.iter().any(|p| p.contains("detached_only")),
        "{files:?}"
    );
}
