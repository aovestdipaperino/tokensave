//! The pre-dispatch resync must not be able to stall an interactive tool call
//! past a client's request deadline (#535).
//!
//! Before the fix, `maybe_sync_if_stale` ran the tree walk and resync inline
//! and unbounded, so a large tree could outlast the caller's timeout — and a
//! client that disables a server on timeout (plank) lost tokensave for the
//! rest of the session. The budget bounds how long the call *waits*; the work
//! itself is never dropped part-way, it finishes in the background.

use std::sync::OnceLock;
use std::time::Duration;
use tempfile::tempdir;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;
use tokio::sync::{Mutex, MutexGuard};

/// Both tests drive the same process-wide `TOKENSAVE_AUTOSYNC_BUDGET_MS`, and
/// `cargo test` runs them on threads of one process — so they must not overlap.
async fn env_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().await
}

/// Builds a tree large enough that a walk plus resync cannot plausibly finish
/// inside a 1 ms budget.
fn write_tree(root: &std::path::Path, files: usize) {
    for i in 0..files {
        let dir = root.join(format!("pkg{}", i % 16));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("m{i}.rs")),
            format!("pub fn f{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
}

/// Push the recorded sync time far enough into the past that the 30 s cooldown
/// in `maybe_sync_if_stale` does not short-circuit the call under test.
async fn backdate_last_sync(server: &McpServer) {
    let long_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        - 3_600;
    server
        .cg()
        .db()
        .set_metadata("last_sync_at", &long_ago.to_string())
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_resync_does_not_block_the_call_past_its_budget() {
    let _env = env_guard().await;
    let tmp = tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    write_tree(&project, 400);

    let cg_init = TokenSave::init(&project).await.unwrap();
    cg_init.sync().await.unwrap();
    drop(cg_init);

    let cg = TokenSave::open(&project).await.unwrap();
    let server = McpServer::new(cg, None).await;
    assert!(
        server
            .wait_for_startup_catch_up(Duration::from_secs(30))
            .await,
        "startup catch-up should finish before the measurement"
    );

    // New work for the lazy resync to find, and a budget it cannot meet.
    write_tree(&project, 800);
    backdate_last_sync(&server).await;
    server.reset_staleness_cooldown();
    std::env::set_var("TOKENSAVE_AUTOSYNC_BUDGET_MS", "1");

    let started = std::time::Instant::now();
    server.maybe_sync_if_stale().await;
    let waited = started.elapsed();

    // The assertion that matters: the call returned on its budget instead of
    // waiting the resync out. Bounds are generous — the point is that this is
    // decoupled from the size of the tree, not that it is microsecond-fast.
    assert!(
        waited < Duration::from_secs(5),
        "call blocked {waited:?} on a 1 ms budget; the resync is still inline"
    );
    assert!(
        server.lazy_sync_in_flight(),
        "the resync should have been deferred to the background, not completed inline — \
         if this fails the tree was too small for the budget to bite"
    );

    // And the deferred work is not lost: it finishes, it is not dropped
    // part-way through its DB writes.
    assert!(
        server.wait_for_lazy_sync(Duration::from_secs(120)).await,
        "the backgrounded resync should still run to completion"
    );

    std::env::remove_var("TOKENSAVE_AUTOSYNC_BUDGET_MS");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_zero_budget_restores_the_unbounded_wait() {
    let _env = env_guard().await;
    let tmp = tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    write_tree(&project, 40);

    let cg_init = TokenSave::init(&project).await.unwrap();
    cg_init.sync().await.unwrap();
    drop(cg_init);

    let cg = TokenSave::open(&project).await.unwrap();
    let server = McpServer::new(cg, None).await;
    assert!(
        server
            .wait_for_startup_catch_up(Duration::from_secs(30))
            .await,
        "startup catch-up should finish before the measurement"
    );

    write_tree(&project, 80);
    backdate_last_sync(&server).await;
    server.reset_staleness_cooldown();
    std::env::set_var("TOKENSAVE_AUTOSYNC_BUDGET_MS", "0");

    server.maybe_sync_if_stale().await;

    // Opting out means the call waits the resync out, as it did before #535.
    assert!(
        !server.lazy_sync_in_flight(),
        "a zero budget must wait for the resync to finish before returning"
    );

    std::env::remove_var("TOKENSAVE_AUTOSYNC_BUDGET_MS");
}
