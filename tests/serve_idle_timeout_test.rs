//! #436: a host that never closes a server's stdin leaks one server per
//! subagent, each holding its index open.
//!
//! The servers are all children of the same *still-live* supervisor, so there
//! is no dead-parent signal to key on and the orphan watchdog cannot see them;
//! the EOF that would normally stop them never arrives either. That is the
//! host's bug, but an opt-in idle deadline bounds the damage without waiting
//! for it.
//!
//! The transport here reproduces the reported condition exactly: the sender
//! stays alive for the whole test, so `read_line` never returns and the server
//! has nothing else to wake it.

#![cfg(feature = "test-transport")]

use std::sync::Arc;
use std::time::Duration;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;

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

fn request(id: u32) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/list"}}"#)
}

/// The reported shape. Without a deadline this server would sit here forever.
#[tokio::test]
async fn an_idle_server_exits_when_a_deadline_is_set() {
    let (_dir, server) = indexed_server().await;
    let (mut transport, _tx, _rx) = ChannelTransport::new();

    // `_tx` is deliberately held for the whole test — that is the supervisor
    // keeping stdin open.
    let run = server.run_with_idle_timeout(&mut transport, Some(Duration::from_secs(1)));
    let finished = tokio::time::timeout(Duration::from_secs(20), run).await;

    assert!(
        finished.is_ok(),
        "a server with an idle deadline must exit while its stdin is still held open"
    );
    assert!(
        finished.expect("completed").is_ok(),
        "exit must be graceful"
    );
}

/// Omitting the flag preserves today's indefinite lifetime, so no existing
/// setup changes behaviour.
#[tokio::test]
async fn no_deadline_means_the_server_still_waits_indefinitely() {
    let (_dir, server) = indexed_server().await;
    let (mut transport, _tx, _rx) = ChannelTransport::new();

    let run = server.run_with_idle_timeout(&mut transport, None);
    let finished = tokio::time::timeout(Duration::from_secs(3), run).await;

    assert!(
        finished.is_err(),
        "without a deadline an idle server must keep waiting"
    );
}

/// The deadline measures *idleness*, not uptime: each request restarts the
/// window. A server busier than its timeout must never be cut off, which is
/// the difference between bounding a leak and killing working servers.
#[tokio::test]
async fn each_request_restarts_the_window() {
    let (_dir, server) = indexed_server().await;
    let (mut transport, tx, _rx) = ChannelTransport::new();

    let feeder = tokio::spawn(async move {
        // Four requests at 400 ms against a 1 s deadline: total elapsed time
        // passes the timeout more than once, but no single gap does.
        for id in 0..4u32 {
            tokio::time::sleep(Duration::from_millis(400)).await;
            if tx.send(format!("{}\n", request(id))).is_err() {
                return false;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        // Holding `tx` until here is what keeps the server parked rather than
        // seeing EOF; dropping it now lets the run loop finish the test.
        true
    });

    let run = server.run_with_idle_timeout(&mut transport, Some(Duration::from_secs(1)));
    let outcome = tokio::time::timeout(Duration::from_secs(20), run).await;

    let all_sent = feeder.await.expect("feeder task");
    assert!(
        all_sent,
        "the server exited mid-stream: the deadline counted uptime rather than idleness"
    );
    assert!(outcome.is_ok(), "the server should have exited on EOF");
}
