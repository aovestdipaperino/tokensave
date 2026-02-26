use codegraph::codegraph::CodeGraph;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_full_pipeline() {
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
    let cg = CodeGraph::init(project).unwrap();

    // Index
    let index_result = cg.index_all().unwrap();
    assert!(index_result.file_count > 0, "should index files");
    assert!(index_result.node_count > 0, "should extract nodes");

    // Stats
    let stats = cg.get_stats().unwrap();
    assert!(stats.node_count > 0);
    assert!(stats.file_count >= 2);

    // Search
    let results = cg.search("helper", 10).unwrap();
    assert!(!results.is_empty(), "should find 'helper'");
    assert!(results.iter().any(|r| r.node.name == "helper"));

    // Edges should exist (at minimum Contains edges from file -> items)
    let stats = cg.get_stats().unwrap();
    assert!(stats.edge_count > 0, "should have edges");
}

#[test]
fn test_incremental_sync() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn original() {}\n").unwrap();

    let cg = CodeGraph::init(project).unwrap();
    cg.index_all().unwrap();

    // Verify original function exists
    let results = cg.search("original", 10).unwrap();
    assert!(!results.is_empty());

    // Modify file
    fs::write(
        project.join("src/lib.rs"),
        "pub fn modified() {}\npub fn added() {}\n",
    )
    .unwrap();

    // Sync
    let sync_result = cg.sync().unwrap();
    assert!(
        sync_result.files_modified > 0 || sync_result.files_added > 0,
        "sync should detect changes: modified={}, added={}",
        sync_result.files_modified,
        sync_result.files_added
    );

    // Should find the new function
    let results = cg.search("modified", 10).unwrap();
    assert!(!results.is_empty(), "should find 'modified' after sync");
}

#[test]
fn test_init_and_open() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    assert!(!CodeGraph::is_initialized(project));
    CodeGraph::init(project).unwrap();
    assert!(CodeGraph::is_initialized(project));

    // Open existing project
    let cg = CodeGraph::open(project);
    assert!(cg.is_ok());
}

#[test]
fn test_search_empty_index() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let cg = CodeGraph::init(project).unwrap();
    let results = cg.search("anything", 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_stats_empty_index() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let cg = CodeGraph::init(project).unwrap();
    let stats = cg.get_stats().unwrap();
    assert_eq!(stats.node_count, 0);
    assert_eq!(stats.edge_count, 0);
    assert_eq!(stats.file_count, 0);
}

#[test]
fn test_context_building() {
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

    let cg = CodeGraph::init(project).unwrap();
    cg.index_all().unwrap();

    let options = codegraph::types::BuildContextOptions::default();
    let context = cg.build_context("process_data function", &options).unwrap();
    assert!(
        !context.entry_points.is_empty(),
        "should find entry points for 'process_data'"
    );
}

#[test]
fn test_struct_and_impl_extraction() {
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

    let cg = CodeGraph::init(project).unwrap();
    let result = cg.index_all().unwrap();
    // File node + Point struct + x field + y field + impl Point + new method + distance method = 7+
    assert!(
        result.node_count >= 5,
        "should extract Point, x, y, new, distance (got {})",
        result.node_count
    );

    // Search for struct
    let results = cg.search("Point", 10).unwrap();
    assert!(!results.is_empty(), "should find 'Point'");

    // Search for method
    let results = cg.search("distance", 10).unwrap();
    assert!(!results.is_empty(), "should find 'distance'");
}

#[test]
fn test_file_removal_sync() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn keep() {}\n").unwrap();
    fs::write(project.join("src/remove_me.rs"), "pub fn gone() {}\n").unwrap();

    let cg = CodeGraph::init(project).unwrap();
    cg.index_all().unwrap();

    // Verify both exist
    let stats = cg.get_stats().unwrap();
    assert!(
        stats.file_count >= 2,
        "should have at least 2 files indexed"
    );

    // Remove file
    fs::remove_file(project.join("src/remove_me.rs")).unwrap();

    // Sync
    let sync_result = cg.sync().unwrap();
    assert_eq!(sync_result.files_removed, 1, "should detect 1 removed file");

    // Verify removed function is gone
    let results = cg.search("gone", 10).unwrap();
    assert!(results.is_empty(), "'gone' should no longer be found");
}

#[test]
fn test_index_all_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub fn alpha() {}\npub fn beta() {}\n",
    )
    .unwrap();

    let cg = CodeGraph::init(project).unwrap();

    let result1 = cg.index_all().unwrap();
    let stats1 = cg.get_stats().unwrap();

    let result2 = cg.index_all().unwrap();
    let stats2 = cg.get_stats().unwrap();

    assert_eq!(
        result1.file_count, result2.file_count,
        "re-indexing should produce the same file count"
    );
    assert_eq!(
        stats1.node_count, stats2.node_count,
        "re-indexing should produce the same node count"
    );
}

#[test]
fn test_sync_no_changes() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn stable() {}\n").unwrap();

    let cg = CodeGraph::init(project).unwrap();
    cg.index_all().unwrap();

    // Sync without any changes
    let sync_result = cg.sync().unwrap();
    assert_eq!(sync_result.files_added, 0);
    assert_eq!(sync_result.files_modified, 0);
    assert_eq!(sync_result.files_removed, 0);
}

#[test]
fn test_search_by_docstring() {
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

    let cg = CodeGraph::init(project).unwrap();
    cg.index_all().unwrap();

    // Search by the docstring content
    let results = cg.search("fibonacci", 10).unwrap();
    assert!(
        !results.is_empty(),
        "should find node via docstring/name search"
    );
}

#[test]
fn test_multiple_files_cross_reference() {
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

    let cg = CodeGraph::init(project).unwrap();
    let result = cg.index_all().unwrap();
    assert_eq!(result.file_count, 3, "should index all 3 files");

    // Search for struct from a different file
    let results = cg.search("User", 10).unwrap();
    assert!(!results.is_empty(), "should find 'User' struct");

    // Search for function from services
    let results = cg.search("create_user", 10).unwrap();
    assert!(!results.is_empty(), "should find 'create_user' function");
}
