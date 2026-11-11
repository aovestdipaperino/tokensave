//! `tools/list` sends only the core tools unless `tools = "full"` (#576).
//!
//! A client sends every listed tool schema on every turn, before any tool is
//! called. The core toolset is the default; `"tools": "full"` or
//! `TOKENSAVE_TOOLS=full` lists every tool. The full surface is a fixed cost of the context window, and on a
//! small-context model it can be more than half of it. The setting selects
//! what the server *lists*. It does not select what the server can run: a tool
//! that is not listed must still answer a `tools/call`, so an agent permission
//! list or a hook that names it keeps working.
//!
//! Run with: `cargo test --features test-transport --test integration toolset_test::`

#![cfg(feature = "test-transport")]

use std::sync::Arc;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::config::Toolset;
use tokensave::mcp::tools::{tool_area, CORE_TOOLS, MORE_TOOL};
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::{McpServer, McpTransport};
use tokensave::tokensave::TokenSave;

/// Creates and indexes a project, and sets `tools` before the server opens it.
async fn setup_server(tools: Option<Toolset>) -> (TempDir, Arc<McpServer>) {
    let (dir, cg) = setup_project(tools).await;
    let server = McpServer::new(cg, None).await;
    (dir, server)
}

/// Creates and indexes a project, sets `tools`, and reopens it.
async fn setup_project(tools: Option<Toolset>) -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("src").join("main.rs"),
        "fn main() { let x = helper(); }\nfn helper() -> i32 { 42 }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    drop(cg);

    if let Some(tools) = tools {
        let mut config = tokensave::config::load_config(project).unwrap();
        config.tools = Some(tools);
        tokensave::config::save_config(project, &config).unwrap();
    }

    let cg = TokenSave::open(project).await.unwrap();
    (dir, cg)
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

fn core_and_more() -> Vec<String> {
    let mut expected: Vec<String> = CORE_TOOLS.iter().map(ToString::to_string).collect();
    // `tokensave_more` is the way to the tools that are not listed.
    expected.push(MORE_TOOL.to_string());
    expected.sort();
    expected
}

/// The core toolset is the default: a project that never set `tools` lists
/// the core tools and `tokensave_more`.
#[tokio::test]
async fn the_default_toolset_lists_only_the_core_tools() {
    let (_dir, server) = setup_server(None).await;
    let mut names = listed_tool_names(&server).await;
    names.sort();
    assert_eq!(names, core_and_more());
}

#[tokio::test]
async fn the_full_toolset_lists_every_tool() {
    let (_dir, server) = setup_server(Some(Toolset::Full)).await;
    let names = listed_tool_names(&server).await;
    // Every tool but the hidden aliases (`tokensave_rename_preview`, #568).
    assert_eq!(
        names.len(),
        tokensave::mcp::tools::get_tool_definitions()
            .iter()
            .filter(|d| !tokensave::mcp::tools::is_hidden_tool(d))
            .count()
    );
    assert!(names.len() > CORE_TOOLS.len());
    assert!(!names.contains(&MORE_TOOL.to_string()));
}

