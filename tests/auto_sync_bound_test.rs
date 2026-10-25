//! Bounds on access-triggered (automatic) syncs — #396, #393.
//!
//! The startup catch-up sync and the per-`tools/call` staleness check both
//! feed `find_stale_files` straight into a sync. `find_stale_files` compares
//! the tree against the `files` table, so an index with no files recorded
//! classifies *every* file on disk as stale, and nothing downstream caps the
//! resulting work. Pointed at a large tree that was never indexed, an
//! automatic background sync becomes a full initial index: #396 reached
//! ~95 GiB RSS+swap on a home directory, #393 burned CPU for 24 minutes on a
//! project that had never been `init`ed.
//!
//! These tests pin the two guards that stop that. They assert on the
//! *decision*, not on wall-clock or memory, so they stay deterministic.

use tempfile::tempdir;
use tokensave::tokensave::{AutoSyncScope, TokenSave};

/// Writes `n` trivial Rust files into `dir`.
fn write_files(dir: &std::path::Path, n: usize) {
    for i in 0..n {
        std::fs::write(dir.join(format!("f{i}.rs")), format!("fn f{i}() {{}}")).unwrap();
    }
}

/// The reproduction for #393 and #396: a project with a database but no
/// indexed files must not have its whole tree extracted by a *background*
/// sync. Building the initial index is `init`'s job, and `init` is explicit.
#[tokio::test]
async fn empty_index_does_not_trigger_an_automatic_full_index() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    write_files(project, 5);

    // `init` creates the database but does not index; the `files` table is
    // empty, which is exactly the state both reporters were in.
    let cg = TokenSave::init(project).await.unwrap();
    assert!(
        cg.get_all_files().await.unwrap().is_empty(),
        "precondition: init must leave the files table empty"
    );

    match cg.find_stale_files_bounded().await {
        AutoSyncScope::Uninitialized => {}
        other => panic!("an empty index must not schedule an automatic sync, got {other:?}"),
    }

    // The unbounded path is what the guard protects against: it reports every
    // file on disk as stale. Asserted so the test fails loudly if
    // `find_stale_files` ever changes and silently makes the guard moot.
    assert_eq!(
        cg.find_stale_files().await.len(),
        5,
        "unbounded staleness should still see every file — the guard is what stops the sync"
    );
}

/// Once the project has genuinely been indexed, an ordinary edit is a
/// catch-up sync and must proceed. This is the case the feature exists for.
#[tokio::test]
async fn an_ordinary_change_still_syncs_automatically() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    write_files(project, 3);

    let cg = TokenSave::init(project).await.unwrap();
    cg.sync().await.unwrap();

    // A *new* file rather than an edit to an existing one: staleness compares
    // mtime against `indexed_at` at one-second granularity, so a rewrite in
    // the same second as the sync is legitimately not stale and the test would
    // be timing-dependent. A file with no DB record is stale unconditionally.
    std::fs::write(project.join("added.rs"), "fn added() {}").unwrap();

    match cg.find_stale_files_bounded().await {
        AutoSyncScope::Sync(files) => {
            assert_eq!(files, vec!["added.rs".to_string()]);
        }
        other => panic!("a one-file change must sync automatically, got {other:?}"),
    }
}

/// Defence in depth for the case the first guard does not cover: an indexed
/// project into which a very large number of files appears at once. A
/// debounce bounds how *often* a sync fires, never how much one sync costs,
/// so the file count is bounded explicitly.
#[tokio::test]
async fn a_change_larger_than_the_cap_is_refused() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    write_files(project, 1);

    let mut cg = TokenSave::init(project).await.unwrap();
    cg.sync().await.unwrap();

    // Lower the cap rather than writing tens of thousands of files: the
    // behaviour under test is the comparison, not the threshold's value.
    cg.set_max_auto_sync_files(4);
    write_files(project, 10);

    match cg.find_stale_files_bounded().await {
        AutoSyncScope::TooManyStale { count, limit } => {
            assert_eq!(limit, 4);
            assert!(count > 4, "count should exceed the limit, got {count}");
        }
        other => panic!("a change past the cap must be refused, got {other:?}"),
    }
}

/// A cap of `0` disables the second guard, for anyone who wants the old
/// unbounded catch-up behaviour on a project they know is small.
#[tokio::test]
async fn a_cap_of_zero_disables_the_file_count_guard() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    write_files(project, 1);

    let mut cg = TokenSave::init(project).await.unwrap();
    cg.sync().await.unwrap();

    cg.set_max_auto_sync_files(0);
    write_files(project, 10);

    match cg.find_stale_files_bounded().await {
        AutoSyncScope::Sync(files) => assert!(files.len() > 4),
        other => panic!("a zero cap must not refuse, got {other:?}"),
    }
}
