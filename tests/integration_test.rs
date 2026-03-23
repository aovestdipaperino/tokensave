use codegraph::codegraph::CodeGraph;
use codegraph::types::EdgeKind;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_full_pipeline() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    // Create a small Rust project
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

    // Init
    let cg = CodeGraph::init(project).await.unwrap();

    // Index
    let index_result = cg.index_all().await.unwrap();
    assert!(index_result.file_count > 0, "should index files");
    assert!(index_result.node_count > 0, "should extract nodes");

    // Stats
    let stats = cg.get_stats().await.unwrap();
    assert!(stats.node_count > 0);
    assert!(stats.file_count >= 2);

    // Search
    let results = cg.search("helper", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'helper'");
    assert!(results.iter().any(|r| r.node.name == "helper"));

    // Edges should exist (at minimum Contains edges from file -> items)
    let stats = cg.get_stats().await.unwrap();
    assert!(stats.edge_count > 0, "should have edges");
}

#[tokio::test]
async fn test_incremental_sync() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn original() {}\n").unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // Verify original function exists
    let results = cg.search("original", 10).await.unwrap();
    assert!(!results.is_empty());

    // Modify file
    fs::write(
        project.join("src/lib.rs"),
        "pub fn modified() {}\npub fn added() {}\n",
    )
    .unwrap();

    // Sync
    let sync_result = cg.sync().await.unwrap();
    assert!(
        sync_result.files_modified > 0 || sync_result.files_added > 0,
        "sync should detect changes: modified={}, added={}",
        sync_result.files_modified,
        sync_result.files_added
    );

    // Should find the new function
    let results = cg.search("modified", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'modified' after sync");
}

#[tokio::test]
async fn test_init_and_open() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    assert!(!CodeGraph::is_initialized(project));
    CodeGraph::init(project).await.unwrap();
    assert!(CodeGraph::is_initialized(project));

    // Open existing project
    let cg = CodeGraph::open(project).await;
    assert!(cg.is_ok());
}

#[tokio::test]
async fn test_search_empty_index() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let cg = CodeGraph::init(project).await.unwrap();
    let results = cg.search("anything", 10).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_stats_empty_index() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let cg = CodeGraph::init(project).await.unwrap();
    let stats = cg.get_stats().await.unwrap();
    assert_eq!(stats.node_count, 0);
    assert_eq!(stats.edge_count, 0);
    assert_eq!(stats.file_count, 0);
}

#[tokio::test]
async fn test_context_building() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
/// Processes incoming data.
pub fn process_data(input: &str) -> String {
    input.to_uppercase()
}
"#,
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let options = codegraph::types::BuildContextOptions::default();
    let context = cg.build_context("process_data function", &options).await.unwrap();
    assert!(
        !context.entry_points.is_empty(),
        "should find entry points for 'process_data'"
    );
}

#[tokio::test]
async fn test_struct_and_impl_extraction() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}
"#,
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    let result = cg.index_all().await.unwrap();
    // File node + Point struct + x field + y field + impl Point + new method + distance method = 7+
    assert!(
        result.node_count >= 5,
        "should extract Point, x, y, new, distance (got {})",
        result.node_count
    );

    // Search for struct
    let results = cg.search("Point", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'Point'");

    // Search for method
    let results = cg.search("distance", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'distance'");
}

#[tokio::test]
async fn test_file_removal_sync() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn keep() {}\n").unwrap();
    fs::write(project.join("src/remove_me.rs"), "pub fn gone() {}\n").unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // Verify both exist
    let stats = cg.get_stats().await.unwrap();
    assert!(
        stats.file_count >= 2,
        "should have at least 2 files indexed"
    );

    // Remove file
    fs::remove_file(project.join("src/remove_me.rs")).unwrap();

    // Sync
    let sync_result = cg.sync().await.unwrap();
    assert_eq!(sync_result.files_removed, 1, "should detect 1 removed file");

    // Verify removed function is gone
    let results = cg.search("gone", 10).await.unwrap();
    assert!(results.is_empty(), "'gone' should no longer be found");
}

#[tokio::test]
async fn test_index_all_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub fn alpha() {}\npub fn beta() {}\n",
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();

    let result1 = cg.index_all().await.unwrap();
    let stats1 = cg.get_stats().await.unwrap();

    let result2 = cg.index_all().await.unwrap();
    let stats2 = cg.get_stats().await.unwrap();

    assert_eq!(
        result1.file_count, result2.file_count,
        "re-indexing should produce the same file count"
    );
    assert_eq!(
        stats1.node_count, stats2.node_count,
        "re-indexing should produce the same node count"
    );
}

#[tokio::test]
async fn test_sync_no_changes() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn stable() {}\n").unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // Sync without any changes
    let sync_result = cg.sync().await.unwrap();
    assert_eq!(sync_result.files_added, 0);
    assert_eq!(sync_result.files_modified, 0);
    assert_eq!(sync_result.files_removed, 0);
}

#[tokio::test]
async fn test_search_by_docstring() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
/// Calculates the fibonacci sequence.
pub fn fibonacci(n: u64) -> u64 {
    if n <= 1 { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
}
"#,
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // Search by the docstring content
    let results = cg.search("fibonacci", 10).await.unwrap();
    assert!(
        !results.is_empty(),
        "should find node via docstring/name search"
    );
}

