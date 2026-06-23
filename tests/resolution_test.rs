use tempfile::TempDir;
use tokensave::db::Database;
use tokensave::resolution::ReferenceResolver;
use tokensave::types::*;

/// Sets up a temporary database pre-populated with two nodes: a `helper`
/// function in `src/utils.rs` and a `main` function in `src/main.rs`.
async fn setup_db_with_nodes() -> (TempDir, Database) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let callee = Node {
        id: generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
        kind: NodeKind::Function,
        name: "helper".to_string(),
        qualified_name: "src/utils.rs::helper".to_string(),
        file_path: "src/utils.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn helper() -> i32".to_string()),
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
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    let caller = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
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
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    db.insert_node(&callee)
        .await
        .expect("failed to insert callee");
    db.insert_node(&caller)
        .await
        .expect("failed to insert caller");
    (dir, db)
}

#[tokio::test]
async fn test_resolve_exact_name_match() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

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

#[tokio::test]
async fn test_resolve_qualified_name_match() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

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

#[tokio::test]
async fn test_resolve_all() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

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

#[tokio::test]
async fn test_unresolvable_reference() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

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

#[tokio::test]
async fn test_unresolvable_in_resolve_all() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

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

#[tokio::test]
async fn test_creates_edges_from_resolved() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

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

#[tokio::test]
async fn test_multiple_candidates_best_match_scoring() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    // Two nodes with the same name "process" in different files.
    let same_file_node = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "process", 10),
        kind: NodeKind::Function,
        name: "process".to_string(),
        qualified_name: "src/main.rs::process".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 10,
        attrs_start_line: 10,
        end_line: 15,
        start_column: 0,
        end_column: 1,
        signature: Some("fn process()".to_string()),
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
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    let other_file_node = Node {
        id: generate_node_id("src/other.rs", &NodeKind::Function, "process", 1),
        kind: NodeKind::Function,
        name: "process".to_string(),
        qualified_name: "src/other.rs::process".to_string(),
        file_path: "src/other.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn process()".to_string()),
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
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    let caller = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "run", 1),
        kind: NodeKind::Function,
        name: "run".to_string(),
        qualified_name: "src/main.rs::run".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn run()".to_string()),
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
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    db.insert_node(&same_file_node)
        .await
        .expect("failed to insert same_file_node");
    db.insert_node(&other_file_node)
        .await
        .expect("failed to insert other_file_node");
    db.insert_node(&caller)
        .await
        .expect("failed to insert caller");

    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

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

#[tokio::test]
async fn test_create_edges_empty_input() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

    let edges = resolver.create_edges(&[]);
    assert!(edges.is_empty());
}

#[tokio::test]
async fn test_resolve_all_empty_input() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

    let result = resolver.resolve_all(&[]);
    assert_eq!(result.total, 0);
    assert_eq!(result.resolved_count, 0);
    assert!(result.resolved.is_empty());
    assert!(result.unresolved.is_empty());
}

/// #141 regression: `resolve_all`'s pre-filter must not drop a qualified
/// `Self::helper` (or `Type::helper`) ref just because the literal string
/// isn't a known name — its trailing simple name is, and `resolve_one`
/// strips the prefix and matches it. Previously these were silently lost.
#[tokio::test]
async fn test_resolve_all_self_qualified_call_not_dropped() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

    let refs = vec![UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "Self::helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    }];

    let result = resolver.resolve_all(&refs);
    assert_eq!(
        result.resolved_count, 1,
        "Self::helper should resolve via the simple-name fallback, not be pre-filtered as hopeless"
    );
    assert_eq!(
        result.resolved[0].target_node_id,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
    );
}

