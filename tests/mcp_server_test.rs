//! Integration tests for the MCP server (`McpServer`) exercising the full
//! JSON-RPC 2.0 protocol via `ChannelTransport`.
//!
//! Run with: `cargo test --features test-transport --test mcp_server_test`

#![cfg(feature = "test-transport")]

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::time::SystemTime;
use tempfile::TempDir;
use tokensave::branch_meta;
use tokensave::db::{migrations::latest_version, Database};
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Creates a temporary Rust project, indexes it, and returns a ready server.
async fn setup_server() -> (TempDir, Arc<McpServer>) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() { let x = helper(); }\nfn helper() -> i32 { 42 }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;
    (dir, server)
}

async fn setup_named_project(function_name: &str) -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        format!("fn {function_name}() -> i32 {{ 42 }}\n"),
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn setup_fallback_project(function_name: &str) -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        format!("fn {function_name}() -> i32 {{ 42 }}\n"),
    )
    .unwrap();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=TokenSave Test",
            "-c",
            "user.email=tokensave@example.invalid",
            "commit",
            "-qm",
            "initial",
        ],
    ] {
        let status = ProcessCommand::new("git")
            .args(args)
            .current_dir(project)
            .status()
            .unwrap();
        assert!(status.success());
    }
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let status = ProcessCommand::new("git")
        .args(["checkout", "-qb", "feature"])
        .current_dir(project)
        .status()
        .unwrap();
    assert!(status.success());
    (dir, cg)
}

fn run_git(project: &Path, args: &[&str]) {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(project)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.invalid")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.invalid")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn setup_selected_branch_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    run_git(project, &["init", "-b", "main"]);
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn main_only() {}\n").unwrap();
    run_git(project, &["add", "src/lib.rs"]);
    run_git(project, &["commit", "-m", "main"]);

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();
    graph.checkpoint().await.unwrap();
    drop(graph);

    run_git(project, &["checkout", "-b", "feature"]);
    fs::write(project.join("src/lib.rs"), "pub fn feature_only() {}\n").unwrap();
    run_git(project, &["add", "src/lib.rs"]);
    run_git(project, &["commit", "-m", "feature"]);

    let tokensave_dir = project.join(".tokensave");
    fs::create_dir_all(tokensave_dir.join("branches")).unwrap();
    fs::copy(
        tokensave_dir.join("tokensave.db"),
        tokensave_dir.join("branches/feature.db"),
    )
    .unwrap();
    let mut meta = branch_meta::load_branch_meta(&tokensave_dir).unwrap();
    meta.add_branch("feature", "branches/feature.db", "main");
    branch_meta::save_branch_meta(&tokensave_dir, &meta).unwrap();

    let feature = TokenSave::open_branch(project, "feature").await.unwrap();
    feature.index_all().await.unwrap();
    feature.checkpoint().await.unwrap();
    drop(feature);

    dir
}

async fn setup_colliding_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    run_git(project, &["init", "-b", "main"]);
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub fn shared_caller() { shared_target(); }\npub fn shared_target() {}\n",
    )
    .unwrap();
    run_git(project, &["add", "src/lib.rs"]);
    run_git(project, &["commit", "-m", "main"]);

    let graph = TokenSave::init(project).await.unwrap();
    graph.index_all().await.unwrap();
    graph.checkpoint().await.unwrap();
    drop(graph);

    let tokensave_dir = project.join(".tokensave");
    fs::create_dir_all(tokensave_dir.join("branches")).unwrap();
    fs::copy(
        tokensave_dir.join("tokensave.db"),
        tokensave_dir.join("branches/feature.db"),
    )
    .unwrap();
    let mut meta = branch_meta::load_branch_meta(&tokensave_dir).unwrap();
    meta.add_branch("feature", "branches/feature.db", "main");
    branch_meta::save_branch_meta(&tokensave_dir, &meta).unwrap();

    dir
}

async fn call_server(server: &Arc<McpServer>, id: i64, name: &str, arguments: Value) -> Value {
    let (mut transport, _sender, mut receiver) = ChannelTransport::new();
    let request = jsonrpc_request(
        json!(id),
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    );
    server.handle_and_write(&request, &mut transport).await;
    let response = receiver.recv().await.expect("expected tool response");
    parse_response(response.trim())
}

fn response_text(response: &Value) -> String {
    response["result"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn read_cache_row_count(db_path: &std::path::Path) -> i64 {
    let db = Database::open_read_only(db_path).await.unwrap();
    let mut rows = db
        .conn()
        .query("SELECT COUNT(*) FROM read_cache", ())
        .await
        .unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}

fn first_graph_id(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => value
            .split(|character: char| {
                !(character.is_ascii_alphanumeric() || character == '_' || character == ':')
            })
            .find(|token| {
                let parts = token.split(':').collect::<Vec<_>>();
                parts.len() == 4
                    && parts[0] == "graph"
                    && parts[1].len() == 32
                    && parts[3].len() == 32
            })
            .map(ToString::to_string),
        Value::Array(values) => values.iter().find_map(first_graph_id),
        Value::Object(values) => values.values().find_map(first_graph_id),
        _ => None,
    }
}

fn raw_graph_id(qualified: &str) -> &str {
    qualified
        .split_once(':')
        .and_then(|(_, rest)| rest.split_once(':'))
        .map(|(_, raw)| raw)
        .unwrap()
}

fn collect_structured_ids(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_structured_ids(value, ids);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key == "id"
                    || key.ends_with("_id")
                    || key.ends_with("_ids")
                    || key == "dispatch_from"
                {
                    collect_reference_values(value, ids);
                } else {
                    collect_structured_ids(value, ids);
                }
            }
        }
        _ => {}
    }
}

fn collect_reference_values(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::String(value) => ids.push(value.clone()),
        Value::Array(values) => {
            for value in values {
                collect_reference_values(value, ids);
            }
        }
        _ => {}
    }
}

fn response_structured_ids(response: &Value) -> Vec<String> {
    response["result"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["text"].as_str())
        .filter_map(|text| serde_json::from_str::<Value>(text).ok())
        .fold(Vec::new(), |mut ids, value| {
            collect_structured_ids(&value, &mut ids);
            ids
        })
}

fn response_payload(response: &Value) -> Value {
    response["result"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["text"].as_str())
        .find_map(|text| serde_json::from_str(text).ok())
        .unwrap_or_else(|| panic!("response contains no JSON payload: {response}"))
}

#[derive(Debug, PartialEq)]
struct SemanticGraphSnapshot {
    nodes: Vec<String>,
    edges: Vec<String>,
    files: Vec<String>,
    metadata: Vec<String>,
    user_version: i64,
}

#[derive(Debug, PartialEq)]
struct ProjectSnapshot {
    source: Vec<u8>,
    source_mtime: SystemTime,
    graph: SemanticGraphSnapshot,
    config: Vec<u8>,
    branch_meta: Option<Vec<u8>>,
    last_sync: i64,
}

async fn query_snapshot_rows(database: &Database, sql: &str) -> Vec<String> {
    let mut rows = database.conn().query(sql, ()).await.unwrap();
    let mut snapshot = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        snapshot.push(row.get::<String>(0).unwrap());
    }
    snapshot
}

async fn semantic_graph_snapshot(database: &Database) -> SemanticGraphSnapshot {
    let nodes = query_snapshot_rows(
        database,
        "SELECT json_array(
            id, kind, name, qualified_name, file_path, start_line, end_line,
            start_column, end_column, docstring, signature, visibility, is_async,
            branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls,
            assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity,
            distinct_operators, distinct_operands, total_operators, total_operands,
            search_terms
         ) FROM nodes ORDER BY id",
    )
    .await;
    let edges = query_snapshot_rows(
        database,
        "SELECT json_array(id, source, target, kind, line) FROM edges ORDER BY id",
    )
    .await;
    let files = query_snapshot_rows(
        database,
        "SELECT json_array(path, content_hash, size, modified_at, indexed_at, node_count)
         FROM files ORDER BY path",
    )
    .await;
    let metadata = query_snapshot_rows(
        database,
        "SELECT json_array(key, value) FROM metadata ORDER BY key",
    )
    .await;
    let mut rows = database
        .conn()
        .query("PRAGMA user_version", ())
        .await
        .unwrap();
    let user_version = rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap();
    SemanticGraphSnapshot {
        nodes,
        edges,
        files,
        metadata,
        user_version,
    }
}

async fn project_snapshot(root: &Path, branch: Option<&str>, source: &str) -> ProjectSnapshot {
    // Active-WAL visibility and writer-checkpoint coexistence are exercised by
    // migration_test's focused read-only tests. This MCP assertion compares
    // logical rows through the same read-only open without introducing a
    // second concurrency test.
    let graph = TokenSave::open_read_only(root, branch).await.unwrap();
    let last_sync = graph.last_sync_timestamp().await;
    let semantic_graph = semantic_graph_snapshot(graph.db()).await;

    let source_path = root.join(source);
    ProjectSnapshot {
        source: fs::read(&source_path).unwrap(),
        source_mtime: fs::metadata(source_path).unwrap().modified().unwrap(),
        graph: semantic_graph,
        config: fs::read(root.join(".tokensave/config.json")).unwrap(),
        branch_meta: fs::read(root.join(".tokensave/branch-meta.json")).ok(),
        last_sync,
    }
}

fn monitor_snapshot(home: &Path) -> Option<Vec<u8>> {
    fs::read(home.join(".tokensave/monitor.mmap")).ok()
}

/// Sends a sequence of JSON-RPC messages to a server, runs it to completion,
/// and returns all non-empty response lines.
async fn run_server_with_messages(server: Arc<McpServer>, messages: Vec<String>) -> Vec<String> {
    let (mut transport, sender, mut receiver) = ChannelTransport::new();

    for msg in messages {
        sender.send(msg).unwrap();
    }
    drop(sender);

    let handle = tokio::spawn(async move {
        server.run(&mut transport).await.unwrap();
    });

    let mut responses = Vec::new();
    while let Some(line) = receiver.recv().await {
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            responses.push(trimmed);
        }
    }
    handle.await.unwrap();
    responses
}

/// A Windows-style scope prefix must be normalized to forward slashes so it
/// matches the `/`-separated paths stored in the DB (#242). Before the fix
/// every scoped query on Windows returned zero results.
#[tokio::test]
async fn test_scope_prefix_backslashes_normalized() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, Some("plugins\\obs\\my-plugin".to_string())).await;
    assert_eq!(server.scope_prefix(), Some("plugins/obs/my-plugin"));
}

