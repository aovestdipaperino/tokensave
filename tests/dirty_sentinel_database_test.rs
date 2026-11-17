//! The dirty sentinel names the database that an interrupted write left
//! unfinished (#865).
//!
//! A project in multi-branch mode has one database for each tracked branch
//! and one sentinel file. The sentinel used to carry no database name, so the
//! next open recovered whichever database it opened. After an interrupted
//! write on `feature` and a checkout of `master`, `master` got a full index it
//! did not need, the sentinel went away, and `feature` kept its unfinished
//! state.

use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};
use tokensave::branch_meta;
use tokensave::config::get_tokensave_dir;
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

/// A project with `master` and `feature` tracked and synced. `feature` has
/// one file more than `master` and is left checked out.
async fn multi_branch_project() -> TempDir {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
    commit_file(root, "base.rs", "fn base() {}\n");
    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    drop(cg);

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

/// The database file of `branch`, as the branch metadata names it.
fn database_of(root: &Path, branch: &str) -> String {
    branch_meta::load_branch_meta(&get_tokensave_dir(root))
        .expect("branch metadata")
        .branches[branch]
        .db_file
        .clone()
}

/// Writes the sentinel that an interrupted write on `databases` leaves.
fn leave_sentinel(root: &Path, databases: &[String]) {
    let mut contents = "pid=1\ntime=0\nversion=test".to_string();
    for database in databases {
        contents.push_str("\ndb=");
        contents.push_str(database);
    }
    std::fs::write(get_tokensave_dir(root).join("dirty"), contents).unwrap();
}

fn sentinel(root: &Path) -> Option<String> {
    std::fs::read_to_string(get_tokensave_dir(root).join("dirty")).ok()
}

async fn indexed_files(cg: &TokenSave) -> Vec<String> {
    cg.get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect()
}

/// Removes the rows of `on_feature.rs`, as a write that stopped after its
/// delete step leaves them.
async fn drop_feature_file_rows(root: &Path) {
    let cg = TokenSave::open(root).await.unwrap();
    cg.db().delete_file("on_feature.rs").await.unwrap();
    assert!(
        !indexed_files(&cg)
            .await
            .contains(&"on_feature.rs".to_string()),
        "precondition"
    );
}

#[tokio::test]
async fn an_interrupted_write_is_recovered_on_its_own_database() {
    let tmp = multi_branch_project().await;
    let root = tmp.path();
    let feature_db = database_of(root, "feature");
    drop_feature_file_rows(root).await;
    leave_sentinel(root, std::slice::from_ref(&feature_db));

    // The next open is on a different branch. That database is complete.
    git(root, &["checkout", "master"]);
    let on_master = TokenSave::open(root).await.unwrap();
    assert_eq!(on_master.serving_branch(), Some("master"), "precondition");
    let kept = sentinel(root).expect("the open on master must keep feature's sentinel");
    assert!(kept.contains(&feature_db), "{kept}");
    assert_eq!(indexed_files(&on_master).await, vec!["base.rs".to_string()]);
    drop(on_master);

    // The open on the interrupted database recovers it.
    git(root, &["checkout", "feature"]);
    let on_feature = TokenSave::open(root).await.unwrap();
    assert!(
        indexed_files(&on_feature)
            .await
            .contains(&"on_feature.rs".to_string()),
        "the open on feature must index its tree again"
    );
    assert_eq!(sentinel(root), None, "recovery must remove the sentinel");
}

#[tokio::test]
async fn a_recovery_keeps_the_sentinel_of_a_different_database() {
    let tmp = multi_branch_project().await;
    let root = tmp.path();
    let master_db = database_of(root, "master");
    let feature_db = database_of(root, "feature");
    drop_feature_file_rows(root).await;
    leave_sentinel(root, &[master_db.clone(), feature_db.clone()]);

    let on_feature = TokenSave::open(root).await.unwrap();
    assert!(indexed_files(&on_feature)
        .await
        .contains(&"on_feature.rs".to_string()));
    drop(on_feature);
    let kept = sentinel(root).expect("master's sentinel must stay");
    assert!(kept.contains(&format!("db={master_db}")), "{kept}");
    assert!(!kept.contains(&format!("db={feature_db}")), "{kept}");

    git(root, &["checkout", "master"]);
    let on_master = TokenSave::open(root).await.unwrap();
    assert_eq!(indexed_files(&on_master).await, vec!["base.rs".to_string()]);
    assert_eq!(sentinel(root), None, "recovery must remove the sentinel");
}
