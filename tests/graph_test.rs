use codegraph::db::Database;
use codegraph::graph::queries::GraphQueryManager;
use codegraph::graph::traversal::GraphTraverser;
use codegraph::types::*;
use tempfile::TempDir;

/// Helper: create a temp database and return (Database, TempDir).
fn setup_db() -> (Database, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let db = Database::initialize(&db_path).expect("failed to initialize database");
    (db, dir)
}

/// Helper: create a function node with sensible defaults.
fn make_node(id: &str, name: &str, file_path: &str, visibility: Visibility) -> Node {
    Node {
        id: id.to_string(),
        kind: NodeKind::Function,
        name: name.to_string(),
        qualified_name: format!("crate::{name}"),
        file_path: file_path.to_string(),
        start_line: 1,
        end_line: 10,
        start_column: 0,
        end_column: 1,
        signature: Some(format!("fn {name}()")),
        docstring: None,
        visibility,
        is_async: false,
        updated_at: 1000,
    }
}

/// Sets up a call chain: main -> process -> validate -> check.
/// Returns the database and temp dir.
fn setup_call_chain() -> (Database, TempDir) {
    let (db, dir) = setup_db();

    let main_node = make_node("n-main", "main", "src/main.rs", Visibility::Pub);
    let process_node = make_node("n-process", "process", "src/main.rs", Visibility::Pub);
    let validate_node = make_node("n-validate", "validate", "src/lib.rs", Visibility::Pub);
    let check_node = make_node("n-check", "check", "src/lib.rs", Visibility::Pub);

    db.insert_nodes(&[main_node, process_node, validate_node, check_node])
        .expect("failed to insert nodes");

    let edges = vec![
        Edge {
            source: "n-main".to_string(),
            target: "n-process".to_string(),
            kind: EdgeKind::Calls,
            line: Some(5),
        },
        Edge {
            source: "n-process".to_string(),
            target: "n-validate".to_string(),
            kind: EdgeKind::Calls,
            line: Some(10),
        },
        Edge {
            source: "n-validate".to_string(),
            target: "n-check".to_string(),
            kind: EdgeKind::Calls,
            line: Some(15),
        },
    ];
    db.insert_edges(&edges).expect("failed to insert edges");

    (db, dir)
}

// ---------------------------------------------------------------------------
// Traversal tests
// ---------------------------------------------------------------------------

#[test]
fn test_get_callers() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let callers = traverser
        .get_callers("n-process", 5)
        .expect("get_callers failed");

    // Direct caller of "process" is "main".
    assert!(
        !callers.is_empty(),
        "process should have at least one caller"
    );
    let caller_names: Vec<&str> = callers.iter().map(|(n, _)| n.name.as_str()).collect();
    assert!(
        caller_names.contains(&"main"),
        "callers of process should include main, got: {caller_names:?}"
    );
}

#[test]
fn test_get_callees() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let callees = traverser
        .get_callees("n-process", 5)
        .expect("get_callees failed");

    let callee_names: Vec<&str> = callees.iter().map(|(n, _)| n.name.as_str()).collect();
    assert!(
        callee_names.contains(&"validate"),
        "callees of process should include validate, got: {callee_names:?}"
    );
}

#[test]
fn test_get_callees_transitive() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let callees = traverser
        .get_callees("n-process", 5)
        .expect("get_callees failed");

    let callee_names: Vec<&str> = callees.iter().map(|(n, _)| n.name.as_str()).collect();
    assert!(
        callee_names.contains(&"validate"),
        "callees should include validate"
    );
    assert!(
        callee_names.contains(&"check"),
        "callees should transitively include check"
    );
}

#[test]
fn test_impact_radius() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let subgraph = traverser
        .get_impact_radius("n-check", 10)
        .expect("get_impact_radius failed");

    let node_names: Vec<&str> = subgraph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        node_names.contains(&"validate"),
        "impact of check should include validate, got: {node_names:?}"
    );
    assert!(
        node_names.contains(&"process"),
        "impact of check should include process, got: {node_names:?}"
    );
    assert!(
        node_names.contains(&"main"),
        "impact of check should include main, got: {node_names:?}"
    );
}

#[test]
fn test_call_graph_bidirectional() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let subgraph = traverser
        .get_call_graph("n-process", 5)
        .expect("get_call_graph failed");

    let node_names: Vec<&str> = subgraph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        node_names.contains(&"main"),
        "call graph of process should include caller 'main', got: {node_names:?}"
    );
    assert!(
        node_names.contains(&"validate"),
        "call graph of process should include callee 'validate', got: {node_names:?}"
    );
    assert!(
        node_names.contains(&"process"),
        "call graph should include the center node 'process', got: {node_names:?}"
    );
}