/// Helper to build a JSON-RPC request string.
fn jsonrpc_request(id: Value, method: &str, params: Value) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    }))
    .unwrap()
}

/// Helper to build a JSON-RPC notification string (no id).
fn jsonrpc_notification(method: &str) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "method": method
    }))
    .unwrap()
}

/// Parses a JSON-RPC response and returns it.
fn parse_response(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

// ---------------------------------------------------------------------------
// 1. test_initialize
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_initialize() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(1), "initialize", json!({}))],
    )
    .await;

    assert!(!responses.is_empty(), "should have at least one response");
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 1);
    assert!(resp["result"]["protocolVersion"].is_string());
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(resp["result"]["serverInfo"]["name"], "tokensave");
    assert!(resp["result"]["serverInfo"]["version"].is_string());
}

/// The response to a replayed `initialize` must be newline-terminated.
/// `serve` consumes the first stdin line when it peeks at `initialize.roots`
/// (#331) and replays it through `handle_and_write`, which wrote the response
/// without the trailing `\n` that `run()` appends. A line-framed MCP client
/// never saw the handshake complete, so the whole roots fallback hung with a
/// live client even once the URI parsing was fixed.
#[tokio::test]
async fn test_replayed_initialize_response_is_newline_terminated() {
    let (_dir, server) = setup_server().await;
    let (mut transport, sender, mut receiver) = ChannelTransport::new();
    let request = jsonrpc_request(json!(1), "initialize", json!({}));
    server.handle_and_write(&request, &mut transport).await;

    let line = receiver.recv().await.expect("expected a response line");
    assert!(
        line.ends_with('\n'),
        "replayed response must be newline-terminated, got: {line:?}"
    );
    let resp = parse_response(line.trim());
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["serverInfo"]["name"], "tokensave");

    // The replay is a real handled request and increments `total_requests`.
    // `run()` must distinguish that from the server loop having already run;
    // otherwise debug builds panic here before accepting the client's next
    // message, even though release builds appear to work.
    sender
        .send(jsonrpc_request(json!(2), "ping", json!({})))
        .unwrap();
    drop(sender);
    server.run(&mut transport).await.unwrap();

    let line = receiver.recv().await.expect("expected a ping response");
    let resp = parse_response(line.trim());
    assert_eq!(resp["id"], 2);
    assert!(resp["result"].is_object());
}

// ---------------------------------------------------------------------------
// 2. test_initialized_notification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_initialized_notification() {
    let (_dir, server) = setup_server().await;
    // Send "initialized" notification (no id), then a ping to verify server is alive.
    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_notification("initialized"),
            jsonrpc_request(json!(2), "ping", json!({})),
        ],
    )
    .await;

    // The notification should produce no response; we should only get the ping response.
    // Filter to find the ping response.
    let ping_responses: Vec<&String> = responses
        .iter()
        .filter(|r| {
            let v = parse_response(r);
            v["id"] == 2
        })
        .collect();
    assert_eq!(
        ping_responses.len(),
        1,
        "should get exactly one ping response"
    );
    let resp = parse_response(ping_responses[0]);
    assert!(resp["error"].is_null(), "ping should succeed");
}

// ---------------------------------------------------------------------------
// 3. test_notifications_initialized
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_notifications_initialized() {
    let (_dir, server) = setup_server().await;
    // Send "notifications/initialized" notification, then ping.
    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_notification("notifications/initialized"),
            jsonrpc_request(json!(3), "ping", json!({})),
        ],
    )
    .await;

    let ping_responses: Vec<&String> = responses
        .iter()
        .filter(|r| {
            let v = parse_response(r);
            v["id"] == 3
        })
        .collect();
    assert_eq!(
        ping_responses.len(),
        1,
        "should get exactly one ping response"
    );
}

// ---------------------------------------------------------------------------
// 4. test_ping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_ping() {
    let (_dir, server) = setup_server().await;
    let responses =
        run_server_with_messages(server, vec![jsonrpc_request(json!(10), "ping", json!({}))]).await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 10);
    assert!(
        resp["result"].is_object(),
        "ping result should be an object"
    );
    assert!(resp["error"].is_null(), "ping should not have an error");
}

// ---------------------------------------------------------------------------
// 5. test_tools_list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_list() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(20), "tools/list", json!({}))],
    )
    .await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 20);
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty(), "tools list should not be empty");
    // Verify at least some well-known tools are present.
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        tool_names.contains(&"tokensave_search"),
        "should have tokensave_search"
    );
    assert!(
        tool_names.contains(&"tokensave_status"),
        "should have tokensave_status"
    );
    assert!(
        tool_names.contains(&"tokensave_context"),
        "should have tokensave_context"
    );
}

// ---------------------------------------------------------------------------
// 6. test_tools_call_search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_search() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(30),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "helper" }
            }),
        )],
    )
    .await;

    // Find the response with id=30 (skip any notifications).
    let resp_str = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 30
        })
        .expect("should have a response for id=30");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "search should not error");
    let content = resp["result"]["content"].as_array().unwrap();
    // At least one content item should contain "helper".
    let has_helper = content.iter().any(|c| {
        c["text"]
            .as_str()
            .map(|t| t.contains("helper"))
            .unwrap_or(false)
    });
    assert!(has_helper, "search results should contain 'helper'");
}

#[tokio::test]
async fn selected_search_is_stateless_and_preserves_local_default() {
    let (local_dir, local) = setup_named_project("local_only").await;
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    drop(foreign);
    let server = McpServer::new(local, None).await;
    let graph_root = foreign_dir.path().display().to_string();

    let selected = call_server(
        &server,
        33,
        "tokensave_search",
        json!({ "query": "foreign_only", "graph_root": graph_root }),
    )
    .await;
    assert!(selected["error"].is_null(), "{selected}");
    let selected_text = response_text(&selected);
    assert!(selected_text.contains("foreign_only"), "{selected_text}");
    assert!(!selected_text.contains("local_only"), "{selected_text}");
    assert_eq!(
        selected["result"]["_meta"]["tokensave"]["graph_root"],
        foreign_dir
            .path()
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
    );
    assert_eq!(selected["result"]["_meta"]["tokensave"]["selected"], true);
    assert!(
        selected_text.contains("tokensave_graph:"),
        "{selected_text}"
    );

    let repeated = call_server(
        &server,
        34,
        "tokensave_search",
        json!({
            "query": "foreign_only",
            "graph_root": foreign_dir.path().display().to_string()
        }),
    )
    .await;
    assert!(repeated["error"].is_null(), "{repeated}");
    assert!(response_text(&repeated).contains("foreign_only"));

    let local = call_server(
        &server,
        35,
        "tokensave_search",
        json!({ "query": "local_only" }),
    )
    .await;
    assert!(local["error"].is_null(), "{local}");
    let local_text = response_text(&local);
    assert!(local_text.contains("local_only"), "{local_text}");
    assert!(!local_text.contains("foreign_only"), "{local_text}");
    assert!(local["result"]["_meta"]["tokensave"].is_null(), "{local}");

    drop(local_dir);
}

#[tokio::test]
async fn selected_explicit_branch_returns_branch_content_and_provenance() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let selected_dir = setup_selected_branch_project().await;
    let server = McpServer::new(local, None).await;

    let response = call_server(
        &server,
        34,
        "tokensave_search",
        json!({
            "query": "feature_only",
            "graph_root": selected_dir.path().display().to_string(),
            "graph_branch": "feature"
        }),
    )
    .await;

    assert!(response["error"].is_null(), "{response}");
    let text = response_text(&response);
    assert!(text.contains("feature_only"), "{text}");
    assert!(!text.contains("main_only"), "{text}");
    assert_eq!(
        response["result"]["_meta"]["tokensave"]["graph_branch"],
        "feature"
    );
}

#[tokio::test]
async fn selected_omitted_branch_routes_tracked_current_branch() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let selected_dir = setup_selected_branch_project().await;
    let server = McpServer::new(local, None).await;

    let response = call_server(
        &server,
        35,
        "tokensave_search",
        json!({
            "query": "feature_only",
            "graph_root": selected_dir.path().display().to_string()
        }),
    )
    .await;

    assert!(response["error"].is_null(), "{response}");
    let text = response_text(&response);
    assert!(text.contains("feature_only"), "{text}");
    assert!(!text.contains("main_only"), "{text}");
    assert!(!text.contains("fallback index"), "{text}");
    assert_eq!(
        response["result"]["_meta"]["tokensave"]["graph_branch"],
        "feature"
    );
}

#[tokio::test]
async fn selected_omitted_branch_uses_checkout_fallback_with_provenance() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let (selected_dir, selected) = setup_fallback_project("main_only").await;
    selected.checkpoint().await.unwrap();
    drop(selected);
    let server = McpServer::new(local, None).await;

    let response = call_server(
        &server,
        35,
        "tokensave_search",
        json!({
            "query": "main_only",
            "graph_root": selected_dir.path().display().to_string()
        }),
    )
    .await;

    assert!(response["error"].is_null(), "{response}");
    let text = response_text(&response);
    assert!(text.contains("main_only"), "{text}");
    assert!(text.contains("is using a fallback index"), "{text}");
    assert_eq!(
        response["result"]["_meta"]["tokensave"]["graph_branch"],
        "main"
    );
}

#[tokio::test]
async fn selected_read_bypasses_cache_and_never_returns_unchanged() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let (foreign_dir, foreign) = setup_named_project("foreign_read_source").await;
    foreign.db().checkpoint().await.unwrap();
    drop(foreign);
    let server = McpServer::new(local, None).await;
    let graph_root = foreign_dir.path().display().to_string();
    let db_path = foreign_dir.path().join(".tokensave/tokensave.db");
    let db_before = fs::read(&db_path).unwrap();
    let cache_rows_before = read_cache_row_count(&db_path).await;
    assert_eq!(cache_rows_before, 0);

    for id in [35, 36] {
        let response = call_server(
            &server,
            id,
            "tokensave_read",
            json!({
                "file": "src/main.rs",
                "mode": "full",
                "graph_root": graph_root
            }),
        )
        .await;

        assert!(response["error"].is_null(), "{response}");
        let text = response_text(&response);
        assert!(text.contains("foreign_read_source"), "{text}");
        assert!(!text.contains("\"unchanged\": true"), "{text}");
    }

    assert_eq!(read_cache_row_count(&db_path).await, cache_rows_before);
    assert_eq!(fs::read(&db_path).unwrap(), db_before);
}

