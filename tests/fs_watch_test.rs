//! The opt-in filesystem watcher (#937).
//!
//! Default-off behaviour, the path scoping that decides what a notification
//! may index, batch application through the existing sync pipeline, and an
//! end-to-end run against the platform's native watcher.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokensave::mcp::fs_watch::{WatchBatch, WatchSettings};
use tokensave::mcp::McpServer;
use tokensave::tokensave::{TokenSave, WatchedChange};

/// An indexed project with a gitignored `build/` directory.
async fn indexed_project() -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join("src")).expect("src dir");
    std::fs::create_dir_all(root.join("build")).expect("build dir");
    std::fs::write(root.join(".gitignore"), "build/\n").expect(".gitignore");
    std::fs::write(root.join("src").join("lib.rs"), "pub fn original() {}\n").expect("lib.rs");
    std::fs::write(root.join("src").join("gone.rs"), "pub fn doomed() {}\n").expect("gone.rs");
    let cg = TokenSave::init(&root).await.expect("init");
    cg.sync().await.expect("sync");
    drop(cg);
    (tmp, root)
}

async fn server_for(root: &Path) -> Arc<McpServer> {
    let cg = TokenSave::open(root).await.expect("open");
    let server = McpServer::new(cg, None).await;
    assert!(
        server
            .wait_for_startup_catch_up(Duration::from_secs(30))
            .await,
        "startup catch-up should finish"
    );
    server
}

fn change(path: PathBuf, structural: bool) -> WatchedChange {
    WatchedChange { path, structural }
}

async fn indexed_paths(server: &McpServer) -> Vec<String> {
    let mut paths: Vec<String> = server
        .cg()
        .expect("default project")
        .get_all_files()
        .await
        .expect("files")
        .into_iter()
        .map(|f| f.path)
        .collect();
    paths.sort();
    paths
}

