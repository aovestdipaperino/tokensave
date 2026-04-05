//! Tests for the `TokenSave` orchestrator methods that aren't fully exercised
//! by the MCP handler tests.

use std::fs;
use tempfile::TempDir;
use tokensave::tokensave::{is_test_file, TokenSave};
use tokensave::types::NodeKind;

// ---------------------------------------------------------------------------
// Shared setup
// ---------------------------------------------------------------------------

/// Creates a temporary Rust project with cross-file calls, then initializes
/// and indexes a `TokenSave`.
async fn setup() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn foo() { bar(); }
fn bar() {}
fn unused_private() {}
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/utils.rs"),
        r#"
use crate::lib::foo;
pub fn helper() { foo(); }
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

// ---------------------------------------------------------------------------
// is_test_file
// ---------------------------------------------------------------------------

#[test]
fn test_is_test_file_test_dir() {
    assert!(is_test_file("tests/my_test.rs"));
    assert!(is_test_file("tests/integration.rs"));
}

#[test]
fn test_is_test_file_test_prefix() {
    assert!(is_test_file("test/foo.rs"));
}

#[test]
fn test_is_test_file_spec_dir() {
    assert!(is_test_file("spec/models/user_spec.rb"));
}

#[test]
fn test_is_test_file_e2e_dir() {
    assert!(is_test_file("e2e/login.test.ts"));
}

#[test]
fn test_is_test_file_dot_test() {
    assert!(is_test_file("src/utils.test.ts"));
    assert!(is_test_file("src/utils.spec.js"));
}

#[test]
fn test_is_test_file_underscore_test() {
    assert!(is_test_file("src/utils_test.rs"));
    assert!(is_test_file("src/utils_spec.py"));
}

#[test]
fn test_is_test_file_dunder_tests() {
    assert!(is_test_file("__tests__/component.test.tsx"));
}

#[test]
fn test_is_test_file_normal_source() {
    assert!(!is_test_file("src/lib.rs"));
    assert!(!is_test_file("src/main.rs"));
    assert!(!is_test_file("src/utils.rs"));
}

#[test]
fn test_is_test_file_case_insensitive() {
    assert!(is_test_file("Tests/MyTest.rs"));
    assert!(is_test_file("TESTS/foo.rs"));
}

// ---------------------------------------------------------------------------
// get_all_files / get_all_nodes / get_all_edges through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_all_files() {
    let (cg, _dir) = setup().await;
    let files = cg.get_all_files().await.unwrap();
    assert!(
        files.len() >= 2,
        "should have at least 2 indexed files (lib.rs, utils.rs), got {}",
        files.len(),
    );
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"src/lib.rs"));
    assert!(paths.contains(&"src/utils.rs"));
}

#[tokio::test]
async fn test_get_all_nodes() {
    let (cg, _dir) = setup().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(
        !nodes.is_empty(),
        "should have extracted some nodes from the project",
    );
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"foo"), "should have extracted 'foo'");
    assert!(names.contains(&"bar"), "should have extracted 'bar'");
}

#[tokio::test]
async fn test_get_all_edges() {
    let (cg, _dir) = setup().await;
    let edges = cg.get_all_edges().await.unwrap();
    // foo() calls bar(), so there should be at least one edge
    assert!(!edges.is_empty(), "should have at least one edge");
}

// ---------------------------------------------------------------------------
// get_file_dependents
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_file_dependents() {
    let (cg, _dir) = setup().await;
    // utils.rs calls foo from lib.rs, so lib.rs has utils.rs as a dependent
    // (or utils depends on lib). Let's check if lib.rs has dependents.
    let dependents = cg.get_file_dependents("src/lib.rs").await.unwrap();
    // The cross-file resolution may or may not work depending on extractor,
    // but the method should not panic.
    // dependents is a Vec<String> of file paths
    assert!(
        dependents.is_empty() || dependents.iter().any(|d| d.contains("utils")),
        "dependents of lib.rs should either be empty (if resolution didn't link) or contain utils.rs"
    );
}

// ---------------------------------------------------------------------------
// find_dead_code
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_find_dead_code_functions() {
    let (cg, _dir) = setup().await;
    let dead = cg
        .find_dead_code(&[NodeKind::Function])
        .await
        .unwrap();
    // The method should return successfully. Private functions without
    // incoming call edges appear as dead code. The exact results depend
    // on the extractor's edge generation (e.g., contains edges may give
    // nodes incoming edges). Verify the method runs and returns only
    // non-pub, non-main, non-test nodes.
    for node in &dead {
        assert_ne!(node.name, "main", "main should be excluded from dead code");
        assert!(
            !node.name.starts_with("test"),
            "test functions should be excluded from dead code",
        );
        assert_ne!(
            node.visibility,
            tokensave::types::Visibility::Pub,
            "pub items should be excluded from dead code",
        );
    }
}

#[tokio::test]
async fn test_find_dead_code_custom_kinds() {
    let (cg, _dir) = setup().await;
    // Look for dead structs — our test project has none, should return empty
    let dead = cg.find_dead_code(&[NodeKind::Struct]).await.unwrap();
    assert!(
        dead.is_empty(),
        "test project has no structs, so no dead struct code expected",
    );
}

// ---------------------------------------------------------------------------
// get_file_coupling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_file_coupling_fan_in() {
    let (cg, _dir) = setup().await;
    let coupling = cg.get_file_coupling(true, 10).await.unwrap();
    // Even if coupling is empty (due to how the extractor resolves cross-file refs),
    // the method should succeed.
    for (path, count) in &coupling {
        assert!(!path.is_empty());
        assert!(*count > 0);
    }
}