#[tokio::test]
async fn selected_read_rejects_paths_outside_canonical_selected_root() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let container = TempDir::new().unwrap();
    let selected_root = container.path().join("selected");
    fs::create_dir_all(selected_root.join("src")).unwrap();
    fs::write(
        selected_root.join("src/main.rs"),
        "fn selected_inside() {}\n",
    )
    .unwrap();
    let selected = TokenSave::init(&selected_root).await.unwrap();
    selected.index_all().await.unwrap();
    selected.checkpoint().await.unwrap();
    drop(selected);

    let outside = container.path().join("outside-secret.txt");
    let secret = "OUTSIDE_BYTES_MUST_NOT_BE_RETURNED";
    fs::write(&outside, secret).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, selected_root.join("linked-secret.txt")).unwrap();

    let server = McpServer::new(local, None).await;
    let graph_root = selected_root.display().to_string();
    let db_path = selected_root.join(".tokensave/tokensave.db");
    let cache_rows_before = read_cache_row_count(&db_path).await;
    let mut paths = vec![
        outside.display().to_string(),
        "../outside-secret.txt".to_string(),
    ];
    #[cfg(unix)]
    paths.push("linked-secret.txt".to_string());

    for (index, file) in paths.into_iter().enumerate() {
        let response = call_server(
            &server,
            37 + index as i64,
            "tokensave_read",
            json!({
                "file": file,
                "mode": "full",
                "graph_root": graph_root
            }),
        )
        .await;

        assert_eq!(response["error"]["code"], -32602, "{response}");
        let serialized = serde_json::to_string(&response).unwrap();
        assert!(!serialized.contains(secret), "{serialized}");
        assert!(!serialized.contains("tokensave_graph:"), "{serialized}");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("selected graph root")),
            "{response}"
        );
    }

    let inside = call_server(
        &server,
        41,
        "tokensave_read",
        json!({
            "file": selected_root.join("src/main.rs").display().to_string(),
            "mode": "full",
            "graph_root": graph_root
        }),
    )
    .await;
    assert!(inside["error"].is_null(), "{inside}");
    assert!(
        response_text(&inside).contains("selected_inside"),
        "{inside}"
    );
    assert_eq!(read_cache_row_count(&db_path).await, cache_rows_before);
}

#[tokio::test]
async fn selected_context_qualifies_ids_without_rewriting_source_literals() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let selected_dir = setup_colliding_project().await;
    let server = McpServer::new(local, None).await;

    let search = call_server(
        &server,
        36,
        "tokensave_search",
        json!({
            "query": "shared_target",
            "graph_root": selected_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    let qualified = first_graph_id(&search["result"]).unwrap();
    let raw = raw_graph_id(&qualified);
    let missing = "function:ffffffffffffffffffffffffffffffff";
    fs::write(
        selected_dir.path().join("src/lib.rs"),
        format!(
            "pub fn shared_caller() {{ shared_target(); }}\n\
             pub fn shared_target() {{ /* real node id: {raw}; nonexistent lookalike: {missing} */ }}\n"
        ),
    )
    .unwrap();

    let context = call_server(
        &server,
        37,
        "tokensave_context",
        json!({
            "task": "Find shared_target and include its implementation",
            "include_code": true,
            "max_code_blocks": 5,
            "graph_root": selected_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    assert!(context["error"].is_null(), "{context}");
    let context_text = response_text(&context);
    assert!(context_text.contains(&qualified), "{context_text}");
    assert!(
        context_text.contains(&format!(
            "real node id: {raw}; nonexistent lookalike: {missing}"
        )),
        "{context_text}"
    );

    let full = call_server(
        &server,
        38,
        "tokensave_read",
        json!({
            "file": "src/lib.rs",
            "mode": "full",
            "graph_root": selected_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    assert!(full["error"].is_null(), "{full}");
    let full_payload = response_payload(&full);
    let source = full_payload["body"].as_str().unwrap();
    assert!(source.contains(raw), "{source}");
    assert!(source.contains(missing), "{source}");
    assert!(!source.contains(&qualified), "{source}");
}

#[tokio::test]
async fn cross_project_selected_queries_leave_both_projects_unchanged() {
    let (local_dir, local) = setup_named_project("local_only").await;
    let selected_dir = setup_colliding_project().await;
    let server = McpServer::new(local, None).await;
    assert!(
        server
            .wait_for_startup_catch_up(std::time::Duration::from_secs(30))
            .await
    );
    server.cg().checkpoint().await.unwrap();

    let local_before = project_snapshot(local_dir.path(), None, "src/main.rs").await;
    let selected_before = project_snapshot(selected_dir.path(), Some("main"), "src/lib.rs").await;

    let search = call_server(
        &server,
        37,
        "tokensave_search",
        json!({
            "query": "shared_target",
            "graph_root": selected_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    assert!(search["error"].is_null(), "{search}");
    let qualified = first_graph_id(&search["result"]).unwrap();

    for (id, name, arguments) in [
        (
            38,
            "tokensave_context",
            json!({
                "task": "Find shared_target and its caller",
                "graph_root": selected_dir.path().display().to_string(),
                "graph_branch": "main"
            }),
        ),
        (
            39,
            "tokensave_callers",
            json!({
                "node_id": qualified,
                "graph_root": selected_dir.path().display().to_string(),
                "graph_branch": "main"
            }),
        ),
        (
            40,
            "tokensave_read",
            json!({
                "file": "src/lib.rs",
                "mode": "full",
                "graph_root": selected_dir.path().display().to_string(),
                "graph_branch": "main"
            }),
        ),
    ] {
        let response = call_server(&server, id, name, arguments).await;
        assert!(response["error"].is_null(), "{response}");
    }

    let local_after = project_snapshot(local_dir.path(), None, "src/main.rs").await;
    let selected_after = project_snapshot(selected_dir.path(), Some("main"), "src/lib.rs").await;
    assert_eq!(local_after, local_before);
    assert_eq!(selected_after, selected_before);
}

#[tokio::test]
async fn selected_truncated_structured_output_returns_clear_error() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let foreign_dir = TempDir::new().unwrap();
    fs::create_dir_all(foreign_dir.path().join("src")).unwrap();
    let source = (0..500)
        .map(|index| format!("fn huge_item_{index:03}() -> usize {{ {index} }}\n"))
        .collect::<String>();
    fs::write(foreign_dir.path().join("src/lib.rs"), source).unwrap();
    let foreign = TokenSave::init(foreign_dir.path()).await.unwrap();
    foreign.index_all().await.unwrap();
    drop(foreign);
    let server = McpServer::new(local, None).await;

    let response = call_server(
        &server,
        36,
        "tokensave_search",
        json!({
            "query": "huge_item",
            "limit": 500,
            "graph_root": foreign_dir.path().display().to_string()
        }),
    )
    .await;

    assert_eq!(response["error"]["code"], -32603, "{response}");
    let message = response["error"]["message"].as_str().unwrap();
    assert!(message.contains("truncated"), "{message}");
    assert!(message.contains("lower limit"), "{message}");
    assert!(message.contains("narrow scope"), "{message}");
}

#[tokio::test]
async fn selectors_are_rejected_for_non_graph_scoped_tools_before_dispatch() {
    let (local_dir, local) = setup_named_project("before_edit").await;
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    drop(foreign);
    let server = McpServer::new(local, None).await;
    let graph_root = foreign_dir.path().display().to_string();

    let calls = [
        (
            "tokensave_status",
            json!({ "graph_root": graph_root.clone() }),
        ),
        (
            "tokensave_str_replace",
            json!({
                "path": "src/main.rs",
                "old_str": "before_edit",
                "new_str": "after_edit",
                "graph_root": graph_root.clone()
            }),
        ),
        (
            "tokensave_run_affected_tests",
            json!({
                "changed_paths": ["src/main.rs"],
                "graph_root": graph_root
            }),
        ),
    ];

    for (index, (name, arguments)) in calls.into_iter().enumerate() {
        let response = call_server(&server, 40 + index as i64, name, arguments).await;
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("does not support graph_root")),
            "{response}"
        );
    }

    let source = fs::read_to_string(local_dir.path().join("src/main.rs")).unwrap();
    assert!(source.contains("before_edit"), "{source}");
    assert!(!source.contains("after_edit"), "{source}");
    let stats = server.server_stats_json().await;
    assert_eq!(stats["tool_calls"], 3, "{stats}");
    assert_eq!(stats["tool_call_counts"]["tokensave_status"], 1, "{stats}");
    assert_eq!(
        stats["tool_call_counts"]["tokensave_str_replace"], 1,
        "{stats}"
    );
    assert_eq!(
        stats["tool_call_counts"]["tokensave_run_affected_tests"], 1,
        "{stats}"
    );
}

#[tokio::test]
async fn diagnostics_selectors_are_rejected_before_subprocess_or_target_creation() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let selected_dir = setup_selected_branch_project().await;
    fs::write(
        selected_dir.path().join("Cargo.toml"),
        "[package]\nname = \"selected-diagnostics\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let server = McpServer::new(local, None).await;
    let graph_root = selected_dir.path().display().to_string();
    let target = selected_dir.path().join(".tokensave/target");

    for (index, arguments) in [
        json!({ "graph_root": graph_root.clone() }),
        json!({
            "graph_root": graph_root,
            "graph_branch": "feature"
        }),
    ]
    .into_iter()
    .enumerate()
    {
        let response = call_server(
            &server,
            43 + index as i64,
            "tokensave_diagnostics",
            arguments,
        )
        .await;
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("does not support graph_root")),
            "{response}"
        );
        assert!(!target.exists(), "diagnostics created {}", target.display());
    }
}

#[tokio::test]
async fn selected_invalid_graph_selectors_return_invalid_params() {
    let (local_dir, local) = setup_named_project("local_only").await;
    let server = McpServer::new(local, None).await;
    let uninitialized = TempDir::new().unwrap();
    let selected_dir = setup_selected_branch_project().await;
    let missing_db_dir = setup_selected_branch_project().await;
    fs::remove_file(missing_db_dir.path().join(".tokensave/branches/feature.db")).unwrap();
    let cases = vec![
        (json!({ "graph_root": "relative" }), "absolute".to_string()),
        (
            json!({ "graph_root": uninitialized.path().display().to_string() }),
            "initialized".to_string(),
        ),
        (
            json!({ "graph_root": local_dir.path().display().to_string() }),
            "same project".to_string(),
        ),
        (
            json!({ "graph_root": local_dir.path().display().to_string() }),
            "omit graph_root to query it".to_string(),
        ),
        (
            json!({
                "graph_root": local_dir.path().display().to_string(),
                "graph_branch": "feature"
            }),
            "not supported".to_string(),
        ),
        (
            json!({
                "graph_root": selected_dir.path().display().to_string(),
                "graph_branch": "untracked"
            }),
            "not tracked".to_string(),
        ),
        (
            json!({
                "graph_root": selected_dir.path().display().to_string(),
                "graph_branch": "untracked"
            }),
            "tokensave branch add".to_string(),
        ),
        (
            json!({
                "graph_root": missing_db_dir.path().display().to_string(),
                "graph_branch": "feature"
            }),
            "DB is missing".to_string(),
        ),
        (
            json!({ "graph_branch": "feature" }),
            "requires a matching graph_root".to_string(),
        ),
        (
            json!({ "graph_branch": "feature" }),
            "omit graph_branch".to_string(),
        ),
    ];

    for (index, (selector, expected)) in cases.into_iter().enumerate() {
        let mut arguments = json!({ "query": "anything" });
        arguments
            .as_object_mut()
            .unwrap()
            .extend(selector.as_object().unwrap().clone());
        let response = call_server(&server, 50 + index as i64, "tokensave_search", arguments).await;
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains(&expected)),
            "expected {expected:?} in {response}"
        );
    }
}

#[tokio::test]
async fn selected_database_open_failure_returns_internal_error() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    foreign.checkpoint().await.unwrap();
    drop(foreign);
    fs::write(
        foreign_dir.path().join(".tokensave/tokensave.db"),
        b"not a sqlite database",
    )
    .unwrap();
    let server = McpServer::new(local, None).await;

    let response = call_server(
        &server,
        58,
        "tokensave_search",
        json!({
            "query": "foreign_only",
            "graph_root": foreign_dir.path().display().to_string()
        }),
    )
    .await;

    assert_eq!(response["error"]["code"], -32603, "{response}");
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("failed to open selected graph")),
        "{response}"
    );
}

#[tokio::test]
async fn selected_schema_mismatches_return_invalid_params() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let server = McpServer::new(local, None).await;

    for (index, version) in [1, latest_version() + 1].into_iter().enumerate() {
        let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
        foreign
            .db()
            .conn()
            .execute(&format!("PRAGMA user_version = {version}"), ())
            .await
            .unwrap();
        foreign.checkpoint().await.unwrap();
        drop(foreign);

        let response = call_server(
            &server,
            59 + index as i64,
            "tokensave_search",
            json!({
                "query": "foreign_only",
                "graph_root": foreign_dir.path().display().to_string()
            }),
        )
        .await;

        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("schema version")),
            "{response}"
        );
    }
}

