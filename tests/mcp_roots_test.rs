//! A server with no default project picks each call's project from the MCP
//! client's roots.
//!
//! One tokensave process behind an MCP proxy serves every client the user
//! runs, each in its own repository, over one MCP session. The proxy starts
//! it from its own folder, so the server has no default project (#606). The
//! proxy forwards a `roots/list` sent during a tool call to the client that
//! made the call, answers one sent outside a call with an empty list, and
//! does not forward `notifications/roots/list_changed`. So the server asks
//! for the roots on each call, never caches the answer, and never asks
//! outside a call.
//!
//! These tests drive the real serve loop over the in-memory transport and
//! play the client (or the proxy) by hand, answering each `roots/list` the
//! server sends with the roots of the client whose call is in flight.

#![cfg(feature = "test-transport")]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::common::reported_root;
use tokensave::agents::kiro::file_resource_uri;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::{McpServer, McpTransport};
use tokensave::tokensave::TokenSave;

/// Indexes a project with one uniquely named function.
async fn project(dir: &Path, symbol: &str) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src").join("lib.rs"),
        format!("pub fn {symbol}() -> u32 {{ 1 }}\n"),
    )
    .unwrap();
    let cg = TokenSave::init(dir).await.unwrap();
    cg.sync().await.unwrap();
}

/// Two indexed projects, `alpha` and `beta`, side by side.
async fn two_projects() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let alpha = tmp.path().join("alpha");
    let beta = tmp.path().join("beta");
    project(&alpha, "alpha_symbol").await;
    project(&beta, "beta_symbol").await;
    (tmp, alpha, beta)
}

/// One MCP session over the serve loop, seen from the client side.
struct Session {
    to_server: UnboundedSender<String>,
    from_server: UnboundedReceiver<String>,
    /// How many `roots/list` requests the server sent.
    roots_requests: usize,
}

impl Session {
    /// Starts the serve loop and completes the handshake, declaring the
    /// `roots` capability when `roots` is set.
    async fn start(server: Arc<McpServer>, roots: bool) -> Self {
        let (transport, to_server, from_server) = ChannelTransport::new();
        let (reader, writer) = McpTransport::split(transport);
        tokio::spawn(async move { server.run_with_halves(reader, writer, None).await });
        let mut session = Self {
            to_server,
            from_server,
            roots_requests: 0,
        };
        let capabilities = if roots {
            json!({ "roots": {} })
        } else {
            json!({})
        };
        session.send(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": capabilities,
                "clientInfo": { "name": "test", "version": "1" }
            }
        }));
        let init = session.response(1, &[]).await;
        assert!(init.get("result").is_some(), "initialize failed: {init}");
        session.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        session
    }

    fn send(&self, message: Value) {
        self.to_server.send(message.to_string()).unwrap();
    }

    /// The next message from the server that is not a `roots/list`
    /// request, answering each of those with `roots` on the way.
    async fn next(&mut self, roots: &[String]) -> Value {
        loop {
            let line = tokio::time::timeout(Duration::from_secs(30), self.from_server.recv())
                .await
                .expect("the server answers in time")
                .expect("the server is still running");
            let message: Value = serde_json::from_str(line.trim()).expect("a JSON line");
            if message["method"] != "roots/list" {
                return message;
            }
            self.roots_requests += 1;
            let roots: Vec<Value> = roots.iter().map(|uri| json!({ "uri": uri })).collect();
            self.send(json!({
                "jsonrpc": "2.0", "id": message["id"], "result": { "roots": roots }
            }));
        }
    }

    /// Reads up to the response to `id`, answering each `roots/list` the
    /// server sends meanwhile with `roots`.
    async fn response(&mut self, id: u64, roots: &[String]) -> Value {
        loop {
            let message = self.next(roots).await;
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    fn send_search(&self, id: u64, symbol: &str) {
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": "tokensave_search", "arguments": { "query": symbol } }
        }));
    }

    /// Calls `tool`, answering any `roots/list` with `roots`.
    async fn call(&mut self, id: u64, tool: &str, arguments: Value, roots: &[String]) -> Value {
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }));
        self.response(id, roots).await
    }

    /// Searches for `symbol` without `graph_root`.
    async fn search(&mut self, id: u64, symbol: &str, roots: &[String]) -> Value {
        self.call(id, "tokensave_search", json!({ "query": symbol }), roots)
            .await
    }
}

/// The root of the selected graph that answered, from the provenance a
/// selected call carries.
fn answered_from(response: &Value) -> &str {
    response["result"]["_meta"]["tokensave"]["graph_root"]
        .as_str()
        .unwrap_or_else(|| panic!("expected an answer from a selected graph, got {response}"))
}