#[test]
fn test_bfs_traversal_with_depth_limit() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let opts = TraversalOptions {
        max_depth: 1,
        edge_kinds: Some(vec![EdgeKind::Calls]),
        node_kinds: None,
        direction: TraversalDirection::Outgoing,
        limit: 100,
        include_start: true,
    };

    let subgraph = traverser
        .traverse_bfs("n-main", &opts)
        .expect("traverse_bfs failed");

    let node_names: Vec<&str> = subgraph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        node_names.contains(&"main"),
        "depth-1 from main should include main itself"
    );
    assert!(
        node_names.contains(&"process"),
        "depth-1 from main should include process"
    );
    assert!(
        !node_names.contains(&"validate"),
        "depth-1 from main should NOT include validate (that is depth 2)"
    );
    assert!(
        !node_names.contains(&"check"),
        "depth-1 from main should NOT include check (that is depth 3)"
    );
}

#[test]
fn test_bfs_traversal_full_depth() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let opts = TraversalOptions {
        max_depth: 10,
        edge_kinds: Some(vec![EdgeKind::Calls]),
        node_kinds: None,
        direction: TraversalDirection::Outgoing,
        limit: 100,
        include_start: true,
    };

    let subgraph = traverser
        .traverse_bfs("n-main", &opts)
        .expect("traverse_bfs failed");

    assert_eq!(
        subgraph.nodes.len(),
        4,
        "full-depth BFS from main should include all 4 nodes"
    );
}

#[test]
fn test_dfs_traversal() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let opts = TraversalOptions {
        max_depth: 10,
        edge_kinds: Some(vec![EdgeKind::Calls]),
        node_kinds: None,
        direction: TraversalDirection::Outgoing,
        limit: 100,
        include_start: true,
    };

    let subgraph = traverser
        .traverse_dfs("n-main", &opts)
        .expect("traverse_dfs failed");

    assert_eq!(
        subgraph.nodes.len(),
        4,
        "full-depth DFS from main should include all 4 nodes"
    );
}

#[test]
fn test_find_path() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let path = traverser
        .find_path("n-main", "n-check", &[EdgeKind::Calls])
        .expect("find_path failed")
        .expect("path should exist from main to check");

    assert!(
        path.len() >= 2,
        "path from main to check should have at least 2 entries"
    );
    assert_eq!(path[0].0.name, "main", "path should start with main");
    assert_eq!(
        path.last().unwrap().0.name,
        "check",
        "path should end with check"
    );
}

#[test]
fn test_find_path_no_route() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    // check -> main has no path via outgoing Calls edges (only reverse direction).
    // But find_path searches bidirectionally. Let's test with a disconnected node.
    let orphan = make_node("n-orphan", "orphan", "src/orphan.rs", Visibility::Private);
    db.insert_node(&orphan).expect("insert orphan failed");

    let path = traverser
        .find_path("n-main", "n-orphan", &[EdgeKind::Calls])
        .expect("find_path failed");

    assert!(
        path.is_none(),
        "there should be no path from main to an orphan node"
    );
}

#[test]
fn test_find_path_same_node() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let path = traverser
        .find_path("n-main", "n-main", &[])
        .expect("find_path failed")
        .expect("path from a node to itself should exist");

    assert_eq!(path.len(), 1, "path from main to main should have 1 entry");
    assert_eq!(path[0].0.name, "main");
}

// ---------------------------------------------------------------------------
// Query tests
// ---------------------------------------------------------------------------

#[test]
fn test_find_dead_code() {
    let (db, _dir) = setup_call_chain();

    // Add an orphan private function with no incoming edges.
    let orphan = make_node(
        "n-orphan",
        "unused_helper",
        "src/util.rs",
        Visibility::Private,
    );
    db.insert_node(&orphan).expect("insert orphan failed");

    let qm = GraphQueryManager::new(&db);
    let dead = qm.find_dead_code(&[]).expect("find_dead_code failed");

    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();
    assert!(
        dead_names.contains(&"unused_helper"),
        "orphan private function should be detected as dead code, got: {dead_names:?}"
    );
    // "main" should NOT be in the dead code list.
    assert!(
        !dead_names.contains(&"main"),
        "main should not be reported as dead code"
    );
}