#[tokio::test]
async fn selected_malformed_node_ids_return_invalid_params_before_dispatch() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    foreign.checkpoint().await.unwrap();
    drop(foreign);
    let server = McpServer::new(local, None).await;
    let graph_root = foreign_dir.path().display().to_string();

    for (index, (name, arguments)) in [
        (
            "tokensave_node",
            json!({
                "node_id": "function:short",
                "graph_root": graph_root.clone()
            }),
        ),
        (
            "tokensave_callers_for",
            json!({
                "node_ids": ["function:short"],
                "graph_root": graph_root
            }),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let response = call_server(&server, 61 + index as i64, name, arguments).await;
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("malformed node ID")),
            "{response}"
        );
    }
}

#[tokio::test]
async fn selected_empty_node_ids_return_invalid_params_before_dispatch() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    foreign.checkpoint().await.unwrap();
    drop(foreign);
    let server = McpServer::new(local, None).await;
    let graph_root = foreign_dir.path().display().to_string();

    for (index, (name, arguments)) in [
        (
            "tokensave_node",
            json!({
                "node_id": "",
                "graph_root": graph_root.clone()
            }),
        ),
        (
            "tokensave_callers_for",
            json!({
                "node_ids": [""],
                "graph_root": graph_root
            }),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let response = call_server(&server, 63 + index as i64, name, arguments).await;
        assert_eq!(response["error"]["code"], -32602, "{response}");
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("malformed node ID")),
            "{response}"
        );
    }
}

#[tokio::test]
async fn graph_scoped_tool_without_selector_preserves_handler_argument_errors() {
    let (_dir, server) = setup_server().await;

    let response = call_server(&server, 59, "tokensave_search", Value::Null).await;

    assert_eq!(response["error"]["code"], -32603, "{response}");
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("missing required parameter: query")),
        "{response}"
    );
}

