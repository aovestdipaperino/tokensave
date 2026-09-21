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
use tokensave::mcp::tools::{tool_area, CORE_TOOLS, MORE_TOOL};
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
    // `tokensave_more` is the way to the tools that are not listed.
    expected.push(MORE_TOOL.to_string());
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

/// Sends the messages through the real run loop, which is the code that
/// writes queued notifications, and returns every line the server wrote.
async fn run_session(server: Arc<McpServer>, messages: Vec<Value>) -> Vec<Value> {
    let (mut transport, sender, mut receiver) = ChannelTransport::new();
    for message in messages {
        sender.send(message.to_string()).unwrap();
    }
    drop(sender);
    let handle = tokio::spawn(async move {
        server.run(&mut transport).await.unwrap();
    });
    let mut lines = Vec::new();
    while let Some(line) = receiver.recv().await {
        if !line.trim().is_empty() {
            lines.push(serde_json::from_str(line.trim()).unwrap());
        }
    }
    handle.await.unwrap();
    lines
}

fn call(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn names_of(response: &Value) -> Vec<&str> {
    response["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect()
}

/// #576 lazy registration: `tokensave_more` lists one area, the server sends
/// `notifications/tools/list_changed` before the call result, and the next
/// `tools/list` has the tools of that area and no other hidden tool.
#[tokio::test]
async fn tokensave_more_lists_an_area_and_notifies_the_client() {
    let (_dir, server) = setup_server(Some(Toolset::Core)).await;
    let more = json!({"name": MORE_TOOL, "arguments": {"area": "git"}});
    let lines = run_session(
        server,
        vec![
            call(1, "initialize", json!({})),
            call(2, "tools/list", json!({})),
            call(3, "tools/call", more.clone()),
            call(4, "tools/list", json!({})),
            // The same area again changes nothing, so no second notification.
            call(5, "tools/call", more),
        ],
    )
    .await;

    let by_id = |id: i64| lines.iter().find(|line| line["id"] == json!(id)).unwrap();
    assert_eq!(
        by_id(1)["result"]["capabilities"]["tools"]["listChanged"],
        json!(true)
    );
    assert!(by_id(1)["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains(MORE_TOOL));

    let before = names_of(by_id(2));
    assert!(before.contains(&MORE_TOOL));
    assert!(!before.contains(&"tokensave_blame"));

    let notifications: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line["method"] == json!("notifications/tools/list_changed"))
        .map(|(index, _)| index)
        .collect();
    let result_index = lines
        .iter()
        .position(|line| line["id"] == json!(3))
        .unwrap();
    assert_eq!(notifications.len(), 1, "got: {lines:?}");
    assert!(notifications[0] < result_index);
    assert!(by_id(3)["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("tokensave_blame"));

    let after = names_of(by_id(4));
    assert!(after.contains(&"tokensave_blame"));
    assert!(after.contains(&MORE_TOOL), "other areas are still hidden");
    for name in &after {
        assert!(
            CORE_TOOLS.contains(name) || *name == MORE_TOOL || tool_area(name) == "git",
            "{name} must not be listed"
        );
    }
}

#[tokio::test]
async fn tokensave_more_rejects_an_unknown_area() {
    let (_dir, server) = setup_server(Some(Toolset::Core)).await;
    let response = request(
        &server,
        1,
        "tools/call",
        json!({"name": MORE_TOOL, "arguments": {"area": "nope"}}),
    )
    .await;
    let message = response["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("analysis") && message.contains("all"),
        "{message}"
    );
}

/// The full toolset never changes, so its handshake does not claim that it can.
#[tokio::test]
async fn the_full_toolset_does_not_declare_list_changed() {
    let (_dir, server) = setup_server(None).await;
    let lines = run_session(
        server,
        vec![
            call(1, "initialize", json!({})),
            call(2, "tools/list", json!({})),
        ],
    )
    .await;
    assert_eq!(lines[0]["result"]["capabilities"]["tools"], json!({}));
    assert!(!names_of(&lines[1]).contains(&MORE_TOOL));
}
