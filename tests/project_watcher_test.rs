use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;
use tokensave::project_watcher::ProjectWatcher;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_sync_callback_fires_after_sync() {
    let tmp = tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    std::fs::write(project.join("a.rs"), "fn a() {}").unwrap();

    // Initialize the project so sync() has a DB to write to.
    let cg = tokensave::tokensave::TokenSave::init(&project).await.unwrap();
    cg.sync().await.unwrap();
    drop(cg);

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_cb = counter.clone();
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();

    let pw = ProjectWatcher::new(project.clone(), Duration::from_millis(100))
        .expect("watcher");

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
