use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokensave::project_watcher::ProjectWatcher;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_sync_callback_fires_after_sync() {
    let tmp = tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    std::fs::write(project.join("a.rs"), "fn a() {}").unwrap();

    // Initialize the project so sync() has a DB to write to.
    let cg = tokensave::tokensave::TokenSave::init(&project)
        .await
        .unwrap();
    cg.sync().await.unwrap();
    drop(cg);

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_cb = counter.clone();
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();

    let pw = ProjectWatcher::new(project.clone(), Duration::from_millis(100)).expect("watcher");

    let handle = tokio::spawn(async move {
        pw.run_with_callback(cancel_for_task, move || {
            let c = counter_cb.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        })
        .await;
    });

    // Give the watcher a moment to arm.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Trigger a change.
    std::fs::write(project.join("a.rs"), "fn a() { let x = 1; }").unwrap();

    // Wait for debounce + sync + callback with a generous ceiling.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while counter.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    cancel.cancel();
    let _ = handle.await;

    assert!(
        counter.load(Ordering::SeqCst) >= 1,
        "callback should fire at least once"
    );
}

/// Issue #80 — writes inside an ignored top-level directory
/// (`target/`, `node_modules/`, `.git/`) must NOT trigger the sync
/// callback. The new scoped watcher achieves this by never registering
/// an OS-level watch for those subtrees; this test pins that down.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writes_in_ignored_dirs_do_not_trigger_sync() {
    let tmp = tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    std::fs::create_dir(project.join("src")).unwrap();
    std::fs::write(project.join("src/a.rs"), "fn a() {}").unwrap();
    // Pre-create every IGNORED_DIRS entry that could plausibly churn
    // (target/, node_modules/, .git/).
    for ignored in ["target", "node_modules", ".git"] {
        std::fs::create_dir(project.join(ignored)).unwrap();
    }

    let cg = tokensave::tokensave::TokenSave::init(&project)
        .await
        .unwrap();
    cg.sync().await.unwrap();
    drop(cg);

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_cb = counter.clone();
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();

    let pw = ProjectWatcher::new(project.clone(), Duration::from_millis(100)).expect("watcher");

    let handle = tokio::spawn(async move {
        pw.run_with_callback(cancel_for_task, move || {
            let c = counter_cb.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        })
        .await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Hammer the ignored directories. None of these should produce
    // sync callbacks — the kernel-side watch is never armed there.
    for i in 0..10 {
        std::fs::write(
            project.join("target").join(format!("artifact-{i}.o")),
            "bin",
        )
        .unwrap();
        std::fs::write(
            project.join("node_modules").join(format!("pkg-{i}.json")),
            "{}",
        )
        .unwrap();
        std::fs::write(project.join(".git").join(format!("ref-{i}")), "deadbeef").unwrap();
    }

    // Generous slack — give the debouncer twice its timeout to flush.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let count_after_ignored = counter.load(Ordering::SeqCst);

    // Now write inside a watched subtree to confirm the watcher is
    // actually alive (rules out "no events at all because watcher
    // crashed").
    std::fs::write(project.join("src/a.rs"), "fn a() { let x = 1; }").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while counter.load(Ordering::SeqCst) == count_after_ignored
        && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    cancel.cancel();
    let _ = handle.await;

    assert_eq!(
        count_after_ignored, 0,
        "writes in ignored dirs must not trigger sync callbacks; got {count_after_ignored}"
    );
    assert!(
        counter.load(Ordering::SeqCst) >= 1,
        "writes in watched subtrees must still trigger sync"
    );
}
