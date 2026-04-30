use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;
use tokensave::config::{load_config, save_config};
use tokensave::tokensave::TokenSave;
use tokensave::types::EdgeKind;

/// Directly test that the ignore crate with add_custom_ignore_filename reads
/// nested .gitignore files, regardless of git repo presence.
#[test]
fn test_ignore_crate_nested_gitignore_direct() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src/vendor")).unwrap();
    fs::write(project.join("src/lib.rs"), "kept").unwrap();
    fs::write(project.join("src/vendor/gen.rs"), "generated").unwrap();
    fs::write(project.join("src/vendor/.gitignore"), "*\n").unwrap();

    let files: Vec<String> = ignore::WalkBuilder::new(project)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .follow_links(true)
        .add_custom_ignore_filename(".gitignore")
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        .filter_map(|e| {
            e.path()
                .strip_prefix(project)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect();

    assert!(
        files.contains(&"src/lib.rs".to_string()),
        "lib.rs must be found"
    );
    assert!(
        !files.iter().any(|f| f.contains("vendor")),
        "nested .gitignore (*) must exclude vendor/gen.rs; got: {files:?}"
    );
}

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
    let cg = TokenSave::init(project).await.unwrap();

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

    let cg = TokenSave::init(project).await.unwrap();
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

    assert!(!TokenSave::is_initialized(project));
    TokenSave::init(project).await.unwrap();
    assert!(TokenSave::is_initialized(project));

    // Open existing project
    let cg = TokenSave::open(project).await;
    assert!(cg.is_ok());
}

#[tokio::test]
async fn test_search_empty_index() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let cg = TokenSave::init(project).await.unwrap();
    let results = cg.search("anything", 10).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_stats_empty_index() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let cg = TokenSave::init(project).await.unwrap();
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

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let options = tokensave::types::BuildContextOptions::default();
    let context = cg
        .build_context("process_data function", &options)
        .await
        .unwrap();
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

    let cg = TokenSave::init(project).await.unwrap();
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

    let cg = TokenSave::init(project).await.unwrap();
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

    let cg = TokenSave::init(project).await.unwrap();

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

    let cg = TokenSave::init(project).await.unwrap();
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

    let cg = TokenSave::init(project).await.unwrap();
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

    let cg = TokenSave::init(project).await.unwrap();
    let result = cg.index_all().await.unwrap();
    assert_eq!(result.file_count, 3, "should index all 3 files");

    // Search for struct from a different file
    let results = cg.search("User", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'User' struct");

    // Search for function from services
    let results = cg.search("create_user", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'create_user' function");
}

#[cfg(unix)]
#[tokio::test]
async fn test_index_follows_symlinked_directories() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    let external = TempDir::new().unwrap();

    fs::create_dir_all(external.path()).unwrap();
    fs::write(
        external.path().join("lib.rs"),
        "pub fn through_symlink() {}\n",
    )
    .unwrap();
    symlink(external.path(), project.join("src")).unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    let result = cg.index_all().await.unwrap();

    assert_eq!(
        result.file_count, 1,
        "should index the file behind the symlink"
    );

    let files = cg.get_all_files().await.unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"src/lib.rs"));

    let results = cg.search("through_symlink", 10).await.unwrap();
    assert!(
        !results.is_empty(),
        "should extract symbols from symlinked source"
    );
}

// ---------------------------------------------------------------------------
// Nested .gitignore tests
// ---------------------------------------------------------------------------

/// Helper: init a project with git_ignore enabled and return the TokenSave.
async fn setup_gitignore_project(project: &std::path::Path) -> TokenSave {
    TokenSave::init(project).await.unwrap();
    let mut config = load_config(project).unwrap();
    config.git_ignore = true;
    save_config(project, &config).unwrap();
    TokenSave::open(project).await.unwrap()
}

