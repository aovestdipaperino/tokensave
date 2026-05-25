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
        None,
        None,
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
        None,
        None,
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
    let result = handle_tool_call(
        &cg,
        "tokensave_callers",
        json!({"node_id": node_id}),
        None,
        None,
    )
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
    let result = handle_tool_call(
        &cg,
        "tokensave_callees",
        json!({"node_id": node_id}),
        None,
        None,
    )
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
    let result = handle_tool_call(
        &cg,
        "tokensave_impact",
        json!({"node_id": node_id}),
        None,
        None,
    )
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
    let result = handle_tool_call(
        &cg,
        "tokensave_node",
        json!({"node_id": node_id}),
        None,
        None,
    )
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
        None,
        None,
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
    let result = handle_tool_call(
        &cg,
        "tokensave_files",
        json!({"pattern": "*.rs"}),
        None,
        None,
    )
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
    let result = handle_tool_call(
        &cg,
        "tokensave_files",
        json!({"format": "flat"}),
        None,
        None,
    )
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
        None,
        None,
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
        None,
        None,
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
    let result = handle_tool_call(
        &cg,
        "tokensave_module_api",
        json!({"path": "src"}),
        None,
        None,
    )
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
    let result = handle_tool_call(
        &cg,
        "tokensave_similar",
        json!({"symbol": "helper"}),
        None,
        None,
    )
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("coverage_percent"),
        "should have coverage_percent key"
    );
}

/// Regression: port_status used to match symbols purely on (name,
/// kind_compat_group), so common method names like `new`, `process`, `fmt`,
/// or `reset` produced wild cross-type "matches" — e.g. `Biquad::new` would
/// pair with an unrelated `Adaa::new` simply because both methods are named
/// "new". The match key must also include the parent type so siblings of
/// distinct owners stay unmatched.
#[tokio::test]
async fn port_status_does_not_match_methods_of_different_parents() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src_a")).unwrap();
    fs::create_dir_all(project.join("src_b")).unwrap();

    fs::write(
        project.join("src_a/biquad.rs"),
        "pub struct Biquad;\n\
         impl Biquad {\n    pub fn new() -> Self { Self }\n    pub fn process(&self) {}\n}\n",
    )
    .unwrap();
    fs::write(
        project.join("src_b/adaa.rs"),
        "pub struct Adaa;\n\
         impl Adaa {\n    pub fn new() -> Self { Self }\n    pub fn process(&self) {}\n}\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_port_status",
        json!({
            "source_dir": "src_a",
            "target_dir": "src_b",
            "kinds": ["method"],
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).expect("response must be JSON");
    let matched: Vec<&Value> = output["matched_symbols"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    // None of the source methods should match because the parent types differ.
    assert!(
        matched.is_empty(),
        "Biquad::* and Adaa::* must not cross-match — got matches: {matched:?}"
    );
    assert_eq!(
        output["matched"].as_u64(),
        Some(0),
        "matched count must be 0; output={output}"
    );
}

/// Sanity: when the same parent type name exists in both dirs, methods do
/// match — confirming the parent-aware key isn't too strict.
#[tokio::test]
async fn port_status_matches_methods_with_same_parent_type() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src_a")).unwrap();
    fs::create_dir_all(project.join("src_b")).unwrap();

    fs::write(
        project.join("src_a/biquad.rs"),
        "pub struct Biquad;\n\
         impl Biquad { pub fn process(&self) {} }\n",
    )
    .unwrap();
    fs::write(
        project.join("src_b/biquad_port.rs"),
        "pub struct Biquad;\n\
         impl Biquad { pub fn process(&self) {} }\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_port_status",
        json!({
            "source_dir": "src_a",
            "target_dir": "src_b",
            "kinds": ["method"],
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).expect("response must be JSON");
    assert_eq!(
        output["matched"].as_u64(),
        Some(1),
        "Biquad::process should match Biquad::process; output={output}"
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
        None,
        None,
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
    let result = handle_tool_call(
        &cg,
        "tokensave_search",
        json!({"query": "helper"}),
        None,
        None,
    )
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
    let result = handle_tool_call(
        &cg,
        "tokensave_distribution",
        json!({"path": "src/"}),
        None,
        None,
    )
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
    let result = handle_tool_call(
        &cg,
        "tokensave_files",
        json!({"format": "grouped"}),
        None,
        None,
    )
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
        None,
        None,
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
        None,
        None,
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
    let result = handle_tool_call(&cg, "tokensave_files", json!({}), None, Some("src"))
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

#[tokio::test]
async fn test_context_scope_prefix_filters() {
    let (cg, _dir) = setup_project().await;
    // Context scoped to "tests" should return results (even if limited to test files)
    let result = handle_tool_call(
        &cg,
        "tokensave_context",
        json!({"task": "understand helper"}),
        None,
        Some("tests"),
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        !text.is_empty(),
        "context should return results even when scoped"
    );
}

#[tokio::test]
async fn test_status_reports_scope_prefix() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_status", json!({}), None, Some("src/mcp"))
        .await
        .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("scope_prefix"),
        "status should report scope_prefix"
    );
    assert!(
        text.contains("src/mcp"),
        "status should show the actual prefix value"
    );
}

#[tokio::test]
async fn test_status_no_scope_prefix() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_status", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("scope_prefix").is_none() || parsed["scope_prefix"].is_null(),
        "status should not have scope_prefix when None"
    );
}

// ---------------------------------------------------------------------------
// Edit tools: tokensave_str_replace, tokensave_multi_str_replace, tokensave_insert_at
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_str_replace_success() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/main.rs"),
        "fn hello() {}\nfn world() {}\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_str_replace",
        json!({
            "path": "src/main.rs",
            "old_str": "fn hello() {}",
            "new_str": "fn hello_updated() {}"
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], true);
    assert_eq!(parsed["matched_str"], "fn hello() {}");
    assert_eq!(parsed["new_str"], "fn hello_updated() {}");

    let content = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert!(content.contains("fn hello_updated() {}"));
    assert!(!content.contains("fn hello() {}"));
}

