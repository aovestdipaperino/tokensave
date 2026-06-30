#![cfg(feature = "lang-sql")]

use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::SqlExtractor;
use tokensave::types::*;

#[test]
fn test_sql_file_node() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sql").unwrap();
    let extractor = SqlExtractor;
    let result = extractor.extract("sample.sql", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.sql");
}

#[test]
fn test_sql_extract_table_classes() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sql").unwrap();
    let extractor = SqlExtractor;
    let result = extractor.extract("sample.sql", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(
        classes.len(),
        3,
        "expected 3 class nodes (2 tables + 1 view), got {}",
        classes.len()
    );
    assert!(classes.iter().any(|n| n.name == "users"));
    assert!(classes.iter().any(|n| n.name == "orders"));
    assert!(
        classes.iter().any(|n| n.name == "active_users"),
        "VIEW should be extracted as Class"
    );
}

#[test]
fn test_sql_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sql").unwrap();
    let extractor = SqlExtractor;
    let result = extractor.extract("sample.sql", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert_eq!(
        contains.len(),
        3,
        "expected 3 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_sql_empty_source_produces_file_only() {
    let extractor = SqlExtractor;
    let result = extractor.extract("empty.sql", "    ");
    // Should produce only the File node even for whitespace-only input
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let nodes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(nodes.len(), 1);
}

#[test]
fn test_sql_empty_comment_source() {
    let extractor = SqlExtractor;
    let result = extractor.extract("comment.sql", "-- This is just a comment\n");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_sql_extensions() {
    let extractor = SqlExtractor;
    assert!(extractor.extensions().contains(&"sql"));
}

#[test]
fn test_sql_language_name() {
    let extractor = SqlExtractor;
    assert_eq!(extractor.language_name(), "SQL");
}

#[test]
fn test_sql_all_edges_are_contains() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sql").unwrap();
    let extractor = SqlExtractor;
    let result = extractor.extract("sample.sql", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    for edge in &result.edges {
        assert_eq!(
            edge.kind,
            EdgeKind::Contains,
            "all SQL edges should be Contains"
        );
    }
}

#[test]
fn test_sql_no_unresolved_refs() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sql").unwrap();
    let extractor = SqlExtractor;
    let result = extractor.extract("sample.sql", &source);
    assert!(result.unresolved_refs.is_empty());
}
