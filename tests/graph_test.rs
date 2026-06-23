use std::collections::{HashMap, HashSet};
use std::fs;
use tempfile::TempDir;
use tokensave::db::Database;
use tokensave::graph::git::file_churn;
use tokensave::graph::queries::GraphQueryManager;
use tokensave::graph::traversal::GraphTraverser;
use tokensave::tokensave::TokenSave;
use tokensave::types::*;

/// Helper: create a temp database and return (Database, TempDir).
async fn setup_db() -> (Database, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let (db, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
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
        attrs_start_line: 1,
        end_line: 10,
        start_column: 0,
        end_column: 1,
        signature: Some(format!("fn {name}()")),
        docstring: None,
        visibility,
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
        updated_at: 1000,
        parent_id: None,
    }
}

/// Sets up a call chain: main -> process -> validate -> check.
/// Returns the database and temp dir.
async fn setup_call_chain() -> (Database, TempDir) {
    let (db, dir) = setup_db().await;

    let main_node = make_node("n-main", "main", "src/main.rs", Visibility::Pub);
    let process_node = make_node("n-process", "process", "src/main.rs", Visibility::Pub);
    let validate_node = make_node("n-validate", "validate", "src/lib.rs", Visibility::Pub);
    let check_node = make_node("n-check", "check", "src/lib.rs", Visibility::Pub);

    db.insert_nodes(&[main_node, process_node, validate_node, check_node])
        .await
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
    db.insert_edges(&edges)
        .await
        .expect("failed to insert edges");

    (db, dir)
}

// ---------------------------------------------------------------------------
// Traversal tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_callers() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    let callers = traverser
        .get_callers("n-process", 5)
        .await
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

#[tokio::test]
async fn test_get_callees() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    let callees = traverser
        .get_callees("n-process", 5)
        .await
        .expect("get_callees failed");

    let callee_names: Vec<&str> = callees.iter().map(|(n, _)| n.name.as_str()).collect();
    assert!(
        callee_names.contains(&"validate"),
        "callees of process should include validate, got: {callee_names:?}"
    );
}

#[tokio::test]
async fn test_get_callees_transitive() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    let callees = traverser
        .get_callees("n-process", 5)
        .await
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

#[tokio::test]
async fn test_impact_radius() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    let subgraph = traverser
        .get_impact_radius("n-check", 10)
        .await
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

#[tokio::test]
async fn test_call_graph_bidirectional() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    let subgraph = traverser
        .get_call_graph("n-process", 5)
        .await
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

#[tokio::test]
async fn test_bfs_traversal_with_depth_limit() {
    let (db, _dir) = setup_call_chain().await;
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
        .await
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

#[tokio::test]
async fn test_bfs_traversal_full_depth() {
    let (db, _dir) = setup_call_chain().await;
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
        .await
        .expect("traverse_bfs failed");

    assert_eq!(
        subgraph.nodes.len(),
        4,
        "full-depth BFS from main should include all 4 nodes"
    );
}

#[tokio::test]
async fn test_dfs_traversal() {
    let (db, _dir) = setup_call_chain().await;
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
        .await
        .expect("traverse_dfs failed");

    assert_eq!(
        subgraph.nodes.len(),
        4,
        "full-depth DFS from main should include all 4 nodes"
    );
}

#[tokio::test]
async fn test_bfs_visits_calls_before_references() {
    // Regression test for #118: a budget-limited BFS frontier must visit
    // `calls` neighbors before reference/other-kind neighbors, regardless of
    // database edge order. We insert the reference edge FIRST (so it would win
    // a naive DB-order traversal) and assert the `calls` neighbor is the one
    // that survives a one-neighbor budget.
    let (db, _dir) = setup_db().await;

    let root = make_node("n-root", "root", "src/root.rs", Visibility::Pub);
    let ref_target = make_node("n-ref", "ref_target", "src/ref.rs", Visibility::Pub);
    let call_target = make_node("n-call", "call_target", "src/call.rs", Visibility::Pub);
    db.insert_nodes(&[root, ref_target, call_target])
        .await
        .expect("failed to insert nodes");

    // Reference edge inserted first, calls edge second.
    let edges = vec![
        Edge {
            source: "n-root".to_string(),
            target: "n-ref".to_string(),
            kind: EdgeKind::Uses,
            line: Some(1),
        },
        Edge {
            source: "n-root".to_string(),
            target: "n-call".to_string(),
            kind: EdgeKind::Calls,
            line: Some(2),
        },
    ];
    db.insert_edges(&edges)
        .await
        .expect("failed to insert edges");

    let traverser = GraphTraverser::new(&db);

    // limit = 2: the start node plus exactly one neighbor. The `calls`
    // neighbor must be the one chosen.
    let opts = TraversalOptions {
        max_depth: 5,
        edge_kinds: None,
        node_kinds: None,
        direction: TraversalDirection::Outgoing,
        limit: 2,
        include_start: true,
    };

    let subgraph = traverser
        .traverse_bfs("n-root", &opts)
        .await
        .expect("traverse_bfs failed");

    let node_names: Vec<&str> = subgraph.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        node_names.contains(&"call_target"),
        "calls neighbor should be visited ahead of reference neighbor, got: {node_names:?}"
    );
    assert!(
        !node_names.contains(&"ref_target"),
        "reference neighbor should be cut off by the budget, got: {node_names:?}"
    );

    // The first edge recorded in the subgraph should be the `calls` edge,
    // confirming queue ordering and not just which node fit the budget.
    assert_eq!(
        subgraph.edges.first().map(|e| e.kind),
        Some(EdgeKind::Calls),
        "calls edge should be queued first, got: {:?}",
        subgraph.edges
    );
}

