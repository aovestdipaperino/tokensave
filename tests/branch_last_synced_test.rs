//! `BranchMeta::last_synced_at` must track syncs — #399.
//!
//! `touch_synced` had exactly one caller, in the `branch add` arm, so the
//! field recorded when a branch entry was *created* and nothing advanced it
//! afterwards. Three surfaces render it as live freshness (`tokensave branch
//! list`, the `tokensave_branch_list` MCP tool, and the
//! `tokensave://branches` resource), so an agent asking "is this branch's
//! index fresh?" got a confidently wrong answer.

use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use tokensave::branch_meta;
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

fn last_synced_at(root: &Path, branch: &str) -> String {
    branch_meta::load_branch_meta(&root.join(".tokensave"))
        .expect("branch metadata")
        .branches
        .get(branch)
        .unwrap_or_else(|| panic!("branch {branch} must be present"))
        .last_synced_at
        .clone()
}

/// Backdates the stored timestamp so the assertion is about *advancing*
/// rather than about wall-clock passing between two calls in the same second.
fn backdate(root: &Path, branch: &str, to: &str) {
    let dir = root.join(".tokensave");
    let mut meta = branch_meta::load_branch_meta(&dir).expect("branch metadata");
    meta.branches
        .get_mut(branch)
        .expect("branch present")
        .last_synced_at = to.to_string();
    branch_meta::save_branch_meta(&dir, &meta).expect("save branch metadata");
}

/// The #399 reproduction: a real sync that indexes a new file must move the
/// timestamp the three listing surfaces render.
#[tokio::test]
async fn a_sync_advances_last_synced_at() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join("base.rs"), "fn base() {}").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    backdate(root, "master", "1000000000");
    assert_eq!(last_synced_at(root, "master"), "1000000000", "precondition");

    std::fs::write(root.join("added.rs"), "fn added() {}").unwrap();
    cg.sync().await.unwrap();

    let after = last_synced_at(root, "master");
    assert_ne!(
        after, "1000000000",
        "a sync that indexed a file must advance last_synced_at"
    );
    assert!(
        after.parse::<i64>().expect("numeric timestamp") > 1_000_000_000,
        "timestamp must move forward, got {after}"
    );
}

/// The incremental path the MCP server uses is a different function from the
/// full `sync()`, and it is the one that runs most often. It must advance the
/// timestamp too, or the field is accurate only for CLI users.
#[tokio::test]
async fn an_incremental_sync_advances_last_synced_at() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join("base.rs"), "fn base() {}").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    backdate(root, "master", "1000000000");

    std::fs::write(root.join("added.rs"), "fn added() {}").unwrap();
    let stale = cg.find_stale_files().await;
    assert!(
        !stale.is_empty(),
        "precondition: the new file must be stale"
    );
    cg.sync_if_stale_silent(&stale).await.unwrap();

    assert_ne!(
        last_synced_at(root, "master"),
        "1000000000",
        "the incremental path the MCP server uses must advance last_synced_at too"
    );
}

/// A tracked non-default branch has its own DB and its own entry; syncing
/// while on it must advance *its* timestamp, not the default branch's.
#[tokio::test]
async fn a_sync_advances_only_the_serving_branch() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join("base.rs"), "fn base() {}").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    drop(cg);

    git(root, &["checkout", "-b", "feature"]);
    assert!(
        tokensave::branch::track_branch_copy(root, &root.join(".tokensave"), "feature")
            .await
            .unwrap(),
        "precondition: feature must become tracked"
    );

    backdate(root, "master", "1000000000");
    backdate(root, "feature", "1000000000");

    let cg = TokenSave::open(root).await.unwrap();
    assert_eq!(cg.active_branch(), Some("feature"), "precondition");
    std::fs::write(root.join("on_feature.rs"), "fn on_feature() {}").unwrap();
    cg.sync().await.unwrap();

    assert_ne!(
        last_synced_at(root, "feature"),
        "1000000000",
        "the branch being served must advance"
    );
    assert_eq!(
        last_synced_at(root, "master"),
        "1000000000",
        "a sync on 'feature' must not claim 'master' was synced"
    );
}