#[test]
fn test_find_dead_code_excludes_pub() {
    let (db, _dir) = setup_db();

    // A public function with no incoming edges should not be flagged.
    let pub_node = make_node("n-pub", "public_api", "src/api.rs", Visibility::Pub);
    db.insert_node(&pub_node).expect("insert pub_node failed");

    let qm = GraphQueryManager::new(&db);
    let dead = qm.find_dead_code(&[]).expect("find_dead_code failed");

    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();
    assert!(
        !dead_names.contains(&"public_api"),
        "pub functions should not be reported as dead code"
    );
}

#[test]
fn test_find_dead_code_with_kind_filter() {
    let (db, _dir) = setup_db();

    let func_node = make_node("n-func", "private_func", "src/lib.rs", Visibility::Private);
    let mut struct_node = make_node("n-struct", "MyStruct", "src/lib.rs", Visibility::Private);
    struct_node.kind = NodeKind::Struct;

    db.insert_nodes(&[func_node, struct_node])
        .expect("insert nodes failed");

    let qm = GraphQueryManager::new(&db);

    // Filter to only Function kind.
    let dead = qm
        .find_dead_code(&[NodeKind::Function])
        .expect("find_dead_code failed");

    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();
    assert!(
        dead_names.contains(&"private_func"),
        "private_func should be dead code"
    );
    assert!(
        !dead_names.contains(&"MyStruct"),
        "MyStruct should not appear when filtering by Function kind"
    );
}

#[test]
fn test_get_node_metrics() {
    let (db, _dir) = setup_call_chain();
    let qm = GraphQueryManager::new(&db);

    let metrics = qm
        .get_node_metrics("n-process")
        .expect("get_node_metrics failed");

    // process has 1 incoming Calls (from main) and 1 outgoing Calls (to validate).
    assert_eq!(metrics.caller_count, 1, "process should have 1 caller");
    assert_eq!(metrics.call_count, 1, "process should have 1 callee");
    assert_eq!(
        metrics.incoming_edge_count, 1,
        "process should have 1 incoming edge total"
    );
    assert_eq!(
        metrics.outgoing_edge_count, 1,
        "process should have 1 outgoing edge total"
    );
}

#[test]
fn test_get_file_dependencies() {
    let (db, _dir) = setup_call_chain();
    let qm = GraphQueryManager::new(&db);

    // src/main.rs has process -> validate (in src/lib.rs), so it depends on src/lib.rs.
    let deps = qm
        .get_file_dependencies("src/main.rs")
        .expect("get_file_dependencies failed");

    assert!(
        deps.contains(&"src/lib.rs".to_string()),
        "src/main.rs should depend on src/lib.rs, got: {deps:?}"
    );
}

#[test]
fn test_get_file_dependents() {
    let (db, _dir) = setup_call_chain();
    let qm = GraphQueryManager::new(&db);

    // src/lib.rs is called from src/main.rs (process -> validate).
    let dependents = qm
        .get_file_dependents("src/lib.rs")
        .expect("get_file_dependents failed");

    assert!(
        dependents.contains(&"src/main.rs".to_string()),
        "src/lib.rs should be depended on by src/main.rs, got: {dependents:?}"
    );
}

#[test]
fn test_find_circular_dependencies() {
    let (db, _dir) = setup_db();

    // Set up a circular dependency: file_a -> file_b -> file_a.
    let node_a = make_node("n-a", "func_a", "src/a.rs", Visibility::Pub);
    let node_b = make_node("n-b", "func_b", "src/b.rs", Visibility::Pub);

    db.insert_nodes(&[node_a, node_b])
        .expect("insert nodes failed");

    // a calls b, b calls a -> circular.
    let edges = vec![
        Edge {
            source: "n-a".to_string(),
            target: "n-b".to_string(),
            kind: EdgeKind::Calls,
            line: Some(1),
        },
        Edge {
            source: "n-b".to_string(),
            target: "n-a".to_string(),
            kind: EdgeKind::Calls,
            line: Some(1),
        },
    ];
    db.insert_edges(&edges).expect("insert edges failed");

    // Register files so they show up in get_all_files.
    let file_a = codegraph::types::FileRecord {
        path: "src/a.rs".to_string(),
        content_hash: "hash_a".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 1,
    };
    let file_b = codegraph::types::FileRecord {
        path: "src/b.rs".to_string(),
        content_hash: "hash_b".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 1,
    };
    db.upsert_file(&file_a).expect("upsert file_a failed");
    db.upsert_file(&file_b).expect("upsert file_b failed");

    let qm = GraphQueryManager::new(&db);
    let cycles = qm
        .find_circular_dependencies()
        .expect("find_circular_dependencies failed");

    assert!(
        !cycles.is_empty(),
        "should detect at least one circular dependency"
    );

    // Verify the cycle contains both files.
    let cycle_files: Vec<&str> = cycles[0].iter().map(|s| s.as_str()).collect();
    assert!(
        cycle_files.contains(&"src/a.rs") && cycle_files.contains(&"src/b.rs"),
        "cycle should contain both src/a.rs and src/b.rs, got: {cycle_files:?}"
    );
}