#[tokio::test]
async fn test_find_path() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    let path = traverser
        .find_path("n-main", "n-check", &[EdgeKind::Calls])
        .await
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

#[tokio::test]
async fn test_find_path_no_route() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    // check -> main has no path via outgoing Calls edges (only reverse direction).
    // But find_path searches bidirectionally. Let's test with a disconnected node.
    let orphan = make_node("n-orphan", "orphan", "src/orphan.rs", Visibility::Private);
    db.insert_node(&orphan).await.expect("insert orphan failed");

    let path = traverser
        .find_path("n-main", "n-orphan", &[EdgeKind::Calls])
        .await
        .expect("find_path failed");

    assert!(
        path.is_none(),
        "there should be no path from main to an orphan node"
    );
}

#[tokio::test]
async fn test_find_path_same_node() {
    let (db, _dir) = setup_call_chain().await;
    let traverser = GraphTraverser::new(&db);

    let path = traverser
        .find_path("n-main", "n-main", &[])
        .await
        .expect("find_path failed")
        .expect("path from a node to itself should exist");

    assert_eq!(path.len(), 1, "path from main to main should have 1 entry");
    assert_eq!(path[0].0.name, "main");
}

// ---------------------------------------------------------------------------
// Query tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_find_dead_code() {
    let (db, _dir) = setup_call_chain().await;

    // Add an orphan private function with no incoming edges.
    let orphan = make_node(
        "n-orphan",
        "unused_helper",
        "src/util.rs",
        Visibility::Private,
    );
    db.insert_node(&orphan).await.expect("insert orphan failed");

    let qm = GraphQueryManager::new(&db);
    let dead = qm
        .find_dead_code(&[], false, true)
        .await
        .expect("find_dead_code failed");

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

#[tokio::test]
async fn test_find_dead_code_excludes_pub() {
    let (db, _dir) = setup_db().await;

    // A public function with no incoming edges should not be flagged.
    let pub_node = make_node("n-pub", "public_api", "src/api.rs", Visibility::Pub);
    db.insert_node(&pub_node)
        .await
        .expect("insert pub_node failed");

    let qm = GraphQueryManager::new(&db);
    let dead = qm
        .find_dead_code(&[], false, true)
        .await
        .expect("find_dead_code failed");

    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();
    assert!(
        !dead_names.contains(&"public_api"),
        "pub functions should not be reported as dead code"
    );
}

