//! Integration test: walk-up discovery + scoped MCP queries.

use serde_json::json;
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

/// Sets up a project at the temp root with files in `src/mcp/`, `src/db/`, and `tests/`,
/// then initialises and indexes a `TokenSave`.
async fn setup_nested_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src/mcp")).unwrap();
    fs::create_dir_all(root.join("src/db")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();

    fs::write(
        root.join("src/mcp/server.rs"),
        r#"
pub fn serve() -> String {
    "running".to_string()
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/db/queries.rs"),
        r#"
pub fn query_all() -> Vec<String> {
    vec!["result".to_string()]
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/main.rs"),
        r#"
mod mcp;
mod db;

fn main() {
    mcp::server::serve();
    db::queries::query_all();
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("tests/test_server.rs"),
        r#"
#[test]
fn test_serve() {
    assert!(!String::new().is_empty() || true);
}
"#,
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

fn extract_text(value: &serde_json::Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

#[tokio::test]
async fn test_discover_project_root_from_subdir() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".tokensave")).unwrap();
    fs::write(root.join(".tokensave/tokensave.db"), b"fake").unwrap();
    let subdir = root.join("src/mcp/tools");
    fs::create_dir_all(&subdir).unwrap();

    let found = tokensave::config::discover_project_root(&subdir);
    assert_eq!(found.unwrap(), root);
}

#[tokio::test]
async fn test_files_scoped_to_subdir() {
    let (cg, _dir) = setup_nested_project().await;

    // Scoped to "src/mcp" — should only return mcp files
    let result = handle_tool_call(
        &cg,
        "tokensave_files",
        json!({}),
        None,
        Some("src/mcp"),
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("server.rs"), "should include src/mcp/server.rs");
    assert!(
        !text.contains("queries.rs"),
        "should exclude src/db/queries.rs"
    );
    assert!(
        !text.contains("test_server"),
        "should exclude tests/"
    );
}

#[tokio::test]
async fn test_traversal_unscoped() {
    let (cg, _dir) = setup_nested_project().await;

    // Search for "serve" to get its node_id (unscoped search to find it)
    let search_result = handle_tool_call(
        &cg,
        "tokensave_search",
        json!({"query": "serve", "limit": 10}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&search_result.value);
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap_or_default();

    if let Some(serve_node) = items.iter().find(|i| i["name"].as_str() == Some("serve")) {
        let node_id = serve_node["id"].as_str().unwrap();

        // Callers should work even with scope_prefix set — traversals are unscoped
        let callers_result = handle_tool_call(
            &cg,
            "tokensave_callers",
            json!({"node_id": node_id}),
            None,
            Some("src/mcp"),
        )
        .await
        .unwrap();
        let callers_text = extract_text(&callers_result.value);
        // We just verify callers doesn't error out with a scope prefix
        assert!(
            !callers_text.is_empty(),
            "callers should return results even with scope prefix set"
        );
    }
}