/// #141 cross-language: Python/TS extractors emit the full dotted callee
/// (`obj.helper`) with no bare-name ref. The resolver must fall back to the
/// trailing method name so the call edge still forms.
#[tokio::test]
async fn test_resolve_all_dotted_method_call() {
    let (_dir, db) = setup_db_with_nodes().await;
    let resolver = ReferenceResolver::from_nodes(&db, &db.get_all_nodes().await.unwrap());

    let refs = vec![UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "obj.helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    }];

    let result = resolver.resolve_all(&refs);
    assert_eq!(
        result.resolved_count, 1,
        "obj.helper should resolve to `helper` via the dotted-call fallback"
    );
    assert_eq!(
        result.resolved[0].target_node_id,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
    );
}

// ---------------------------------------------------------------------------
// #141 Option 2: build-variant call-edge propagation
// ---------------------------------------------------------------------------

fn variant_node(id: &str, kind: NodeKind, name: &str, qn: &str, file: &str) -> Node {
    Node {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        qualified_name: qn.to_string(),
        file_path: file.to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some(format!("fn {name}()")),
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
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    }
}

fn calls_edge(from: &str, to: &str) -> Edge {
    Edge {
        source: from.to_string(),
        target: to.to_string(),
        kind: EdgeKind::Calls,
        line: Some(1),
    }
}

/// Rust `#[cfg]` twins (same qualified_name, both cfg-gated): a call landing on
/// one variant must propagate to the other so neither looks dead.
#[test]
fn test_variant_fanout_rust_cfg() {
    let nodes = vec![
        variant_node(
            "fn:caller",
            NodeKind::Function,
            "main",
            "src/main.rs::main",
            "src/main.rs",
        ),
        variant_node(
            "fn:macos",
            NodeKind::Function,
            "copy",
            "src/c.rs::copy",
            "src/c.rs",
        ),
        variant_node(
            "fn:other",
            NodeKind::Function,
            "copy",
            "src/c.rs::copy",
            "src/c.rs",
        ),
        variant_node(
            "au:1",
            NodeKind::AnnotationUsage,
            "cfg",
            "src/c.rs::cfg",
            "src/c.rs",
        ),
        variant_node(
            "au:2",
            NodeKind::AnnotationUsage,
            "cfg",
            "src/c.rs::cfg",
            "src/c.rs",
        ),
    ];
    let edges = vec![
        Edge {
            source: "au:1".into(),
            target: "fn:macos".into(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        Edge {
            source: "au:2".into(),
            target: "fn:other".into(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        calls_edge("fn:caller", "fn:macos"),
    ];
    let extra = tokensave::resolution::propagate_variant_edges(&nodes, &edges);
    assert!(
        extra.iter().any(|e| e.source == "fn:caller"
            && e.target == "fn:other"
            && e.kind == EdgeKind::Calls),
        "call should propagate to the cfg sibling, got: {extra:?}"
    );
}

/// Go platform files (`foo_linux.go` / `foo_windows.go`): same package
/// directory + function name across different files = build variants.
#[test]
fn test_variant_fanout_go_platform_files() {
    let nodes = vec![
        variant_node(
            "fn:caller",
            NodeKind::Function,
            "Main",
            "pkg/main.go::Main",
            "pkg/main.go",
        ),
        variant_node(
            "fn:linux",
            NodeKind::Function,
            "Do",
            "pkg/foo_linux.go::Do",
            "pkg/foo_linux.go",
        ),
        variant_node(
            "fn:win",
            NodeKind::Function,
            "Do",
            "pkg/foo_windows.go::Do",
            "pkg/foo_windows.go",
        ),
    ];
    let edges = vec![calls_edge("fn:caller", "fn:linux")];
    let extra = tokensave::resolution::propagate_variant_edges(&nodes, &edges);
    assert!(
        extra
            .iter()
            .any(|e| e.source == "fn:caller" && e.target == "fn:win"),
        "call should propagate to the windows platform-file sibling, got: {extra:?}"
    );
}

/// Negative: two functions sharing a qualified_name but NOT cfg-gated (e.g.
/// distinct trait impls) must NOT be fused — that would invent false edges.
#[test]
fn test_no_fanout_without_cfg() {
    let nodes = vec![
        variant_node(
            "fn:caller",
            NodeKind::Function,
            "main",
            "src/main.rs::main",
            "src/main.rs",
        ),
        variant_node(
            "m:a",
            NodeKind::Method,
            "from",
            "src/t.rs::T::from",
            "src/t.rs",
        ),
        variant_node(
            "m:b",
            NodeKind::Method,
            "from",
            "src/t.rs::T::from",
            "src/t.rs",
        ),
    ];
    let edges = vec![calls_edge("fn:caller", "m:a")];
    let extra = tokensave::resolution::propagate_variant_edges(&nodes, &edges);
    assert!(
        extra.is_empty(),
        "non-cfg same-qualified-name nodes must not fan out, got: {extra:?}"
    );
}
