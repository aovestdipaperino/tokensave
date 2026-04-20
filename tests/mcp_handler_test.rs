//! Integration tests for MCP tool handlers (`handle_tool_call`).
//!
//! Each test exercises a real `TokenSave` instance with indexed test data,
//! ensuring that the MCP dispatch layer formats results correctly.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

// ---------------------------------------------------------------------------
// Shared setup
// ---------------------------------------------------------------------------

/// Creates a temporary Rust project with cross-file calls, structs, impls,
/// test files, and doc comments, then initialises and indexes a `TokenSave`.
async fn setup_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/main.rs"),
        r#"
use crate::utils::helper;
mod utils;

fn main() {
    let result = helper();
    println!("{}", result);
}
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/utils.rs"),
        r#"
/// Returns a greeting string.
pub fn helper() -> String {
    format_greeting("world")
}

fn format_greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#,
    )
    .unwrap();

    // Test file so affected-tests can find something
    fs::create_dir_all(project.join("tests")).unwrap();
    fs::write(
        project.join("tests/test_utils.rs"),
        r#"
use crate::utils::helper;

#[test]
fn test_helper() { assert!(!helper().is_empty()); }
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

/// Extracts the text content from a `ToolResult` value (the standard
/// `content[0].text` envelope).
fn extract_text(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

/// Searches for `name` via the search handler and returns the first matching
/// node id whose name field equals `name`.
async fn find_node_id(cg: &TokenSave, name: &str) -> String {
    let result = handle_tool_call(cg, "tokensave_search", json!({"query": name}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let items: Vec<Value> = serde_json::from_str(text).unwrap();
    items
        .iter()
        .find(|item| item["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("node '{}' not found via search", name))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

// ---------------------------------------------------------------------------
// 1. tokensave_search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_search",
        json!({"query": "helper", "limit": 5}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
    assert!(
        text.contains("helper"),
        "search results should contain 'helper'"
    );
}

// ---------------------------------------------------------------------------
// 2. tokensave_context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_context() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_context",
        json!({"task": "understand the helper function"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
}

// ---------------------------------------------------------------------------
// 3. tokensave_callers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_callers() {
    let (cg, _dir) = setup_project().await;
    let node_id = find_node_id(&cg, "helper").await;
    let result = handle_tool_call(&cg, "tokensave_callers", json!({"node_id": node_id}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
}

// ---------------------------------------------------------------------------
// 4. tokensave_callees
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_callees() {
    let (cg, _dir) = setup_project().await;
    let node_id = find_node_id(&cg, "helper").await;
    let result = handle_tool_call(&cg, "tokensave_callees", json!({"node_id": node_id}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
}

// ---------------------------------------------------------------------------
// 5. tokensave_impact
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_impact() {
    let (cg, _dir) = setup_project().await;
    let node_id = find_node_id(&cg, "helper").await;
    let result = handle_tool_call(&cg, "tokensave_impact", json!({"node_id": node_id}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("node_count"));
}

// ---------------------------------------------------------------------------
// 6. tokensave_node — existing node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_node_existing() {
    let (cg, _dir) = setup_project().await;
    let node_id = find_node_id(&cg, "helper").await;
    let result = handle_tool_call(&cg, "tokensave_node", json!({"node_id": node_id}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("helper"),
        "node detail should contain the name"
    );
    assert!(
        text.contains("start_line"),
        "node detail should contain start_line"
    );
    assert!(
        text.contains("signature"),
        "node detail should contain signature"
    );
    assert!(
        text.contains("visibility"),
        "node detail should contain visibility"
    );
}

// ---------------------------------------------------------------------------
// 7. tokensave_node — nonexistent node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_node_not_found() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_node",
        json!({"node_id": "nonexistent_id_12345"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("Node not found"),
        "should report 'Node not found', got: {}",
        text,
    );
}

// ---------------------------------------------------------------------------
// 8. tokensave_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_status() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_status",
        json!({}),
        Some(json!({"uptime": 100})),
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("node_count"),
        "status should include node_count"
    );
    assert!(
        text.contains("server"),
        "status should include server stats"
    );
}

// ---------------------------------------------------------------------------
// 9. tokensave_files — no filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_files_no_filter() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_files", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty(), "files listing should not be empty");
    assert!(
        text.contains("indexed files"),
        "should have 'indexed files' header"
    );
}

// ---------------------------------------------------------------------------
// 10. tokensave_files — path filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_files_path_filter() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_files", json!({"path": "src"}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
    // The test file lives under tests/, so if path filter works it should
    // only contain src/ files.
    assert!(
        !text.contains("tests/test_utils"),
        "path filter should exclude files outside 'src'"
    );
}

// ---------------------------------------------------------------------------
// 11. tokensave_files — pattern filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_files_pattern_filter() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_files", json!({"pattern": "*.rs"}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
}

// ---------------------------------------------------------------------------
// 12. tokensave_files — flat format
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_files_flat_format() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_files", json!({"format": "flat"}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
    // Flat format includes "bytes" per entry
    assert!(text.contains("bytes"), "flat format should show byte sizes");
}

// ---------------------------------------------------------------------------
// 13. tokensave_affected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_affected() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_affected",
        json!({"files": ["src/utils.rs"]}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("affected_tests"),
        "should have affected_tests key"
    );
    assert!(text.contains("count"), "should have count key");
}

// ---------------------------------------------------------------------------
// 14. tokensave_dead_code
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dead_code() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_dead_code", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("dead_code_count"),
        "should have dead_code_count key"
    );
}

// ---------------------------------------------------------------------------
// 15. tokensave_diff_context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_diff_context() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_diff_context",
        json!({"files": ["src/utils.rs"]}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("changed_files"),
        "should have changed_files key"
    );
    assert!(
        text.contains("modified_symbols"),
        "should have modified_symbols key"
    );
}

// ---------------------------------------------------------------------------
// 16. tokensave_module_api
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_module_api() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_module_api", json!({"path": "src"}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("public_symbol_count"),
        "should have public_symbol_count key"
    );
    // helper is pub so it should appear
    assert!(
        text.contains("helper"),
        "pub fn helper should appear in module API"
    );
}