async fn has_symbol(server: &McpServer, name: &str) -> bool {
    !server
        .cg()
        .expect("default project")
        .get_nodes_by_name(name)
        .await
        .expect("nodes")
        .is_empty()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watcher_is_off_unless_started() {
    let (_tmp, root) = indexed_project().await;
    let server = server_for(&root).await;
    let status = server.server_stats_json().await;
    assert_eq!(status["filesystem_watch"]["state"], "off", "{status}");
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_keeps_only_in_scope_changes() {
    let (_tmp, root) = indexed_project().await;
    let cg = TokenSave::open(&root).await.expect("open");

    std::fs::write(root.join("src").join("new.rs"), "pub fn fresh() {}\n").expect("new.rs");
    std::fs::write(root.join("build").join("out.rs"), "pub fn built() {}\n").expect("out.rs");
    std::fs::write(root.join("src").join("lib.rs~"), "backup").expect("backup");
    std::fs::write(root.join("src").join("notes.unknownext"), "x").expect("notes");
    std::fs::remove_file(root.join("src").join("gone.rs")).expect("remove");

    let plan = cg
        .plan_watch_batch(&[
            change(root.join("src").join("lib.rs"), false),
            change(root.join("src").join("new.rs"), true),
            change(root.join("src").join("gone.rs"), true),
            change(root.join("build").join("out.rs"), true),
            change(root.join("src").join("lib.rs~"), true),
            change(root.join("src").join("notes.unknownext"), true),
            change(root.join(".tokensave").join("tokensave.db"), false),
            change(root.join(".git").join("index"), false),
            change(root.join("..").join("elsewhere.rs"), true),
        ])
        .await;
    assert_eq!(
        plan.files,
        vec![
            "src/gone.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/new.rs".to_string()
        ]
    );
    assert!(!plan.reconcile);
    assert!(plan.new_dirs.is_empty());

    // A new in-scope directory needs a reconciliation and, per directory,
    // a watch; a new ignored one needs neither.
    std::fs::create_dir_all(root.join("src").join("nested")).expect("nested");
    std::fs::create_dir_all(root.join("build").join("deeper")).expect("deeper");
    let plan = cg
        .plan_watch_batch(&[
            change(root.join("src").join("nested"), true),
            change(root.join("build").join("deeper"), true),
        ])
        .await;
    assert!(plan.reconcile);
    assert_eq!(plan.new_dirs, vec![root.join("src").join("nested")]);

    // A plain write to a directory is not a structural change.
    let plan = cg
        .plan_watch_batch(&[change(root.join("src"), false)])
        .await;
    assert_eq!(plan, tokensave::tokensave::WatchPlan::default());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watch_directories_skip_ignored_and_hidden_trees() {
    let (_tmp, root) = indexed_project().await;
    std::fs::create_dir_all(root.join(".hidden").join("inner")).expect("hidden");
    std::fs::create_dir_all(root.join("src").join("deep")).expect("deep");
    let cg = TokenSave::open(&root).await.expect("open");
    let mut dirs = cg.watch_directories(&root, 1_000).expect("under the limit");
    dirs.sort();
    let mut expected = vec![
        root.clone(),
        root.join("src"),
        root.join("src").join("deep"),
    ];
    expected.sort();
    assert_eq!(dirs, expected);
    assert_eq!(cg.watch_directories(&root, 1), None, "over the limit");
    assert_eq!(
        cg.watch_directories(&root.join("build"), 1_000),
        Some(Vec::new()),
        "an ignored directory yields nothing to watch"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batches_apply_through_the_sync_pipeline() {
    let (_tmp, root) = indexed_project().await;
    let server = server_for(&root).await;

    std::fs::write(
        root.join("src").join("lib.rs"),
        "pub fn original() {}\npub fn edited_in_place() {}\n",
    )
    .expect("edit");
    std::fs::write(root.join("src").join("new.rs"), "pub fn fresh() {}\n").expect("new");
    std::fs::write(root.join("build").join("out.rs"), "pub fn built() {}\n").expect("built");
    std::fs::remove_file(root.join("src").join("gone.rs")).expect("remove");

    let outcome = server
        .apply_watch_batch(WatchBatch {
            changes: vec![
                change(root.join("src").join("lib.rs"), false),
                change(root.join("src").join("new.rs"), true),
                change(root.join("src").join("gone.rs"), true),
                change(root.join("build").join("out.rs"), true),
            ],
            rescan: false,
            reasons: Vec::new(),
        })
        .await;
    assert!(!outcome.reconciled);
    assert_eq!(
        indexed_paths(&server).await,
        vec!["src/lib.rs".to_string(), "src/new.rs".to_string()]
    );
    assert!(has_symbol(&server, "edited_in_place").await);
    assert!(!has_symbol(&server, "built").await);

    // An overflowed batch reconciles with the bounded scan, which finds a
    // change that never produced an event.
    std::fs::write(root.join("src").join("silent.rs"), "pub fn silent() {}\n").expect("silent");
    let outcome = server
        .apply_watch_batch(WatchBatch {
            changes: Vec::new(),
            rescan: true,
            reasons: vec!["test overflow".to_string()],
        })
        .await;
    assert!(outcome.reconciled);
    assert!(has_symbol(&server, "silent").await);

    // A second reconciliation inside the minimum interval is deferred.
    let outcome = server
        .apply_watch_batch(WatchBatch {
            changes: Vec::new(),
            rescan: true,
            reasons: vec!["again".to_string()],
        })
        .await;
    assert!(!outcome.reconciled);
    assert!(outcome.retry_rescan_after.is_some());
    server.shutdown().await;
}

/// End to end through the platform's native watcher: a write lands in the
/// index without a tool call, and shutdown stops the watcher.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_watcher_indexes_changes_and_stops() {
    let (_tmp, root) = indexed_project().await;
    let server = server_for(&root).await;
    server.start_filesystem_watch(WatchSettings {
        debounce: Duration::from_millis(100),
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let status = server.filesystem_watch_status();
        match status["state"].as_str() {
            Some("active") => break,
            Some("failed") => panic!("watcher failed to start: {status}"),
            _ => {}
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "watcher never started"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Some backends deliver events registered just before the watch began;
    // give the registration a moment so the write below is unambiguous.
    tokio::time::sleep(Duration::from_millis(300)).await;

    std::fs::write(
        root.join("src").join("watched.rs"),
        "pub fn seen_by_watcher() {}\n",
    )
    .expect("write");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !has_symbol(&server, "seen_by_watcher").await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the watcher did not index the new file: {}",
            server.filesystem_watch_status()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    server.shutdown().await;
    assert_eq!(server.filesystem_watch_status()["state"], "stopped");
}
