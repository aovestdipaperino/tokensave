//! The MCP server answers read-only tool calls concurrently (#685).
//!
//! The server used to handle one request at a time, so a burst of parallel
//! searches queued behind each other and the client waited for the sum of
//! them. A read-only tool call does not change the index, so the server now
//! runs those side by side, up to a bounded number at a time.
//!
//! These tests pin the policy — which calls may run together — and that a
//! burst of them is answered completely.

#![cfg(feature = "test-transport")]

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::{McpServer, McpTransport};
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

async fn indexed_server() -> (TempDir, Arc<McpServer>) {
    let dir = TempDir::new().unwrap();
    let project = dir.path().to_path_buf();

    git(&project, &["init", "-b", "master"]);
    std::fs::write(
        project.join("lib.rs"),
        "pub fn alpha() {}\npub fn beta() {}\npub fn gamma() {}\n",
    )
    .unwrap();
    git(&project, &["add", "-A"]);
    git(&project, &["commit", "-m", "init"]);

    let graph = TokenSave::init(&project).await.unwrap();
    graph.index_all().await.unwrap();
    graph.checkpoint().await.unwrap();

    let server = McpServer::new(graph, None).await;
    (dir, server)
}

fn request(id: i64, name: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
    .to_string()
}

/// The calls the server may run side by side are the ones that only read the
/// graph. Everything that changes the index still runs alone.
#[tokio::test]
async fn only_read_only_tools_are_concurrent_safe() {
    let (_dir, server) = indexed_server().await;
    for name in [
        "tokensave_search",
        "tokensave_context",
        "tokensave_read",
        "tokensave_body",
        "tokensave_callers",
        "tokensave_impact",
        "tokensave_files",
    ] {
        assert!(server.is_read_only_tool(name), "{name} should be read-only");
    }
    for name in [
        "tokensave_str_replace",
        "tokensave_multi_str_replace",
        "tokensave_insert_at",
        "tokensave_delete_symbol",
        "tokensave_rename",
        "tokensave_replace_lines",
        "tokensave_ast_grep_rewrite",
    ] {
        assert!(
            !server.is_read_only_tool(name),
            "{name} must not be read-only"
        );
    }
}

/// A burst of read-only calls is answered completely, and every response
/// carries the id of its request.
#[tokio::test]
async fn a_burst_of_read_only_calls_is_answered_completely() {
    let (_dir, server) = indexed_server().await;
    let (transport, sender, mut receiver) = ChannelTransport::new();
    let (reader, writer) = McpTransport::split(transport);

    // Six calls, sent without waiting for any answer: the server used to
    // handle them one at a time, so the client waited for the sum of them.
    for id in 1..=6 {
        sender
            .send(request(id, "tokensave_search", json!({ "query": "alpha" })))
            .unwrap();
    }
    drop(sender);

    let run = tokio::spawn(async move { server.run_with_halves(reader, writer, None).await });

    let mut ids = Vec::new();
    for _ in 0..6 {
        let line = receiver.recv().await.expect("expected a response line");
        let value: Value = serde_json::from_str(line.trim()).expect("response is JSON");
        ids.push(value["id"].as_i64().expect("response carries an id"));
    }
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3, 4, 5, 6]);

    // The server ends when the client closes the transport.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), run).await;
}