#[tokio::test]
async fn qualified_colliding_id_traversal_isolated_by_root_and_branch() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let first_dir = setup_colliding_project().await;
    let second_dir = setup_colliding_project().await;
    let server = McpServer::new(local, None).await;

    let first_search = call_server(
        &server,
        60,
        "tokensave_search",
        json!({
            "query": "shared_target",
            "graph_root": first_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    let second_search = call_server(
        &server,
        61,
        "tokensave_search",
        json!({
            "query": "shared_target",
            "graph_root": second_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    let first_qualified = first_graph_id(&first_search["result"]).unwrap_or_else(|| {
        panic!("selected response contains no qualified node ID: {first_search}")
    });
    let second_qualified = first_graph_id(&second_search["result"]).unwrap_or_else(|| {
        panic!("selected response contains no qualified node ID: {second_search}")
    });
    let raw = raw_graph_id(&first_qualified);

    assert_eq!(raw, raw_graph_id(&second_qualified));
    assert_ne!(first_qualified, second_qualified);
    let structured_ids = response_structured_ids(&first_search);
    assert!(!structured_ids.is_empty(), "{first_search}");
    assert!(
        structured_ids.iter().all(|id| id.starts_with("graph:")),
        "{structured_ids:?}"
    );
    assert!(structured_ids.contains(&first_qualified));

    let follow_up = call_server(
        &server,
        62,
        "tokensave_callers",
        json!({
            "node_id": first_qualified.clone(),
            "graph_root": first_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    assert!(follow_up["error"].is_null(), "{follow_up}");
    assert!(response_text(&follow_up).contains("shared_caller"));
    assert_eq!(follow_up["result"]["_meta"]["tokensave"]["selected"], true);

    let raw_response = call_server(
        &server,
        63,
        "tokensave_node",
        json!({
            "node_id": raw,
            "graph_root": first_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    assert_eq!(raw_response["error"]["code"], -32602, "{raw_response}");
    assert!(
        raw_response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("graph-qualified")),
        "{raw_response}"
    );

    let wrong_graph = call_server(
        &server,
        64,
        "tokensave_node",
        json!({
            "node_id": first_qualified.clone(),
            "graph_root": second_dir.path().display().to_string(),
            "graph_branch": "main"
        }),
    )
    .await;
    assert_eq!(wrong_graph["error"]["code"], -32602, "{wrong_graph}");
    assert!(
        wrong_graph["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("does not match graph_root or graph_branch")),
        "{wrong_graph}"
    );

    let wrong_branch = call_server(
        &server,
        65,
        "tokensave_node",
        json!({
            "node_id": first_qualified.clone(),
            "graph_root": first_dir.path().display().to_string(),
            "graph_branch": "feature"
        }),
    )
    .await;
    assert_eq!(wrong_branch["error"]["code"], -32602, "{wrong_branch}");
    assert!(
        wrong_branch["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("does not match graph_root or graph_branch")),
        "{wrong_branch}"
    );

    let no_selector = call_server(
        &server,
        66,
        "tokensave_node",
        json!({ "node_id": first_qualified }),
    )
    .await;
    assert_eq!(no_selector["error"]["code"], -32602, "{no_selector}");
    assert!(
        no_selector["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("repeat matching graph_root and graph_branch")),
        "{no_selector}"
    );
}

#[tokio::test]
async fn selected_graph_ignores_local_scope_and_local_warnings() {
    let (local_dir, local) = setup_named_project("local_only").await;
    fs::write(
        local_dir.path().join("src/main.rs"),
        "fn locally_stale() {}\n",
    )
    .unwrap();
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    drop(foreign);
    let server = McpServer::new(local, Some("path/that/does/not/exist".to_string())).await;

    let selected = call_server(
        &server,
        65,
        "tokensave_search",
        json!({
            "query": "foreign_only",
            "graph_root": foreign_dir.path().display().to_string()
        }),
    )
    .await;
    assert!(selected["error"].is_null(), "{selected}");
    let text = response_text(&selected);
    assert!(text.contains("foreign_only"), "{text}");
    assert!(!text.contains("were edited after the last sync"), "{text}");
    assert!(!text.contains("worktree"), "{text}");
    assert!(!text.contains("tokensave v"), "{text}");
}

#[tokio::test]
async fn selected_warnings_use_canonical_selected_root_remedies() {
    let (_local_dir, local) = setup_named_project("local_only").await;
    let (foreign_dir, foreign) = setup_fallback_project("foreign_only").await;
    foreign
        .db()
        .set_metadata("last_sync_at", "1")
        .await
        .unwrap();
    foreign.checkpoint().await.unwrap();
    drop(foreign);
    let server = McpServer::new(local, None).await;
    let canonical_root = foreign_dir.path().canonicalize().unwrap();

    let response = call_server(
        &server,
        66,
        "tokensave_search",
        json!({
            "query": "foreign_only",
            "graph_root": foreign_dir.path().display().to_string()
        }),
    )
    .await;

    assert!(response["error"].is_null(), "{response}");
    let text = response_text(&response);
    let quoted_root = format!("\"{}\"", canonical_root.display());
    assert!(text.contains(&quoted_root), "{text}");
    assert!(
        text.contains("Run Tokensave synchronization from selected project root"),
        "{text}"
    );
    assert!(
        text.contains("Add or refresh selected branch \"feature\""),
        "{text}"
    );
    assert!(!text.contains('`'), "{text}");
    assert!(!text.contains("tokensave sync --path"), "{text}");
    assert!(!text.contains("tokensave branch add"), "{text}");
}

#[tokio::test]
async fn selected_calls_skip_all_accounting_and_preserve_local_schema_charge() {
    let Some(home) = std::env::var_os("TOKENSAVE_SELECTED_ACCOUNTING_HOME") else {
        let home = TempDir::new().unwrap();
        let mut command = ProcessCommand::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "selected_calls_skip_all_accounting_and_preserve_local_schema_charge",
                "--nocapture",
            ])
            .env("TOKENSAVE_SELECTED_ACCOUNTING_HOME", home.path())
            .env("HOME", home.path())
            .env("RUST_TEST_THREADS", "1");
        #[cfg(target_os = "windows")]
        command.env("USERPROFILE", home.path());
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "isolated accounting helper failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    };
    let home = std::path::PathBuf::from(home);
    let (local_dir, local) = setup_named_project("local_only").await;
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    drop(foreign);
    let server = McpServer::new(local, None).await;
    assert!(
        server
            .wait_for_startup_catch_up(std::time::Duration::from_secs(30))
            .await
    );
    let listed = run_server_with_messages(
        Arc::clone(&server),
        vec![jsonrpc_request(json!(70), "tools/list", json!({}))],
    )
    .await;
    assert!(parse_response(&listed[0])["error"].is_null());

    let global = tokensave::global_db::GlobalDb::open().await.unwrap();
    let local_path = local_dir.path().to_string_lossy();
    let foreign_path = foreign_dir.path().to_string_lossy();
    let local_tokens_before = server.cg().get_tokens_saved().await.unwrap();
    let total_ledger_before = global.sum_savings(None, 0).await.calls;
    let local_ledger_before = global.sum_savings(Some(&local_path), 0).await.calls;
    let foreign_ledger_before = global.sum_savings(Some(&foreign_path), 0).await.calls;
    let monitor_before = monitor_snapshot(&home);
    let accounting_started_before = server.accounting_tasks_started();
    let accounting_pending_before = server.accounting_tasks_pending();
    assert_eq!(accounting_started_before, 0);
    assert_eq!(accounting_pending_before, 0);

    let selected = call_server(
        &server,
        71,
        "tokensave_search",
        json!({
            "query": "foreign_only",
            "graph_root": foreign_dir.path().display().to_string()
        }),
    )
    .await;
    assert!(selected["error"].is_null(), "{selected}");
    assert!(
        !response_text(&selected).contains("tokensave_metrics:"),
        "{selected}"
    );
    assert_eq!(
        server.accounting_tasks_started(),
        accounting_started_before,
        "selected call must not spawn global ledger persistence"
    );
    assert_eq!(
        server.accounting_tasks_pending(),
        accounting_pending_before,
        "selected call must not leave accounting work pending"
    );
    assert_eq!(
        server.cg().get_tokens_saved().await.unwrap(),
        local_tokens_before
    );
    assert_eq!(global.sum_savings(None, 0).await.calls, total_ledger_before);
    assert_eq!(
        global.sum_savings(Some(&local_path), 0).await.calls,
        local_ledger_before
    );
    assert_eq!(
        global.sum_savings(Some(&foreign_path), 0).await.calls,
        foreign_ledger_before
    );
    assert_eq!(monitor_snapshot(&home), monitor_before);

    let first_local = call_server(
        &server,
        72,
        "tokensave_search",
        json!({ "query": "local_only" }),
    )
    .await;
    let second_local = call_server(
        &server,
        73,
        "tokensave_search",
        json!({ "query": "local_only" }),
    )
    .await;
    assert!(first_local["error"].is_null(), "{first_local}");
    assert!(second_local["error"].is_null(), "{second_local}");
    let first_local_text = serde_json::to_string(&first_local).unwrap();
    let second_local_text = serde_json::to_string(&second_local).unwrap();
    assert!(
        extract_metrics_field(&first_local_text, "after")
            > extract_metrics_field(&second_local_text, "after"),
        "selected call must not consume the local schema charge"
    );
    assert!(
        server
            .wait_for_accounting_idle(std::time::Duration::from_secs(5))
            .await,
        "local ledger persistence did not drain"
    );
    assert!(
        global.sum_savings(None, 0).await.calls > total_ledger_before,
        "later local calls must increase the global ledger total"
    );
    assert!(
        global.sum_savings(Some(&local_path), 0).await.calls > local_ledger_before,
        "later local calls must be recorded in the global ledger"
    );
    assert!(
        monitor_snapshot(&home) != monitor_before,
        "later local calls must be recorded in the monitor"
    );
}

#[tokio::test]
async fn first_selected_call_does_not_consume_local_version_reindex_gate() {
    let (local_dir, local) = setup_named_project("local_only").await;
    let mut config = tokensave::config::load_config(local_dir.path()).unwrap();
    config.last_indexed_version = String::new();
    tokensave::config::save_config(local_dir.path(), &config).unwrap();
    let (foreign_dir, foreign) = setup_named_project("foreign_only").await;
    drop(foreign);
    let server = McpServer::new(local, None).await;

    let selected = call_server(
        &server,
        74,
        "tokensave_search",
        json!({
            "query": "foreign_only",
            "graph_root": foreign_dir.path().display().to_string()
        }),
    )
    .await;
    assert!(selected["error"].is_null(), "{selected}");
    assert!(
        !server.version_reindex_started(),
        "selected call must not start evaluating the local reindex gate"
    );
    assert!(
        !server.version_reindex_done(),
        "selected call must not evaluate the local reindex gate"
    );

    let local = call_server(
        &server,
        75,
        "tokensave_search",
        json!({ "query": "local_only" }),
    )
    .await;
    assert!(local["error"].is_null(), "{local}");
    assert!(
        server
            .wait_for_version_reindex(std::time::Duration::from_secs(30))
            .await,
        "later local call must trigger the version reindex"
    );
    assert_eq!(
        tokensave::config::load_config(local_dir.path())
            .unwrap()
            .last_indexed_version,
        env!("CARGO_PKG_VERSION")
    );
}

// ---------------------------------------------------------------------------
// 6b. test_tools_call_timings_flag
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_timings_flag_off_by_default() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(31),
            "tools/call",
            json!({"name": "tokensave_search", "arguments": {"query": "helper"}}),
        )],
    )
    .await;
    let resp = parse_response(
        responses
            .iter()
            .find(|r| parse_response(r)["id"] == 31)
            .expect("response with id 31"),
    );
    assert!(
        resp["result"]["_meta"]["duration_us"].is_null(),
        "duration_us must NOT be present when timings flag is off — got {}",
        resp["result"]["_meta"]
    );
}

#[tokio::test]
async fn test_tools_call_timings_flag_on_emits_duration_us() {
    let (_dir, server) = setup_server().await;
    server.set_timings_enabled(true);
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(32),
            "tools/call",
            json!({"name": "tokensave_search", "arguments": {"query": "helper"}}),
        )],
    )
    .await;
    let resp = parse_response(
        responses
            .iter()
            .find(|r| parse_response(r)["id"] == 32)
            .expect("response with id 32"),
    );
    let dur = resp["result"]["_meta"]["duration_us"]
        .as_u64()
        .expect("duration_us must be a u64 when timings are enabled");
    // Lower-bound sanity: any real query takes at least a few microseconds.
    // Upper bound is generous so the test isn't flaky on slow CI runners.
    assert!(
        dur < 5_000_000,
        "duration_us should be well under 5 s, got {dur}"
    );
}

// ---------------------------------------------------------------------------
// 7. test_tools_call_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_status() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(40),
            "tools/call",
            json!({
                "name": "tokensave_status",
                "arguments": {}
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 40
        })
        .expect("should have a response for id=40");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "status should not error");
    let content = resp["result"]["content"].as_array().unwrap();
    let text = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        text.contains("node_count") || text.contains("file_count"),
        "status response should contain node_count or file_count, got: {}",
        text
    );
}

// ---------------------------------------------------------------------------
// 7b. test_search_metrics_are_capped_and_net
// ---------------------------------------------------------------------------

/// `tokensave_search` is a `Reference`-policy tool: it returns file/line
/// references, not file content, so an agent would never have read every
/// touched file in full. Before the token-savings fix, `before` charged the
/// full weight of every touched file regardless — here that would be a
/// multi-hundred-line padded file. This asserts the corrected behavior: the
/// baseline is capped near what the response actually delivered, and the
/// reported `saved` is the net `before - after`, not the gross `before`.
#[tokio::test]
async fn test_search_metrics_are_capped_and_net() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let padded_source = format!(
        "fn main() {{ let x = helper(); }}\n{}\nfn helper() -> i32 {{ 42 }}\n",
        "// padding line to inflate file size\n".repeat(500)
    );
    fs::write(project.join("src/main.rs"), &padded_source).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;

    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(70),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "helper" }
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 70)
        .expect("should have a response for id=70");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "search should not error");
    let content = resp["result"]["content"].as_array().unwrap();
    let text = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("");

    let metrics_line = text
        .lines()
        .find(|l| l.starts_with("tokensave_metrics:"))
        .unwrap_or_else(|| panic!("no tokensave_metrics line in response: {text}"));
    assert!(
        metrics_line.contains(" saved="),
        "metrics line should report net savings via saved=, got: {metrics_line}"
    );

    let field = |key: &str| -> u64 {
        metrics_line
            .split_whitespace()
            .find_map(|tok| tok.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("no {key}= field in: {metrics_line}"))
            .parse()
            .unwrap_or_else(|e| panic!("{key}= field not a u64 in {metrics_line}: {e}"))
    };
    let before = field("before");
    let after = field("after");
    let saved = field("saved");

    assert_eq!(
        saved,
        before.saturating_sub(after),
        "saved must be the net before - after, not the gross before"
    );
    // The padded file alone is ~500 lines (~3000+ tokens at 4 chars/token);
    // a search response capped to a small multiple of what it returned must
    // stay far below that raw file weight.
    assert!(
        before < 2_000,
        "reference-tool baseline should be capped near the response size \
         instead of charging the full padded file, got before={before}"
    );
}

// ---------------------------------------------------------------------------
// 7c. test_schema_overhead_survives_a_failed_first_call
// ---------------------------------------------------------------------------