// ---------------------------------------------------------------------------
// 17. tokensave_circular
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_circular() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_circular", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("cycle_count"), "should have cycle_count key");
}

// ---------------------------------------------------------------------------
// 18. tokensave_hotspots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_hotspots() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_hotspots", json!({"limit": 5}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("hotspot_count"),
        "should have hotspot_count key"
    );
}

// ---------------------------------------------------------------------------
// 19. tokensave_similar
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_similar() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_similar", json!({"symbol": "helper"}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
    assert!(
        text.contains("helper"),
        "similar results should include 'helper'"
    );
}

// ---------------------------------------------------------------------------
// 20. tokensave_rename_preview
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rename_preview() {
    let (cg, _dir) = setup_project().await;
    let node_id = find_node_id(&cg, "helper").await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rename_preview",
        json!({"node_id": node_id}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("reference_count"),
        "should have reference_count key"
    );
    assert!(text.contains("node"), "should have node key");
}

// ---------------------------------------------------------------------------
// 21. tokensave_unused_imports
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unused_imports() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("unused_import_count"),
        "should have unused_import_count key"
    );
}

// ---------------------------------------------------------------------------
// 22. tokensave_rank
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rank() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rank",
        json!({"edge_kind": "calls", "direction": "incoming"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("ranking"), "should have ranking key");
    assert!(
        text.contains("result_count"),
        "should have result_count key"
    );
}

// ---------------------------------------------------------------------------
// 23. tokensave_rank — invalid direction
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rank_invalid_direction() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rank",
        json!({"edge_kind": "calls", "direction": "sideways"}),
        None, None,
    )
    .await;
    match result {
        Err(err) => {
            let err_msg = format!("{}", err);
            assert!(
                err_msg.contains("invalid direction"),
                "error should mention 'invalid direction', got: {}",
                err_msg,
            );
        }
        Ok(_) => panic!("invalid direction should produce an error"),
    }
}

// ---------------------------------------------------------------------------
// 24. tokensave_largest
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_largest() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_largest", json!({"limit": 5}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("ranking"), "should have ranking key");
    assert!(
        text.contains("result_count"),
        "should have result_count key"
    );
}

// ---------------------------------------------------------------------------
// 25. tokensave_coupling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_coupling() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_coupling",
        json!({"direction": "fan_in"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("ranking"), "should have ranking key");
}

