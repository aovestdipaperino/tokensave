//! `report_savings = false` stops tokensave from asking the agent to narrate
//! savings, without stopping the accounting that makes them measurable (#356).
//!
//! The server's whole point is cutting *input* tokens. Asking the model to
//! report the saving on nearly every turn spends *output* tokens — the more
//! expensive kind — which for some users offsets the win. Two things drive
//! that narration: the `tokensave_metrics:` line appended to tool results, and
//! the sentence in the MCP `instructions` telling the agent to report it. This
//! setting removes both, and these tests pin down that the ledger keeps
//! recording either way, so `tokensave gain` still works when it is off.
//!
//! Run with: `cargo test --features test-transport --test report_savings_test`

#![cfg(feature = "test-transport")]

use std::sync::Arc;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;

/// Creates and indexes a project, optionally turning `report_savings` off
/// before the server opens it.
async fn setup_server(report_savings: bool) -> (TempDir, Arc<McpServer>) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("src/main.rs"),
        "fn main() { let x = helper(); }\nfn helper() -> i32 { 42 }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    drop(cg);

    if !report_savings {
        let mut config = tokensave::config::load_config(project).unwrap();
        config.report_savings = false;
        tokensave::config::save_config(project, &config).unwrap();
    }

    let cg = TokenSave::open(project).await.unwrap();
    let server = McpServer::new(cg, None).await;
    (dir, server)
}

fn jsonrpc_request(id: i64, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

/// Sends one request and returns the parsed response.
async fn request(server: &Arc<McpServer>, id: i64, method: &str, params: Value) -> Value {
    let (mut transport, _sender, mut receiver) = ChannelTransport::new();
    let req = jsonrpc_request(id, method, params);
    server.handle_and_write(&req, &mut transport).await;
    let response = receiver.recv().await.expect("expected a response");
    serde_json::from_str(response.trim()).unwrap()
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

/// Runs one `tokensave_search` and returns its text content.
async fn search_text(server: &Arc<McpServer>) -> String {
    let parsed = request(
        server,
        1,
        "tools/call",
        json!({"name": "tokensave_search", "arguments": {"query": "helper"}}),
    )
    .await;
    assert!(parsed["error"].is_null(), "{parsed}");
    response_text(&parsed)
}

async fn initialize_instructions(server: &Arc<McpServer>) -> String {
    let parsed = request(server, 1, "initialize", json!({})).await;
    parsed["result"]["instructions"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn metrics_line_is_emitted_by_default() {
    // The default must stay on — this is a diagnostic users rely on, and #356
    // asked for a way to opt out, not for a change of default.
    let (_dir, server) = setup_server(true).await;
    assert!(
        search_text(&server).await.contains("tokensave_metrics:"),
        "default configuration must keep surfacing savings"
    );
}

#[tokio::test]
async fn metrics_line_is_suppressed_when_reporting_is_off() {
    let (_dir, server) = setup_server(false).await;
    let text = search_text(&server).await;
    assert!(
        !text.contains("tokensave_metrics:"),
        "report_savings = false must remove the metrics line; got: {text}"
    );
    assert!(
        !text.is_empty(),
        "suppressing the metrics line must not empty the result"
    );
}

#[tokio::test]
async fn instructions_ask_for_narration_only_when_reporting_is_on() {
    let (_dir, on) = setup_server(true).await;
    assert!(
        initialize_instructions(&on)
            .await
            .contains("report the savings"),
        "default instructions should still ask for the report"
    );

    let (_dir, off) = setup_server(false).await;
    let quiet = initialize_instructions(&off).await;
    assert!(
        !quiet.contains("report the savings"),
        "report_savings = false must drop the narration instruction; got: {quiet}"
    );
    assert!(
        quiet.contains("code-graph MCP server"),
        "the rest of the instructions must survive; got: {quiet}"
    );
}

#[tokio::test]
async fn savings_are_still_accounted_when_reporting_is_off() {
    // The point of the flag is to stop the model talking about savings, not to
    // stop measuring them. The savings ledger is what `tokensave gain` reads,
    // so that is what has to keep growing with reporting off.
    let (dir, server) = setup_server(false).await;
    let project = tokensave::global_db::normalize_project_key(dir.path());
    let gdb = tokensave::global_db::GlobalDb::open().await.unwrap();
    let calls_before = gdb.sum_savings(Some(&project), 0).await.calls;

    let text = search_text(&server).await;
    assert!(
        !text.contains("tokensave_metrics:"),
        "precondition: reporting is off"
    );
    server
        .wait_for_accounting_idle(std::time::Duration::from_secs(10))
        .await;

    assert_eq!(
        gdb.sum_savings(Some(&project), 0).await.calls,
        calls_before + 1,
        "the call must still reach the ledger that `tokensave gain` reports from"
    );
}