#[tokio::test]
async fn test_str_replace_not_found() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(project.join("src/main.rs"), "fn hello() {}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_str_replace",
        json!({
            "path": "src/main.rs",
            "old_str": "fn not_exists() {}",
            "new_str": "fn replaced() {}"
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], false);
    assert!(parsed["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn test_str_replace_multiple_matches_fails() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(project.join("src/main.rs"), "fn foo() {}\nfn foo() {}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_str_replace",
        json!({
            "path": "src/main.rs",
            "old_str": "fn foo() {}",
            "new_str": "fn bar() {}"
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], false);
    assert!(parsed["message"]
        .as_str()
        .unwrap()
        .contains("matches 2 times"));
}

#[tokio::test]
async fn test_multi_str_replace_success() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/main.rs"),
        "fn foo() {}\nfn bar() {}\nfn baz() {}\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_multi_str_replace",
        json!({
            "path": "src/main.rs",
            "replacements": [
                ["fn foo() {}", "fn foo_replaced() {}"],
                ["fn bar() {}", "fn bar_replaced() {}"]
            ]
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], true);
    assert_eq!(parsed["applied_count"], 2);

    let content = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert!(content.contains("fn foo_replaced()"));
    assert!(content.contains("fn bar_replaced()"));
    assert!(content.contains("fn baz() {}"));
}

#[tokio::test]
async fn test_multi_str_replace_atomic_failure() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(project.join("src/main.rs"), "fn foo() {}\nfn baz() {}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_multi_str_replace",
        json!({
            "path": "src/main.rs",
            "replacements": [
                ["fn not_exists() {}", "fn replaced() {}"],
                ["fn baz() {}", "fn baz_replaced() {}"]
            ]
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], false);
    assert!(parsed["message"]
        .as_str()
        .unwrap()
        .contains("must match exactly once"));

    let content = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert!(content.contains("fn foo() {}"));
    assert!(content.contains("fn baz() {}"));
    assert!(!content.contains("fn replaced()"));
}

#[tokio::test]
async fn test_multi_str_replace_unicode_preview_does_not_panic() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    let original = "fn main() {}\n";
    fs::write(project.join("src/main.rs"), original).unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let missing_old = format!("{}é", "a".repeat(19));
    let result = handle_tool_call(
        &cg,
        "tokensave_multi_str_replace",
        json!({
            "path": "src/main.rs",
            "replacements": [
                [missing_old, "replacement"]
            ]
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], false);
    let message = parsed["message"].as_str().unwrap();
    assert!(message.contains("matches 0 times"));
    assert!(message.contains("must match exactly once"));

    let content = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_str_replace_unsupported_file_type_succeeds() {
    // Regression: editing unsupported types (e.g. .css) previously wrote the
    // file then returned a reindex error, silently mutating the file.
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::write(project.join("style.css"), ".foo {\n\tfont-size: 14px;\n}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_str_replace",
        json!({
            "path": "style.css",
            "old_str": "\tfont-size: 14px;",
            "new_str": "\tfont-size: 0.85rem;"
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], true);

    let content = fs::read_to_string(project.join("style.css")).unwrap();
    assert!(content.contains("0.85rem"));
    assert!(!content.contains("14px"));
}

#[tokio::test]
async fn ast_grep_rewrite_has_literal_fallback_when_binary_missing() {
    if tokensave::mcp::tools::ast_grep_available() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn old_name() {}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_ast_grep_rewrite",
        json!({"path": "src/lib.rs", "pattern": "old_name", "rewrite": "new_name"}),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert_eq!(output["success"].as_bool(), Some(true), "{output}");
    assert!(
        fs::read_to_string(project.join("src/lib.rs"))
            .unwrap()
            .contains("new_name"),
        "literal fallback should update the file"
    );
}

#[tokio::test]
async fn ast_grep_rewrite_uses_current_cli_update_flag() {
    if !tokensave::mcp::tools::ast_grep_available() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub fn caller() { old_name(); }\npub fn old_name() {}\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_ast_grep_rewrite",
        json!({"path": "src/lib.rs", "pattern": "old_name()", "rewrite": "new_name()"}),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert_eq!(output["success"].as_bool(), Some(true), "{output}");
    let content = fs::read_to_string(project.join("src/lib.rs")).unwrap();
    assert!(
        content.contains("new_name();"),
        "ast-grep rewrite should apply with the installed CLI: {content}"
    );
    assert!(
        !output["message"]
            .as_str()
            .unwrap_or_default()
            .contains("unexpected argument '-d'"),
        "rewrite must not use the removed -d flag: {output}"
    );
}

/// Regression: `branch_diff` previously errored with `MCP error -32603: base
/// and head are the same branch` when base == head. `pr_context` handles the
/// same case gracefully (empty arrays); branch_diff must match that shape so
/// callers can rely on consistent behaviour.
#[tokio::test]
async fn branch_diff_returns_empty_when_base_equals_head() {
    let (cg, _dir) = setup_project().await;

    // branch_diff requires branch tracking metadata to be present.
    let tokensave_dir = tokensave::config::get_tokensave_dir(cg.project_root());
    let meta = tokensave::branch_meta::BranchMeta::new("master");
    tokensave::branch_meta::save_branch_meta(&tokensave_dir, &meta).unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_branch_diff",
        json!({"base": "master", "head": "master"}),
        None,
        None,
    )
    .await
    .expect("branch_diff must not error when base == head");

    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).expect("response must be valid JSON");
    assert_eq!(output["summary"]["added"].as_u64(), Some(0));
    assert_eq!(output["summary"]["removed"].as_u64(), Some(0));
    assert_eq!(output["summary"]["changed"].as_u64(), Some(0));
    assert_eq!(output["added"].as_array().map(Vec::len), Some(0));
    assert_eq!(output["removed"].as_array().map(Vec::len), Some(0));
    assert_eq!(output["changed"].as_array().map(Vec::len), Some(0));
}

/// Regression: when ast-grep exits non-zero with empty stderr (no language
/// inferred from the file extension, or pattern matches nothing), the tool
/// used to surface `"ast-grep failed: "` — a useless empty trailer. The
/// message must instead explain the likely cause so the caller can act on it.
#[tokio::test]
async fn ast_grep_rewrite_surfaces_useful_error_on_empty_stderr() {
    if !tokensave::mcp::tools::ast_grep_available() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn foo() {}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_ast_grep_rewrite",
        json!({
            "path": "src/lib.rs",
            "pattern": "__NONEXISTENT_PATTERN__",
            "rewrite": "whatever"
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert_eq!(output["success"].as_bool(), Some(false), "{output}");
    let message = output["message"].as_str().unwrap_or_default();
    assert!(
        !message.trim_end_matches(':').trim().eq("ast-grep failed"),
        "message must not end as an empty 'ast-grep failed:' — got: {message:?}"
    );
    assert!(
        message.contains("exit") || message.contains("0 nodes") || message.contains("no language"),
        "message must explain the likely cause (exit code / no language / 0 matches), got: {message:?}"
    );
}

#[tokio::test]
async fn test_multi_str_replace_unsupported_file_type_succeeds() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::write(
        project.join("style.css"),
        ".foo {\n\tfont-size: 14px;\n}\n.bar {\n\tfont-size: 16px;\n}\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_multi_str_replace",
        json!({
            "path": "style.css",
            "replacements": [
                ["\tfont-size: 14px;", "\tfont-size: 0.85rem;"],
                ["\tfont-size: 16px;", "\tfont-size: 1rem;"]
            ]
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], true);
    assert_eq!(parsed["applied_count"], 2);

    let content = fs::read_to_string(project.join("style.css")).unwrap();
    assert!(content.contains("0.85rem"));
    assert!(content.contains("1rem"));
    assert!(!content.contains("14px"));
    assert!(!content.contains("16px"));
}

#[tokio::test]
async fn test_insert_at_string_anchor_before() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/main.rs"),
        "line one\nline two\nline three\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_insert_at",
        json!({
            "path": "src/main.rs",
            "anchor": "line two",
            "content": "inserted line",
            "before": true
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], true);

    let content = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert!(
        content.ends_with('\n'),
        "trailing newline must be preserved"
    );
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines[0], "line one");
    assert_eq!(lines[1], "inserted line");
    assert_eq!(lines[2], "line two");
    assert_eq!(lines[3], "line three");
}

#[tokio::test]
async fn test_insert_at_line_number() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/main.rs"),
        "line one\nline two\nline three\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_insert_at",
        json!({
            "path": "src/main.rs",
            "anchor": "2",
            "content": "inserted at line 2",
            "before": false
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], true);
    assert_eq!(parsed["anchor_line"], 2);

    let content = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert!(
        content.ends_with('\n'),
        "trailing newline must be preserved"
    );
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines[0], "line one");
    assert_eq!(lines[1], "line two");
    assert_eq!(lines[2], "inserted at line 2");
    assert_eq!(lines[3], "line three");
}

#[tokio::test]
async fn test_insert_at_anchor_not_found() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(project.join("src/main.rs"), "line one\nline two\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_insert_at",
        json!({
            "path": "src/main.rs",
            "anchor": "nonexistent",
            "content": "should not be inserted",
            "before": true
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], false);
    assert!(parsed["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn test_insert_at_unicode_anchor_prefix_does_not_panic() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    let original = "line one\nline two\n";
    fs::write(project.join("src/main.rs"), original).unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let long_anchor = format!("{}é", "a".repeat(99));
    let result = handle_tool_call(
        &cg,
        "tokensave_insert_at",
        json!({
            "path": "src/main.rs",
            "anchor": long_anchor,
            "content": "should not be inserted",
            "before": true
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], false);
    assert!(parsed["message"].as_str().unwrap().contains("not found"));

    let content = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn test_insert_at_ambiguous_anchor() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/main.rs"),
        "line foo\nline foo\nline bar\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_insert_at",
        json!({
            "path": "src/main.rs",
            "anchor": "foo",
            "content": "should not be inserted",
            "before": true
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], false);
    assert!(parsed["message"]
        .as_str()
        .unwrap()
        .contains("matches 2 lines"));
}

// Regression: insert_at must not strip trailing newline (#57)
#[tokio::test]
async fn test_insert_at_preserves_trailing_newline() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    let original = "fn hello() {}\n\nfn world() {}\n";
    fs::write(project.join("src/lib.rs"), original).unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_insert_at",
        json!({
            "path": "src/lib.rs",
            "anchor": "fn world",
            "content": "fn extra() {}",
            "before": true
        }),
        None,
        None,
    )
    .await
    .unwrap();

    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["success"], true);

    let content = fs::read_to_string(project.join("src/lib.rs")).unwrap();
    assert!(
        content.ends_with('\n'),
        "file must end with newline after insert_at, got: {:?}",
        &content[content.len().saturating_sub(20)..]
    );
    assert_eq!(content, "fn hello() {}\n\nfn extra() {}\nfn world() {}\n");
}

// ---------------------------------------------------------------------------
// tokensave_gini
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_gini() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_gini",
        json!({ "metric": "lines" }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("gini").is_some(),
        "gini field should exist, got: {}",
        text
    );
    assert!(
        parsed.get("interpretation").is_some(),
        "interpretation field should exist"
    );
}

#[tokio::test]
async fn test_gini_default_metric() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_gini", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("gini").is_some(),
        "gini field should exist with default args, got: {}",
        text
    );
}

// ---------------------------------------------------------------------------
// tokensave_dependency_depth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dependency_depth() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_dependency_depth",
        json!({ "limit": 5 }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("max_depth").is_some(),
        "max_depth field should exist, got: {}",
        text
    );
    assert!(
        parsed.get("ideal_depth").is_some(),
        "ideal_depth field should exist"
    );
}

// ---------------------------------------------------------------------------
// tokensave_health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_summary() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_health", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("quality_signal").is_some(),
        "quality_signal field should exist, got: {}",
        text
    );
    assert!(
        parsed.get("files_analyzed").is_some(),
        "files_analyzed field should exist"
    );
}

#[tokio::test]
async fn test_health_detailed() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_health",
        json!({ "details": true }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("quality_signal").is_some(),
        "quality_signal should exist, got: {}",
        text
    );
    let dims = parsed.get("dimensions").expect("dimensions should exist");
    assert!(dims.get("acyclicity").is_some(), "acyclicity score missing");
    assert!(dims.get("depth").is_some(), "depth score missing");
    assert!(dims.get("equality").is_some(), "equality score missing");
    assert!(dims.get("redundancy").is_some(), "redundancy score missing");
    assert!(dims.get("modularity").is_some(), "modularity score missing");
}