// ---------------------------------------------------------------------------
// 26. tokensave_inheritance_depth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_inheritance_depth() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_inheritance_depth",
        json!({"limit": 5}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("result_count"),
        "should have result_count key"
    );
}

// ---------------------------------------------------------------------------
// 27. tokensave_distribution — default and summary mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_distribution_default() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_distribution", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("per_file"), "default mode should be per_file");
}

#[tokio::test]
async fn test_distribution_summary() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_distribution",
        json!({"summary": true}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("summary"),
        "summary mode should report 'summary'"
    );
    assert!(
        text.contains("distribution"),
        "should have distribution key"
    );
}

// ---------------------------------------------------------------------------
// 28. tokensave_recursion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_recursion() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_recursion", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("cycle_count"), "should have cycle_count key");
}

// ---------------------------------------------------------------------------
// 29. tokensave_complexity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_complexity() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_complexity", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("ranking"), "should have ranking key");
    assert!(text.contains("formula"), "should have formula key");
}

// ---------------------------------------------------------------------------
// 30. tokensave_doc_coverage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_doc_coverage() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_doc_coverage", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("total_undocumented"),
        "should have total_undocumented key"
    );
}

// ---------------------------------------------------------------------------
// 31. tokensave_god_class
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_god_class() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_god_class", json!({"limit": 5}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("result_count"),
        "should have result_count key"
    );
}

// ---------------------------------------------------------------------------
// 32. tokensave_changelog — requires git refs, expect graceful error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_changelog_no_git() {
    let (cg, _dir) = setup_project().await;
    // The temp dir is not a git repo, so this should return a "git diff failed"
    // message rather than a hard error.
    let result = handle_tool_call(
        &cg,
        "tokensave_changelog",
        json!({"from_ref": "HEAD~1", "to_ref": "HEAD"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("git diff failed"),
        "changelog on non-git dir should report git diff failure, got: {}",
        text,
    );
}

// ---------------------------------------------------------------------------
// 33. tokensave_port_status — no matching dirs expected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_port_status() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_port_status",
        json!({"source_dir": "src", "target_dir": "tests"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("coverage_percent"),
        "should have coverage_percent key"
    );
}

// ---------------------------------------------------------------------------
// 34. tokensave_port_order
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_port_order() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_port_order",
        json!({"source_dir": "src"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("total_symbols"),
        "should have total_symbols key"
    );
    assert!(text.contains("levels"), "should have levels key");
}

// ---------------------------------------------------------------------------
// 35. Unknown tool
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unknown_tool() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_unknown", json!({}), None, None).await;
    match result {
        Err(err) => {
            let err_msg = format!("{}", err);
            assert!(
                err_msg.contains("unknown tool"),
                "error should mention 'unknown tool', got: {}",
                err_msg,
            );
        }
        Ok(_) => panic!("unknown tool should produce an error"),
    }
}

// ---------------------------------------------------------------------------
// 36. Missing required params — search without query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_missing_required_params() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_search", json!({}), None, None).await;
    let err_msg = match result {
        Err(err) => format!("{}", err),
        Ok(_) => panic!("missing query should produce an error"),
    };
    assert!(
        err_msg.contains("missing required parameter"),
        "error should mention 'missing required parameter', got: {}",
        err_msg,
    );
}

// ---------------------------------------------------------------------------
// 37. Node ID alias — using "id" instead of "node_id"
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_node_id_alias() {
    let (cg, _dir) = setup_project().await;
    let node_id = find_node_id(&cg, "helper").await;
    // Use "id" instead of "node_id"
    let result = handle_tool_call(&cg, "tokensave_node", json!({"id": node_id}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("helper"),
        "node lookup via 'id' alias should still find the node"
    );
}

// ---------------------------------------------------------------------------
// Extra: tokensave_status without server_stats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_status_without_server_stats() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_status", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("node_count"),
        "status should include node_count"
    );
    // Should NOT contain "server" key when None is passed
    assert!(
        !text.contains("\"server\""),
        "status without server_stats should not include 'server' key"
    );
}

// ---------------------------------------------------------------------------
// Extra: touched_files populated for search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_populates_touched_files() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_search", json!({"query": "helper"}), None, None)
        .await
        .unwrap();
    assert!(
        !result.touched_files.is_empty(),
        "search results should populate touched_files"
    );
}