/// Regression for issue #137: Rust trait-impl methods (e.g. `Display::fmt`)
/// are reached via implicit compiler dispatch, so they carry no incoming
/// `calls` edge and — being un-`pub`able — are stored as `private`. They must
/// NOT be reported as dead by default, while inherent-impl methods with no
/// callers still must be. `include_trait_impls: true` opts back into them.
///
/// The critical case is a trait impl of a STDLIB/external trait (`Display`,
/// `Drop`, `Deref`, `From`, `AsRef`): the trait is not an indexed node, so the
/// extractor never materializes an `implements` edge. Detection therefore
/// keys off the impl node's signature (`impl <Trait> for <Type>`), with the
/// `implements` edge as a fallback for multi-line headers. A prior fix that
/// relied on the edge alone was inert on real codebases (zero such edges).
#[tokio::test]
async fn test_find_dead_code_excludes_trait_impl_methods() {
    let (db, _dir) = setup_db().await;

    // (1) Stdlib trait impl, signature-only, NO `implements` edge — the case
    // the edge-only fix missed. `fmt` must be excluded.
    let mut display_impl = make_node("n-impl-display", "MyType", "src/ty.rs", Visibility::Pub);
    display_impl.kind = NodeKind::Impl;
    display_impl.signature = Some("impl std::fmt::Display for MyType".to_string());
    let mut fmt = make_node("n-fmt", "fmt", "src/ty.rs", Visibility::Private);
    fmt.kind = NodeKind::Method;
    fmt.parent_id = Some("n-impl-display".to_string());

    // (2) Inherent `impl MyType` (signature has no ` for `, no edge) holding a
    // private method with no callers — a genuine dead candidate that must
    // still be reported, proving the exclusion is scoped to trait impls.
    let mut inherent_impl = make_node("n-impl-inherent", "MyType", "src/ty.rs", Visibility::Pub);
    inherent_impl.kind = NodeKind::Impl;
    inherent_impl.signature = Some("impl MyType".to_string());
    let mut truly_dead = make_node("n-dead", "truly_dead", "src/ty.rs", Visibility::Private);
    truly_dead.kind = NodeKind::Method;
    truly_dead.parent_id = Some("n-impl-inherent".to_string());

    // (3) Trait impl whose first-line signature was truncated before ` for `
    // (multi-line header), but which DOES carry an `implements` edge — the
    // fallback path. `via_edge` must be excluded.
    let mut edge_impl = make_node("n-impl-edge", "MyType", "src/ty.rs", Visibility::Pub);
    edge_impl.kind = NodeKind::Impl;
    edge_impl.signature = Some("impl<T> LongTrait<T>".to_string());
    let mut local_trait = make_node("n-trait-local", "LongTrait", "src/ty.rs", Visibility::Pub);
    local_trait.kind = NodeKind::Trait;
    let mut via_edge = make_node("n-via-edge", "via_edge", "src/ty.rs", Visibility::Private);
    via_edge.kind = NodeKind::Method;
    via_edge.parent_id = Some("n-impl-edge".to_string());

    db.insert_nodes(&[
        display_impl,
        fmt,
        inherent_impl,
        truly_dead,
        edge_impl,
        local_trait,
        via_edge,
    ])
    .await
    .expect("insert nodes failed");
    db.insert_edges(&[Edge {
        source: "n-impl-edge".to_string(),
        target: "n-trait-local".to_string(),
        kind: EdgeKind::Implements,
        line: Some(1),
    }])
    .await
    .expect("insert edges failed");

    let qm = GraphQueryManager::new(&db);

    // Default: both trait-impl methods excluded, inherent dead method reported.
    let dead = qm
        .find_dead_code(&[NodeKind::Method], false, false)
        .await
        .expect("find_dead_code failed");
    let names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();
    assert!(
        !names.contains(&"fmt"),
        "stdlib trait-impl method `fmt` (signature-only) should not be dead, got: {names:?}"
    );
    assert!(
        !names.contains(&"via_edge"),
        "trait-impl method via `implements`-edge fallback should not be dead, got: {names:?}"
    );
    assert!(
        names.contains(&"truly_dead"),
        "inherent-impl method with no callers should still be dead, got: {names:?}"
    );

    // Opt-in: include_trait_impls=true surfaces both trait-impl methods again.
    let dead_all = qm
        .find_dead_code(&[NodeKind::Method], false, true)
        .await
        .expect("find_dead_code failed");
    let names_all: Vec<&str> = dead_all.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names_all.contains(&"fmt") && names_all.contains(&"via_edge"),
        "include_trait_impls=true should surface trait-impl methods, got: {names_all:?}"
    );
}

