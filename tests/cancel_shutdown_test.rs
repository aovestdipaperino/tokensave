//! #450: a shutdown request must reach work that is not waiting for a signal.
//!
//! The MCP run loop holds a SIGTERM stream for the life of the loop, but polls
//! it only while parked on the next request. That is enough for an idle
//! server and not enough for two cases the issue reports: a signal arriving
//! during a long sync, and a server orphaned by a dead parent, where there is
//! no signal at all. Both go through the process-global flag in `cancel`.

#![cfg(feature = "test-transport")]

use std::sync::Arc;
use std::time::Duration;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;

/// The cancellation flag is process-global, and `#[tokio::test]`s in one
/// binary run concurrently — one test's `request()` would otherwise cancel
/// another's indexing. Every test here takes this first.
fn flag_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn indexed_server() -> (tempfile::TempDir, Arc<McpServer>) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let project = dir.path();
    std::fs::create_dir_all(project.join("src")).expect("create src");
    std::fs::write(project.join("src/main.rs"), "fn main() {}\n").expect("write src");
    let cg = TokenSave::init(project).await.expect("init");
    cg.index_all().await.expect("index");
    let server = McpServer::new(cg, None).await;
    (dir, server)
}

/// Both orderings, in one test on purpose: the flag is process-global, and two
/// `#[tokio::test]`s in one binary run concurrently and would race for it.
///
/// First the reported shape — stdin is held open by a supervisor, so
/// `read_line` never returns and the run loop has nothing else to wake it.
/// Keeping the sender alive is what makes this a real reproduction: drop it
/// and the loop leaves on EOF for the wrong reason.
///
/// Then the race the signal handler and the orphan watchdog can win, where the
/// request lands before the loop ever parks and must not be lost.
#[tokio::test]
async fn a_cancellation_request_leaves_a_run_loop_whose_stdin_never_closes() {
    let _serial = flag_lock().lock().await;
    tokensave::cancel::reset();
    let (_dir, server) = indexed_server().await;
    let (mut transport, _held_open, _out) = ChannelTransport::new();
    // Built up front: indexing is itself cancellable now, so a server created
    // after the request below would fail to index rather than fail to park.
    let (_dir2, server2) = indexed_server().await;
    let (mut transport2, _held_open2, _out2) = ChannelTransport::new();

    let run = tokio::spawn(async move {
        server.run(&mut transport).await.expect("run");
    });

    // Nothing has been requested yet, so the loop must still be parked.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !run.is_finished(),
        "the loop must wait while nothing asks it to stop"
    );

    tokensave::cancel::request();
    let exited = tokio::time::timeout(Duration::from_secs(5), run).await;
    assert!(
        exited.is_ok(),
        "the run loop must leave on a cancellation request, not wait for an EOF that never comes"
    );

    // Flag still set: a loop that starts after the request must observe it on
    // its first poll rather than parking forever.
    let run2 = tokio::spawn(async move {
        server2.run(&mut transport2).await.expect("run");
    });
    let exited2 = tokio::time::timeout(Duration::from_secs(5), run2).await;
    tokensave::cancel::reset();
    assert!(
        exited2.is_ok(),
        "an already-set flag must be observed on the first poll"
    );
}

/// The half of #450 that motivated all of this: a sync in flight must stop,
/// and must leave the index marked stale so the next one redoes the work.
/// (`serve`'s own automatic sync runs this same code.)
#[tokio::test]
async fn a_cancelled_sync_stops_and_leaves_the_index_marked_stale() {
    let _serial = flag_lock().lock().await;
    tokensave::cancel::reset();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let project = dir.path();
    std::fs::create_dir_all(project.join("src")).expect("create src");
    for i in 0..50 {
        std::fs::write(
            project.join(format!("src/m{i}.rs")),
            format!("fn f{i}() -> i32 {{ {i} }}\n"),
        )
        .expect("write src");
    }
    let cg = TokenSave::init(project).await.expect("init");

    tokensave::cancel::request();
    let result = cg.index_all().await;
    tokensave::cancel::reset();

    let err = match result {
        Ok(_) => panic!("a cancelled index must not report success"),
        Err(err) => err.to_string(),
    };
    assert!(
        err.contains("interrupted by shutdown signal"),
        "the error must name the cause, got: {err}"
    );
    assert!(
        project.join(".tokensave").join("dirty").exists(),
        "the dirty sentinel must survive, so the next sync redoes the work"
    );
    // The lock is RAII, so a later sync must be able to run at all — the
    // observable form of "the lock was released".
    tokensave::cancel::reset();
    cg.index_all()
        .await
        .expect("a sync after a cancelled one must be able to take the lock");
    assert!(
        !project.join(".tokensave").join("dirty").exists(),
        "the completing sync must clear the sentinel"
    );
}
