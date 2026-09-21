//! `tools = "core"` makes `tools/list` send only the core tools (#576).
//!
//! A client sends every listed tool schema on every turn, before any tool is
//! called. The full surface is a fixed cost of the context window, and on a
//! small-context model it can be more than half of it. The setting selects
//! what the server *lists*. It does not select what the server can run: a tool
//! that is not listed must still answer a `tools/call`, so an agent permission
//! list or a hook that names it keeps working.
//!
//! Run with: `cargo test --features test-transport --test toolset_test`

#![cfg(feature = "test-transport")]

use std::sync::Arc;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::config::Toolset;
use tokensave::mcp::tools::CORE_TOOLS;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;

/// Creates and indexes a project, and sets `tools` before the server opens it.
async fn setup_server(tools: Option<Toolset>) -> (TempDir, Arc<McpServer>) {
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

    if let Some(tools) = tools {
        let mut config = tokensave::config::load_config(project).unwrap();
        config.tools = tools;
        tokensave::config::save_config(project, &config).unwrap();
    }

    let cg = TokenSave::open(project).await.unwrap();
    let server = McpServer::new(cg, None).await;
    (dir, server)
}

/// Sends one request and returns the parsed response.
async fn request(server: &Arc<McpServer>, id: i64, method: &str, params: Value) -> Value {
    let (mut transport, _sender, mut receiver) = ChannelTransport::new();
    let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
    server.handle_and_write(&req, &mut transport).await;
    let response = receiver.recv().await.expect("expected a response");
    serde_json::from_str(response.trim()).unwrap()
}

async fn listed_tool_names(server: &Arc<McpServer>) -> Vec<String> {
    let response = request(server, 1, "tools/list", json!({})).await;
    response["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn the_default_toolset_lists_every_tool() {
    let (_dir, server) = setup_server(None).await;
    let names = listed_tool_names(&server).await;
    assert_eq!(
        names.len(),
        tokensave::mcp::tools::get_tool_definitions().len()
    );
    assert!(names.len() > CORE_TOOLS.len());
}

#[tokio::test]
async fn the_core_toolset_lists_only_the_core_tools() {
    let (_dir, server) = setup_server(Some(Toolset::Core)).await;
    let mut names = listed_tool_names(&server).await;
    names.sort();
    let mut expected: Vec<String> = CORE_TOOLS.iter().map(ToString::to_string).collect();
    expected.sort();
    assert_eq!(names, expected);
}

/// Hidden is not disabled: `tokensave_todos` is outside the core set, and a
/// call by name still gets a result.
#[tokio::test]
async fn a_tool_outside_the_core_toolset_still_answers_a_call() {
    assert!(!CORE_TOOLS.contains(&"tokensave_todos"));
    let (_dir, server) = setup_server(Some(Toolset::Core)).await;
    let response = request(
        &server,
        2,
        "tools/call",
        json!({"name": "tokensave_todos", "arguments": {}}),
    )
    .await;
    assert!(response.get("error").is_none(), "got: {response}");
    assert_ne!(
        response["result"]["isError"],
        json!(true),
        "got: {response}"
    );
}

/// A config written before #576 has no `tools` key and must keep listing
/// everything, and the value round-trips as a lowercase string.
#[test]
fn a_config_without_the_key_means_full_and_the_value_round_trips() {
    let mut value = serde_json::to_value(tokensave::config::TokenSaveConfig::default()).unwrap();
    assert_eq!(value["tools"], json!("full"));
    value.as_object_mut().unwrap().remove("tools");
    let old: tokensave::config::TokenSaveConfig = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(old.tools, Toolset::Full);

    value["tools"] = json!("core");
    let core: tokensave::config::TokenSaveConfig = serde_json::from_value(value).unwrap();
    assert_eq!(core.tools, Toolset::Core);
}

#[test]
fn an_env_value_that_names_no_toolset_is_ignored() {
    assert_eq!(Toolset::parse("core"), Some(Toolset::Core));
    assert_eq!(Toolset::parse(" FULL "), Some(Toolset::Full));
    assert_eq!(Toolset::parse("lean"), None);
    assert_eq!(Toolset::parse(""), None);
}