fn text(response: &Value) -> String {
    response["result"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn error(response: &Value) -> &str {
    response["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("expected an error, got {response}"))
}

#[tokio::test]
async fn behind_a_proxy_each_call_answers_from_its_callers_project() {
    let (_tmp, alpha, beta) = two_projects().await;
    let server = McpServer::new_without_project(Vec::new()).await;
    let mut proxy = Session::start(server, true).await;

    // Client A works in a subfolder of alpha, client B at beta's root.
    let a = [file_resource_uri(&alpha.join("src"))];
    let b = [file_resource_uri(&beta)];

    let from_a = proxy.search(2, "alpha_symbol", &a).await;
    assert_eq!(answered_from(&from_a), reported_root(&alpha));
    assert!(text(&from_a).contains("alpha_symbol"), "{from_a}");

    let from_b = proxy.search(3, "beta_symbol", &b).await;
    assert_eq!(answered_from(&from_b), reported_root(&beta));
    assert!(text(&from_b).contains("beta_symbol"), "{from_b}");

    // Back to A: nothing learned from B's call carries over.
    let again = proxy.search(4, "alpha_symbol", &a).await;
    assert_eq!(answered_from(&again), reported_root(&alpha));

    // Asked during each call, and never outside one, where the proxy would
    // have had no client to ask.
    assert_eq!(proxy.roots_requests, 3);
}

#[tokio::test]
async fn a_caller_without_roots_is_refused_with_the_explanation() {
    // A REST or CLI call through the proxy, or a client without the
    // capability behind it: the proxy answers an empty list.
    let server = McpServer::new_without_project(Vec::new()).await;
    let mut proxy = Session::start(server, true).await;
    let response = proxy.search(2, "alpha_symbol", &[]).await;
    let message = error(&response);
    assert!(message.contains("no roots"), "{message}");
    assert!(message.contains("graph_root"), "{message}");

    // `tokensave_status` still answers, with the same explanation.
    let status = proxy.call(3, "tokensave_status", json!({}), &[]).await;
    assert!(text(&status).contains("no roots"), "{status}");
}

#[tokio::test]
async fn roots_in_no_project_or_in_several_are_refused() {
    let (tmp, alpha, beta) = two_projects().await;
    let server = McpServer::new_without_project(Vec::new()).await;
    let mut session = Session::start(server, true).await;

    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let response = session
        .search(2, "alpha_symbol", &[file_resource_uri(&outside)])
        .await;
    assert!(error(&response).contains("indexed project"), "{response}");

    let both = [file_resource_uri(&alpha), file_resource_uri(&beta)];
    let response = session.search(3, "alpha_symbol", &both).await;
    let message = error(&response);
    assert!(message.contains("several"), "{message}");
    assert!(
        message.contains("alpha") && message.contains("beta"),
        "{message}"
    );
}

#[tokio::test]
async fn a_client_without_the_roots_capability_is_never_asked() {
    let server = McpServer::new_without_project(Vec::new()).await;
    let mut session = Session::start(server, false).await;
    let response = session.search(2, "alpha_symbol", &[]).await;
    let message = error(&response);
    assert!(
        message.contains("has no graph to answer from") && message.contains("graph_root"),
        "the #606 refusal is unchanged: {message}"
    );
    assert_eq!(session.roots_requests, 0);
}

#[tokio::test]
async fn an_explicit_graph_root_wins_over_the_roots() {
    let (_tmp, alpha, beta) = two_projects().await;
    let server = McpServer::new_without_project(Vec::new()).await;
    let mut session = Session::start(server, true).await;
    let response = session
        .call(
            2,
            "tokensave_search",
            json!({ "query": "beta_symbol", "graph_root": reported_root(&beta) }),
            &[file_resource_uri(&alpha)],
        )
        .await;
    assert_eq!(answered_from(&response), reported_root(&beta));
    assert_eq!(session.roots_requests, 0, "graph_root needs no roots");
}

#[tokio::test]
async fn a_server_with_a_default_project_never_asks_for_roots() {
    // Direct use: the client launched tokensave inside its project.
    let (tmp, alpha, beta) = two_projects().await;
    let cg = TokenSave::open(&alpha).await.unwrap();
    let server = McpServer::new_with_global_db_path(cg, None, tmp.path().join("global.db")).await;
    let mut session = Session::start(server, true).await;

    // Roots naming another project change nothing.
    let response = session
        .search(2, "alpha_symbol", &[file_resource_uri(&beta)])
        .await;
    assert!(response.get("error").is_none(), "{response}");
    assert!(text(&response).contains("alpha_symbol"), "{response}");
    assert!(
        response["result"]["_meta"]["tokensave"]["graph_root"].is_null(),
        "answered from the default project, not a selected one: {response}"
    );
    assert_eq!(session.roots_requests, 0);
}

#[tokio::test]
async fn calls_waiting_for_roots_do_not_stall_the_server() {
    // Read-only calls run four at a time (#685). With four of them waiting
    // for roots, the serve loop waits for a permit to start the fifth, so
    // the roots requests and their answers must not pass through the loop.
    let (_tmp, alpha, _beta) = two_projects().await;
    let server = McpServer::new_without_project(Vec::new()).await;
    let mut session = Session::start(server, true).await;
    let roots = [file_resource_uri(&alpha)];

    for id in 10..16 {
        session.send_search(id, "alpha_symbol");
    }
    let mut answered = std::collections::HashSet::new();
    while answered.len() < 6 {
        let response = session.next(&roots).await;
        assert_eq!(answered_from(&response), reported_root(&alpha));
        answered.insert(response["id"].as_u64().expect("a response id"));
    }
    assert_eq!(session.roots_requests, 6);
}
