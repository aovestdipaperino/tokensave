//! A server must not write one branch's files into another branch's DB — #400.
//!
//! `TokenSave::open` resolves the active branch once and the MCP server holds
//! that handle for its whole life. After a `git checkout` the handle still
//! points at the branch the server started on, so an automatic sync indexes
//! the *new* branch's working tree into the *old* branch's database. The
//! reported symptom is `main`'s index holding a file that exists only on
//! `feature` — not merely stale, but describing a tree that never existed.
//!
//! These tests pin the detection and the refusal. Re-resolving the branch
//! handle in place is a separate change; refusing to write across branches is
//! what stops the corruption.

use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
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

/// A project in multi-branch mode, indexed on `master`, with `feature`
/// tracked. Returns the open handle, still resolved to `master`.
async fn project_on_master_with_tracked_feature() -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    git(&root, &["init", "-b", "master"]);
    std::fs::write(root.join("base.rs"), "fn base() {}").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "base"]);

    let cg = TokenSave::init(&root).await.unwrap();
    cg.sync().await.unwrap();

    // Bootstrap multi-branch mode: `feature` gets its own DB.
    git(&root, &["checkout", "-b", "feature"]);
    let tokensave_dir = root.join(".tokensave");
    assert!(
        tokensave::branch::track_branch_copy(&root, &tokensave_dir, "feature")
            .await
            .unwrap(),
        "precondition: feature must become tracked"
    );

    // Back to master, and open the handle a server would hold.
    git(&root, &["checkout", "master"]);
    let cg = TokenSave::open(&root).await.unwrap();
    assert_eq!(cg.active_branch(), Some("master"), "precondition");
    (tmp, cg)
}

/// The #400 reproduction: the handle says `master`, the working tree is on
/// `feature`, and an automatic sync must decline rather than write
/// `feature`'s files into `master`'s database.
#[tokio::test]
async fn a_checkout_under_a_live_handle_is_detected_as_drift() {
    let (tmp, cg) = project_on_master_with_tracked_feature().await;
    let root = tmp.path();

    assert!(
        cg.branch_drift().is_none(),
        "no drift while the tree is on the branch the handle resolved"
    );

    // What the server cannot see: a checkout under a handle it already holds.
    git(root, &["checkout", "feature"]);
    std::fs::write(root.join("on_feature.rs"), "fn on_feature() {}").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "feature-only file"]);

    let drift = cg
        .branch_drift()
        .expect("a checkout under a live handle must be detected");
    assert_eq!(drift.serving, "master");
    assert_eq!(drift.working_tree, "feature");
}

/// The guard that actually prevents the corruption: with drift present, the
/// bounded scope used by automatic syncs refuses instead of returning files.
#[tokio::test]
async fn an_automatic_sync_refuses_while_the_branch_has_drifted() {
    let (tmp, cg) = project_on_master_with_tracked_feature().await;
    let root = tmp.path();

    git(root, &["checkout", "feature"]);
    std::fs::write(root.join("on_feature.rs"), "fn on_feature() {}").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "feature-only file"]);

    match cg.find_stale_files_bounded().await {
        tokensave::tokensave::AutoSyncScope::BranchDrifted(drift) => {
            assert_eq!(drift.serving, "master");
            assert_eq!(drift.working_tree, "feature");
        }
        other => panic!("an automatic sync must refuse under drift, got {other:?}"),
    }

    // The corruption this prevents: `on_feature.rs` must never reach the
    // master DB. Asserted through the index rather than the decision, since
    // that is the state the reporter observed.
    let indexed: Vec<String> = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert!(
        !indexed.iter().any(|p| p.contains("on_feature")),
        "master's index must not contain a file that exists only on feature: {indexed:?}"
    );
}

/// A single-DB project has one index by design, so switching branches there
/// is ordinary and must keep syncing. Guarding on branch name alone would
/// break every project that never opted into multi-branch mode.
#[tokio::test]
async fn single_db_projects_are_unaffected_by_a_checkout() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join("base.rs"), "fn base() {}").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    git(root, &["checkout", "-b", "feature"]);
    std::fs::write(root.join("added.rs"), "fn added() {}").unwrap();

    assert!(
        cg.branch_drift().is_none(),
        "a project with no per-branch DBs has nothing to drift from"
    );
    match cg.find_stale_files_bounded().await {
        tokensave::tokensave::AutoSyncScope::Sync(files) => {
            assert!(files.iter().any(|f| f.contains("added")));
        }
        other => panic!("single-DB projects must keep syncing across branches, got {other:?}"),
    }
}