// ---------------------------------------------------------------------------
// Extra: rename_preview with nonexistent node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rename_preview_not_found() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rename_preview",
        json!({"node_id": "nonexistent_id_12345"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("Node not found"),
        "rename_preview with bad id should report 'Node not found', got: {}",
        text,
    );
}

// ---------------------------------------------------------------------------
// Extra: coupling with fan_out direction
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_coupling_fan_out() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_coupling",
        json!({"direction": "fan_out"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("fan_out"), "should report fan_out direction");
}

// ---------------------------------------------------------------------------
// Extra: rank with outgoing direction
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rank_outgoing() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rank",
        json!({"edge_kind": "calls", "direction": "outgoing"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("outgoing"),
        "should reflect outgoing direction"
    );
}

// ---------------------------------------------------------------------------
// Extra: missing required params for other handlers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_context_missing_task() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_context", json!({}), None, None).await;
    assert!(result.is_err(), "context without task should error");
}

#[tokio::test]
async fn test_callers_missing_node_id() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_callers", json!({}), None, None).await;
    assert!(result.is_err(), "callers without node_id should error");
}

#[tokio::test]
async fn test_affected_missing_files() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_affected", json!({}), None, None).await;
    assert!(result.is_err(), "affected without files should error");
}

#[tokio::test]
async fn test_module_api_missing_path() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_module_api", json!({}), None, None).await;
    assert!(result.is_err(), "module_api without path should error");
}

#[tokio::test]
async fn test_rank_missing_edge_kind() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rank",
        json!({"direction": "incoming"}),
        None, None,
    )
    .await;
    assert!(result.is_err(), "rank without edge_kind should error");
}

#[tokio::test]
async fn test_similar_missing_symbol() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_similar", json!({}), None, None).await;
    assert!(result.is_err(), "similar without symbol should error");
}

#[tokio::test]
async fn test_diff_context_missing_files() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_diff_context", json!({}), None, None).await;
    assert!(result.is_err(), "diff_context without files should error");
}

#[tokio::test]
async fn test_changelog_missing_refs() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_changelog", json!({}), None, None).await;
    assert!(result.is_err(), "changelog without from_ref should error");
}

#[tokio::test]
async fn test_port_status_missing_dirs() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_port_status", json!({}), None, None).await;
    assert!(
        result.is_err(),
        "port_status without source_dir should error"
    );
}

#[tokio::test]
async fn test_port_order_missing_source_dir() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_port_order", json!({}), None, None).await;
    assert!(
        result.is_err(),
        "port_order without source_dir should error"
    );
}

// ---------------------------------------------------------------------------
// Extra: tokensave_changelog with a real git repo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_changelog_with_real_git() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    // Initialize git repo and make a first commit
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .expect("git init failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(project)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(project)
        .output()
        .unwrap();

    fs::write(project.join("src/lib.rs"), "pub fn original() {}\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(project)
        .output()
        .unwrap();

    // Make a second commit with changes
    fs::write(
        project.join("src/lib.rs"),
        "pub fn original() {}\npub fn added() {}\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "add function"])
        .current_dir(project)
        .output()
        .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_changelog",
        json!({"from_ref": "HEAD~1", "to_ref": "HEAD"}),
        None, None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    // Should not report "git diff failed" since it's a real git repo
    assert!(
        !text.contains("git diff failed"),
        "changelog in git repo should not fail, got: {}",
        text,
    );
    assert!(
        text.contains("changed_file_count") || text.contains("lib.rs"),
        "changelog should mention changed files, got: {}",
        text,
    );
}

// ---------------------------------------------------------------------------
// Extra: tokensave_distribution with path prefix filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_distribution_with_path_filter() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_distribution", json!({"path": "src/"}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(text.contains("per_file"), "default mode should be per_file");
    // Should only contain src/ files, not tests/
    assert!(
        !text.contains("tests/test_utils"),
        "path filter should exclude files outside 'src/'",
    );
}

// ---------------------------------------------------------------------------
// Extra: tokensave_files — grouped format
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_files_grouped_format() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_files", json!({"format": "grouped"}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(!text.is_empty());
    // Grouped format shows directory headers like "src/ (N files)"
    assert!(
        text.contains("indexed files"),
        "grouped format should have 'indexed files' header"
    );
    assert!(
        text.contains("files)"),
        "grouped format should show file counts per directory"
    );
}