/// A nested `.gitignore` in a subdirectory must exclude files inside that
/// subdirectory even when the root `.gitignore` has no matching rule.
#[tokio::test]
async fn test_nested_gitignore_excludes_files_in_subdir() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src/vendor")).unwrap();
    // This file should be indexed.
    fs::write(project.join("src/lib.rs"), "pub fn kept() {}\n").unwrap();
    // This file is excluded by the nested .gitignore only.
    fs::write(project.join("src/vendor/gen.rs"), "pub fn generated() {}\n").unwrap();
    // Nested .gitignore ignores everything in vendor/.
    fs::write(project.join("src/vendor/.gitignore"), "*\n").unwrap();

    let cg = setup_gitignore_project(project).await;
    let result = cg.index_all().await.unwrap();

    assert_eq!(
        result.file_count, 1,
        "vendor/ should be excluded by nested .gitignore"
    );

    let files = cg.get_all_files().await.unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"src/lib.rs"), "src/lib.rs must be indexed");
    assert!(
        !paths.iter().any(|p| p.contains("vendor")),
        "vendor files must be excluded by nested .gitignore"
    );
}

/// A nested `.gitignore` must not affect files outside its own directory.
/// Only `src/internal/` should be excluded; `src/lib.rs` must still be indexed.
#[tokio::test]
async fn test_nested_gitignore_scope_is_limited_to_its_directory() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src/internal")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn public_api() {}\n").unwrap();
    fs::write(
        project.join("src/internal/secret.rs"),
        "pub fn secret() {}\n",
    )
    .unwrap();
    // The nested .gitignore only covers files within src/internal/.
    fs::write(project.join("src/internal/.gitignore"), "*.rs\n").unwrap();

    let cg = setup_gitignore_project(project).await;
    cg.index_all().await.unwrap();

    let files = cg.get_all_files().await.unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        paths.contains(&"src/lib.rs"),
        "src/lib.rs must not be affected by nested .gitignore in src/internal/"
    );
    assert!(
        !paths.iter().any(|p| p.contains("secret")),
        "src/internal/secret.rs must be excluded by its own directory's .gitignore"
    );
}

/// A nested `.gitignore` negation (`!`) must un-ignore a file that a higher-level
/// rule would otherwise exclude. The `ignore` crate replicates git's precedence:
/// a more specific (deeper) rule wins over a less specific (shallower) one.
#[tokio::test]
async fn test_nested_gitignore_negation_overrides_parent_rule() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src/exceptions")).unwrap();
    // Root .gitignore ignores all .rs files.
    fs::write(project.join(".gitignore"), "*.rs\n").unwrap();
    // The nested .gitignore un-ignores the one file we actually want indexed.
    fs::write(project.join("src/exceptions/.gitignore"), "!important.rs\n").unwrap();
    fs::write(
        project.join("src/exceptions/important.rs"),
        "pub fn must_be_indexed() {}\n",
    )
    .unwrap();
    // This sibling file stays excluded by the root rule.
    fs::write(
        project.join("src/exceptions/ignored.rs"),
        "pub fn ignored() {}\n",
    )
    .unwrap();

    let cg = setup_gitignore_project(project).await;
    cg.index_all().await.unwrap();

    let results = cg.search("must_be_indexed", 10).await.unwrap();
    assert!(
        !results.is_empty(),
        "nested .gitignore negation must un-ignore important.rs even though root rule excludes *.rs"
    );

    let files = cg.get_all_files().await.unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        !paths.iter().any(|p| p.ends_with("ignored.rs")),
        "ignored.rs must remain excluded by root .gitignore"
    );
}

