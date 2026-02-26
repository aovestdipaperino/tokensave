use codegraph::db::Database;
use codegraph::resolution::ReferenceResolver;
use codegraph::types::*;
use tempfile::TempDir;

/// Sets up a temporary database pre-populated with two nodes: a `helper`
/// function in `src/utils.rs` and a `main` function in `src/main.rs`.
fn setup_db_with_nodes() -> (TempDir, Database) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db = Database::initialize(&dir.path().join("test.db")).expect("failed to init db");

    let callee = Node {
        id: generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
        kind: NodeKind::Function,
        name: "helper".to_string(),
        qualified_name: "src/utils.rs::helper".to_string(),
        file_path: "src/utils.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn helper() -> i32".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        updated_at: 0,
    };

    let caller = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn main()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        updated_at: 0,
    };

    db.insert_node(&callee).expect("failed to insert callee");
    db.insert_node(&caller).expect("failed to insert caller");
    (dir, db)
}

#[test]
fn test_resolve_exact_name_match() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let uref = UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_some(), "should resolve the helper reference");
    let resolved = result.unwrap();
    assert!(
        resolved.confidence >= 0.7,
        "confidence should be at least 0.7, got {}",
        resolved.confidence
    );
    assert_eq!(
        resolved.target_node_id,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
    );
}

#[test]
fn test_resolve_qualified_name_match() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let uref = UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "src/utils.rs::helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_some(), "should resolve via qualified name match");
    let resolved = result.unwrap();
    assert!(
        (resolved.confidence - 0.95).abs() < f64::EPSILON,
        "qualified match should have confidence 0.95, got {}",
        resolved.confidence
    );
    assert_eq!(resolved.resolved_by, "qualified-match");
}

#[test]
fn test_resolve_all() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let refs = vec![UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    }];

    let result = resolver.resolve_all(&refs);
    assert_eq!(result.total, 1);
    assert_eq!(result.resolved_count, 1);
    assert_eq!(result.resolved.len(), 1);
    assert!(result.unresolved.is_empty());
}

#[test]
fn test_unresolvable_reference() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let uref = UnresolvedRef {
        from_node_id: "function:caller".to_string(),
        reference_name: "nonexistent".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 5,
        column: 8,
        file_path: "src/main.rs".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref).is_none(),
        "nonexistent reference should not resolve"
    );
}

#[test]
fn test_unresolvable_in_resolve_all() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let refs = vec![
        UnresolvedRef {
            from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
            reference_name: "helper".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 3,
            column: 12,
            file_path: "src/main.rs".to_string(),
        },
        UnresolvedRef {
            from_node_id: "function:caller".to_string(),
            reference_name: "nonexistent".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 5,
            column: 8,
            file_path: "src/main.rs".to_string(),
        },
    ];

    let result = resolver.resolve_all(&refs);
    assert_eq!(result.total, 2);
    assert_eq!(result.resolved_count, 1);
    assert_eq!(result.unresolved.len(), 1);
    assert_eq!(result.unresolved[0].reference_name, "nonexistent");
}

#[test]
fn test_creates_edges_from_resolved() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let resolved = ResolvedRef {
        original: UnresolvedRef {
            from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
            reference_name: "helper".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 3,
            column: 12,
            file_path: "src/main.rs".to_string(),
        },
        target_node_id: generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
        confidence: 0.9,
        resolved_by: "exact-match".to_string(),
    };

    let edges = resolver.create_edges(&[resolved]);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].kind, EdgeKind::Calls);
    assert_eq!(edges[0].line, Some(3));
    assert_eq!(
        edges[0].source,
        generate_node_id("src/main.rs", &NodeKind::Function, "main", 1)
    );
    assert_eq!(
        edges[0].target,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1)
    );
}

#[test]
fn test_multiple_candidates_best_match_scoring() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db = Database::initialize(&dir.path().join("test.db")).expect("failed to init db");

    // Two nodes with the same name "process" in different files.
    let same_file_node = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "process", 10),
        kind: NodeKind::Function,
        name: "process".to_string(),
        qualified_name: "src/main.rs::process".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 10,
        end_line: 15,
        start_column: 0,
        end_column: 1,
        signature: Some("fn process()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        updated_at: 0,
    };

    let other_file_node = Node {
        id: generate_node_id("src/other.rs", &NodeKind::Function, "process", 1),
        kind: NodeKind::Function,
        name: "process".to_string(),
        qualified_name: "src/other.rs::process".to_string(),
        file_path: "src/other.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn process()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        updated_at: 0,
    };

    let caller = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "run", 1),
        kind: NodeKind::Function,
        name: "run".to_string(),
        qualified_name: "src/main.rs::run".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn run()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        updated_at: 0,
    };

    db.insert_node(&same_file_node)
        .expect("failed to insert same_file_node");
    db.insert_node(&other_file_node)
        .expect("failed to insert other_file_node");
    db.insert_node(&caller).expect("failed to insert caller");

    let resolver = ReferenceResolver::new(&db);

    // Reference from src/main.rs should prefer the same-file candidate.
    let uref = UnresolvedRef {
        from_node_id: caller.id.clone(),
        reference_name: "process".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 4,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_some(), "should resolve with multiple candidates");
    let resolved = result.unwrap();
    assert_eq!(
        resolved.target_node_id, same_file_node.id,
        "should prefer the same-file candidate"
    );
    assert!(
        (resolved.confidence - 0.7).abs() < f64::EPSILON,
        "multiple-match confidence should be 0.7, got {}",
        resolved.confidence
    );
}

#[test]
fn test_create_edges_empty_input() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let edges = resolver.create_edges(&[]);
    assert!(edges.is_empty());
}

#[test]
fn test_resolve_all_empty_input() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let result = resolver.resolve_all(&[]);
    assert_eq!(result.total, 0);
    assert_eq!(result.resolved_count, 0);
    assert!(result.resolved.is_empty());
    assert!(result.unresolved.is_empty());
}