#[tokio::test]
async fn test_multiple_files_cross_reference() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub mod models;
pub mod services;
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/models.rs"),
        r#"
pub struct User {
    pub name: String,
    pub email: String,
}
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/services.rs"),
        r#"
use crate::models::User;

pub fn create_user(name: &str, email: &str) -> String {
    format!("{}:{}", name, email)
}
"#,
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    let result = cg.index_all().await.unwrap();
    assert_eq!(result.file_count, 3, "should index all 3 files");

    // Search for struct from a different file
    let results = cg.search("User", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'User' struct");

    // Search for function from services
    let results = cg.search("create_user", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'create_user' function");
}

// ---------------------------------------------------------------------------
// Call edge regression tests
// ---------------------------------------------------------------------------

/// Helper: create a temp project with the given source files, init CodeGraph,
/// and return the (TempDir, CodeGraph) pair. TempDir must be held alive.
async fn setup_call_edge_project() -> (TempDir, CodeGraph) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/lib.rs"),
        r#"
pub mod caller_mod;
pub mod callee_mod;
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/callee_mod.rs"),
        r#"
/// The target function that should be found via call edges.
pub fn target_fn() -> u32 {
    42
}
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/caller_mod.rs"),
        r#"
use crate::callee_mod::target_fn;

pub fn caller_fn() -> u32 {
    target_fn()
}
"#,
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    (dir, cg)
}

/// Finds the node ID for a function by name, panicking if not found.
async fn find_node_id(cg: &CodeGraph, name: &str) -> String {
    let results = cg.search(name, 10).await.unwrap();
    results
        .iter()
        .find(|r| r.node.name == name)
        .unwrap_or_else(|| panic!("node '{name}' not found in index"))
        .node
        .id
        .clone()
}

#[tokio::test]
async fn test_index_all_produces_call_edges() {
    let (_dir, cg) = setup_call_edge_project().await;
    cg.index_all().await.unwrap();

    let target_id = find_node_id(&cg, "target_fn").await;

    let callers = cg.get_callers(&target_id, 3).await.unwrap();
    assert!(
        callers
            .iter()
            .any(|(node, edge)| node.name == "caller_fn" && edge.kind == EdgeKind::Calls),
        "index_all should produce a Calls edge from caller_fn -> target_fn"
    );
}

#[tokio::test]
async fn test_sync_produces_call_edges() {
    let (_dir, cg) = setup_call_edge_project().await;

    // Use sync (not index_all) as the *only* indexing path.
    // Before the fix, this would store unresolved refs but never resolve them.
    cg.sync().await.unwrap();

    let target_id = find_node_id(&cg, "target_fn").await;

    let callers = cg.get_callers(&target_id, 3).await.unwrap();
    assert!(
        callers
            .iter()
            .any(|(node, edge)| node.name == "caller_fn" && edge.kind == EdgeKind::Calls),
        "sync should produce a Calls edge from caller_fn -> target_fn"
    );
}

#[tokio::test]
async fn test_sync_produces_call_edges_after_file_modification() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn base_fn() -> u32 { 1 }
pub fn consumer() -> u32 { base_fn() }
"#,
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // Modify the file to add a new call chain.
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn base_fn() -> u32 { 1 }
pub fn middle_fn() -> u32 { base_fn() }
pub fn top_fn() -> u32 { middle_fn() }
"#,
    )
    .unwrap();

    // Incremental sync should resolve the new call edges.
    cg.sync().await.unwrap();

    let base_id = find_node_id(&cg, "base_fn").await;
    let middle_id = find_node_id(&cg, "middle_fn").await;

    // middle_fn -> base_fn
    let base_callers = cg.get_callers(&base_id, 1).await.unwrap();
    assert!(
        base_callers
            .iter()
            .any(|(node, _)| node.name == "middle_fn"),
        "sync should resolve middle_fn -> base_fn call edge after modification"
    );

    // top_fn -> middle_fn
    let middle_callers = cg.get_callers(&middle_id, 1).await.unwrap();
    assert!(
        middle_callers
            .iter()
            .any(|(node, _)| node.name == "top_fn"),
        "sync should resolve top_fn -> middle_fn call edge after modification"
    );

    // Transitive: top_fn should appear as a depth-2 caller of base_fn
    let transitive_callers = cg.get_callers(&base_id, 3).await.unwrap();
    assert!(
        transitive_callers
            .iter()
            .any(|(node, _)| node.name == "top_fn"),
        "sync should support transitive call edge traversal"
    );
}

#[tokio::test]
async fn test_sync_resolves_cross_file_call_edges_for_new_files() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();

    // Start with a single file.
    fs::write(
        project.join("src/lib.rs"),
        r#"
pub mod engine;
pub fn entry_point() -> u32 { 0 }
"#,
    )
    .unwrap();

    let cg = CodeGraph::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    // Add a new file that calls the existing function.
    fs::write(
        project.join("src/engine.rs"),
        r#"
use crate::entry_point;

pub fn run_engine() -> u32 {
    entry_point()
}
"#,
    )
    .unwrap();

    cg.sync().await.unwrap();

    let entry_id = find_node_id(&cg, "entry_point").await;

    let callers = cg.get_callers(&entry_id, 3).await.unwrap();
    assert!(
        callers
            .iter()
            .any(|(node, _)| node.name == "run_engine"),
        "sync should resolve cross-file call edges when a new file is added"
    );
}