/// Issue #83: tokensave_redundancy must surface AST-isomorphic duplicate
/// pairs and rank them by composite similarity. Plant two structurally
/// identical functions in a fixture and assert the pair surfaces in the
/// top hit with the `definite` severity bucket.
#[tokio::test]
async fn test_redundancy_finds_planted_duplicate() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    // Two functions: identical structure, renamed identifiers.
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn compute_a(value: i32) -> i32 {
    let mut acc = 0;
    for i in 0..value {
        if i % 2 == 0 {
            acc += i;
        } else {
            acc -= i;
        }
    }
    acc
}

pub fn compute_b(input: i32) -> i32 {
    let mut total = 0;
    for j in 0..input {
        if j % 2 == 0 {
            total += j;
        } else {
            total -= j;
        }
    }
    total
}

pub fn unrelated(x: i32) -> i32 {
    x * 2
}
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_redundancy",
        json!({ "min_lines": 5, "similarity_threshold": 0.5 }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    let pair_count = parsed["pair_count"].as_u64().unwrap_or(0);
    assert!(
        pair_count >= 1,
        "expected at least 1 duplicate pair, got: {text}"
    );

    let pairs = parsed["pairs"].as_array().expect("pairs array");
    let top = &pairs[0];
    let kind = top["overlap_kind"].as_str().unwrap_or("");
    assert_eq!(
        kind, "ast_isomorphic",
        "top pair should be AST-isomorphic; full output: {text}"
    );
    let severity = top["severity"].as_str().unwrap_or("");
    assert_eq!(
        severity, "definite",
        "AST-identical pair should be 'definite'"
    );
    let names: Vec<&str> = vec![
        top["a"]["name"].as_str().unwrap_or(""),
        top["b"]["name"].as_str().unwrap_or(""),
    ];
    assert!(
        names.contains(&"compute_a") && names.contains(&"compute_b"),
        "expected compute_a/compute_b in pair, got {names:?}"
    );

    // Calling again should be a cache hit (no panic, same result).
    let result2 = handle_tool_call(
        &cg,
        "tokensave_redundancy",
        json!({ "min_lines": 5, "similarity_threshold": 0.5 }),
        None,
        None,
    )
    .await
    .unwrap();
    let parsed2: serde_json::Value = serde_json::from_str(extract_text(&result2.value)).unwrap();
    assert_eq!(parsed2["pair_count"], parsed["pair_count"]);
}

/// Issue #80: `tokensave_runtime` must surface process + DB telemetry so
/// users hitting unexpected CPU/RAM can capture a structured snapshot
/// without leaving the chat session.
#[tokio::test]
async fn test_runtime_snapshot_exposes_process_and_db_signals() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_runtime", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

    // Top-level envelope.
    assert!(parsed.get("captured_at").is_some());
    assert!(parsed["tokensave_version"].is_string());
    assert!(parsed["host_os"].is_string());

    // Process block — PID must match our own.
    let proc = &parsed["process"];
    assert_eq!(
        proc["pid"].as_u64().unwrap_or(0),
        u64::from(std::process::id()),
        "snapshot must report this process's PID"
    );
    assert!(
        proc["rss_bytes"].as_u64().unwrap_or(0) > 0,
        "RSS should be non-zero"
    );
    assert!(proc["system_cpu_count"].as_u64().unwrap_or(0) >= 1);
    assert!(proc["system_total_memory_bytes"].as_u64().unwrap_or(0) > 0);

    // Database block — the DB file we just opened must be present and sized.
    let db = &parsed["database"];
    assert!(db["db_path"].is_string());
    assert!(
        db["db_size_bytes"].as_u64().unwrap_or(0) > 0,
        "DB file should have non-zero size"
    );
    assert!(
        db["node_count"].as_u64().unwrap_or(0) > 0,
        "fixture indexed > 0 nodes"
    );
    // journal_mode pragma should be readable on a libsql connection.
    assert!(db["journal_mode"].is_string() || db["journal_mode"].is_null());
}

/// Issue #82: `details=true` must surface raw counts + interpretation per
/// dimension, not just the scalar score, so callers don't have to compose
/// six separate tools to reproduce the breakdown.
#[tokio::test]
async fn test_health_detailed_includes_raw_signals() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_health",
        json!({ "details": true }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let dims = parsed.get("dimensions").expect("dimensions should exist");

    for dim in [
        "acyclicity",
        "depth",
        "equality",
        "redundancy",
        "modularity",
        "coverage_discipline",
    ] {
        let d = dims.get(dim).unwrap_or_else(|| panic!("missing {dim}"));
        assert!(
            d.get("score").is_some(),
            "{dim}: 'score' field missing in details view"
        );
        assert!(
            d.get("source").is_some(),
            "{dim}: 'source' formula attribution missing"
        );
    }

    // Specific raw signals that the issue called out as missing today.
    assert!(dims["equality"].get("gini").is_some());
    assert!(dims["equality"].get("interpretation").is_some());
    assert!(dims["acyclicity"].get("edges_in_cycles").is_some());
    assert!(dims["depth"].get("max_chain").is_some());
    assert!(dims["depth"].get("ideal_chain").is_some());
    assert!(dims["modularity"].get("interpretation").is_some());
    assert!(dims["redundancy"].get("dead_count").is_some());
}

// ---------------------------------------------------------------------------
// tokensave_dsm
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dsm_stats() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_dsm",
        json!({ "format": "stats" }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("files").is_some(),
        "files field should exist, got: {}",
        text
    );
    assert!(
        parsed.get("density").is_some(),
        "density field should exist"
    );
}

#[tokio::test]
async fn test_dsm_clusters() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_dsm",
        json!({ "format": "clusters" }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        parsed.get("clusters").is_some(),
        "clusters array should exist, got: {}",
        text
    );
}

// ---------------------------------------------------------------------------
// tokensave_test_risk
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_test_risk() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_test_risk",
        json!({ "limit": 10 }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let summary = parsed.get("summary").expect("summary should exist");
    assert!(
        summary
            .get("total_functions")
            .and_then(|v| v.as_u64())
            .is_some_and(|v| v > 0),
        "total_functions should be > 0, got: {}",
        text
    );
    assert!(parsed.get("risks").is_some(), "risks array should exist");
}

// ---------------------------------------------------------------------------
// Session start / end tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_start() {
    let (cg, dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_session_start", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(output["quality_signal"].as_u64().is_some());
    assert_eq!(output["status"].as_str().unwrap(), "baseline_saved");
    let baseline_path = dir.path().join(".tokensave/session_baseline.json");
    assert!(baseline_path.exists(), "baseline file should exist");
}

#[tokio::test]
async fn test_session_end() {
    let (cg, dir) = setup_project().await;
    handle_tool_call(&cg, "tokensave_session_start", json!({}), None, None)
        .await
        .unwrap();
    let result = handle_tool_call(&cg, "tokensave_session_end", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(output["signal_before"].as_u64().is_some());
    assert!(output["signal_after"].as_u64().is_some());
    assert!(output["delta"].is_number());
    let baseline_path = dir.path().join(".tokensave/session_baseline.json");
    assert!(
        !baseline_path.exists(),
        "baseline should be removed after session_end"
    );
}

#[tokio::test]
async fn test_session_end_no_baseline() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_session_end", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(output["status"].as_str().unwrap(), "no_baseline");
}

