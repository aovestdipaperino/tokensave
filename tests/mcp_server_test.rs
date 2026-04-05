//! Integration tests for the MCP server (`McpServer`) exercising the full
//! JSON-RPC 2.0 protocol via `ChannelTransport`.
//!
//! Run with: `cargo test --features test-transport --test mcp_server_test`

#![cfg(feature = "test-transport")]

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::McpServer;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::tokensave::TokenSave;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Creates a temporary Rust project, indexes it, and returns a ready server.
async fn setup_server() -> (McpServer, TempDir) {
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
    let server = McpServer::new(cg).await;
    (server, dir)
}

/// Sends a sequence of JSON-RPC messages to a server, runs it to completion,
/// and returns all non-empty response lines.
async fn run_server_with_messages(server: McpServer, messages: Vec<String>) -> Vec<String> {
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(1), "initialize", json!({})),
    ])
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_notification("initialized"),
        jsonrpc_request(json!(2), "ping", json!({})),
    ])
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
    assert_eq!(ping_responses.len(), 1, "should get exactly one ping response");
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_notification("notifications/initialized"),
        jsonrpc_request(json!(3), "ping", json!({})),
    ])
    .await;

    let ping_responses: Vec<&String> = responses
        .iter()
        .filter(|r| {
            let v = parse_response(r);
            v["id"] == 3
        })
        .collect();
    assert_eq!(ping_responses.len(), 1, "should get exactly one ping response");
}

// ---------------------------------------------------------------------------
// 4. test_ping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_ping() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(10), "ping", json!({})),
    ])
    .await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 10);
    assert!(resp["result"].is_object(), "ping result should be an object");
    assert!(resp["error"].is_null(), "ping should not have an error");
}

// ---------------------------------------------------------------------------
// 5. test_tools_list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_list() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(20), "tools/list", json!({})),
    ])
    .await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 20);
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty(), "tools list should not be empty");
    // Verify at least some well-known tools are present.
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(tool_names.contains(&"tokensave_search"), "should have tokensave_search");
    assert!(tool_names.contains(&"tokensave_status"), "should have tokensave_status");
    assert!(tool_names.contains(&"tokensave_context"), "should have tokensave_context");
}

// ---------------------------------------------------------------------------
// 6. test_tools_call_search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_search() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(30), "tools/call", json!({
            "name": "tokensave_search",
            "arguments": { "query": "helper" }
        })),
    ])
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
// 7. test_tools_call_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_status() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(40), "tools/call", json!({
            "name": "tokensave_status",
            "arguments": {}
        })),
    ])
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
// 8. test_tools_call_missing_params
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tools_call_missing_params() {
    let (server, _dir) = setup_server().await;
    // Send tools/call with no params at all.
    let responses = run_server_with_messages(server, vec![
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 50,
            "method": "tools/call"
        }))
        .unwrap(),
    ])
    .await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 50);
    assert!(resp["error"].is_object(), "should have an error");
    assert_eq!(resp["error"]["code"], -32602, "should be InvalidParams error");
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(60), "tools/call", json!({
            "arguments": { "query": "test" }
        })),
    ])
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
    assert_eq!(resp["error"]["code"], -32602, "should be InvalidParams error");
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(70), "some/unknown/method", json!({})),
    ])
    .await;

    assert!(!responses.is_empty());
    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 70);
    assert!(resp["error"].is_object(), "should have an error");
    assert_eq!(resp["error"]["code"], -32601, "should be MethodNotFound error");
}

// ---------------------------------------------------------------------------
// 11. test_malformed_json
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_malformed_json() {
    let (server, _dir) = setup_server().await;
    // Send invalid JSON, then a valid ping to verify server continues.
    let responses = run_server_with_messages(server, vec![
        "this is not json {{{".to_string(),
        jsonrpc_request(json!(80), "ping", json!({})),
    ])
    .await;

    // Should have at least 2 responses: parse error + ping response.
    assert!(
        responses.len() >= 2,
        "should have at least 2 responses (parse error + ping), got {}",
        responses.len()
    );

    // First response should be a parse error.
    let error_resp = parse_response(&responses[0]);
    assert!(error_resp["error"].is_object(), "first response should be an error");
    assert_eq!(error_resp["error"]["code"], -32700, "should be ParseError (-32700)");

    // Second (or later) should be the ping response.
    let ping_resp = responses
        .iter()
        .find(|r| {
            let v = parse_response(r);
            v["id"] == 80
        })
        .expect("should have a ping response after malformed JSON");
    let ping = parse_response(ping_resp);
    assert!(ping["error"].is_null(), "ping after malformed JSON should succeed");
}

// ---------------------------------------------------------------------------
// 12. test_blank_lines_skipped
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_blank_lines_skipped() {
    let (server, _dir) = setup_server().await;
    // Send blank/whitespace lines, then a ping.
    let responses = run_server_with_messages(server, vec![
        "".to_string(),
        "   ".to_string(),
        "\t".to_string(),
        jsonrpc_request(json!(90), "ping", json!({})),
    ])
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(100), "initialize", json!({})),
        jsonrpc_request(json!(101), "ping", json!({})),
        jsonrpc_request(json!(102), "tools/list", json!({})),
        jsonrpc_request(json!(103), "tools/call", json!({
            "name": "tokensave_search",
            "arguments": { "query": "main" }
        })),
    ])
    .await;

    // Collect response IDs (filtering out notifications which have no "id" or null id).
    let response_ids: Vec<i64> = responses
        .iter()
        .filter_map(|r| {
            let v = parse_response(r);
            v["id"].as_i64()
        })
        .collect();

    assert!(response_ids.contains(&100), "should have response for id=100 (initialize)");
    assert!(response_ids.contains(&101), "should have response for id=101 (ping)");
    assert!(response_ids.contains(&102), "should have response for id=102 (tools/list)");
    assert!(response_ids.contains(&103), "should have response for id=103 (tools/call)");
}