// ---------------------------------------------------------------------------
// Extra: tokensave_dead_code with custom kinds parameter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dead_code_custom_kinds() {
    let (cg, _dir) = setup_project().await;
    // Ask only for struct dead code
    let result = handle_tool_call(
        &cg,
        "tokensave_dead_code",
        json!({"kinds": ["struct"]}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("dead_code_count"),
        "should have dead_code_count key"
    );
    // Parse and verify any returned items are structs
    let parsed: Value = serde_json::from_str(text).unwrap_or(json!({}));
    if let Some(items) = parsed["dead_code"].as_array() {
        for item in items {
            assert_eq!(
                item["kind"].as_str().unwrap_or(""),
                "struct",
                "dead code items should be structs when kinds=['struct']"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Extra: tokensave_affected with custom filter glob
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_affected_with_custom_filter() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_affected",
        json!({"files": ["src/utils.rs"], "filter": "**/*test*"}),
        None, None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("affected_tests"),
        "should have affected_tests key"
    );
    assert!(text.contains("count"), "should have count key");
}

// ---------------------------------------------------------------------------
// Extra: tokensave_complexity — verify response structure
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_complexity_response_fields() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_complexity", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert!(parsed.get("ranking").is_some(), "should have ranking key");
    assert!(parsed.get("formula").is_some(), "should have formula key");
    // Check ranking items have expected fields
    if let Some(items) = parsed["ranking"].as_array() {
        if let Some(first) = items.first() {
            assert!(
                first.get("cyclomatic_complexity").is_some(),
                "ranking item should have cyclomatic_complexity"
            );
            assert!(
                first.get("branches").is_some(),
                "ranking item should have branches"
            );
            assert!(
                first.get("max_nesting").is_some(),
                "ranking item should have max_nesting"
            );
            assert!(
                first.get("fan_out").is_some(),
                "ranking item should have fan_out"
            );
            assert!(
                first.get("score").is_some(),
                "ranking item should have score"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Extra: tokensave_doc_coverage — verify response structure
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_doc_coverage_response_structure() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_doc_coverage", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("total_undocumented").is_some(),
        "should have total_undocumented"
    );
    assert!(parsed.get("file_count").is_some(), "should have file_count");
    assert!(parsed.get("files").is_some(), "should have files array");
    // If there are files, check their structure
    if let Some(files) = parsed["files"].as_array() {
        if let Some(first) = files.first() {
            assert!(first.get("file").is_some(), "file entry should have 'file'");
            assert!(
                first.get("count").is_some(),
                "file entry should have 'count'"
            );
            assert!(
                first.get("symbols").is_some(),
                "file entry should have 'symbols'"
            );
        }
    }
}

#[tokio::test]
async fn test_files_scope_prefix_filters() {
    let (cg, _dir) = setup_project().await;
    // With scope_prefix "src", should only return files under src/
    let result = handle_tool_call(
        &cg,
        "tokensave_files",
        json!({}),
        None,
        Some("src"),
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        !text.contains("tests/"),
        "scope_prefix 'src' should exclude test files"
    );
    assert!(text.contains("main.rs"), "should include src/main.rs");
}

#[tokio::test]
async fn test_search_scope_prefix_filters() {
    let (cg, _dir) = setup_project().await;
    // Search for "helper" but scoped to "tests" — should only return test file results
    let result = handle_tool_call(
        &cg,
        "tokensave_search",
        json!({"query": "helper", "limit": 20}),
        None,
        Some("tests"),
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap_or_default();
    for item in &items {
        let file = item["file"].as_str().unwrap_or("");
        assert!(
            file.starts_with("tests"),
            "scoped search should only return files under 'tests', got: {}",
            file
        );
    }
}

#[tokio::test]
async fn test_files_explicit_path_overrides_scope() {
    let (cg, _dir) = setup_project().await;
    // Explicit path "tests" should override scope_prefix "src"
    let result = handle_tool_call(
        &cg,
        "tokensave_files",
        json!({"path": "tests"}),
        None,
        Some("src"),
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        !text.contains("src/main.rs"),
        "explicit path 'tests' should exclude src files"
    );
}
