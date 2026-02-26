use codegraph::context::*;
use codegraph::types::*;

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
    };
    let json = format_context_as_json(&context);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["query"], "test");
}

#[test]
fn test_build_context_with_db() {
    use codegraph::context::ContextBuilder;
    use codegraph::db::Database;
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    // Create a source file
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn process_data() {}\n").unwrap();

    // Init DB and insert a node
    let db = Database::initialize(&project.join(".codegraph/codegraph.db")).unwrap();
    let node = Node {
        id: "function:test123".to_string(),
        kind: NodeKind::Function,
        name: "process_data".to_string(),
        qualified_name: "src/lib.rs::process_data".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        end_line: 1,
        start_column: 0,
        end_column: 24,
        signature: Some("pub fn process_data()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        updated_at: 0,
    };
    db.insert_node(&node).unwrap();

    let builder = ContextBuilder::new(&db, project);
    let result = builder.build_context("process_data", &BuildContextOptions::default());
    assert!(result.is_ok());
    let ctx = result.unwrap();
    assert!(!ctx.entry_points.is_empty());
}

#[test]
fn test_get_code_reads_source_file() {
    use codegraph::context::ContextBuilder;
    use codegraph::db::Database;
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();

    let db = Database::initialize(&project.join(".codegraph/codegraph.db")).unwrap();

    let node = Node {
        id: "function:main123".to_string(),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 3,
        start_column: 0,
        end_column: 1,
        signature: Some("fn main()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        updated_at: 0,
    };

    let builder = ContextBuilder::new(&db, project);
    let code = builder.get_code(&node).unwrap();
    assert!(code.is_some());
    let content = code.unwrap();
    assert!(content.contains("fn main()"));
    assert!(content.contains("println!"));
}

#[test]
fn test_get_code_returns_none_for_missing_file() {
    use codegraph::context::ContextBuilder;
    use codegraph::db::Database;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let db = Database::initialize(&project.join(".codegraph/codegraph.db")).unwrap();

    let node = Node {
        id: "function:missing".to_string(),
        kind: NodeKind::Function,
        name: "missing".to_string(),
        qualified_name: "nonexistent.rs::missing".to_string(),
        file_path: "nonexistent.rs".to_string(),
        start_line: 1,
        end_line: 1,
        start_column: 0,
        end_column: 10,
        signature: None,
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        updated_at: 0,
    };

    let builder = ContextBuilder::new(&db, project);
    let code = builder.get_code(&node).unwrap();
    assert!(code.is_none());
}

#[test]
fn test_find_relevant_context() {
    use codegraph::context::ContextBuilder;
    use codegraph::db::Database;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let project = dir.path();

    let db = Database::initialize(&project.join(".codegraph/codegraph.db")).unwrap();
    let node = Node {
        id: "function:ctx_test".to_string(),
        kind: NodeKind::Function,
        name: "compute".to_string(),
        qualified_name: "src/lib.rs::compute".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("pub fn compute()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        updated_at: 0,
    };
    db.insert_node(&node).unwrap();

    let builder = ContextBuilder::new(&db, project);
    let subgraph = builder
        .find_relevant_context("compute", &BuildContextOptions::default())
        .unwrap();
    assert!(!subgraph.nodes.is_empty());
}