// ---------------------------------------------------------------------------
// 14. test_server_stats_initial
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_server_stats_initial() {
    let (server, _dir) = setup_server().await;
    let stats = server.server_stats_json().await;
    assert!(stats["uptime_secs"].is_number(), "should have uptime_secs");
    assert_eq!(stats["total_requests"], 0, "initial total_requests should be 0");
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(200), "initialize", json!({})),
        jsonrpc_request(json!(201), "ping", json!({})),
        jsonrpc_request(json!(202), "tools/call", json!({
            "name": "tokensave_status",
            "arguments": {}
        })),
    ])
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(300), "unknown/method", json!({})),
        jsonrpc_request(json!(301), "tools/call", json!({
            "name": "tokensave_status",
            "arguments": {}
        })),
    ])
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
    assert!(error_resp["error"].is_object(), "unknown method should produce error");

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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(1), "initialize", json!({})),
    ])
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(1), "initialize", json!({})),
    ])
    .await;

    let resp = parse_response(&responses[0]);
    let instructions = resp["result"]["instructions"].as_str()
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
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(400), "resources/list", json!({})),
    ])
    .await;

    let resp = parse_response(&responses[0]);
    assert_eq!(resp["id"], 400);
    assert!(resp["error"].is_null(), "resources/list should not error");
    let resources = resp["result"]["resources"].as_array()
        .expect("should have resources array");
    assert_eq!(resources.len(), 3, "should expose 3 resources");

    let uris: Vec<&str> = resources.iter()
        .filter_map(|r| r["uri"].as_str())
        .collect();
    assert!(uris.contains(&"tokensave://status"), "should have status resource");
    assert!(uris.contains(&"tokensave://files"), "should have files resource");
    assert!(uris.contains(&"tokensave://overview"), "should have overview resource");

    // All resources should have name, description, and mimeType.
    for resource in resources {
        assert!(resource["name"].is_string(), "resource should have name");
        assert!(resource["description"].is_string(), "resource should have description");
        assert!(resource["mimeType"].is_string(), "resource should have mimeType");
    }
}

// ---------------------------------------------------------------------------
// 20. test_resources_read_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_status() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(410), "resources/read", json!({
            "uri": "tokensave://status"
        })),
    ])
    .await;

    let resp_str = responses.iter()
        .find(|r| parse_response(r)["id"] == 410)
        .expect("should have response for id=410");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "resources/read status should not error");

    let contents = resp["result"]["contents"].as_array()
        .expect("should have contents array");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "tokensave://status");
    assert_eq!(contents[0]["mimeType"], "application/json");

    let text = contents[0]["text"].as_str().unwrap();
    assert!(text.contains("node_count"), "status resource should contain node_count");
    assert!(text.contains("file_count"), "status resource should contain file_count");
}

// ---------------------------------------------------------------------------
// 21. test_resources_read_files
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_files() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(420), "resources/read", json!({
            "uri": "tokensave://files"
        })),
    ])
    .await;

    let resp_str = responses.iter()
        .find(|r| parse_response(r)["id"] == 420)
        .expect("should have response for id=420");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "resources/read files should not error");

    let contents = resp["result"]["contents"].as_array()
        .expect("should have contents array");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "tokensave://files");
    assert_eq!(contents[0]["mimeType"], "text/plain");

    let text = contents[0]["text"].as_str().unwrap();
    assert!(text.contains("indexed files"), "files resource should contain file count summary");
    assert!(text.contains("main.rs"), "files resource should list main.rs");
}

// ---------------------------------------------------------------------------
// 22. test_resources_read_overview
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_overview() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(430), "resources/read", json!({
            "uri": "tokensave://overview"
        })),
    ])
    .await;

    let resp_str = responses.iter()
        .find(|r| parse_response(r)["id"] == 430)
        .expect("should have response for id=430");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_null(), "resources/read overview should not error");

    let contents = resp["result"]["contents"].as_array()
        .expect("should have contents array");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], "tokensave://overview");
    assert_eq!(contents[0]["mimeType"], "text/plain");

    let text = contents[0]["text"].as_str().unwrap();
    assert!(text.contains("Project:"), "overview should start with Project:");
    assert!(text.contains("Graph:"), "overview should contain Graph summary");
    assert!(text.contains("nodes"), "overview should mention nodes");
}

// ---------------------------------------------------------------------------
// 23. test_resources_read_unknown_uri
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_unknown_uri() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(440), "resources/read", json!({
            "uri": "tokensave://nonexistent"
        })),
    ])
    .await;

    let resp_str = responses.iter()
        .find(|r| parse_response(r)["id"] == 440)
        .expect("should have response for id=440");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_object(), "unknown URI should produce error");
    assert_eq!(resp["error"]["code"], -32602, "should be InvalidParams error");
}

// ---------------------------------------------------------------------------
// 24. test_resources_read_missing_uri
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resources_read_missing_uri() {
    let (server, _dir) = setup_server().await;
    let responses = run_server_with_messages(server, vec![
        jsonrpc_request(json!(450), "resources/read", json!({})),
    ])
    .await;

    let resp_str = responses.iter()
        .find(|r| parse_response(r)["id"] == 450)
        .expect("should have response for id=450");
    let resp = parse_response(resp_str);
    assert!(resp["error"].is_object(), "missing URI should produce error");
    assert_eq!(resp["error"]["code"], -32602, "should be InvalidParams error");
}