#[tokio::test]
async fn the_core_toolset_lists_only_the_core_tools() {
    let (_dir, server) = setup_server(Some(Toolset::Core)).await;
    let mut names = listed_tool_names(&server).await;
    names.sort();
    assert_eq!(names, core_and_more());
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

/// An unset `tools` is not written, so a later change of the default reaches
/// the project; an explicit value round-trips as a lowercase string.
#[test]
fn an_unset_toolset_is_not_written_and_an_explicit_one_round_trips() {
    let mut value = serde_json::to_value(tokensave::config::TokenSaveConfig::default()).unwrap();
    assert!(value.get("tools").is_none(), "got: {value}");
    let unset: tokensave::config::TokenSaveConfig = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(unset.tools, None);
    assert_eq!(unset.tools.unwrap_or_default(), Toolset::Core);

    value["tools"] = json!("full");
    let full: tokensave::config::TokenSaveConfig = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(full.tools, Some(Toolset::Full));
    assert_eq!(serde_json::to_value(&full).unwrap()["tools"], json!("full"));

    value["tools"] = json!("core");
    let core: tokensave::config::TokenSaveConfig = serde_json::from_value(value).unwrap();
    assert_eq!(core.tools, Some(Toolset::Core));
}

/// Writes a `config.json` the way 7.13.0 did: schema version 1, with every
/// field written out, `tools` included.
fn write_version_1_config(project: &std::path::Path, tools: &str) {
    let mut value = serde_json::to_value(tokensave::config::TokenSaveConfig::default()).unwrap();
    value["version"] = json!(1);
    value["tools"] = json!(tools);
    std::fs::create_dir_all(tokensave::config::get_tokensave_dir(&project)).unwrap();
    std::fs::write(
        tokensave::config::get_tokensave_dir(&project).join("config.json"),
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .unwrap();
}

/// 7.13.0 wrote `"tools": "full"` into every config it saved, because full was
/// its default. That value is the old default, not a choice, so it is read as
/// unset and the next save drops it.
#[test]
fn a_full_written_by_a_version_1_config_is_read_as_the_old_default() {
    let dir = TempDir::new().unwrap();
    write_version_1_config(dir.path(), "full");

    let config = tokensave::config::load_config(dir.path()).unwrap();
    assert_eq!(config.tools, None);
    assert_eq!(config.version, tokensave::config::CONFIG_VERSION);

    tokensave::config::save_config(dir.path(), &config).unwrap();
    let text = std::fs::read_to_string(
        tokensave::config::get_tokensave_dir(dir.path()).join("config.json"),
    )
    .unwrap();
    let saved: Value = serde_json::from_str(&text).unwrap();
    assert!(saved.get("tools").is_none(), "got: {text}");
    assert_eq!(saved["version"], json!(tokensave::config::CONFIG_VERSION));
}

/// `core` was never the 7.13.0 default, so a version 1 config that says it
/// chose it.
#[test]
fn a_core_written_by_a_version_1_config_is_kept() {
    let dir = TempDir::new().unwrap();
    write_version_1_config(dir.path(), "core");
    let config = tokensave::config::load_config(dir.path()).unwrap();
    assert_eq!(config.tools, Some(Toolset::Core));
}

/// The opt-out: `"tools": "full"` in a current config is honoured.
#[tokio::test]
async fn full_in_a_current_config_is_the_opt_out() {
    let (dir, cg) = setup_project(Some(Toolset::Full)).await;
    let reloaded = tokensave::config::load_config(dir.path()).unwrap();
    assert_eq!(reloaded.tools, Some(Toolset::Full));
    assert_eq!(cg.toolset(), Toolset::Full);
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
        server.run(transport).await.unwrap();
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
    let (_dir, server) = setup_server(Some(Toolset::Full)).await;
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

async fn initialize_instructions(server: &Arc<McpServer>) -> String {
    let response = request(server, 1, "initialize", json!({})).await;
    response["result"]["instructions"]
        .as_str()
        .expect("instructions")
        .to_string()
}

/// A client that defers tool schemas (Claude Code) sees the server
/// instructions before any schema, so with the default toolset they name
/// every core tool, `tokensave_more`, and each area it can list.
#[tokio::test]
async fn the_default_instructions_map_the_core_tools_and_the_more_areas() {
    let (_dir, server) = setup_server(None).await;
    let instructions = initialize_instructions(&server).await;
    for name in CORE_TOOLS {
        assert!(
            instructions.contains(name),
            "{name} missing: {instructions}"
        );
    }
    assert!(instructions.contains(MORE_TOOL), "{instructions}");
    for (area, _, _) in tokensave::mcp::tools::TOOL_AREAS {
        assert!(
            instructions.contains(&format!("{area} (")),
            "area {area} missing: {instructions}"
        );
    }
}

/// The map costs the full toolset nothing: every tool is already listed.
#[tokio::test]
async fn the_full_instructions_do_not_carry_the_map() {
    let (_dir, server) = setup_server(Some(Toolset::Full)).await;
    let instructions = initialize_instructions(&server).await;
    assert!(!instructions.contains(MORE_TOOL), "{instructions}");
    assert!(!instructions.contains("tokensave_multi_str_replace"));
}