/// Schema overhead (the approximate cost of the `tools/list` payload) is
/// meant to be charged exactly once per session, into `after` on the first
/// *successful* `tools/call` once `tools/list` has actually been served.
/// Before the "gate on `prev_tool_calls == 0`" bug was fixed, a failing
/// first call silently burned the "first call" slot and no call that
/// session ever got charged the schema overhead. Before the "charge
/// regardless of whether `tools/list` ran" bug was fixed, the charge would
/// have landed even without the `tools/list` call this test sends first.
/// This drives `tools/list`, then an unknown-tool call (which errors),
/// then two identical successful calls, and asserts the first success's
/// `after` is strictly greater than the second's — the only possible
/// source of that gap is the one-time schema charge landing on the first
/// success rather than being lost to the earlier error or never accrued at
/// all.
#[tokio::test]
async fn test_schema_overhead_survives_a_failed_first_call() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let padded_source = format!(
        "fn main() {{ let x = helper(); }}\n{}\nfn helper() -> i32 {{ 42 }}\n",
        "// padding line to inflate file size\n".repeat(500)
    );
    fs::write(project.join("src/main.rs"), &padded_source).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;

    let search_call = || {
        jsonrpc_request(
            json!(72),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "helper" }
            }),
        )
    };

    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_request(json!(70), "tools/list", json!({})),
            jsonrpc_request(
                json!(71),
                "tools/call",
                json!({
                    "name": "tokensave_bogus_unknown_tool_xyz",
                    "arguments": {}
                }),
            ),
            search_call(),
            search_call(),
        ],
    )
    .await;

    let err_resp = parse_response(
        responses
            .iter()
            .find(|r| parse_response(r)["id"] == 71)
            .expect("should have a response for id=71"),
    );
    assert!(
        !err_resp["error"].is_null(),
        "unknown tool name must produce a dispatch error"
    );

    // Both successful calls share id=72 (duplicated on purpose to drive two
    // identical dispatches); pull each occurrence out in order.
    let success_ids: Vec<&String> = responses
        .iter()
        .filter(|r| parse_response(r)["id"] == 72)
        .collect();
    assert_eq!(
        success_ids.len(),
        2,
        "expected two responses for the two identical search calls"
    );
    let extract_after = |resp_str: &str| -> u64 {
        let resp = parse_response(resp_str);
        assert!(resp["error"].is_null(), "search should not error");
        let content = resp["result"]["content"].as_array().unwrap();
        let text = content
            .iter()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        let metrics_line = text
            .lines()
            .find(|l| l.starts_with("tokensave_metrics:"))
            .unwrap_or_else(|| panic!("no tokensave_metrics line in response: {text}"));
        metrics_line
            .split_whitespace()
            .find_map(|tok| tok.strip_prefix("after="))
            .unwrap_or_else(|| panic!("no after= field in: {metrics_line}"))
            .parse()
            .unwrap_or_else(|e| panic!("after= field not a u64 in {metrics_line}: {e}"))
    };

    let after_first_success = extract_after(success_ids[0]);
    let after_second_success = extract_after(success_ids[1]);

    assert!(
        after_first_success > after_second_success,
        "the first successful call (preceded by a failed dispatch) should still \
         carry the one-time schema overhead: after_first={after_first_success} \
         after_second={after_second_success}"
    );
}

// ---------------------------------------------------------------------------
// 7d. baseline policy tracks what a tool actually delivered, not its name
// ---------------------------------------------------------------------------

fn extract_metrics_field(resp_str: &str, key: &str) -> u64 {
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "call should not error: {resp}");
    let content = resp["result"]["content"].as_array().unwrap();
    let text = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    let metrics_line = text
        .lines()
        .find(|l| l.starts_with("tokensave_metrics:"))
        .unwrap_or_else(|| panic!("no tokensave_metrics line in response: {text}"));
    metrics_line
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("no {key}= field in: {metrics_line}"))
        .parse()
        .unwrap_or_else(|e| panic!("{key}= field not a u64 in {metrics_line}: {e}"))
}

/// A genuine, uncached `mode=full` read of a large file must not have its
/// baseline shrunk by the reference cap — the JSON-wrapped response is
/// always at least as large as the source it carries, so `before` should
/// land at (or above) the file's raw weight, not some small multiple of a
/// tiny response. Guards the invariant `accounting::baseline_policy` relies
/// on to classify every tool as `Reference` and still charge full weight
/// for a real full-file delivery.
#[tokio::test]
async fn test_uncached_full_read_baseline_matches_file_weight() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let padded_source = format!(
        "fn main() {{ let x = helper(); }}\n{}\nfn helper() -> i32 {{ 42 }}\n",
        "// padding line to inflate file size\n".repeat(500)
    );
    fs::write(project.join("src/main.rs"), &padded_source).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;

    let file_tokens = (padded_source.len() / 4) as u64;

    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(80),
            "tools/call",
            json!({
                "name": "tokensave_read",
                "arguments": { "file": "src/main.rs", "mode": "full" }
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 80)
        .expect("should have a response for id=80");
    let before = extract_metrics_field(resp_str, "before");

    assert!(
        before >= file_tokens,
        "an uncached full-file read must not be capped below the file's raw \
         weight: before={before} file_tokens={file_tokens}"
    );
}

/// A cache-hit re-read of the same file returns a small `{"unchanged": ...}`
/// stub, not the file content — its baseline must be capped near that stub,
/// not the full file. Before the fix, `tokensave_read` was unconditionally
/// classified `FullFile`, so a cached read of a large file claimed the
/// entire file as "saved" for a response that carried none of it.
#[tokio::test]
async fn test_cached_read_baseline_is_capped_not_full_file() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let padded_source = format!(
        "fn main() {{ let x = helper(); }}\n{}\nfn helper() -> i32 {{ 42 }}\n",
        "// padding line to inflate file size\n".repeat(500)
    );
    fs::write(project.join("src/main.rs"), &padded_source).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;

    let file_tokens = (padded_source.len() / 4) as u64;
    let read_call = |id: i64| {
        jsonrpc_request(
            json!(id),
            "tools/call",
            json!({
                "name": "tokensave_read",
                "arguments": { "file": "src/main.rs", "mode": "full" }
            }),
        )
    };

    let responses = run_server_with_messages(server, vec![read_call(81), read_call(82)]).await;

    let second_resp = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 82)
        .expect("should have a response for id=82");
    let text = {
        let resp = parse_response(second_resp);
        resp["result"]["content"].as_array().unwrap()[0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert!(
        text.contains("\"unchanged\""),
        "second identical read should be served from the read cache as an \
         unchanged stub, got: {text}"
    );

    let before = extract_metrics_field(second_resp, "before");
    assert!(
        before < file_tokens / 4,
        "a cache-hit stub must not claim the full file's weight as its \
         baseline: before={before} file_tokens={file_tokens}"
    );
}

// ---------------------------------------------------------------------------
// 7e. schema overhead only accrues once tools/list is actually served
// ---------------------------------------------------------------------------

/// A client that never calls `tools/list` and instead invokes a known tool
/// name directly should never be charged for the schema payload it never
/// received. Compares a session with no `tools/list` call against the
/// existing schema-overhead behavior via `schema_overhead_tokens` directly.
#[tokio::test]
async fn test_schema_overhead_not_charged_without_tools_list() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let padded_source = format!(
        "fn main() {{ let x = helper(); }}\n{}\nfn helper() -> i32 {{ 42 }}\n",
        "// padding line to inflate file size\n".repeat(500)
    );
    fs::write(project.join("src/main.rs"), &padded_source).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;

    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(90),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "helper" }
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 90)
        .expect("should have a response for id=90");
    let after = extract_metrics_field(resp_str, "after");

    // Without a preceding tools/list, `after` should be small — just the
    // response, request overhead, and metrics line, with no ~85-tool schema
    // payload folded in. A generous upper bound keeps this robust to minor
    // response wording changes while still catching a schema charge, which
    // would add several thousand tokens.
    assert!(
        after < 500,
        "after should not include schema overhead when tools/list was never \
         called: after={after}"
    );
}

/// Once `tools/list` has actually run, the next accounted call should carry
/// the one-time schema overhead — mirroring (but now correctly gated on) the
/// prior "first call" behavior.
#[tokio::test]
async fn test_schema_overhead_charged_after_tools_list_call() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let padded_source = format!(
        "fn main() {{ let x = helper(); }}\n{}\nfn helper() -> i32 {{ 42 }}\n",
        "// padding line to inflate file size\n".repeat(500)
    );
    fs::write(project.join("src/main.rs"), &padded_source).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;

    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_request(json!(91), "tools/list", json!({})),
            jsonrpc_request(
                json!(92),
                "tools/call",
                json!({
                    "name": "tokensave_search",
                    "arguments": { "query": "helper" }
                }),
            ),
        ],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 92)
        .expect("should have a response for id=92");
    let after = extract_metrics_field(resp_str, "after");

    // With tools/list served first, the schema payload (~85 tool
    // definitions) should be folded into `after`, pushing it well past the
    // no-tools/list baseline asserted in the sibling test above.
    assert!(
        after > 500,
        "after should include schema overhead once tools/list has been \
         served: after={after}"
    );
}

/// `tokensave_status` never touches a file (`touched_files` is always
/// empty), so its `before` is always `0` — the case where the metrics line
/// is never appended to the response at all. Guards the invariant that a
/// `before == 0` response never carries a `tokensave_metrics` line, since
/// `after`'s token count (and what gets persisted to the ledger) is only
/// honest when it matches exactly what the response contains.
#[tokio::test]
async fn test_zero_before_call_never_gets_a_metrics_line() {
    let (_dir, server) = setup_server().await;

    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(93),
            "tools/call",
            json!({
                "name": "tokensave_status",
                "arguments": {}
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 93)
        .expect("should have a response for id=93");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "status should not error");
    let content = resp["result"]["content"].as_array().unwrap();
    let text = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        !text.contains("tokensave_metrics:"),
        "a before=0 call must not carry a metrics line: {text}"
    );
}

/// A single call whose `after` dwarfs its `before` — the one that absorbs
/// the one-time schema charge — must not silently discard the excess.
/// Before the fix, every call added its own saturating `before - after`
/// straight to the persisted counter, so the schema charge was mostly lost
/// on the one call that happened to carry it, and every later call's
/// savings were credited in full regardless. This drives `tools/list`, a
/// `tokensave_status` call (whose `before` is always `0`, so it absorbs the
/// whole schema charge as debt), then a few real `tokensave_search` calls
/// whose own `before` is comfortably smaller than that debt, and asserts
/// the persisted `approx_tokens_saved` stays flat while those calls'
/// displayed `saved=` figures are individually positive — proof the surplus
/// is paying down debt rather than being credited on top of it.
#[tokio::test]
async fn test_schema_debt_is_paid_down_not_discarded() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let padded_source = format!(
        "fn main() {{ let x = helper(); }}\n{}\nfn helper() -> i32 {{ 42 }}\n",
        "// padding line to inflate file size\n".repeat(500)
    );
    fs::write(project.join("src/main.rs"), &padded_source).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let server = McpServer::new(cg, None).await;

    let status_call = |id: i64| {
        jsonrpc_request(
            json!(id),
            "tools/call",
            json!({ "name": "tokensave_status", "arguments": {} }),
        )
    };
    let search_call = |id: i64| {
        jsonrpc_request(
            json!(id),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "helper" }
            }),
        )
    };

    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_request(json!(100), "tools/list", json!({})),
            status_call(101), // absorbs the schema charge as debt (before=0)
            search_call(102),
            search_call(103),
            search_call(104),
            status_call(199), // read the persisted counter afterward
        ],
    )
    .await;

    let approx_tokens_saved = |id: i64| -> u64 {
        let resp_str = responses
            .iter()
            .find(|r| parse_response(r)["id"] == id)
            .unwrap_or_else(|| panic!("should have a response for id={id}"));
        let resp = parse_response(resp_str);
        assert!(resp["error"].is_null(), "status should not error");
        let content = resp["result"]["content"].as_array().unwrap();
        let text = content
            .iter()
            .filter_map(|c| c["text"].as_str())
            .find(|t| t.contains("approx_tokens_saved"))
            .unwrap_or_else(|| panic!("no status JSON in response: {resp}"));
        let status: Value = serde_json::from_str(text).unwrap();
        status["server"]["approx_tokens_saved"].as_u64().unwrap()
    };

    let baseline = approx_tokens_saved(101);
    let after_searches = approx_tokens_saved(199);

    let mut sum_displayed_saved = 0u64;
    for id in [102, 103, 104] {
        let resp_str = responses
            .iter()
            .find(|r| parse_response(r)["id"] == id)
            .unwrap_or_else(|| panic!("should have a response for id={id}"));
        sum_displayed_saved += extract_metrics_field(resp_str, "saved");
    }

    assert!(
        sum_displayed_saved > 0,
        "the search calls should each report positive per-call savings"
    );
    assert_eq!(
        after_searches, baseline,
        "the persisted counter must not move while debt from the schema \
         charge is still being paid down, even though the calls in between \
         individually displayed saved={sum_displayed_saved} tokens: \
         baseline={baseline} after_searches={after_searches}"
    );
}

