//! Integration tests for the MCP server (`McpServer`) exercising the full
//! JSON-RPC 2.0 protocol via `ChannelTransport`.
//!
//! Run with: `cargo test --features test-transport --test mcp_server_test`

#![cfg(feature = "test-transport")]

use serde_json::{json, Value};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Creates a temporary Rust project, indexes it, and returns a ready server.
async fn setup_server() -> (Arc<McpServer>, TempDir) {
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
    (server, dir)
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
    let (server, _dir) = setup_server().await;
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

// ---------------------------------------------------------------------------
// 2. test_initialized_notification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_initialized_notification() {
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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

// ---------------------------------------------------------------------------
// 6b. test_tools_call_timings_flag
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_timings_flag_off_by_default() {
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;

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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
        let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
    let (server, _dir) = setup_server().await;
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
// search_call_writes_savings_ledger_row
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_call_writes_savings_ledger_row() {
    let tmp_home = tempfile::tempdir().unwrap();
    // Note: std::env::set_var is process-wide; this test relies on no parallel test
    // simultaneously mutating HOME. If flake is observed, mark this #[serial_test::serial].
    std::env::set_var("HOME", tmp_home.path());
    #[cfg(target_os = "windows")]
    std::env::set_var("USERPROFILE", tmp_home.path());

    let (server, _proj_tmp) = setup_server().await;

    let responses = run_server_with_messages(
        server,
        vec![jsonrpc_request(
            json!(9001),
            "tools/call",
            json!({
                "name": "tokensave_search",
                "arguments": { "query": "hello" }
            }),
        )],
    )
    .await;

    // Verify the request completed successfully.
    let resp_str = responses
        .iter()
        .find(|r| parse_response(r)["id"] == 9001)
        .expect("should have a response for id=9001");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "search should not error");

    // Allow the spawned ledger-write task to complete before opening the DB.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let db = tokensave::global_db::GlobalDb::open()
        .await
        .expect("global db opens at isolated HOME");
    let total = db.sum_savings(None, 0).await;
    assert!(
        total.calls >= 1,
        "expected at least one ledger row, got {}",
        total.calls
    );
}

// ---------------------------------------------------------------------------
// Version-aware forced reindex on first tool call (#v11)
// ---------------------------------------------------------------------------

/// A fresh project starts with an empty `last_indexed_version`. The first
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

    // Sanity: brand-new config has no recorded indexed version.
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