/// Files in deeply nested subdirectories must be excluded by a `.gitignore`
/// anywhere in their ancestor chain, not just the root.
#[tokio::test]
async fn test_nested_gitignore_applies_to_deeper_descendants() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src/mid/deep")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn top() {}\n").unwrap();
    // The mid-level .gitignore excludes the deep/ subtree.
    fs::write(project.join("src/mid/.gitignore"), "deep/\n").unwrap();
    fs::write(project.join("src/mid/mid.rs"), "pub fn mid() {}\n").unwrap();
    fs::write(project.join("src/mid/deep/leaf.rs"), "pub fn leaf() {}\n").unwrap();

    let cg = setup_gitignore_project(project).await;
    cg.index_all().await.unwrap();

    let files = cg.get_all_files().await.unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"src/lib.rs"), "src/lib.rs must be indexed");
    assert!(
        paths.contains(&"src/mid/mid.rs"),
        "src/mid/mid.rs must be indexed"
    );
    assert!(
        !paths.iter().any(|p| p.contains("deep")),
        "src/mid/deep/leaf.rs must be excluded by mid-level .gitignore"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_gitignore_scan_follows_symlinked_directories() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    let external = TempDir::new().unwrap();

    fs::create_dir_all(external.path()).unwrap();
    fs::write(
        external.path().join("lib.rs"),
        "pub fn through_gitignore_symlink() {}\n",
    )
    .unwrap();
    symlink(external.path(), project.join("src")).unwrap();

    TokenSave::init(project).await.unwrap();

    let mut config = load_config(project).unwrap();
    config.git_ignore = true;
    save_config(project, &config).unwrap();

    let cg = TokenSave::open(project).await.unwrap();
    let result = cg.index_all().await.unwrap();

    assert_eq!(
        result.file_count, 1,
        "gitignore-aware scan should follow symlinks"
    );

    let results = cg.search("through_gitignore_symlink", 10).await.unwrap();
    assert!(
        !results.is_empty(),
        "should extract symbols through symlink with gitignore-aware walker"
    );
}

// ---------------------------------------------------------------------------
// Call edge regression tests
// ---------------------------------------------------------------------------

/// Helper: create a temp project with the given source files, init TokenSave,
/// and return the (TempDir, TokenSave) pair. TempDir must be held alive.
async fn setup_call_edge_project() -> (TempDir, TokenSave) {
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

    let cg = TokenSave::init(project).await.unwrap();
    (dir, cg)
}

/// Finds the node ID for a function by name, panicking if not found.
async fn find_node_id(cg: &TokenSave, name: &str) -> String {
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

    let cg = TokenSave::init(project).await.unwrap();
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
        middle_callers.iter().any(|(node, _)| node.name == "top_fn"),
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

    let cg = TokenSave::init(project).await.unwrap();
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
        callers.iter().any(|(node, _)| node.name == "run_engine"),
        "sync should resolve cross-file call edges when a new file is added"
    );
}

#[tokio::test]
async fn test_sync_does_not_duplicate_edges() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();

    // Three files: a callee, a caller that will be modified, and
    // an unchanged caller whose edges must not be duplicated.
    fs::write(
        project.join("src/callee.rs"),
        "pub fn target_fn() -> u32 { 42 }\n",
    )
    .unwrap();

    fs::write(
        project.join("src/caller_a.rs"),
        "pub fn caller_a() -> u32 { target_fn() }\n",
    )
    .unwrap();

    fs::write(
        project.join("src/caller_b.rs"),
        "pub fn caller_b() -> u32 { target_fn() }\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let stats_before = cg.get_stats().await.unwrap();
    let edges_before = stats_before.edge_count;

    // Modify caller_a only — caller_b is unchanged.
    fs::write(
        project.join("src/caller_a.rs"),
        "pub fn caller_a() -> u32 { target_fn() + 1 }\n",
    )
    .unwrap();

    cg.sync().await.unwrap();

    let stats_after = cg.get_stats().await.unwrap();
    assert_eq!(
        edges_before, stats_after.edge_count,
        "sync must not create duplicate edges (before={edges_before}, after={})",
        stats_after.edge_count
    );

    // Run a second sync with no changes — edge count must still be stable.
    // Force a content-hash change by touching caller_a again with same content
    // so there are no stale files and to_index is empty.
    cg.sync().await.unwrap();

    let stats_final = cg.get_stats().await.unwrap();
    assert_eq!(
        edges_before, stats_final.edge_count,
        "repeated sync must not grow edges (before={edges_before}, final={})",
        stats_final.edge_count
    );
}

#[tokio::test]
async fn test_concurrent_sync_is_rejected() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn f() {}\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();

    // Simulate an in-progress sync by placing a lockfile with our own PID.
    let lock_path = project.join(".tokensave/sync.lock");
    fs::write(&lock_path, format!("{}", std::process::id())).unwrap();

    let err = cg.sync().await.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("another sync is already in progress"),
        "expected sync lock error, got: {msg}"
    );

    // After removing the lockfile, sync should succeed.
    fs::remove_file(&lock_path).unwrap();
    cg.sync().await.unwrap();
}