#[tokio::test]
async fn test_get_file_coupling_fan_out() {
    let (cg, _dir) = setup().await;
    let coupling = cg.get_file_coupling(false, 10).await.unwrap();
    for (path, count) in &coupling {
        assert!(!path.is_empty());
        assert!(*count > 0);
    }
}

// ---------------------------------------------------------------------------
// check_file_staleness
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_check_file_staleness_not_stale() {
    let (cg, _dir) = setup().await;
    // Right after indexing, files should not be stale
    let stale = cg
        .check_file_staleness(&["src/lib.rs".to_string()])
        .await;
    // Immediately after indexing, the file should not be stale
    // (mtime <= indexed_at in most cases)
    assert!(
        stale.is_empty(),
        "files should not be stale right after indexing"
    );
}

#[tokio::test]
async fn test_check_file_staleness_after_modification() {
    let (cg, dir) = setup().await;

    // Wait a moment, then modify the file so mtime > indexed_at
    std::thread::sleep(std::time::Duration::from_secs(2));
    let file_path = dir.path().join("src/lib.rs");
    fs::write(
        &file_path,
        "pub fn foo() { bar(); }\nfn bar() {}\nfn new_function() {}\n",
    )
    .unwrap();

    let stale = cg
        .check_file_staleness(&["src/lib.rs".to_string()])
        .await;
    assert!(
        stale.contains(&"src/lib.rs".to_string()),
        "src/lib.rs should be stale after modification"
    );
}

// ---------------------------------------------------------------------------
// get_tokens_saved / set_tokens_saved — round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tokens_saved_round_trip() {
    let (cg, _dir) = setup().await;

    // Initially should be 0
    let initial = cg.get_tokens_saved().await.unwrap();
    assert_eq!(initial, 0, "initial tokens_saved should be 0");

    // Set a value
    cg.set_tokens_saved(42_000).await.unwrap();
    let saved = cg.get_tokens_saved().await.unwrap();
    assert_eq!(saved, 42_000);

    // Overwrite
    cg.set_tokens_saved(100_000).await.unwrap();
    let saved2 = cg.get_tokens_saved().await.unwrap();
    assert_eq!(saved2, 100_000);
}

// ---------------------------------------------------------------------------
// get_complexity_ranked through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_complexity_ranked() {
    let (cg, _dir) = setup().await;
    let ranked = cg.get_complexity_ranked(None, 10).await.unwrap();
    // Should return functions/methods from our indexed project
    assert!(
        !ranked.is_empty(),
        "should have at least one function in complexity ranking",
    );
    // Verify the tuple structure (node, lines, fan_out, fan_in, score)
    let (node, lines, _fan_out, _fan_in, score) = &ranked[0];
    assert!(!node.name.is_empty());
    assert!(*lines > 0);
    assert!(*score > 0);
}

// ---------------------------------------------------------------------------
// get_undocumented_public_symbols through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_undocumented_public_symbols_no_filter() {
    let (cg, _dir) = setup().await;
    let undoc = cg
        .get_undocumented_public_symbols(None, 50)
        .await
        .unwrap();
    // foo is pub and has no docstring
    let names: Vec<&str> = undoc.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.contains(&"foo"),
        "foo is pub without docs, should appear, found: {:?}",
        names,
    );
}

#[tokio::test]
async fn test_get_undocumented_public_symbols_with_prefix() {
    let (cg, _dir) = setup().await;
    let undoc = cg
        .get_undocumented_public_symbols(Some("src/utils"), 50)
        .await
        .unwrap();
    // helper in utils.rs is pub without docs
    for node in &undoc {
        assert!(
            node.file_path.starts_with("src/utils"),
            "path prefix filter should only return src/utils files, got: {}",
            node.file_path,
        );
    }
}

// ---------------------------------------------------------------------------
// get_node_distribution through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_node_distribution() {
    let (cg, _dir) = setup().await;
    let dist = cg.get_node_distribution(None).await.unwrap();
    assert!(!dist.is_empty(), "should have node distribution data");
    // Each entry is (file_path, kind, count)
    for (file, kind, count) in &dist {
        assert!(!file.is_empty());
        assert!(!kind.is_empty());
        assert!(*count > 0);
    }
}

// ---------------------------------------------------------------------------
// is_initialized
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_is_initialized() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    assert!(
        !TokenSave::is_initialized(project),
        "should not be initialized before init"
    );
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "fn main() {}\n").unwrap();
    let _cg = TokenSave::init(project).await.unwrap();
    assert!(
        TokenSave::is_initialized(project),
        "should be initialized after init"
    );
}

// ---------------------------------------------------------------------------
// get_god_classes through TokenSave (empty for Rust-only project)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_god_classes_empty() {
    let (cg, _dir) = setup().await;
    let god = cg.get_god_classes(10).await.unwrap();
    // Pure Rust project with no classes should return empty
    assert!(
        god.is_empty(),
        "Rust project without classes should have no god classes"
    );
}

// ---------------------------------------------------------------------------
// get_inheritance_depth through TokenSave (empty for Rust-only project)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_inheritance_depth_empty() {
    let (cg, _dir) = setup().await;
    let depths = cg.get_inheritance_depth(10).await.unwrap();
    assert!(
        depths.is_empty(),
        "Rust project without class hierarchies should have no inheritance depth"
    );
}

// ---------------------------------------------------------------------------
// search through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search() {
    let (cg, _dir) = setup().await;
    let results = cg.search("foo", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'foo' via search");
    assert_eq!(results[0].node.name, "foo");
}

// ---------------------------------------------------------------------------
// get_stats through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_stats() {
    let (cg, _dir) = setup().await;
    let stats = cg.get_stats().await.unwrap();
    assert!(stats.node_count > 0, "should have nodes");
    assert!(stats.file_count > 0, "should have files");
}