#[test]
fn test_type_hierarchy() {
    let (db, _dir) = setup_db();

    let mut trait_node = make_node("n-trait", "MyTrait", "src/lib.rs", Visibility::Pub);
    trait_node.kind = NodeKind::Trait;
    let mut struct_node = make_node("n-struct", "MyStruct", "src/lib.rs", Visibility::Pub);
    struct_node.kind = NodeKind::Struct;
    let mut impl_node = make_node("n-impl", "impl_block", "src/lib.rs", Visibility::Private);
    impl_node.kind = NodeKind::Impl;

    db.insert_nodes(&[trait_node, struct_node, impl_node])
        .expect("insert nodes failed");

    let edge = Edge {
        source: "n-impl".to_string(),
        target: "n-trait".to_string(),
        kind: EdgeKind::Implements,
        line: None,
    };
    db.insert_edge(&edge).expect("insert edge failed");

    let traverser = GraphTraverser::new(&db);
    let subgraph = traverser
        .get_type_hierarchy("n-trait")
        .expect("get_type_hierarchy failed");

    let node_names: Vec<&str> = subgraph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        node_names.contains(&"MyTrait"),
        "hierarchy should contain the trait"
    );
    assert!(
        node_names.contains(&"impl_block"),
        "hierarchy should contain the impl that implements the trait"
    );
}

#[test]
fn test_traversal_with_limit() {
    let (db, _dir) = setup_call_chain();
    let traverser = GraphTraverser::new(&db);

    let opts = TraversalOptions {
        max_depth: 10,
        edge_kinds: Some(vec![EdgeKind::Calls]),
        node_kinds: None,
        direction: TraversalDirection::Outgoing,
        limit: 2,
        include_start: true,
    };

    let subgraph = traverser
        .traverse_bfs("n-main", &opts)
        .expect("traverse_bfs with limit failed");

    assert!(
        subgraph.nodes.len() <= 2,
        "limit=2 should cap the result to at most 2 nodes, got: {}",
        subgraph.nodes.len()
    );
}

#[test]
fn test_traversal_nonexistent_start() {
    let (db, _dir) = setup_db();
    let traverser = GraphTraverser::new(&db);

    let opts = TraversalOptions::default();
    let subgraph = traverser
        .traverse_bfs("nonexistent", &opts)
        .expect("traverse_bfs should not error on missing start");

    assert!(
        subgraph.nodes.is_empty(),
        "traversal from nonexistent node should return empty subgraph"
    );
}

#[test]
fn test_node_metrics_depth() {
    let (db, _dir) = setup_db();

    // Build a containment hierarchy: file -> module -> function.
    let mut file_node = make_node("n-file", "main.rs", "src/main.rs", Visibility::Pub);
    file_node.kind = NodeKind::File;

    let mut module_node = make_node("n-module", "utils", "src/main.rs", Visibility::Pub);
    module_node.kind = NodeKind::Module;

    let func_node = make_node("n-func", "helper", "src/main.rs", Visibility::Private);

    db.insert_nodes(&[file_node, module_node, func_node])
        .expect("insert nodes failed");

    let edges = vec![
        Edge {
            source: "n-file".to_string(),
            target: "n-module".to_string(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "n-module".to_string(),
            target: "n-func".to_string(),
            kind: EdgeKind::Contains,
            line: None,
        },
    ];
    db.insert_edges(&edges).expect("insert edges failed");

    let qm = GraphQueryManager::new(&db);

    let file_metrics = qm.get_node_metrics("n-file").expect("metrics failed");
    assert_eq!(file_metrics.depth, 0, "file should be at depth 0");
    assert_eq!(
        file_metrics.child_count, 1,
        "file should have 1 child (module)"
    );

    let module_metrics = qm.get_node_metrics("n-module").expect("metrics failed");
    assert_eq!(module_metrics.depth, 1, "module should be at depth 1");

    let func_metrics = qm.get_node_metrics("n-func").expect("metrics failed");
    assert_eq!(func_metrics.depth, 2, "function should be at depth 2");
}