// ---------------------------------------------------------------------------
// 8. test_tools_call_missing_params
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_missing_params() {
    let (_dir, server) = setup_server().await;
    // Send tools/call with no params at all.
    let responses = run_server_with_messages(
        server,
        vec![serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 50,
            "method": "tools/call"
        }))
        .unwrap()],
    )
    .await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 50);
    assert!(resp["error"].is_object(), "should have an error");
    assert_eq!(
        resp["error"]["code"], -32602,
        "should be InvalidParams error"
    );
    assert!(
        resp["error"]["message"]
            .as_str()
            .unwrap()
            .contains("missing params"),
        "error message should mention missing params"
    );
}

// ---------------------------------------------------------------------------
// 9. test_tools_call_missing_name
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_missing_name() {
    let (_dir, server) = setup_server().await;
    // Send tools/call with params but no "name" key.
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(60),
            "tools/call",
            json!({
                "arguments": { "query": "test" }
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 60
        })
        .expect("should have a response for id=60");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_object(), "should have an error");
    assert_eq!(
        resp["error"]["code"], -32602,
        "should be InvalidParams error"
    );
    assert!(
        resp["error"]["message"]
            .as_str()
            .unwrap()
            .contains("missing 'name'"),
        "error message should mention missing name"
    );
}

// ---------------------------------------------------------------------------
// 10. test_unknown_method
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unknown_method() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(70), "some/unknown/method", json!({}))],
    )
    .await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 70);
    assert!(resp["error"].is_object(), "should have an error");
    assert_eq!(
        resp["error"]["code"], -32601,
        "should be MethodNotFound error"
    );
}

// ---------------------------------------------------------------------------
// 11. test_malformed_json
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_malformed_json() {
    let (_dir, server) = setup_server().await;
    // Send invalid JSON, then a valid ping to verify server continues.
    let responses = run_server_with_messages(
        server,
        vec![
            "this is not json {{{".to_string(),
            jsonrpc_request(json!(80), "ping", json!({})),
        ],
    )
    .await;

    // Should have at least 2 responses: parse error + ping response.
    assert!(
        responses.len() >= 2,
        "should have at least 2 responses (parse error + ping), got {}",
        responses.len()
    );

    // First response should be a parse error.
    let error_resp = parse_response(&responses[0]);
    assert!(
        error_resp["error"].is_object(),
        "first response should be an error"
    );
    assert_eq!(
        error_resp["error"]["code"], -32700,
        "should be ParseError (-32700)"
    );

    // Second (or later) should be the ping response.
    let ping_resp = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 80
        })
        .expect("should have a ping response after malformed JSON");
    let ping = parse_response(ping_resp);
    assert!(
        ping["error"].is_null(),
        "ping after malformed JSON should succeed"
    );
}

// ---------------------------------------------------------------------------
// 12. test_blank_lines_skipped
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_blank_lines_skipped() {
    let (_dir, server) = setup_server().await;
    // Send blank/whitespace lines, then a ping.
    let responses = run_server_with_messages(
        server,
        vec![
            "".to_string(),
            "   ".to_string(),
            "\t".to_string(),
            jsonrpc_request(json!(90), "ping", json!({})),
        ],
    )
    .await;

    // Only the ping response should come through.
    let ping_responses: Vec<&String> = responses
        .iter()
        .filter(|r| {
            let v: Value = serde_json::from_str(r).unwrap_or(json!(null));
            v["id"] == 90
        })
        .collect();
    assert_eq!(
        ping_responses.len(),
        1,
        "should get exactly 1 response (ping only), got {}",
        responses.len()
    );
}

// ---------------------------------------------------------------------------
// 13. test_multiple_tool_calls
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multiple_tool_calls() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_request(json!(100), "initialize", json!({})),
            jsonrpc_request(json!(101), "ping", json!({})),
            jsonrpc_request(json!(102), "tools/list", json!({})),
            jsonrpc_request(
                json!(103),
                "tools/call",
                json!({
                    "name": "tokensave_search",
                    "arguments": { "query": "main" }
                }),
            ),
        ],
    )
    .await;

    // Collect response IDs (filtering out notifications which have no "id" or null id).
    let response_ids: Vec<i64> = responses
        .iter()
        .filter_map(|r| {
            let v = parse_response(r);
            v["id"].as_i64()
        })
        .collect();

    assert!(
        response_ids.contains(&100),
        "should have response for id=100 (initialize)"
    );
    assert!(
        response_ids.contains(&101),
        "should have response for id=101 (ping)"
    );
    assert!(
        response_ids.contains(&102),
        "should have response for id=102 (tools/list)"
    );
    assert!(
        response_ids.contains(&103),
        "should have response for id=103 (tools/call)"
    );
}

// ---------------------------------------------------------------------------
// 14. test_server_stats_initial
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_server_stats_initial() {
    let (_dir, server) = setup_server().await;
    let stats = server.server_stats_json().await;
    assert!(stats["uptime_secs"].is_number(), "should have uptime_secs");
    assert_eq!(
        stats["total_requests"], 0,
        "initial total_requests should be 0"
    );
    assert_eq!(stats["tool_calls"], 0, "initial tool_calls should be 0");
    assert_eq!(stats["errors"], 0, "initial errors should be 0");
}

// ---------------------------------------------------------------------------
// 15. test_server_stats_after_run (indirect via tokensave_status response)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_server_stats_after_run() {
    let (_dir, server) = setup_server().await;
    // Send several requests then a tokensave_status to check stats are embedded.
    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_request(json!(200), "initialize", json!({})),
            jsonrpc_request(json!(201), "ping", json!({})),
            jsonrpc_request(
                json!(202),
                "tools/call",
                json!({
                    "name": "tokensave_status",
                    "arguments": {}
                }),
            ),
        ],
    )
    .await;

    let status_resp_str = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 202
        })
        .expect("should have a response for id=202");
    let resp = parse_response(status_resp_str);
    assert!(resp["error"].is_null(), "status should not error");
    let content = resp["result"]["content"].as_array().unwrap();
    let text = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    // The server stats should be embedded in the status response and reflect
    // that requests have been processed.
    assert!(
        text.contains("server") || text.contains("total_requests") || text.contains("tool_calls"),
        "status response should contain server stats, got: {}",
        text
    );
}

// ---------------------------------------------------------------------------
// 16. test_error_tracking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_error_tracking() {
    let (_dir, server) = setup_server().await;
    // Send an unknown method (which produces an error), then check status.
    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_request(json!(300), "unknown/method", json!({})),
            jsonrpc_request(
                json!(301),
                "tools/call",
                json!({
                    "name": "tokensave_status",
                    "arguments": {}
                }),
            ),
        ],
    )
    .await;

    // Verify the unknown method produced an error.
    let error_resp_str = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 300
        })
        .expect("should have a response for id=300");
    let error_resp = parse_response(error_resp_str);
    assert!(
        error_resp["error"].is_object(),
        "unknown method should produce error"
    );

    // Check status to verify errors count increased.
    let status_resp_str = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 301
        })
        .expect("should have a response for id=301");
    let status_resp = parse_response(status_resp_str);
    assert!(status_resp["error"].is_null(), "status should not error");
    let content = status_resp["result"]["content"].as_array().unwrap();
    let text = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    // Parse the server stats from the status text to verify errors > 0.
    assert!(
        text.contains("\"errors\"") || text.contains("errors"),
        "status should contain errors field, got: {}",
        text
    );
    // The error count should be at least 1 (from the unknown method).
    // The server stats JSON is embedded in the text; try to find it.
    if let Some(server_start) = text.find("\"server\"") {
        let server_section = &text[server_start..];
        assert!(
            server_section.contains("\"errors\": 1") || server_section.contains("\"errors\":1"),
            "errors should be at least 1 after sending unknown method, section: {}",
            server_section
        );
    }
}

// ---------------------------------------------------------------------------
// 17. test_initialize_has_resources_capability
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_initialize_has_resources_capability() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(1), "initialize", json!({}))],
    )
    .await;

    let resp = parse_response(&responses[0]);
    assert!(
        resp["result"]["capabilities"]["resources"].is_object(),
        "initialize should advertise resources capability"
    );
}

// ---------------------------------------------------------------------------
// 18. test_initialize_has_instructions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_initialize_has_instructions() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(1), "initialize", json!({}))],
    )
    .await;

    let resp = parse_response(&responses[0]);
    let instructions = resp["result"]["instructions"]
        .as_str()
        .expect("initialize should have instructions string");
    assert!(
        instructions.contains("tokensave_context"),
        "instructions should mention tokensave_context"
    );
}

// ---------------------------------------------------------------------------
// 19. test_resources_list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_list() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(400), "resources/list", json!({}))],
    )
    .await;

    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 400);
    assert!(resp["error"].is_null(), "resources/list should not error");
    let resources = resp["result"]["resources"]
        .as_array()
        .expect("should have resources array");
    assert_eq!(resources.len(), 5, "should expose 5 resources");

    let uris: Vec<&str> = resources.iter().filter_map(|r| r["uri"].as_str()).collect();
    assert!(
        uris.contains(&"tokensave://status"),
        "should have status resource"
    );
    assert!(
        uris.contains(&"tokensave://files"),
        "should have files resource"
    );
    assert!(
        uris.contains(&"tokensave://overview"),
        "should have overview resource"
    );
    assert!(
        uris.contains(&"tokensave://branches"),
        "should have branches resource"
    );
    assert!(
        uris.contains(&"tokensave://schema"),
        "should have schema resource"
    );

    // All resources should have name, description, and mimeType.
    for resource in resources {
        assert!(resource["name"].is_string(), "resource should have name");
        assert!(
            resource["description"].is_string(),
            "resource should have description"
        );
        assert!(
            resource["mimeType"].is_string(),
            "resource should have mimeType"
        );
    }
}

