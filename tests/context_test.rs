use tokensave::context::*;
use tokensave::types::*;

#[tokio::test]
async fn test_reranking_demotes_fixture_nodes() {
    use tempfile::TempDir;
    use tokensave::context::ContextBuilder;
    use tokensave::db::Database;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let (db, _) = Database::initialize(&project.join(".tokensave/tokensave.db"))
        .await
        .unwrap();

    // Fixture node: enum variant in tests/fixtures/
    let fixture_node = Node {
        id: "enum_variant:fixture_debug".to_string(),
        kind: NodeKind::EnumVariant,
        name: "debug".to_string(),
        qualified_name: "tests/fixtures/sample.dart::LogLevel::debug".to_string(),
        file_path: "tests/fixtures/sample.dart".to_string(),
        start_line: 14,
        attrs_start_line: 14,
        end_line: 14,
        start_column: 0,
        end_column: 10,
        signature: Some("debug".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    };
    db.insert_node(&fixture_node).await.unwrap();

    // Source node: function in src/
    let source_node = Node {
        id: "function:debug_handler".to_string(),
        kind: NodeKind::Function,
        name: "debug_handler".to_string(),
        qualified_name: "src/debug.rs::debug_handler".to_string(),
        file_path: "src/debug.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 10,
        start_column: 0,
        end_column: 1,
        signature: Some("pub fn debug_handler()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    };
    db.insert_node(&source_node).await.unwrap();

    let builder = ContextBuilder::new(&db, project);
    let result = builder
        .build_context("debug", &BuildContextOptions::default())
        .await
        .unwrap();

    assert!(!result.entry_points.is_empty());
    assert_eq!(
        result.entry_points[0].id, "function:debug_handler",
        "source function should outrank fixture enum variant after re-ranking"
    );
}

#[test]
fn test_extract_symbols_from_query() {
    let symbols = extract_symbols_from_query("fix the process_request function");
    assert!(symbols.contains(&"process_request".to_string()));
}

#[test]
fn test_extract_camel_case_symbols() {
    let symbols = extract_symbols_from_query("update UserService handler");
    assert!(symbols.contains(&"UserService".to_string()));
}

#[test]
fn test_extract_qualified_symbols() {
    let symbols = extract_symbols_from_query("look at crate::types::Node");
    assert!(symbols.iter().any(|s| s.contains("Node")));
}

#[test]
fn test_extract_screaming_snake_symbols() {
    let symbols = extract_symbols_from_query("increase MAX_RETRIES");
    assert!(symbols.contains(&"MAX_RETRIES".to_string()));
}

#[test]
fn test_extract_no_symbols_from_plain_english() {
    let symbols = extract_symbols_from_query("the is in for to a an");
    assert!(symbols.is_empty());
}

#[test]
fn test_format_context_markdown() {
    let context = TaskContext {
        query: "test query".to_string(),
        summary: "Test summary".to_string(),
        subgraph: Subgraph::default(),
        entry_points: vec![],
        code_blocks: vec![],
        related_files: vec![],
        seen_node_ids: vec![],
    };
    let md = format_context_as_markdown(&context);
    assert!(md.contains("## Code Context"));
    assert!(md.contains("test query"));
}

#[test]
fn test_format_context_json() {
    let context = TaskContext {
        query: "test".to_string(),
        summary: "Summary".to_string(),
        subgraph: Subgraph::default(),
        entry_points: vec![],
        code_blocks: vec![],
        related_files: vec![],
        seen_node_ids: vec![],
    };
    let json = format_context_as_json(&context);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["query"], "test");
}

#[tokio::test]
async fn test_build_context_with_db() {
    use std::fs;
    use tempfile::TempDir;
    use tokensave::context::ContextBuilder;
    use tokensave::db::Database;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    // Create a source file
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn process_data() {}\n").unwrap();

    // Init DB and insert a node
    let (db, _) = Database::initialize(&project.join(".tokensave/tokensave.db"))
        .await
        .unwrap();
    let node = Node {
        id: "function:test123".to_string(),
        kind: NodeKind::Function,
        name: "process_data".to_string(),
        qualified_name: "src/lib.rs::process_data".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 1,
        start_column: 0,
        end_column: 24,
        signature: Some("pub fn process_data()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    };
    db.insert_node(&node).await.unwrap();

    let builder = ContextBuilder::new(&db, project);
    let result = builder
        .build_context("process_data", &BuildContextOptions::default())
        .await;
    assert!(result.is_ok());
    let ctx = result.unwrap();
    assert!(!ctx.entry_points.is_empty());
}

#[tokio::test]
async fn test_get_code_reads_source_file() {
    use std::fs;
    use tempfile::TempDir;
    use tokensave::context::ContextBuilder;
    use tokensave::db::Database;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();

    let (db, _) = Database::initialize(&project.join(".tokensave/tokensave.db"))
        .await
        .unwrap();

    let node = Node {
        id: "function:main123".to_string(),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 3,
        start_column: 0,
        end_column: 1,
        signature: Some("fn main()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    };

    let builder = ContextBuilder::new(&db, project);
    let code = builder.get_code(&node).unwrap();
    assert!(code.is_some());
    let content = code.unwrap();
    assert!(content.contains("fn main()"));
    assert!(content.contains("println!"));
}

#[tokio::test]
async fn test_get_code_returns_none_for_missing_file() {
    use tempfile::TempDir;
    use tokensave::context::ContextBuilder;
    use tokensave::db::Database;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let (db, _) = Database::initialize(&project.join(".tokensave/tokensave.db"))
        .await
        .unwrap();

    let node = Node {
        id: "function:missing".to_string(),
        kind: NodeKind::Function,
        name: "missing".to_string(),
        qualified_name: "nonexistent.rs::missing".to_string(),
        file_path: "nonexistent.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 1,
        start_column: 0,
        end_column: 10,
        signature: None,
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    };

    let builder = ContextBuilder::new(&db, project);
    let code = builder.get_code(&node).unwrap();
    assert!(code.is_none());
}

#[tokio::test]
async fn test_find_relevant_context() {
    use tempfile::TempDir;
    use tokensave::context::ContextBuilder;
    use tokensave::db::Database;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let (db, _) = Database::initialize(&project.join(".tokensave/tokensave.db"))
        .await
        .unwrap();
    let node = Node {
        id: "function:ctx_test".to_string(),
        kind: NodeKind::Function,
        name: "compute".to_string(),
        qualified_name: "src/lib.rs::compute".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("pub fn compute()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    };
    db.insert_node(&node).await.unwrap();

    let builder = ContextBuilder::new(&db, project);
    let subgraph = builder
        .find_relevant_context("compute", &BuildContextOptions::default())
        .await
        .unwrap();
    assert!(!subgraph.nodes.is_empty());
}

#[tokio::test]
async fn test_exclude_node_ids_deduplication() {
    use tempfile::TempDir;
    use tokensave::context::ContextBuilder;
    use tokensave::db::Database;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let (db, _) = Database::initialize(&project.join(".tokensave/tokensave.db"))
        .await
        .unwrap();

    for (id, name) in [("fn:first", "compute"), ("fn:second", "compute_batch")] {
        db.insert_node(&Node {
            id: id.to_string(),
            kind: NodeKind::Function,
            name: name.to_string(),
            qualified_name: format!("src/lib.rs::{name}"),
            file_path: "src/lib.rs".to_string(),
            start_line: 1,
            attrs_start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 1,
            signature: Some(format!("pub fn {name}()")),
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            updated_at: 0,
        })
        .await
        .unwrap();
    }

    let builder = ContextBuilder::new(&db, project);

    // First call — fn:first should appear
    let opts = BuildContextOptions::default();
    let ctx1 = builder.build_context("compute", &opts).await.unwrap();
    assert!(ctx1.entry_points.iter().any(|n| n.id == "fn:first"));

    // Second call — exclude fn:first
    let opts2 = BuildContextOptions {
        exclude_node_ids: vec!["fn:first".to_string()].into_iter().collect(),
        ..Default::default()
    };
    let ctx2 = builder.build_context("compute", &opts2).await.unwrap();
    assert!(
        !ctx2.entry_points.iter().any(|n| n.id == "fn:first"),
        "excluded node should not appear in second call"
    );
}

#[tokio::test]
async fn test_merge_adjacent_code_blocks() {
    use std::fs;
    use tempfile::TempDir;
    use tokensave::context::ContextBuilder;
    use tokensave::db::Database;

    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "fn alpha() {}\n\nfn beta() {}\n\nfn gamma() {}\n",
    )
    .unwrap();

    let (db, _) = Database::initialize(&project.join(".tokensave/tokensave.db"))
        .await
        .unwrap();

    // Two adjacent functions in same file
    db.insert_node(&Node {
        id: "fn:alpha".to_string(),
        kind: NodeKind::Function,
        name: "alpha".to_string(),
        qualified_name: "src/lib.rs::alpha".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 1,
        start_column: 0,
        end_column: 13,
        signature: Some("fn alpha()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    })
    .await
    .unwrap();

    db.insert_node(&Node {
        id: "fn:beta".to_string(),
        kind: NodeKind::Function,
        name: "beta".to_string(),
        qualified_name: "src/lib.rs::beta".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 3,
        attrs_start_line: 3,
        end_line: 3,
        start_column: 0,
        end_column: 12,
        signature: Some("fn beta()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    })
    .await
    .unwrap();

    let builder = ContextBuilder::new(&db, project);
    let ctx = builder
        .build_context(
            "alpha beta",
            &BuildContextOptions {
                include_code: true,
                merge_adjacent: true,
                max_code_blocks: 10,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // With merge_adjacent, the two adjacent blocks should merge into one
    assert_eq!(
        ctx.code_blocks.len(),
        1,
        "adjacent blocks should merge into one, got {}",
        ctx.code_blocks.len()
    );
    assert!(ctx.code_blocks[0].content.contains("alpha"));
    assert!(ctx.code_blocks[0].content.contains("beta"));
}