// ---------------------------------------------------------------------------
// tokensave_body
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_body_returns_full_function_source() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_body",
        json!({"symbol": "format_greeting"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert_eq!(output["match_count"].as_u64().unwrap(), 1);
    let m = &output["matches"][0];
    let body = m["body"].as_str().unwrap();
    assert!(
        body.contains("fn format_greeting"),
        "body should contain the function signature, got: {body}"
    );
    assert!(
        body.contains("Hello"),
        "body should contain the function body, got: {body}"
    );
    // Regression for issue #62: the function's outer closing brace must be
    // included so the body is byte-exact usable as an Edit `old_string`.
    assert!(
        body.trim_end().ends_with('}'),
        "body should end with the function's closing brace, got: {body:?}"
    );
    // Line numbers are surfaced 1-based so they match what the user sees in
    // their editor and what Edit-style tools expect.
    let start_line = m["start_line"].as_u64().unwrap() as usize;
    let end_line = m["end_line"].as_u64().unwrap() as usize;
    assert!(start_line >= 1, "start_line should be 1-based");
    assert!(
        end_line >= start_line,
        "end_line should not precede start_line"
    );
    let file_rel = m["file"].as_str().unwrap();
    let file_abs = _dir.path().join(file_rel);
    let source = std::fs::read_to_string(&file_abs).unwrap();
    let lines: Vec<&str> = source.lines().collect();
    let end_line_text = lines
        .get(end_line - 1)
        .copied()
        .unwrap_or_else(|| panic!("end_line {end_line} out of bounds in {file_rel}"));
    assert!(
        end_line_text.trim_end().ends_with('}'),
        "end_line ({end_line}) should point at the closing brace; line text: {end_line_text:?}"
    );
}

#[tokio::test]
async fn test_body_unknown_symbol() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_body",
        json!({"symbol": "no_such_symbol_anywhere"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    assert!(
        text.contains("No symbol named"),
        "should report no match, got: {text}"
    );
}

#[tokio::test]
async fn test_body_missing_symbol_param() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_body", json!({}), None, None).await;
    assert!(result.is_err(), "should error when symbol is missing");
}

// ---------------------------------------------------------------------------
// tokensave_todos
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_todos_finds_markers() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        r#"
fn main() {
    // TODO: refactor this
    let x = 1;
    // FIXME: handle the error case
    let y = 2;
    println!("{} {}", x, y);
}

fn helper() {
    // not a marker: rendered todoist
    let _ = 0;
}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(&cg, "tokensave_todos", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let count = output["match_count"].as_u64().unwrap();
    assert_eq!(count, 2, "should find exactly TODO and FIXME, got: {text}");
    let kinds: Vec<&str> = output["markers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"TODO"));
    assert!(kinds.contains(&"FIXME"));
    let enclosing: Vec<&str> = output["markers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["enclosing"].as_str())
        .collect();
    assert!(
        enclosing.iter().any(|e| e.contains("main")),
        "TODO inside main should report main as enclosing, got: {enclosing:?}"
    );
}

#[tokio::test]
async fn test_todos_filters_by_kind() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        r#"
fn main() {
    // TODO: a
    // FIXME: b
    // HACK: c
    let _ = 0;
}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_todos",
        json!({"kinds": ["FIXME"]}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert_eq!(output["match_count"].as_u64().unwrap(), 1);
    assert_eq!(output["markers"][0]["kind"].as_str().unwrap(), "FIXME");
}

#[tokio::test]
async fn test_todos_empty_when_clean() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_todos", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert_eq!(output["match_count"].as_u64().unwrap(), 0);
}

// ---------------------------------------------------------------------------
// tokensave_callers_for — bulk caller lookup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_callers_for_returns_caller_set_per_id() {
    let (cg, _dir) = setup_project().await;

    // Look up two distinct targets in one call.
    let helper_id = find_node_id(&cg, "helper").await;
    let format_id = find_node_id(&cg, "format_greeting").await;

    let result = handle_tool_call(
        &cg,
        "tokensave_callers_for",
        json!({"node_ids": [helper_id.clone(), format_id.clone()]}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();

    // Response shape: { callers: { id: [...], id2: [...] }, truncated: bool, max_per_item: N }
    assert_eq!(output["truncated"], json!(false));
    assert!(output["max_per_item"].as_u64().unwrap() > 0);

    let callers = &output["callers"];
    let helper_callers = callers[&helper_id].as_array().unwrap();
    let format_callers = callers[&format_id].as_array().unwrap();

    // helper is called from main; format_greeting is called from helper.
    assert!(
        !helper_callers.is_empty(),
        "expected helper to have at least one caller"
    );
    assert!(
        !format_callers.is_empty(),
        "expected format_greeting to have at least one caller"
    );
}

#[tokio::test]
async fn test_callers_for_includes_unmatched_ids_as_empty() {
    let (cg, _dir) = setup_project().await;
    let helper_id = find_node_id(&cg, "helper").await;
    let bogus_id = "function:0000000000000000000000000000ffff".to_string();

    let result = handle_tool_call(
        &cg,
        "tokensave_callers_for",
        json!({"node_ids": [helper_id.clone(), bogus_id.clone()]}),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let callers = &output["callers"];
    assert!(callers[&bogus_id].as_array().unwrap().is_empty());
    assert!(!callers[&helper_id].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_callers_for_respects_max_per_item() {
    let (cg, _dir) = setup_project().await;
    let helper_id = find_node_id(&cg, "helper").await;
    // Cap at 0 — every caller should be marked truncated.
    let result = handle_tool_call(
        &cg,
        "tokensave_callers_for",
        json!({"node_ids": [helper_id.clone()], "max_per_item": 0}),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    assert_eq!(output["truncated"], json!(true));
    assert!(output["callers"][&helper_id].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_callers_for_rejects_empty_input() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_callers_for",
        json!({"node_ids": []}),
        None,
        None,
    )
    .await;
    let Err(err) = result else {
        panic!("expected error for empty node_ids");
    };
    assert!(format!("{err}").contains("non-empty"));
}

#[tokio::test]
async fn test_callers_for_rejects_unknown_kind() {
    let (cg, _dir) = setup_project().await;
    let helper_id = find_node_id(&cg, "helper").await;
    let result = handle_tool_call(
        &cg,
        "tokensave_callers_for",
        json!({"node_ids": [helper_id], "kind": "not_a_real_kind"}),
        None,
        None,
    )
    .await;
    let Err(err) = result else {
        panic!("expected error for unknown edge kind");
    };
    assert!(format!("{err}").contains("unknown edge kind"));
}

// ---------------------------------------------------------------------------
// tokensave_by_qualified_name — cross-run lookup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_by_qualified_name_finds_indexed_node() {
    let (cg, _dir) = setup_project().await;
    // Find the qualified name of `helper` first.
    let helper = cg
        .get_node(&find_node_id(&cg, "helper").await)
        .await
        .unwrap()
        .unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_by_qualified_name",
        json!({"qualified_name": helper.qualified_name}),
        None,
        None,
    )
    .await
    .unwrap();
    let items: Vec<Value> = serde_json::from_str(extract_text(&result.value)).unwrap();
    assert!(
        !items.is_empty(),
        "expected at least one match for helper qname"
    );
    assert!(items.iter().any(|i| i["name"] == "helper"));
    // The handler exposes attrs_start_line in the response shape.
    assert!(items[0].get("attrs_start_line").is_some());
}

#[tokio::test]
async fn test_by_qualified_name_returns_empty_for_unknown() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_by_qualified_name",
        json!({"qualified_name": "crate::does::not::exist"}),
        None,
        None,
    )
    .await
    .unwrap();
    let items: Vec<Value> = serde_json::from_str(extract_text(&result.value)).unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn test_by_qualified_name_requires_param() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(&cg, "tokensave_by_qualified_name", json!({}), None, None).await;
    let Err(err) = result else {
        panic!("expected error when qualified_name is missing");
    };
    assert!(format!("{err}").contains("qualified_name"));
}

// ---------------------------------------------------------------------------
// Memory handler tests (record_decision, record_code_area, session_recall)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_handle_record_decision() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_record_decision",
        json!({"text": "use JWT", "reason": "legal flagged sessions"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert!(
        output.get("id").is_some(),
        "response should contain 'id', got: {output}"
    );
    assert_eq!(
        output["status"].as_str().unwrap(),
        "recorded",
        "status should be 'recorded', got: {output}"
    );
}

#[tokio::test]
async fn test_handle_record_code_area() {
    let (cg, _dir) = setup_project().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_record_code_area",
        json!({"path": "src/auth.rs", "description": "OAuth provider"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        output["status"].as_str().unwrap(),
        "recorded",
        "status should be 'recorded', got: {output}"
    );
}

#[tokio::test]
async fn test_handle_session_recall_returns_recorded_decision() {
    let (cg, _dir) = setup_project().await;
    // Seed a decision first
    handle_tool_call(
        &cg,
        "tokensave_record_decision",
        json!({"text": "use JWT", "reason": "legal flagged sessions"}),
        None,
        None,
    )
    .await
    .unwrap();
    // Recall and verify the seeded decision appears
    let result = handle_tool_call(
        &cg,
        "tokensave_session_recall",
        json!({"query": "JWT"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let decisions = output["decisions"]
        .as_array()
        .expect("decisions should be an array");
    assert!(
        !decisions.is_empty(),
        "recall should return at least one decision after seeding"
    );
    let found = decisions
        .iter()
        .any(|d| d["text"].as_str().unwrap_or("").contains("JWT"));
    assert!(
        found,
        "seeded 'JWT' decision should appear in recall results"
    );
}

// ---------------------------------------------------------------------------
// Bug-report regressions: sonium-codebase issues
// ---------------------------------------------------------------------------

/// Regression for bug #1: `tokensave_body` should prefer the `fn foo()` over
/// a field/variant also named `foo`. Setup mirrors what sonium hit when
/// searching for `gmres`: the codebase has both a `pub fn gmres(...)` and a
/// struct field literally named `gmres`. The function — the body the user
/// actually wants — must outrank the field.
async fn setup_function_vs_field_collision() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub struct Solvers {
    pub gmres: u32,
}

pub fn gmres(x: u32) -> u32 {
    x + 1
}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn body_prefers_function_over_field_with_same_name() {
    let (cg, _dir) = setup_function_vs_field_collision().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_body",
        json!({"symbol": "gmres"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let matches = output["matches"].as_array().unwrap();
    let first = &matches[0];
    assert_eq!(
        first["kind"].as_str(),
        Some("function"),
        "first match should be the function definition, got {first}"
    );
    let body = first["body"].as_str().unwrap();
    assert!(
        body.contains("pub fn gmres"),
        "body should be the function source, got: {body}"
    );
}

/// Regression for bug #5: `tokensave_diff_context.impacted_symbols` must not
/// list the same downstream node more than once. The sonium report showed
/// the same id appearing 6+ times consecutively when several modified
/// symbols all reached the same dependent.
#[tokio::test]
async fn diff_context_dedupes_impacted_symbols() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    // Two functions in `mod.rs` both call `shared` in `dep.rs`. Without dedup,
    // `shared` appears twice in `impacted_symbols`.
    fs::write(
        project.join("src/lib.rs"),
        r#"
mod dep;
pub fn first() { dep::shared(); }
pub fn second() { dep::shared(); }
"#,
    )
    .unwrap();
    fs::write(project.join("src/dep.rs"), "pub fn shared() {}\n").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_diff_context",
        json!({"files": ["src/lib.rs"], "depth": 3}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let impacted = output["impacted_symbols"].as_array().unwrap();
    let mut ids: Vec<&str> = impacted.iter().filter_map(|v| v["id"].as_str()).collect();
    ids.sort();
    let before = ids.len();
    ids.dedup();
    let after = ids.len();
    assert_eq!(
        before, after,
        "impacted_symbols must not contain duplicates by id; got {before} entries, {after} unique"
    );
}

/// Regression for bug #6 / review P1: `tokensave_recursion` must preserve
/// genuine direct recursion while filtering length-1 self-edge artifacts.
#[tokio::test]
async fn recursion_keeps_direct_recursion() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn recurse(n: u32) -> u32 {
    if n == 0 { 0 } else { recurse(n - 1) }
}

pub fn nonrecursive() -> u32 { 42 }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(&cg, "tokensave_recursion", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycles = output["cycles"].as_array().unwrap();
    let has_recurse = cycles.iter().any(|cycle| {
        cycle["chain"].as_array().is_some_and(|chain| {
            chain
                .iter()
                .filter_map(|n| n["name"].as_str())
                .filter(|name| *name == "recurse")
                .count()
                >= 2
        })
    });
    assert!(
        has_recurse,
        "direct self-recursive function should be reported; got {cycles:?}"
    );
}

#[tokio::test]
async fn recursion_filters_self_edge_artifacts() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub struct Triplet {
    rows: Vec<usize>,
}

impl Triplet {
    pub fn push(&mut self, row: usize) {
        self.rows.push(row);
    }
}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(&cg, "tokensave_recursion", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycles = output["cycles"].as_array().unwrap();
    let mentions_push = cycles.iter().any(|cycle| {
        cycle["chain"]
            .as_array()
            .is_some_and(|chain| chain.iter().any(|n| n["name"].as_str() == Some("push")))
    });
    assert!(
        !mentions_push,
        "`self.rows.push(...)` should not be reported as recursive; got {cycles:?}"
    );
}

#[tokio::test]
async fn recursion_reports_real_cycle_path() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn a() { b(); }
pub fn b() { c(); }
pub fn c() { a(); }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(&cg, "tokensave_recursion", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycles = output["cycles"].as_array().unwrap();
    let chain = cycles
        .iter()
        .find_map(|cycle| {
            let chain = cycle["chain"].as_array()?;
            let names: Vec<&str> = chain.iter().filter_map(|n| n["name"].as_str()).collect();
            (names.len() == 4).then_some(names)
        })
        .expect("expected a three-node cycle path");
    let valid_edges = [("a", "b"), ("b", "c"), ("c", "a")];
    for pair in chain.windows(2) {
        assert!(
            valid_edges.contains(&(pair[0], pair[1])),
            "chain must follow real call edges; got {chain:?}"
        );
    }
}

/// Regression for bug #4: `tokensave_changelog`'s response must not list
/// directories under `files_not_indexed`. We construct a small git repo
/// with a real commit history that touches both a real file and a
/// (synthesised) directory path then verify the handler filters out the
/// directory.
#[tokio::test]
async fn changelog_filters_directory_paths() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "t@t"])
        .current_dir(project)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "t"])
        .current_dir(project)
        .output()
        .unwrap();
    fs::create_dir_all(project.join("src/sub")).unwrap();
    fs::write(project.join("src/sub/keep.rs"), "pub fn k() {}\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(project)
        .output()
        .unwrap();
    fs::write(
        project.join("src/sub/keep.rs"),
        "pub fn k() { let _ = 1; }\n",
    )
    .unwrap();
    fs::write(project.join("src/sub/added.rs"), "pub fn a() {}\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "two"])
        .current_dir(project)
        .output()
        .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    // Intentionally skipping `index_all` — the changelog handler reads from
    // git directly, not the index, and including the index sync subjects
    // this test to a pre-existing SyncLock contention flake.

    let result = handle_tool_call(
        &cg,
        "tokensave_changelog",
        json!({"from_ref": "HEAD~1", "to_ref": "HEAD"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let changed: Vec<&str> = output["changed_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for entry in &changed {
        let p = project.join(entry);
        assert!(
            !p.is_dir(),
            "changed_files must not include directories; got {entry:?}"
        );
    }
}

/// Regression for bug #8b: `tokensave_unused_imports` must actually flag
/// unused imports. The previous implementation tested `incoming.is_empty()`
/// for every Use node, but Use nodes always have at least one incoming
/// edge (from their containing module/file via Contains), so the
/// condition never fired and the tool returned 0 on every real codebase.
#[tokio::test]
async fn unused_imports_detects_truly_unused() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
use std::collections::HashMap;
use std::collections::HashSet;
mod inner;

pub fn used_one() -> HashMap<u32, u32> { HashMap::new() }
"#,
    )
    .unwrap();
    fs::write(project.join("src/inner.rs"), "pub fn inner_fn() {}\n").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let imports = output["imports"].as_array().unwrap();
    let names: Vec<&str> = imports.iter().filter_map(|u| u["name"].as_str()).collect();
    // `HashSet` is imported but never used in the file body.
    assert!(
        names.iter().any(|n| n.contains("HashSet")),
        "HashSet should be reported as unused; got names={names:?}"
    );
}

/// Regression for bug #8a: `tokensave_dead_code` must support `include_public`
/// so agents can audit pub items with no callers in the indexed scope. The
/// previous SQL hard-coded `visibility != 'public'`, so on a codebase that
/// is mostly `pub` the tool reported 0 dead symbols.
#[tokio::test]
async fn dead_code_with_include_public_finds_pub_unreferenced() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn called() {}
pub fn never_called_anywhere() {}
pub fn caller() { called(); }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let default_result = handle_tool_call(&cg, "tokensave_dead_code", json!({}), None, None)
        .await
        .unwrap();
    let default_text = extract_text(&default_result.value);
    let default_output: Value = serde_json::from_str(default_text).unwrap();
    assert_eq!(
        default_output["dead_code_count"].as_u64().unwrap_or(99),
        0,
        "default dead_code (no include_public) must still skip pub items"
    );

    let with_pub = handle_tool_call(
        &cg,
        "tokensave_dead_code",
        json!({"include_public": true}),
        None,
        None,
    )
    .await
    .unwrap();
    let with_pub_text = extract_text(&with_pub.value);
    let with_pub_output: Value = serde_json::from_str(with_pub_text).unwrap();
    let symbols: Vec<&str> = with_pub_output["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(
        symbols.contains(&"never_called_anywhere"),
        "with include_public, the pub unreferenced fn should appear; got {symbols:?}"
    );
}

/// Regression for bug #7: `build_file_adjacency` previously included
/// `implements` and `extends` edges, which are heavily resolver-fuzzy-bound
/// to nonsense targets in unrelated files. After the fix, only `uses` and
/// `calls` edges count for file-level dependency depth.
#[tokio::test]
async fn dependency_depth_excludes_implements_and_extends() {
    // Public helper exposed from the lib for unit-test inspection.
    use tokensave::graph::queries::GraphQueryManager;
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    // file_a derives Debug — extractor emits derives_macro and the
    // resolver historically pollutes implements edges across files.
    fs::write(
        project.join("src/lib.rs"),
        r#"
mod a;
mod b;
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/a.rs"),
        r#"
#[derive(Debug, Clone)]
pub struct A;
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/b.rs"),
        r#"
pub trait T {}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let qm = GraphQueryManager::new(cg.db());
    let adj = qm.build_file_adjacency(None).await.unwrap();
    // Neither a.rs nor b.rs imports the other; the only edges between
    // them would come from implements/extends junk. After the fix, adj
    // should report no cross-file deps between the two leaf files.
    let from_a = adj.get("src/a.rs").cloned().unwrap_or_default();
    let from_b = adj.get("src/b.rs").cloned().unwrap_or_default();
    assert!(
        !from_a.contains("src/b.rs"),
        "src/a.rs must not depend on src/b.rs; got adj={from_a:?}"
    );
    assert!(
        !from_b.contains("src/a.rs"),
        "src/b.rs must not depend on src/a.rs; got adj={from_b:?}"
    );
}

/// Regression: `tokensave_run_affected_tests` must dispatch the test
/// functions that are themselves in `changed_paths`. Previously the
/// handler walked callers of every node in the changed file — but
/// `#[test]` functions are leaves with no callers, so a PR that only
/// edits `tests/foo.rs` would return "no tests cover the changed
/// paths" and skip running anything.
#[tokio::test]
async fn run_affected_tests_dispatches_directly_changed_test_files() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("tests")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn util() -> u32 { 1 }\n").unwrap();
    fs::write(
        project.join("Cargo.toml"),
        r#"[package]
name = "t"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(
        project.join("tests/edited_only.rs"),
        r#"
#[test]
fn edited_only_test() {
    assert_eq!(2, 2);
}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_run_affected_tests",
        json!({
            "changed_paths": ["tests/edited_only.rs"],
            "timeout_secs": 60,
            "max_tests": 5
        }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    // If no tests get dispatched the handler short-circuits with a
    // note: "no tests cover the changed paths (1 file(s))". After the
    // fix, the test in the edited file itself must be dispatched.
    let dispatched = output["dispatched_tests"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        dispatched.iter().any(|n| n.contains("edited_only_test")),
        "expected edited_only_test to be dispatched; got dispatched={dispatched:?} note={:?}",
        output["note"]
    );
}

/// Regression: `tokensave_diagnose` must normalize span paths before
/// looking them up in the graph. cargo emits absolute and (on Windows)
/// backslash-separated paths; the graph stores project-relative,
/// forward-slash paths. Without normalization a diagnostic with span
/// `/abs/path/to/project/src/lib.rs:42:1` or `src\lib.rs:42:1` resolves
/// to `node: null` even though the file is indexed.
#[tokio::test]
async fn diagnose_normalizes_absolute_and_backslash_paths() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn target() {}\n").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let abs_path = project.join("src/lib.rs");
    let abs_str = abs_path.to_string_lossy().to_string();
    let backslash_str = "src\\lib.rs";
    let cargo_output = format!(
        "error[E0001]: synthetic error\n  --> {abs_str}:1:1\n   |\n\nerror[E0002]: backslash form\n  --> {backslash_str}:1:1\n   |\n"
    );

    let result = handle_tool_call(
        &cg,
        "tokensave_diagnose",
        json!({"cargo_output": cargo_output, "include_callers": false}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let mapped = output["mapped_to_node"].as_u64().unwrap_or(0);
    assert_eq!(
        mapped, 2,
        "both diagnostics should map to nodes after path normalization; got mapped={mapped} full={output:#}"
    );
}

/// Regression: PR8's resolver kind-compatibility filter must apply to
/// the same-file blocklist branches too. Without it, common names like
/// `new`/`default`/`clone` can still bind a `Calls` reference to a
/// non-callable same-file symbol — e.g. a const literally named
/// `default` — when it's the only same-file match for a blocklisted
/// name.
#[tokio::test]
async fn resolver_blocklist_branch_respects_kind_filter() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    // Use a struct named after a blocklisted identifier ("new") plus a
    // call site that the parser definitely treats as a call_expression.
    // Pre-fix the resolver's same-file blocklist branch would bind the
    // Calls ref to this struct because no other "new" lives in the file.
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub struct new;

pub fn caller() {
    let _ = new();
    helper();
}

pub fn helper() {}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let caller_id = find_node_id(&cg, "caller").await;
    let result = handle_tool_call(
        &cg,
        "tokensave_callees",
        json!({"node_id": caller_id, "max_depth": 1, "resolve_dispatch": false}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let items: Value = serde_json::from_str(text).unwrap();
    let arr = items.as_array().unwrap();
    for entry in arr {
        let kind = entry["kind"].as_str().unwrap_or("");
        let name = entry["name"].as_str().unwrap_or("");
        let callable = matches!(
            kind,
            "function" | "method" | "struct_method" | "constructor" | "macro" | "arrow_function"
        );
        assert!(
            callable,
            "caller's callees must be callable kinds; got name={name} kind={kind} full={arr:#?}"
        );
    }
}

/// Regression for bug #11: when an `impl Trait for X` reference cannot
/// resolve to a real trait node (e.g. `Default` lives in std and isn't
/// indexed), the resolver MUST NOT fuzzy-bind it to an unrelated node
/// kind. The sonium codebase had a parser `Token` enum whose `Default`
/// variant became the target of 150 stray `implements` edges from
/// manual `impl Default for X` blocks, completely poisoning
/// `tokensave_rank --edge-kind implements`. Implements/Extends/derives
/// references must only resolve to trait-shaped targets.
#[tokio::test]
async fn implements_refs_dont_resolve_to_enum_variants() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub enum Token { Default, Plus }

pub struct A;
impl Default for A { fn default() -> Self { A } }

pub struct B;
impl Default for B { fn default() -> Self { B } }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_rank",
        json!({"edge_kind": "implements", "direction": "incoming"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let ranking = output["ranking"].as_array().unwrap();
    for entry in ranking {
        let kind = entry["kind"].as_str().unwrap_or("");
        let name = entry["name"].as_str().unwrap_or("");
        assert!(
            kind != "enum_variant" && kind != "field",
            "implements edges must not target {kind} (got name={name})"
        );
    }
}

/// Regression for bug #10: `tokensave_circular` must report one entry per
/// strongly-connected component, not every walk through the cycle. The
/// sonium codebase had 73 "cycles" that were all different DFS paths
/// through the same SCC. After the SCC refactor, the same data yields
/// one entry per genuine component.
#[tokio::test]
async fn circular_reports_one_entry_per_scc_not_per_walk() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    // Three-file cycle: a uses b, b uses c, c uses a. Multiple DFS walks
    // through this triangle would have reported 3+ "cycles" pre-fix
    // (a→b→c→a, b→c→a→b, c→a→b→c).
    fs::write(project.join("src/lib.rs"), "mod a; mod b; mod c;\n").unwrap();
    fs::write(
        project.join("src/a.rs"),
        "use crate::b::b_fn;\npub fn a_fn() { b_fn(); }\n",
    )
    .unwrap();
    fs::write(
        project.join("src/b.rs"),
        "use crate::c::c_fn;\npub fn b_fn() { c_fn(); }\n",
    )
    .unwrap();
    fs::write(
        project.join("src/c.rs"),
        "use crate::a::a_fn;\npub fn c_fn() { a_fn(); }\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(&cg, "tokensave_circular", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycle_count = output["cycle_count"].as_u64().unwrap();
    assert_eq!(
        cycle_count, 1,
        "three-file SCC must report exactly one cycle entry, got {cycle_count}"
    );
    let cycle = output["cycles"][0].as_array().unwrap();
    assert_eq!(
        cycle.len(),
        3,
        "the cycle should list all three files in the SCC; got {cycle:?}"
    );
}

/// Regression for bug #12: `tokensave_port_order`'s `cycles` output must
/// expose the SCCs forming each cycle separately, instead of collapsing
/// all unsorted nodes into a single mega-blob. Without this, on a real
/// codebase the cycle entry contained 200+ unrelated symbols and the
/// agent had no way to know what to break first.
#[tokio::test]
async fn port_order_reports_separate_scc_groups() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    // Two disjoint mutually-recursive pairs: (a, b) and (c, d). Before
    // the fix, both pairs would be lumped into a single "Mutual
    // dependency" entry. After the fix, each pair appears as its own
    // cycle group.
    fs::write(project.join("src/lib.rs"), "pub mod m;\n").unwrap();
    fs::write(
        project.join("src/m.rs"),
        r#"
pub fn a() { b(); }
pub fn b() { a(); }
pub fn c() { d(); }
pub fn d() { c(); }
pub fn leaf() {}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_port_order",
        json!({"source_dir": "src"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycles = output["cycles"].as_array().unwrap();
    assert!(
        cycles.len() >= 2,
        "expected at least 2 disjoint cycle groups; got {} entries: {cycles:?}",
        cycles.len()
    );
    // No cycle entry should mix both (a,b) and (c,d) names — that would
    // mean the fix didn't actually separate them. (Each symbol is now an
    // object: {name, kind, file, line, in_cycle_out_degree, ...}.)
    for c in cycles {
        let names: Vec<&str> = c["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["name"].as_str().or_else(|| s.as_str()))
            .collect();
        let has_ab = names.iter().any(|n| *n == "a" || *n == "b");
        let has_cd = names.iter().any(|n| *n == "c" || *n == "d");
        assert!(
            !(has_ab && has_cd),
            "one cycle entry contains both SCCs (a/b mixed with c/d): {names:?}"
        );
    }
}

/// Regression for new bug-report batch (#25): `tokensave_port_order` must
/// expose intra-cycle ordering signals so an agent can pick a starting
/// point inside a 200-symbol SCC instead of staring at an undifferentiated
/// blob. We expect each cycle entry to carry per-symbol in-cycle degree
/// data, a file-level member-count breakdown, and explicit `entry_point`
/// / `break_point_candidate` suggestions.
#[tokio::test]
async fn port_order_provides_intra_cycle_ordering() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    // a → b → c → a, plus a "hub" h that all three call into and that
    // calls a back. h is the central node (highest in-cycle in-degree).
    fs::write(project.join("src/lib.rs"), "pub mod m;\n").unwrap();
    fs::write(
        project.join("src/m.rs"),
        r#"
pub fn a() { b(); h(); }
pub fn b() { c(); h(); }
pub fn c() { a(); h(); }
pub fn h() { a(); }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_port_order",
        json!({"source_dir": "src"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycles = output["cycles"].as_array().unwrap();
    assert!(!cycles.is_empty(), "expected at least one cycle");
    let cycle = &cycles[0];
    assert!(
        cycle["files"].as_array().is_some(),
        "cycle must carry a `files` breakdown"
    );
    let files_arr = cycle["files"].as_array().unwrap();
    for f in files_arr {
        assert!(
            f.is_object() && f["members_in_cycle"].as_u64().is_some(),
            "files entries must be objects with `members_in_cycle`, got {f}"
        );
    }
    let symbols = cycle["symbols"].as_array().unwrap();
    for s in symbols {
        assert!(
            s["in_cycle_out_degree"].as_u64().is_some(),
            "each symbol must report in_cycle_out_degree; got {s}"
        );
        assert!(
            s["in_cycle_in_degree"].as_u64().is_some(),
            "each symbol must report in_cycle_in_degree; got {s}"
        );
    }
    assert!(
        cycle["entry_point"].is_object(),
        "cycle must surface a suggested entry_point; got {cycle}"
    );
    assert!(
        cycle["break_point_candidate"].is_object(),
        "cycle must surface a break_point_candidate; got {cycle}"
    );
    // The break point should be `h` (most internal callers).
    assert_eq!(
        cycle["break_point_candidate"]["name"].as_str(),
        Some("h"),
        "break_point_candidate should be the hub function `h`; got {cycle}"
    );
}

/// Regression for the Sonium port-order report: self-edges from fuzzy
/// resolution (`self.rows.push(...)` inside a method named `push`) should
/// not make singleton symbols appear as cycles.
#[tokio::test]
async fn port_order_ignores_self_edges() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub mod m;\n").unwrap();
    fs::write(
        project.join("src/m.rs"),
        r#"
pub struct Triplet {
    rows: Vec<usize>,
}

impl Triplet {
    pub fn push(&mut self, row: usize) {
        self.rows.push(row);
    }
}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_port_order",
        json!({"source_dir": "src"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycles = output["cycles"].as_array().unwrap();
    assert!(
        cycles.is_empty(),
        "self-edge-only methods should stay out of port_order cycles: {cycles:?}"
    );
}

/// Regression for bug #9: `tokensave_inheritance_depth` must surface Rust
/// supertrait chains (`trait T: U`) as `Extends` edges.
#[tokio::test]
async fn inheritance_depth_walks_rust_supertraits() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub trait Base {}
pub trait Middle: Base {}
pub trait Leaf: Middle {}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(&cg, "tokensave_inheritance_depth", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let ranking = output["ranking"].as_array().unwrap();
    let names: Vec<&str> = ranking.iter().filter_map(|r| r["name"].as_str()).collect();
    assert!(
        names.contains(&"Leaf"),
        "expected Leaf trait in inheritance_depth ranking; got {names:?}"
    );
    let leaf = ranking
        .iter()
        .find(|r| r["name"].as_str() == Some("Leaf"))
        .unwrap();
    let depth = leaf["depth"].as_u64().unwrap();
    assert!(depth >= 2, "Leaf depth should be >= 2 hops, got {depth}");
}

/// Regression for new bug-report batch (#26): `tokensave_circular` must
/// emit *disjoint* SCCs — no file should appear in more than one cycle
/// entry. The sonium run reported 216 cycles "sharing long tails", which
/// would only be possible if the SCC condensation step were broken. This
/// stress test wires up many disjoint cycles plus DAG-style tails between
/// them and asserts no file leaks into a second cycle entry.
#[tokio::test]
async fn circular_emits_disjoint_sccs_under_load() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let mut lib_rs = String::new();
    // Build 5 disjoint 3-file cycles with shared DAG tails between them.
    // Cycle k = (a_k -> b_k -> c_k -> a_k); plus a one-way edge from c_k
    // to a_{k+1} that introduces a non-cyclic "shared tail" between the
    // SCCs. Tarjan must still emit each cycle as its own SCC.
    for k in 0..5 {
        lib_rs.push_str(&format!("pub mod a{k};\npub mod b{k};\npub mod c{k};\n"));
    }
    fs::write(project.join("src/lib.rs"), lib_rs).unwrap();
    for k in 0..5 {
        let next = (k + 1) % 5;
        fs::write(
            project.join(format!("src/a{k}.rs")),
            format!("use crate::b{k}::b_fn;\npub fn a_fn() {{ b_fn(); }}\n"),
        )
        .unwrap();
        fs::write(
            project.join(format!("src/b{k}.rs")),
            format!("use crate::c{k}::c_fn;\npub fn b_fn() {{ c_fn(); }}\n"),
        )
        .unwrap();
        fs::write(
            project.join(format!("src/c{k}.rs")),
            format!(
                "use crate::a{k}::a_fn;\nuse crate::a{next}::a_fn as next_a;\npub fn c_fn() {{ a_fn(); next_a(); }}\n"
            ),
        )
        .unwrap();
    }
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(&cg, "tokensave_circular", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let cycles = output["cycles"].as_array().unwrap();
    // All cycles forming one giant SCC since c_k → a_{k+1} chains them.
    // The critical invariant is *disjointness*: no file appears twice.
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    for cycle in cycles {
        let files = cycle.as_array().unwrap();
        for f in files {
            let s = f.as_str().unwrap().to_string();
            assert!(
                seen.insert(s.clone()),
                "file {s} appears in more than one cycle entry; SCCs must be disjoint"
            );
        }
    }
}

/// Regression for new bug-report batch (#24): `tokensave_diff_context`'s
/// `modified_symbols` must dedup by node id, even when callers pass the
/// same path multiple times in `files`. The sonium run showed an
/// `hmatrix.rs` file node listed 7× in a row because the caller had the
/// same file path duplicated upstream.
#[tokio::test]
async fn diff_context_dedupes_modified_symbols_on_duplicate_input() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub struct S; pub fn one() {} pub fn two() {}\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_diff_context",
        json!({"files": ["src/lib.rs", "src/lib.rs", "src/lib.rs"], "depth": 1}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let modified = output["modified_symbols"].as_array().unwrap();
    let mut ids: Vec<&str> = modified.iter().filter_map(|v| v["id"].as_str()).collect();
    let before = ids.len();
    ids.sort();
    ids.dedup();
    let after = ids.len();
    assert_eq!(
        before, after,
        "modified_symbols must not contain duplicate ids even when input has the same file 3×; got {before} entries, {after} unique"
    );
}

/// Regression for new bug-report batch (#23): when a whole subtree is
/// removed in a diff, `tokensave_changelog` must not report the deleted
/// directory under `files_not_indexed`. The previous `is_dir()` filter
/// missed this case because the path was gone from disk by the time we
/// checked. The fix uses gix's `entry_mode` flag to skip tree entries
/// before they're ever pushed into the change list.
#[tokio::test]
async fn changelog_filters_deleted_directory_entries() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fn git(cwd: &std::path::Path, args: &[&str]) {
        std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap_or_else(|_| panic!("git {args:?} failed"));
    }
    git(project, &["init"]);
    git(project, &["config", "user.email", "t@t"]);
    git(project, &["config", "user.name", "t"]);
    fs::create_dir_all(project.join("crates/sub")).unwrap();
    fs::write(project.join("crates/sub/keep.rs"), "pub fn k() {}\n").unwrap();
    fs::write(project.join("main.rs"), "fn main() {}\n").unwrap();
    git(project, &["add", "."]);
    git(project, &["commit", "-m", "init"]);
    // Remove the whole subtree so gix's tree-diff yields a directory-mode
    // deletion entry.
    fs::remove_dir_all(project.join("crates")).unwrap();
    git(project, &["add", "-A"]);
    git(project, &["commit", "-m", "drop crates"]);
    let cg = TokenSave::init(project).await.unwrap();
    // Intentionally skipping `index_all` — the changelog handler reads from
    // git directly and the sync lock has a pre-existing parallel-test flake.
    let result = handle_tool_call(
        &cg,
        "tokensave_changelog",
        json!({"from_ref": "HEAD~1", "to_ref": "HEAD"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let changed: Vec<String> = output["changed_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let problematic: Vec<&String> = changed.iter().filter(|p| !p.ends_with(".rs")).collect();
    assert!(
        problematic.is_empty(),
        "changed_files should be file paths only (no directories like 'crates' or 'crates/sub'); got problematic={problematic:?} full={changed:?}"
    );
}

/// Regression for new bug-report batch (#22): `tokensave_pr_context` must
/// NOT explode Cargo.toml (or any .toml/.yaml/.json config file) into one
/// symbol per `[name]`, `[version]`, `[dependencies]` key. On real PRs a
/// Cargo.toml change with ~30 dependency lines produced ~70 entries that
/// pushed the response past 760k tokens. Config files should collapse to
/// a single summary symbol.
#[tokio::test]
async fn pr_context_collapses_cargo_toml_keys() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fn git(cwd: &std::path::Path, args: &[&str]) {
        std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap_or_else(|_| panic!("git {args:?} failed"));
    }
    git(project, &["init"]);
    git(project, &["config", "user.email", "t@t"]);
    git(project, &["config", "user.name", "t"]);
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
    )
    .unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
    git(project, &["add", "."]);
    git(project, &["commit", "-m", "init"]);
    // Second commit: bloat Cargo.toml with many deps.
    let mut bloated = String::from(
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
    );
    for i in 0..50 {
        bloated.push_str(&format!("dep{i} = \"0.1.{i}\"\n"));
    }
    fs::write(project.join("Cargo.toml"), &bloated).unwrap();
    git(project, &["add", "."]);
    git(project, &["commit", "-m", "deps"]);

    let cg = TokenSave::init(project).await.unwrap();
    // Intentionally skipping `index_all()` — pr_context reads the diff
    // from git directly and classifies Cargo.toml as `config` before any
    // index lookup, so we don't need the index to verify the collapse
    // behaviour. Calling `index_all()` here triggers the pre-existing
    // SyncLock parallel-test flake (#test_changelog_with_real_git).

    let result = handle_tool_call(
        &cg,
        "tokensave_pr_context",
        json!({"base_ref": "HEAD~1", "head_ref": "HEAD"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let added = output["added"].as_array().unwrap();
    let modified = output["modified"].as_array().unwrap();
    let count_cargo = |arr: &[Value]| -> usize {
        arr.iter()
            .filter(|v| v["file"].as_str() == Some("Cargo.toml"))
            .count()
    };
    let cargo_total = count_cargo(added) + count_cargo(modified);
    assert!(
        cargo_total <= 1,
        "Cargo.toml should collapse to at most one summary symbol; got {cargo_total} entries. added={added:?}, modified={modified:?}"
    );
    // And the surviving entry must be a config summary, not a regular key.
    let summary = modified
        .iter()
        .find(|v| v["file"].as_str() == Some("Cargo.toml"));
    assert!(
        summary.is_some(),
        "expected one config_summary entry for Cargo.toml in modified; got {modified:?}"
    );
    assert_eq!(
        summary.unwrap()["kind"].as_str(),
        Some("config_summary"),
        "Cargo.toml entry should be kind=config_summary"
    );
}

/// Regression for new bug-report batch (#21): `tokensave_unused_imports`
/// must flag genuinely unused identifiers inside grouped `use foo::{A, B}`
/// imports. Real-world Rust style is dominated by grouped imports
/// (`use std::collections::{HashMap, HashSet, BTreeMap};`); without
/// per-identifier splitting, the heuristic could never flag anything from
/// a grouped import, which is why the user's run reported 0 / 3,404 use
/// nodes.
#[tokio::test]
async fn unused_imports_handles_grouped_use() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
use std::collections::{HashMap, HashSet};

pub fn used() -> HashMap<u32, u32> { HashMap::new() }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let imports = output["imports"].as_array().unwrap();
    let payloads: Vec<String> = imports
        .iter()
        .map(|u| {
            format!(
                "{}::{}",
                u["name"].as_str().unwrap_or(""),
                u["unused"].as_str().unwrap_or("")
            )
        })
        .collect();
    let mentions_hashset = imports.iter().any(|u| {
        u["unused"].as_str().is_some_and(|s| s.contains("HashSet"))
            || u["name"].as_str().is_some_and(|n| n.contains("HashSet"))
    });
    assert!(
        mentions_hashset,
        "HashSet from grouped use should be reported as unused; got {payloads:?}"
    );
    // Critically, the *used* identifier HashMap must NOT be reported. If the
    // handler treats the whole grouped use as one opaque identifier it'll
    // either flag both or neither — both modes are wrong.
    let any_falsely_flags_hashmap = imports
        .iter()
        .any(|u| u["unused"].as_str().is_some_and(|s| s == "HashMap"));
    assert!(
        !any_falsely_flags_hashmap,
        "HashMap is used (HashMap::new()) and must not appear in `unused`; got {payloads:?}"
    );
}

/// Regression for new bug-report batch (#20): `tokensave_dead_code` must not
/// consider non-reference edges like `annotates` or `derives_macro` as
/// "this function is alive" evidence. Previously, a private helper with no
/// callers but an `#[inline]` (or any other attribute) on it had an
/// incoming `annotates` edge from the synthesised annotation_usage node,
/// which the SQL `NOT EXISTS (target = id AND kind != 'contains')` filter
/// accepted as a live reference. Real-world Rust codebases use attributes
/// pervasively, which is why the user's run found zero dead functions
/// across 5,715.
#[tokio::test]
async fn dead_code_flags_unreferenced_fn_with_attribute() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
fn caller() {
    used_helper();
}

#[inline]
fn used_helper() {}

#[inline]
fn dead_helper_with_attr() {}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(&cg, "tokensave_dead_code", json!({}), None, None)
        .await
        .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap();
    let symbols = output["symbols"].as_array().unwrap();
    let names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        names.contains(&"dead_helper_with_attr"),
        "private fn with #[inline] and no callers should be dead; got {names:?}"
    );
    assert!(
        !names.contains(&"used_helper"),
        "used_helper has a real caller and must NOT appear; got {names:?}"
    );
}

/// Regression for new bug-report batch (#19): `tokensave_search` must rank
/// trait/struct/function definitions above `use` re-exports of the same name.
/// Previously, several `use foo::LinearOperator;` lines could outrank the
/// `pub trait LinearOperator { … }` definition because BM25 scored short
/// re-export rows highly. We now force a kind tier ahead of BM25 score so a
/// real def always beats `use` rows.
#[tokio::test]
async fn search_ranks_trait_definition_above_use_reexports() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src/a")).unwrap();
    fs::create_dir_all(project.join("src/b")).unwrap();
    fs::create_dir_all(project.join("src/c")).unwrap();
    fs::create_dir_all(project.join("src/d")).unwrap();
    fs::create_dir_all(project.join("src/e")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub mod operator;
pub mod a;
pub mod b;
pub mod c;
pub mod d;
pub mod e;
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/operator.rs"),
        "pub trait LinearOperator { fn apply(&self); }\n",
    )
    .unwrap();
    for sub in ["a", "b", "c", "d", "e"] {
        fs::write(
            project.join(format!("src/{sub}/mod.rs")),
            "pub use crate::operator::LinearOperator;\n",
        )
        .unwrap();
    }
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let result = handle_tool_call(
        &cg,
        "tokensave_search",
        json!({"query": "LinearOperator", "limit": 10}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let items: Value = serde_json::from_str(text).unwrap();
    let arr = items.as_array().unwrap();
    let first_kind = arr[0]["kind"].as_str().unwrap_or("");
    assert_eq!(
        first_kind, "trait",
        "first search hit for LinearOperator should be the trait definition, got '{first_kind}' (full: {arr:?})"
    );
}

// ---------------------------------------------------------------------------
// McpServer::refresh_file_token_map
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_file_token_map_picks_up_new_files() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path();
    std::fs::write(project.join("a.rs"), "fn a() {}").unwrap();

    let cg = tokensave::tokensave::TokenSave::init(project)
        .await
        .unwrap();
    cg.sync().await.unwrap();

    let server = tokensave::mcp::McpServer::new(cg, None).await;
    let initial_map = server.file_token_map_snapshot();
    let initial_keys: std::collections::HashSet<_> = initial_map.keys().cloned().collect();

    // Add a new file, sync it, then refresh.
    std::fs::write(project.join("b.rs"), "fn b() { let y = 2; }").unwrap();
    let cg2 = tokensave::tokensave::TokenSave::open(project)
        .await
        .unwrap();
    cg2.sync().await.unwrap();

    server.refresh_file_token_map().await;
    let after_map = server.file_token_map_snapshot();
    let after_keys: std::collections::HashSet<_> = after_map.keys().cloned().collect();

    assert!(
        after_keys.len() > initial_keys.len(),
        "refresh should pick up b.rs"
    );
}

// ---------------------------------------------------------------------------
// McpServer-owned embedded watcher
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_owns_watcher_and_refreshes_token_map_on_change() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path();
    std::fs::write(project.join("a.rs"), "fn a() {}").unwrap();

    let cg = tokensave::tokensave::TokenSave::init(project)
        .await
        .unwrap();
    cg.sync().await.unwrap();

    let server = tokensave::mcp::McpServer::new(cg, None).await;

    // `McpServer::new` returns immediately and the embedded watcher attaches
    // on a background task (#84). Wait for it to register before writing —
    // FSEvents/inotify only deliver events that happen *after* the watch
    // is attached, so a write that lands during the attach window is lost.
    assert!(
        server
            .wait_for_watcher_attached(std::time::Duration::from_secs(10))
            .await,
        "embedded watcher should attach within 10s"
    );

    let initial_count = server.file_token_map_snapshot().len();

    // Edit a file. The embedded watcher should debounce + sync + refresh.
    std::fs::write(project.join("b.rs"), "fn b() {}").unwrap();

    // Wait for debounce + sync + refresh with a generous ceiling.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while server.file_token_map_snapshot().len() <= initial_count
        && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let after_count = server.file_token_map_snapshot().len();
    assert!(
        after_count > initial_count,
        "embedded watcher should have refreshed map ({initial_count} -> {after_count})"
    );

    server.shutdown().await;
}