// ---------------------------------------------------------------------------
// 20. test_resources_read_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_status() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(410),
            "resources/read",
            json!({
                "uri": "tokensave://status"
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 410)
        .expect("should have response for id=410");
    let resp = parse_response(resp_str);
    assert!(
        resp["error"].is_null(),
        "resources/read status should not error"
    );

    let contents = resp["result"]["contents"]
        .as_array()
        .expect("should have contents array");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "tokensave://status");
    assert_eq!(contents[0]["mimeType"], "application/json");

    let text = contents[0]["text"].as_str().unwrap();
    assert!(
        text.contains("node_count"),
        "status resource should contain node_count"
    );
    assert!(
        text.contains("file_count"),
        "status resource should contain file_count"
    );
}

// ---------------------------------------------------------------------------
// 21. test_resources_read_files
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_files() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(420),
            "resources/read",
            json!({
                "uri": "tokensave://files"
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 420)
        .expect("should have response for id=420");
    let resp = parse_response(resp_str);
    assert!(
        resp["error"].is_null(),
        "resources/read files should not error"
    );

    let contents = resp["result"]["contents"]
        .as_array()
        .expect("should have contents array");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "tokensave://files");
    assert_eq!(contents[0]["mimeType"], "text/plain");

    let text = contents[0]["text"].as_str().unwrap();
    assert!(
        text.contains("indexed files"),
        "files resource should contain file count summary"
    );
    assert!(
        text.contains("main.rs"),
        "files resource should list main.rs"
    );
}

// ---------------------------------------------------------------------------
// 22. test_resources_read_overview
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_overview() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(430),
            "resources/read",
            json!({
                "uri": "tokensave://overview"
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 430)
        .expect("should have response for id=430");
    let resp = parse_response(resp_str);
    assert!(
        resp["error"].is_null(),
        "resources/read overview should not error"
    );

    let contents = resp["result"]["contents"]
        .as_array()
        .expect("should have contents array");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "tokensave://overview");
    assert_eq!(contents[0]["mimeType"], "text/plain");

    let text = contents[0]["text"].as_str().unwrap();
    assert!(
        text.contains("Project:"),
        "overview should start with Project:"
    );
    assert!(
        text.contains("Graph:"),
        "overview should contain Graph summary"
    );
    assert!(text.contains("nodes"), "overview should mention nodes");
}

// ---------------------------------------------------------------------------
// 23. test_resources_read_unknown_uri
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_unknown_uri() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(440),
            "resources/read",
            json!({
                "uri": "tokensave://nonexistent"
            }),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 440)
        .expect("should have response for id=440");
    let resp = parse_response(resp_str);
    assert!(
        resp["error"].is_object(),
        "unknown URI should produce error"
    );
    assert_eq!(
        resp["error"]["code"], -32602,
        "should be InvalidParams error"
    );
}

// ---------------------------------------------------------------------------
// 24. test_resources_read_missing_uri
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_missing_uri() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(450), "resources/read", json!({}))],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 450)
        .expect("should have response for id=450");
    let resp = parse_response(resp_str);
    assert!(
        resp["error"].is_object(),
        "missing URI should produce error"
    );
    assert_eq!(
        resp["error"]["code"], -32602,
        "should be InvalidParams error"
    );
}

// ---------------------------------------------------------------------------
// Regression: logging/setLevel must be handled (not return MethodNotFound)
// ---------------------------------------------------------------------------

/// The MCP client sends `logging/setLevel` immediately after initialisation
/// whenever the server advertises the `logging` capability. Before the fix the
/// server returned -32601 (MethodNotFound), which Claude Code logged as an
/// error on every session start.
#[tokio::test]
async fn test_logging_set_level_returns_success() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(500),
            "logging/setLevel",
            json!({"level": "info"}),
        )],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 500)
        .expect("should have response for id=500");
    let resp = parse_response(resp_str);
    assert!(
        resp["error"].is_null(),
        "logging/setLevel must not return an error, got: {resp}"
    );
    assert!(
        resp["result"].is_object(),
        "logging/setLevel must return an object result"
    );
}

/// Verify every log level accepted by RFC 5424 is handled without error.
#[tokio::test]
async fn test_logging_set_level_all_levels() {
    let levels = [
        "debug",
        "info",
        "notice",
        "warning",
        "error",
        "critical",
        "alert",
        "emergency",
    ];
    for (idx, level) in levels.iter().enumerate() {
        let id = json!(600 + idx as u64);
        let (_dir, server) = setup_server().await;
        let responses = run_server_with_messages(
            server,
            vec![jsonrpc_request(
                id.clone(),
                "logging/setLevel",
                json!({"level": level}),
            )],
        )
        .await;
        let resp_str = responses
            .iter()
            .find(|r| parse_response(r)["id"] == id)
            .unwrap_or_else(|| panic!("no response for level={level}"));
        let resp = parse_response(resp_str);
        assert!(
            resp["error"].is_null(),
            "logging/setLevel with level={level} must not error, got: {resp}"
        );
    }
}

/// `logging/setLevel` mid-session must not disrupt subsequent tool calls.
#[tokio::test]
async fn test_logging_set_level_does_not_break_session() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![
            jsonrpc_request(json!(700), "logging/setLevel", json!({"level": "warning"})),
            jsonrpc_request(json!(701), "ping", json!({})),
        ],
    )
    .await;

    let set_level = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 700)
        .expect("missing response for logging/setLevel");
    assert!(
        parse_response(set_level)["error"].is_null(),
        "logging/setLevel should succeed"
    );

    let ping = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 701)
        .expect("missing response for ping after logging/setLevel");
    assert!(
        parse_response(ping)["result"].is_object(),
        "ping after setLevel should succeed"
    );
}

/// The `initialize` response must advertise the `logging` capability so that
/// clients know they may send `logging/setLevel`.
#[tokio::test]
async fn test_initialize_advertises_logging_capability() {
    let (_dir, server) = setup_server().await;
    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(json!(800), "initialize", json!({}))],
    )
    .await;

    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 800)
        .expect("missing initialize response");
    let resp = parse_response(resp_str);
    assert!(
        resp["result"]["capabilities"]["logging"].is_object(),
        "initialize must advertise logging capability, got: {resp}"
    );
}

// ---------------------------------------------------------------------------
// Version-aware forced reindex on first tool call (#v11)
// ---------------------------------------------------------------------------

/// A pre-7.0 project carries an empty `last_indexed_version`. The first
/// `tools/call` must trigger a background forced reindex that, on completion,
/// advances the marker in the persisted project config to the running version.
#[tokio::test]
async fn test_first_tool_call_backfills_last_indexed_version() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() { let x = helper(); }\nfn helper() -> i32 { 42 }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // `index_all` now stamps the marker (#320), so reset it to reproduce the
    // pre-7.0 state that this backfill path exists to handle.
    let mut seed = tokensave::config::load_config(project).unwrap();
    seed.last_indexed_version = String::new();
    tokensave::config::save_config(project, &seed).unwrap();

    let before = tokensave::config::load_config(project).unwrap();
    assert_eq!(before.last_indexed_version, "");

    let server = McpServer::new(cg, None).await;
    let server_for_drive = Arc::clone(&server);

    // Drive one tool call through the full JSON-RPC transport.
    let _ = run_server_with_messages(
        server_for_drive,
        vec![jsonrpc_request(
            json!(1),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "main" }
            }),
        )],
    )
    .await;

    let completed = server
        .wait_for_version_reindex(std::time::Duration::from_secs(30))
        .await;
    assert!(completed, "version reindex task did not complete in time");

    let after = tokensave::config::load_config(project).unwrap();
    assert_eq!(
        after.last_indexed_version,
        env!("CARGO_PKG_VERSION"),
        "marker should advance to the running version after reindex"
    );
}

/// When the project was already indexed by the running version and the schema
/// is current, the first tool call must not regress the marker.
#[tokio::test]
async fn test_first_tool_call_keeps_current_marker() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() { let x = helper(); }\nfn helper() -> i32 { 42 }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // Pre-seed the marker to the running version so no reindex is needed.
    let mut config = tokensave::config::load_config(project).unwrap();
    config.last_indexed_version = env!("CARGO_PKG_VERSION").to_string();
    tokensave::config::save_config(project, &config).unwrap();

    let server = McpServer::new(cg, None).await;
    let server_for_drive = Arc::clone(&server);
    let _ = run_server_with_messages(
        server_for_drive,
        vec![jsonrpc_request(
            json!(1),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "main" }
            }),
        )],
    )
    .await;

    let completed = server
        .wait_for_version_reindex(std::time::Duration::from_secs(30))
        .await;
    assert!(completed, "version reindex gate did not settle in time");

    let after = tokensave::config::load_config(project).unwrap();
    assert_eq!(after.last_indexed_version, env!("CARGO_PKG_VERSION"));
}

/// A full index records the version that produced it, so a CLI-indexed project
/// is not mistaken for a pre-7.0 one and force-reindexed on the first tool call
/// of every session (#320).
#[tokio::test]
async fn test_index_all_records_running_version() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() { let x = helper(); }\nfn helper() -> i32 { 42 }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let config = tokensave::config::load_config(project).unwrap();
    assert_eq!(
        config.last_indexed_version,
        env!("CARGO_PKG_VERSION"),
        "a full index must record the version that produced it"
    );
}

/// A project indexed by the running version must not trigger a forced reindex
/// on the first tool call, so the index is never cleared for no reason (#320).
#[tokio::test]
async fn test_cli_indexed_project_needs_no_forced_reindex() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() { let x = helper(); }\nfn helper() -> i32 { 42 }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    let indexed = cg.index_all().await.unwrap();
    assert!(indexed.node_count > 0, "fixture should produce nodes");

    let server = McpServer::new(cg, None).await;
    let server_for_drive = Arc::clone(&server);
    let _ = run_server_with_messages(
        server_for_drive,
        vec![jsonrpc_request(
            json!(1),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "main" }
            }),
        )],
    )
    .await;

    let completed = server
        .wait_for_version_reindex(std::time::Duration::from_secs(30))
        .await;
    assert!(completed, "version reindex gate did not settle in time");

    let after = tokensave::config::load_config(project).unwrap();
    assert_eq!(after.last_indexed_version, env!("CARGO_PKG_VERSION"));
}