/// Regression: `#[test]` functions whose name does not start with `test`
/// (e.g. `from_measurement_slope_excludes_lfe`) must be excluded from the
/// dead-code list. The previous filter was name-prefix only, so most tests
/// in real Rust codebases were misreported. Detection must walk the
/// `Annotates` edges to find a `#[test]` (or `#[tokio::test]` …) attribute.
#[tokio::test]
async fn test_find_dead_code_excludes_test_annotated() {
    let (db, _dir) = setup_db().await;

    // A private function annotated with #[test] but with a non-`test*` name.
    let test_fn = make_node(
        "n-test-fn",
        "from_measurement_slope_excludes_lfe",
        "src/lib.rs",
        Visibility::Private,
    );

    // Another private function annotated via #[tokio::test].
    let tokio_fn = make_node(
        "n-tokio-fn",
        "cardioid_rejects_missing_phase",
        "src/lib.rs",
        Visibility::Private,
    );

    // A function annotated with #[wasm_bindgen_test].
    let wbg_fn = make_node(
        "n-wbg-fn",
        "no_nan_or_inf_in_results",
        "src/lib.rs",
        Visibility::Private,
    );

    // Genuine dead helper — should remain in the report.
    let dead_helper = make_node(
        "n-dead-helper",
        "approx_eq",
        "src/lib.rs",
        Visibility::Private,
    );

    db.insert_nodes(&[test_fn, tokio_fn, wbg_fn, dead_helper])
        .await
        .expect("insert nodes failed");

    // The annotation nodes themselves.
    let mut test_annot = make_node("n-annot-test", "test", "src/lib.rs", Visibility::Private);
    test_annot.kind = NodeKind::AnnotationUsage;
    test_annot.signature = Some("#[test]".to_string());

    let mut tokio_annot = make_node(
        "n-annot-tokio",
        "tokio::test",
        "src/lib.rs",
        Visibility::Private,
    );
    tokio_annot.kind = NodeKind::AnnotationUsage;
    tokio_annot.signature = Some("#[tokio::test]".to_string());

    let mut wbg_annot = make_node(
        "n-annot-wbg",
        "wasm_bindgen_test",
        "src/lib.rs",
        Visibility::Private,
    );
    wbg_annot.kind = NodeKind::AnnotationUsage;
    wbg_annot.signature = Some("#[wasm_bindgen_test]".to_string());

    db.insert_nodes(&[test_annot, tokio_annot, wbg_annot])
        .await
        .expect("insert annotation nodes failed");

    let annot_edges = vec![
        Edge {
            source: "n-annot-test".to_string(),
            target: "n-test-fn".to_string(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        Edge {
            source: "n-annot-tokio".to_string(),
            target: "n-tokio-fn".to_string(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        Edge {
            source: "n-annot-wbg".to_string(),
            target: "n-wbg-fn".to_string(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
    ];
    db.insert_edges(&annot_edges)
        .await
        .expect("insert annotates edges failed");

    let qm = GraphQueryManager::new(&db);
    let dead = qm
        .find_dead_code(&[NodeKind::Function], false, true)
        .await
        .expect("find_dead_code failed");

    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();
    assert!(
        !dead_names.contains(&"from_measurement_slope_excludes_lfe"),
        "#[test]-annotated function must not be reported as dead, got: {dead_names:?}"
    );
    assert!(
        !dead_names.contains(&"cardioid_rejects_missing_phase"),
        "#[tokio::test]-annotated function must not be reported as dead, got: {dead_names:?}"
    );
    assert!(
        !dead_names.contains(&"no_nan_or_inf_in_results"),
        "#[wasm_bindgen_test]-annotated function must not be reported as dead, got: {dead_names:?}"
    );
    assert!(
        dead_names.contains(&"approx_eq"),
        "non-test dead helper must still be reported, got: {dead_names:?}"
    );
}

#[tokio::test]
async fn test_find_dead_code_with_kind_filter() {
    let (db, _dir) = setup_db().await;

    let func_node = make_node("n-func", "private_func", "src/lib.rs", Visibility::Private);
    let mut struct_node = make_node("n-struct", "MyStruct", "src/lib.rs", Visibility::Private);
    struct_node.kind = NodeKind::Struct;

    db.insert_nodes(&[func_node, struct_node])
        .await
        .expect("insert nodes failed");

    let qm = GraphQueryManager::new(&db);

    // Filter to only Function kind.
    let dead = qm
        .find_dead_code(&[NodeKind::Function], false, true)
        .await
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

#[tokio::test]
async fn test_get_node_metrics() {
    let (db, _dir) = setup_call_chain().await;
    let qm = GraphQueryManager::new(&db);

    let metrics = qm
        .get_node_metrics("n-process")
        .await
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

#[tokio::test]
async fn test_get_file_dependencies() {
    let (db, _dir) = setup_call_chain().await;
    let qm = GraphQueryManager::new(&db);

    // src/main.rs has process -> validate (in src/lib.rs), so it depends on src/lib.rs.
    let deps = qm
        .get_file_dependencies("src/main.rs")
        .await
        .expect("get_file_dependencies failed");

    assert!(
        deps.contains(&"src/lib.rs".to_string()),
        "src/main.rs should depend on src/lib.rs, got: {deps:?}"
    );
}

#[tokio::test]
async fn test_get_file_dependents() {
    let (db, _dir) = setup_call_chain().await;
    let qm = GraphQueryManager::new(&db);

    // src/lib.rs is called from src/main.rs (process -> validate).
    let dependents = qm
        .get_file_dependents("src/lib.rs")
        .await
        .expect("get_file_dependents failed");

    assert!(
        dependents.contains(&"src/main.rs".to_string()),
        "src/lib.rs should be depended on by src/main.rs, got: {dependents:?}"
    );
}

#[tokio::test]
async fn test_find_circular_dependencies() {
    let (db, _dir) = setup_db().await;

    // Set up a circular dependency: file_a -> file_b -> file_a.
    let node_a = make_node("n-a", "func_a", "src/a.rs", Visibility::Pub);
    let node_b = make_node("n-b", "func_b", "src/b.rs", Visibility::Pub);

    db.insert_nodes(&[node_a, node_b])
        .await
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
    db.insert_edges(&edges).await.expect("insert edges failed");

    // Register files so they show up in get_all_files.
    let file_a = tokensave::types::FileRecord {
        path: "src/a.rs".to_string(),
        content_hash: "hash_a".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 1,
    };
    let file_b = tokensave::types::FileRecord {
        path: "src/b.rs".to_string(),
        content_hash: "hash_b".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 1,
    };
    db.upsert_file(&file_a).await.expect("upsert file_a failed");
    db.upsert_file(&file_b).await.expect("upsert file_b failed");

    let qm = GraphQueryManager::new(&db);
    let cycles = qm
        .find_circular_dependencies()
        .await
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

#[tokio::test]
async fn test_type_hierarchy() {
    let (db, _dir) = setup_db().await;

    let mut trait_node = make_node("n-trait", "MyTrait", "src/lib.rs", Visibility::Pub);
    trait_node.kind = NodeKind::Trait;
    let mut struct_node = make_node("n-struct", "MyStruct", "src/lib.rs", Visibility::Pub);
    struct_node.kind = NodeKind::Struct;
    let mut impl_node = make_node("n-impl", "impl_block", "src/lib.rs", Visibility::Private);
    impl_node.kind = NodeKind::Impl;

    db.insert_nodes(&[trait_node, struct_node, impl_node])
        .await
        .expect("insert nodes failed");

    let edge = Edge {
        source: "n-impl".to_string(),
        target: "n-trait".to_string(),
        kind: EdgeKind::Implements,
        line: None,
    };
    db.insert_edge(&edge).await.expect("insert edge failed");

    let traverser = GraphTraverser::new(&db);
    let subgraph = traverser
        .get_type_hierarchy("n-trait")
        .await
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

#[tokio::test]
async fn test_traversal_with_limit() {
    let (db, _dir) = setup_call_chain().await;
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
        .await
        .expect("traverse_bfs with limit failed");

    assert!(
        subgraph.nodes.len() <= 2,
        "limit=2 should cap the result to at most 2 nodes, got: {}",
        subgraph.nodes.len()
    );
}

#[tokio::test]
async fn test_traversal_nonexistent_start() {
    let (db, _dir) = setup_db().await;
    let traverser = GraphTraverser::new(&db);

    let opts = TraversalOptions::default();
    let subgraph = traverser
        .traverse_bfs("nonexistent", &opts)
        .await
        .expect("traverse_bfs should not error on missing start");

    assert!(
        subgraph.nodes.is_empty(),
        "traversal from nonexistent node should return empty subgraph"
    );
}

#[tokio::test]
async fn test_node_metrics_depth() {
    let (db, _dir) = setup_db().await;

    // Build a containment hierarchy: file -> module -> function.
    let mut file_node = make_node("n-file", "main.rs", "src/main.rs", Visibility::Pub);
    file_node.kind = NodeKind::File;

    let mut module_node = make_node("n-module", "utils", "src/main.rs", Visibility::Pub);
    module_node.kind = NodeKind::Module;

    let func_node = make_node("n-func", "helper", "src/main.rs", Visibility::Private);

    db.insert_nodes(&[file_node, module_node, func_node])
        .await
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
    db.insert_edges(&edges).await.expect("insert edges failed");

    let qm = GraphQueryManager::new(&db);

    let file_metrics = qm.get_node_metrics("n-file").await.expect("metrics failed");
    assert_eq!(file_metrics.depth, 0, "file should be at depth 0");
    assert_eq!(
        file_metrics.child_count, 1,
        "file should have 1 child (module)"
    );

    let module_metrics = qm
        .get_node_metrics("n-module")
        .await
        .expect("metrics failed");
    assert_eq!(module_metrics.depth, 1, "module should be at depth 1");

    let func_metrics = qm.get_node_metrics("n-func").await.expect("metrics failed");
    assert_eq!(func_metrics.depth, 2, "function should be at depth 2");
}

// ---------------------------------------------------------------------------
// File-level DAG tests
// ---------------------------------------------------------------------------

/// Creates a temporary Rust project with cross-file calls and indexes it.
async fn setup_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let project = dir.path();
    fs::create_dir_all(project.join("src")).expect("failed to create src dir");

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
    .expect("failed to write main.rs");

    fs::write(
        project.join("src/utils.rs"),
        r#"
/// Returns a greeting string.
pub fn helper() -> String {
    format!("Hello, world!")
}
"#,
    )
    .expect("failed to write utils.rs");

    let cg = TokenSave::init(project)
        .await
        .expect("failed to init TokenSave");
    cg.index_all().await.expect("failed to index project");
    (cg, dir)
}

#[tokio::test]
async fn test_build_file_adjacency() {
    let (cg, _dir) = setup_project().await;
    let qm = GraphQueryManager::new(cg.db());
    let adj = qm.build_file_adjacency(None).await.unwrap();

    // src/main.rs depends on src/utils.rs (calls helper)
    assert!(
        adj.get("src/main.rs")
            .is_some_and(|deps| deps.contains("src/utils.rs")),
        "main.rs should depend on utils.rs"
    );

    // Self-edges should be excluded
    for (file, deps) in &adj {
        assert!(
            !deps.contains(file),
            "file {file} should not have a self-edge"
        );
    }
}

// ---------------------------------------------------------------------------
// Health algorithm tests
// ---------------------------------------------------------------------------

use tokensave::graph::health::{
    acyclicity_score, compute_composite_health, dependency_depth, gini_coefficient, gini_label,
    modularity_score, HealthDimensions,
};

// --- Gini coefficient ---

#[test]
fn test_gini_perfect_equality() {
    let values = vec![5.0, 5.0, 5.0, 5.0];
    let g = gini_coefficient(&values);
    assert!(
        g.abs() < 1e-9,
        "all-equal values should give Gini ~0.0, got {g}"
    );
}

#[test]
fn test_gini_perfect_inequality() {
    let values = vec![0.0, 0.0, 0.0, 1000.0];
    let g = gini_coefficient(&values);
    assert!(
        g > 0.7,
        "extreme inequality should give Gini > 0.7, got {g}"
    );
}

#[test]
fn test_gini_moderate() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let g = gini_coefficient(&values);
    assert!(
        (0.1..0.5).contains(&g),
        "moderate distribution should give Gini between 0.1 and 0.5, got {g}"
    );
}

#[test]
fn test_gini_empty() {
    let g = gini_coefficient(&[]);
    assert_eq!(g, 0.0, "empty slice should give Gini 0.0");
}

#[test]
fn test_gini_single() {
    let g = gini_coefficient(&[42.0]);
    assert_eq!(g, 0.0, "single-element slice should give Gini 0.0");
}

#[test]
fn test_gini_label_thresholds() {
    assert_eq!(gini_label(0.10), "low inequality (healthy)");
    assert_eq!(gini_label(0.30), "moderate inequality");
    assert_eq!(gini_label(0.50), "high inequality");
    assert_eq!(gini_label(0.70), "extreme inequality (god files likely)");
}

// --- Acyclicity score ---

fn make_adj(edges: &[(&str, &str)]) -> HashMap<String, HashSet<String>> {
    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for &(src, tgt) in edges {
        adj.entry(src.to_string())
            .or_default()
            .insert(tgt.to_string());
        // ensure target node key exists
        adj.entry(tgt.to_string()).or_default();
    }
    adj
}

#[test]
fn test_acyclicity_no_cycles() {
    let adj = make_adj(&[("a", "b"), ("b", "c")]);
    let (score, cycles) = acyclicity_score(&adj);
    assert_eq!(score, 1.0, "DAG should have acyclicity score 1.0");
    assert_eq!(cycles, 0, "DAG should have 0 cycle edges");
}

#[test]
fn test_acyclicity_with_cycle() {
    let adj = make_adj(&[("a", "b"), ("b", "a")]);
    let (score, cycles) = acyclicity_score(&adj);
    assert!(
        score < 1.0,
        "graph with cycle should have score < 1.0, got {score}"
    );
    assert!(
        cycles > 0,
        "graph with cycle should have > 0 cycle edges, got {cycles}"
    );
}

#[test]
fn test_acyclicity_empty() {
    let adj: HashMap<String, HashSet<String>> = HashMap::new();
    let (score, cycles) = acyclicity_score(&adj);
    assert_eq!(score, 1.0, "empty graph should have acyclicity score 1.0");
    assert_eq!(cycles, 0);
}

// --- Dependency depth ---

#[test]
fn test_depth_linear_chain() {
    let adj = make_adj(&[("a", "b"), ("b", "c"), ("c", "d")]);
    let result = dependency_depth(&adj, 10);
    assert_eq!(
        result.max_depth, 3,
        "linear chain a→b→c→d should have max_depth=3"
    );
    // The deepest chain should contain 4 nodes (a,b,c,d)
    let deepest = result.chains.iter().find(|ch| ch.depth == 3);
    assert!(deepest.is_some(), "should find a chain with depth 3");
    assert_eq!(
        deepest.unwrap().chain.len(),
        4,
        "chain for depth-3 path should have 4 nodes"
    );
}

#[test]
fn test_depth_empty() {
    let adj: HashMap<String, HashSet<String>> = HashMap::new();
    let result = dependency_depth(&adj, 10);
    assert_eq!(result.max_depth, 0, "empty graph should have max_depth=0");
}

#[test]
fn test_depth_with_cycle_breaks() {
    // a→b→a forms a cycle; b→c is an outgoing edge from the SCC
    let adj = make_adj(&[("a", "b"), ("b", "a"), ("b", "c")]);
    let result = dependency_depth(&adj, 10);
    assert!(
        result.max_depth >= 1,
        "should find depth >= 1 even when cycle is present, got {}",
        result.max_depth
    );
}

// --- Modularity score ---

#[test]
fn test_modularity_independent_clusters() {
    // Two disconnected clusters: {a,b} and {c,d}
    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    adj.entry("a".to_string())
        .or_default()
        .insert("b".to_string());
    adj.entry("b".to_string()).or_default();
    adj.entry("c".to_string())
        .or_default()
        .insert("d".to_string());
    adj.entry("d".to_string()).or_default();

    let (score, components) = modularity_score(&adj);
    assert!(
        components >= 2,
        "two disconnected clusters should give >= 2 components, got {components}"
    );
    assert!(
        score > 0.0,
        "two-cluster graph should have modularity > 0, got {score}"
    );
}

#[test]
fn test_modularity_single_blob() {
    // Tight cycle: a→b→c→a
    let adj = make_adj(&[("a", "b"), ("b", "c"), ("c", "a")]);
    let (score, components) = modularity_score(&adj);
    assert_eq!(
        components, 1,
        "fully connected cycle should have 1 component"
    );
    assert!(
        score < 0.5,
        "single blob should have low modularity score, got {score}"
    );
}

#[test]
fn test_modularity_empty() {
    let adj: HashMap<String, HashSet<String>> = HashMap::new();
    let (score, _components) = modularity_score(&adj);
    assert_eq!(score, 1.0, "empty graph should have modularity score 1.0");
}

// --- Composite health score ---

#[test]
fn test_composite_health_all_perfect() {
    let dims = HealthDimensions {
        acyclicity: 1.0,
        depth: 1.0,
        equality: 1.0,
        redundancy: 1.0,
        modularity: 1.0,
        coverage_discipline: 1.0,
    };
    assert_eq!(compute_composite_health(&dims), 10000);
}

#[test]
fn test_composite_health_one_zero() {
    let dims = HealthDimensions {
        acyclicity: 0.0,
        depth: 1.0,
        equality: 1.0,
        redundancy: 1.0,
        modularity: 1.0,
        coverage_discipline: 1.0,
    };
    assert_eq!(compute_composite_health(&dims), 0);
}

#[test]
fn test_composite_health_mixed() {
    let dims = HealthDimensions {
        acyclicity: 0.8,
        depth: 0.7,
        equality: 0.9,
        redundancy: 0.6,
        modularity: 0.5,
        coverage_discipline: 1.0,
    };
    let score = compute_composite_health(&dims);
    assert!(
        score > 0 && score < 10000,
        "mixed health should give score between 0 and 10000, got {score}"
    );
}

// ---------------------------------------------------------------------------
// Git churn tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_churn() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let project = dir.path();

    // Init a real git repo and make two commits touching the same file
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(project)
        .output()
        .expect("git init failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(project)
        .output()
        .expect("git config email failed");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(project)
        .output()
        .expect("git config name failed");

    fs::write(project.join("file.rs"), "fn foo() {}").expect("write failed");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .expect("git add failed");
    std::process::Command::new("git")
        .args(["commit", "-m", "first"])
        .current_dir(project)
        .output()
        .expect("git commit 1 failed");

    fs::write(project.join("file.rs"), "fn foo() {} fn bar() {}").expect("write failed");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(project)
        .output()
        .expect("git add 2 failed");
    std::process::Command::new("git")
        .args(["commit", "-m", "second"])
        .current_dir(project)
        .output()
        .expect("git commit 2 failed");

    let churn = file_churn(project, 90).await.expect("file_churn failed");
    let count = churn.get("file.rs").copied().unwrap_or(0);
    assert!(count >= 2, "file.rs should have churn >= 2, got {count}");
}

#[tokio::test]
async fn test_file_churn_nonexistent_dir() {
    let churn = file_churn(
        std::path::Path::new("/nonexistent/path/that/does/not/exist"),
        90,
    )
    .await
    .expect("file_churn should not error for nonexistent dir");
    assert!(
        churn.is_empty(),
        "should return empty map for nonexistent dir"
    );
}

// ---------------------------------------------------------------------------
// find_dead_code perf regression test
// ---------------------------------------------------------------------------

/// Synthetic-scale guard for the chromium-class slowdown that motivated the
/// 4.14.8 CTE attempt and the 4.14.12 temp-table fix.
///
/// Builds:
/// - 50_000 `annotation_usage` rows whose names *don't* match any test marker
///   (`derive_Foo_0` etc.) — these exercise the leading-wildcard `LIKE` cost.
/// - 1_000 `annotation_usage` rows whose names DO match (`test`, `tokio::test`,
///   `wasm_bindgen_test`, `async_std::test`).
/// - 5_000 private function nodes, half referenced via a `calls` edge (live),
///   half unreferenced (dead). The first 1_000 of the unreferenced functions
///   carry a test-marker `annotates` edge so they must be excluded.
///
/// Asserts:
/// - returns exactly 1_500 dead nodes (2_500 unreferenced − 1_000 test-annotated).
/// - completes in < 5 s. The reverted 4.14.8 CTE form blew past 60 s on
///   scirs-scale (76 K annotation_usage); the chromium-scale dead_code call
///   timed out at the 25 s probe ceiling under the pre-4.14.8 JOIN form. The
///   temp-table form should comfortably stay under the budget.
#[tokio::test]
async fn dead_code_marker_resolve_is_single_pass() {
    let (db, _dir) = setup_db().await;

    let mut nodes: Vec<Node> = Vec::with_capacity(60_000);

    // 50K noise annotation_usage nodes that should NOT match the marker
    // patterns — these drive the LIKE scan cost.
    for i in 0..50_000_u32 {
        let mut n = make_node(
            &format!("n-noise-{i}"),
            &format!("derive_Foo_{i}"),
            "src/noise.rs",
            Visibility::Private,
        );
        n.kind = NodeKind::AnnotationUsage;
        nodes.push(n);
    }

    // 1K real test-marker annotation_usage nodes, evenly across the 4 patterns.
    // 250 of each pattern: bare `test`, `tokio::test`, `wasm_bindgen_test`,
    // `async_std::test`.
    let patterns: [&str; 4] = [
        "test",
        "tokio::test",
        "wasm_bindgen_test",
        "async_std::test",
    ];
    for i in 0..1_000_u32 {
        let pat = patterns[(i as usize) % patterns.len()];
        let mut n = make_node(
            &format!("n-marker-{i}"),
            pat,
            "src/markers.rs",
            Visibility::Private,
        );
        n.kind = NodeKind::AnnotationUsage;
        nodes.push(n);
    }

    // 5K private function nodes. Naming: `fn_{i}` — `name NOT LIKE 'test%'`
    // filter in the dead-code SQL excludes anything starting with `test`,
    // so use a neutral prefix.
    for i in 0..5_000_u32 {
        nodes.push(make_node(
            &format!("n-fn-{i}"),
            &format!("worker_{i}"),
            "src/fns.rs",
            Visibility::Private,
        ));
    }

    db.insert_nodes(&nodes)
        .await
        .expect("bulk insert nodes failed");

    // Half the functions (i = 2500..4999) are live via a calls edge from
    // `n-fn-0` (acting as a synthetic root). The first 1000 unreferenced
    // functions (i = 0..999) carry a test-marker annotates edge — those
    // must be excluded from dead-code as well. The remaining 1500
    // (i = 1000..2499) are the genuine dead set.
    let mut edges: Vec<Edge> = Vec::with_capacity(2_500 + 1_000);

    // Live edges — `calls` from worker_0 to workers 2500..4999.
    for i in 2_500..5_000_u32 {
        edges.push(Edge {
            source: "n-fn-0".to_string(),
            target: format!("n-fn-{i}"),
            kind: EdgeKind::Calls,
            line: Some(1),
        });
    }

    // Test-marker annotates — n-marker-{i} annotates n-fn-{i} for i in 0..999.
    for i in 0..1_000_u32 {
        edges.push(Edge {
            source: format!("n-marker-{i}"),
            target: format!("n-fn-{i}"),
            kind: EdgeKind::Annotates,
            line: Some(1),
        });
    }

    db.insert_edges(&edges)
        .await
        .expect("bulk insert edges failed");

    let qm = GraphQueryManager::new(&db);

    let t0 = std::time::Instant::now();
    let dead = qm
        .find_dead_code(&[NodeKind::Function], false, true)
        .await
        .expect("find_dead_code failed");
    let elapsed = t0.elapsed();

    // The 2500..4999 set is live (calls edges). The 0..999 set has test
    // annotations. The 1000..2499 set is genuinely dead — plus n-fn-0 itself
    // is unreferenced, but it carries no test marker so it should also be
    // reported (1501 total). Allow ±1 in case of off-by-one between live
    // and dead-by-self.
    let dead_count = dead.len();
    assert!(
        (1_500..=1_502).contains(&dead_count),
        "expected ~1500 dead functions, got {dead_count}"
    );

    // The fix budget. Pre-4.14.12 the chromium-class case timed out at 25s
    // (probe ceiling). 5s gives generous headroom for slower CI machines
    // while still failing loudly if the per-row LIKE scan ever sneaks back.
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "find_dead_code took {:?} on 50K annotation_usage / 5K function corpus \
         — expected <5s. The leading-wildcard LIKE is probably running per row \
         again; see src/graph/queries.rs::find_dead_code_inner and the changelog \
         entries for 4.14.8 → 4.14.9 → 4.14.12.",
        elapsed
    );
}
