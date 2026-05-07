use tempfile::TempDir;
use tokensave::db::Database;
use tokensave::types::*;

/// Helper: create a temp database and return (Database, TempDir).
/// The TempDir is returned so that it stays alive for the duration of the test.
async fn setup_db() -> (Database, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let (db, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
    (db, dir)
}

/// Helper: create a sample node with reasonable defaults.
fn sample_node(id: &str, name: &str, file_path: &str) -> Node {
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
        docstring: Some(format!("Documentation for {name}")),
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 1000,
    }
}

fn sample_edge(source: &str, target: &str, kind: EdgeKind) -> Edge {
    Edge {
        source: source.to_string(),
        target: target.to_string(),
        kind,
        line: Some(5),
    }
}

fn sample_file(path: &str) -> FileRecord {
    FileRecord {
        path: path.to_string(),
        content_hash: format!("hash_{path}"),
        size: 1024,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 3,
    }
}

// -------------------------------------------------------------------------
// get_nodes_by_kind
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_nodes_by_kind() {
    let (db, _dir) = setup_db().await;

    let mut func_node = sample_node("n1", "my_func", "src/lib.rs");
    func_node.kind = NodeKind::Function;

    let mut struct_node = sample_node("n2", "MyStruct", "src/lib.rs");
    struct_node.kind = NodeKind::Struct;

    let mut method_node = sample_node("n3", "my_method", "src/lib.rs");
    method_node.kind = NodeKind::Method;

    let mut func_node2 = sample_node("n4", "other_func", "src/other.rs");
    func_node2.kind = NodeKind::Function;

    db.insert_nodes(&[func_node, struct_node, method_node, func_node2])
        .await
        .expect("insert_nodes failed");

    let functions = db
        .get_nodes_by_kind(NodeKind::Function)
        .await
        .expect("get_nodes_by_kind failed");
    assert_eq!(functions.len(), 2);
    assert!(functions.iter().all(|n| n.kind == NodeKind::Function));

    let structs = db
        .get_nodes_by_kind(NodeKind::Struct)
        .await
        .expect("get_nodes_by_kind failed");
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "MyStruct");

    let methods = db
        .get_nodes_by_kind(NodeKind::Method)
        .await
        .expect("get_nodes_by_kind failed");
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "my_method");

    let traits = db
        .get_nodes_by_kind(NodeKind::Trait)
        .await
        .expect("get_nodes_by_kind failed");
    assert!(traits.is_empty());
}

// -------------------------------------------------------------------------
// get_all_nodes
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_all_nodes() {
    let (db, _dir) = setup_db().await;

    let nodes: Vec<Node> = (0..5)
        .map(|i| sample_node(&format!("all-{i}"), &format!("func_{i}"), "src/lib.rs"))
        .collect();

    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let all = db.get_all_nodes().await.expect("get_all_nodes failed");
    assert_eq!(all.len(), 5);
}

// -------------------------------------------------------------------------
// get_all_edges
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_all_edges() {
    let (db, _dir) = setup_db().await;

    let n1 = sample_node("ea", "fa", "src/lib.rs");
    let n2 = sample_node("eb", "fb", "src/lib.rs");
    let n3 = sample_node("ec", "fc", "src/lib.rs");
    db.insert_nodes(&[n1, n2, n3])
        .await
        .expect("insert_nodes failed");

    let e1 = sample_edge("ea", "eb", EdgeKind::Calls);
    let e2 = sample_edge("eb", "ec", EdgeKind::Uses);
    db.insert_edge(&e1).await.expect("insert_edge failed");
    db.insert_edge(&e2).await.expect("insert_edge failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 2);
}

// -------------------------------------------------------------------------
// insert_edges (batch)
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_insert_edges_batch() {
    let (db, _dir) = setup_db().await;

    let nodes: Vec<Node> = (0..4)
        .map(|i| sample_node(&format!("be-{i}"), &format!("f{i}"), "src/lib.rs"))
        .collect();
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edges = vec![
        sample_edge("be-0", "be-1", EdgeKind::Calls),
        sample_edge("be-1", "be-2", EdgeKind::Uses),
        sample_edge("be-2", "be-3", EdgeKind::Contains),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 3);
}

#[tokio::test]
async fn test_insert_edges_empty() {
    let (db, _dir) = setup_db().await;
    db.insert_edges(&[])
        .await
        .expect("insert_edges with empty slice should succeed");
    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert!(all.is_empty());
}

// -------------------------------------------------------------------------
// insert_edges — missing-node scenarios (#58)
// -------------------------------------------------------------------------

/// Both source and target are missing — edge must be silently skipped.
#[tokio::test]
async fn test_insert_edges_both_endpoints_missing() {
    let (db, _dir) = setup_db().await;

    let edges = vec![sample_edge("ghost-a", "ghost-b", EdgeKind::Calls)];
    db.insert_edges(&edges)
        .await
        .expect("insert_edges should not fail for missing endpoints");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert!(all.is_empty(), "edge with two missing endpoints must be skipped");
}

/// Source exists but target is missing — edge must be skipped.
#[tokio::test]
async fn test_insert_edges_missing_target() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("src-ok", "func_a", "src/lib.rs");
    db.insert_nodes(&[node]).await.expect("insert_nodes failed");

    let edges = vec![sample_edge("src-ok", "no-such-target", EdgeKind::Calls)];
    db.insert_edges(&edges)
        .await
        .expect("insert_edges should not fail for missing target");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert!(all.is_empty(), "edge with missing target must be skipped");
}

/// Target exists but source is missing — edge must be skipped.
#[tokio::test]
async fn test_insert_edges_missing_source() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("tgt-ok", "func_b", "src/lib.rs");
    db.insert_nodes(&[node]).await.expect("insert_nodes failed");

    let edges = vec![sample_edge("no-such-source", "tgt-ok", EdgeKind::Uses)];
    db.insert_edges(&edges)
        .await
        .expect("insert_edges should not fail for missing source");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert!(all.is_empty(), "edge with missing source must be skipped");
}

/// Mixed batch: some edges valid, some with missing endpoints.
/// Valid edges must be inserted; invalid ones silently skipped.
#[tokio::test]
async fn test_insert_edges_mixed_valid_and_missing() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("mx-a", "fa", "src/a.rs"),
        sample_node("mx-b", "fb", "src/a.rs"),
        sample_node("mx-c", "fc", "src/b.rs"),
    ];
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edges = vec![
        sample_edge("mx-a", "mx-b", EdgeKind::Calls),       // valid
        sample_edge("mx-a", "ghost-1", EdgeKind::Uses),      // missing target
        sample_edge("ghost-2", "mx-c", EdgeKind::Contains),  // missing source
        sample_edge("mx-b", "mx-c", EdgeKind::Calls),        // valid
        sample_edge("ghost-3", "ghost-4", EdgeKind::Uses),    // both missing
    ];
    db.insert_edges(&edges)
        .await
        .expect("insert_edges should not fail for mixed batch");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 2, "only edges with both endpoints present must be inserted");
}

/// Singular insert_edge also skips when target is missing.
#[tokio::test]
async fn test_insert_edge_singular_missing_target() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("se-a", "fa", "src/lib.rs");
    db.insert_nodes(&[node]).await.expect("insert_nodes failed");

    let edge = sample_edge("se-a", "missing", EdgeKind::Calls);
    db.insert_edge(&edge)
        .await
        .expect("insert_edge should not fail for missing target");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert!(all.is_empty());
}

/// Singular insert_edge also skips when source is missing.
#[tokio::test]
async fn test_insert_edge_singular_missing_source() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("se-b", "fb", "src/lib.rs");
    db.insert_nodes(&[node]).await.expect("insert_nodes failed");

    let edge = sample_edge("missing", "se-b", EdgeKind::Uses);
    db.insert_edge(&edge)
        .await
        .expect("insert_edge should not fail for missing source");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert!(all.is_empty());
}

/// Singular insert_edge works normally when both endpoints exist.
#[tokio::test]
async fn test_insert_edge_singular_valid() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("sv-a", "fa", "src/lib.rs"),
        sample_node("sv-b", "fb", "src/lib.rs"),
    ];
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edge = sample_edge("sv-a", "sv-b", EdgeKind::Calls);
    db.insert_edge(&edge).await.expect("insert_edge failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].source, "sv-a");
    assert_eq!(all[0].target, "sv-b");
}

/// Duplicate edges (same source/target/kind) are ignored via INSERT OR IGNORE.
#[tokio::test]
async fn test_insert_edges_duplicate_ignored() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("dup-a", "fa", "src/lib.rs"),
        sample_node("dup-b", "fb", "src/lib.rs"),
    ];
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edges = vec![
        sample_edge("dup-a", "dup-b", EdgeKind::Calls),
        sample_edge("dup-a", "dup-b", EdgeKind::Calls), // duplicate
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 1, "duplicate edge must be ignored");
}

/// Cross-file edges inserted after all nodes are present succeed.
/// This simulates the incremental sync reordering fix.
#[tokio::test]
async fn test_insert_edges_cross_file_after_all_nodes() {
    let (db, _dir) = setup_db().await;

    // Simulate phase 1: insert nodes from two different files
    let nodes_file_a = vec![sample_node("cf-a1", "func_a", "src/a.rs")];
    let nodes_file_b = vec![sample_node("cf-b1", "func_b", "src/b.rs")];
    db.insert_nodes(&nodes_file_a).await.expect("insert_nodes a failed");
    db.insert_nodes(&nodes_file_b).await.expect("insert_nodes b failed");

    // Simulate phase 2: insert cross-file edges after all nodes are in
    let edges = vec![
        sample_edge("cf-a1", "cf-b1", EdgeKind::Calls), // a.rs -> b.rs
        sample_edge("cf-b1", "cf-a1", EdgeKind::Uses),   // b.rs -> a.rs
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 2, "cross-file edges must succeed when nodes are present");
}

/// Large batch with many missing endpoints does not abort the transaction.
#[tokio::test]
async fn test_insert_edges_large_batch_with_missing() {
    let (db, _dir) = setup_db().await;

    // Only insert one node
    let node = sample_node("lb-0", "f0", "src/lib.rs");
    db.insert_nodes(&[node]).await.expect("insert_nodes failed");

    // Create 100 edges, all referencing missing targets
    let edges: Vec<Edge> = (0..100)
        .map(|i| sample_edge("lb-0", &format!("missing-{i}"), EdgeKind::Calls))
        .collect();
    db.insert_edges(&edges)
        .await
        .expect("insert_edges should not abort on large batch with missing endpoints");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert!(all.is_empty(), "all edges with missing targets must be skipped");
}

/// Edges with None line values and missing endpoints are handled correctly.
#[tokio::test]
async fn test_insert_edges_null_line_with_missing() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("nl-a", "fa", "src/lib.rs"),
        sample_node("nl-b", "fb", "src/lib.rs"),
    ];
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edges = vec![
        Edge {
            source: "nl-a".to_string(),
            target: "nl-b".to_string(),
            kind: EdgeKind::Calls,
            line: None, // valid, null line
        },
        Edge {
            source: "nl-a".to_string(),
            target: "missing".to_string(),
            kind: EdgeKind::Uses,
            line: None, // missing target, null line
        },
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 1, "only the valid edge should be inserted");
    assert!(all[0].line.is_none());
}

// -------------------------------------------------------------------------
// insert_all (bulk)
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_insert_all_bulk() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("bulk-1", "func_a", "src/a.rs"),
        sample_node("bulk-2", "func_b", "src/a.rs"),
        sample_node("bulk-3", "func_c", "src/b.rs"),
    ];

    let edges = vec![
        sample_edge("bulk-1", "bulk-2", EdgeKind::Calls),
        sample_edge("bulk-2", "bulk-3", EdgeKind::Uses),
    ];

    let files = vec![sample_file("src/a.rs"), sample_file("src/b.rs")];

    db.insert_all(&nodes, &edges, &files)
        .await
        .expect("insert_all failed");

    let all_nodes = db.get_all_nodes().await.expect("get_all_nodes failed");
    assert_eq!(all_nodes.len(), 3);

    let all_edges = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all_edges.len(), 2);

    let all_files = db.get_all_files().await.expect("get_all_files failed");
    assert_eq!(all_files.len(), 2);
}

// -------------------------------------------------------------------------
// delete_edges_by_source
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_delete_edges_by_source() {
    let (db, _dir) = setup_db().await;

    let nodes: Vec<Node> = ["ds-a", "ds-b", "ds-c"]
        .iter()
        .map(|id| sample_node(id, id, "src/lib.rs"))
        .collect();
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edges = vec![
        sample_edge("ds-a", "ds-b", EdgeKind::Calls),
        sample_edge("ds-a", "ds-c", EdgeKind::Uses),
        sample_edge("ds-b", "ds-c", EdgeKind::Calls),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    db.delete_edges_by_source("ds-a")
        .await
        .expect("delete_edges_by_source failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].source, "ds-b");
    assert_eq!(all[0].target, "ds-c");
}

// -------------------------------------------------------------------------
// get_ranked_nodes_by_edge_kind
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_ranked_nodes_by_edge_kind_incoming() {
    let (db, _dir) = setup_db().await;

    // Create target nodes that receive calls
    let target_a = sample_node("rt-a", "popular", "src/lib.rs");
    let target_b = sample_node("rt-b", "less_popular", "src/lib.rs");
    let caller1 = sample_node("rt-c1", "caller1", "src/lib.rs");
    let caller2 = sample_node("rt-c2", "caller2", "src/lib.rs");
    let caller3 = sample_node("rt-c3", "caller3", "src/lib.rs");

    db.insert_nodes(&[target_a, target_b, caller1, caller2, caller3])
        .await
        .expect("insert_nodes failed");

    // rt-a gets called by 3 callers, rt-b by 1
    let edges = vec![
        sample_edge("rt-c1", "rt-a", EdgeKind::Calls),
        sample_edge("rt-c2", "rt-a", EdgeKind::Calls),
        sample_edge("rt-c3", "rt-a", EdgeKind::Calls),
        sample_edge("rt-c1", "rt-b", EdgeKind::Calls),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let ranked = db
        .get_ranked_nodes_by_edge_kind(&EdgeKind::Calls, None, true, None, 10)
        .await
        .expect("get_ranked_nodes_by_edge_kind failed");

    assert_eq!(ranked.len(), 2);
    // Most called first
    assert_eq!(ranked[0].0.id, "rt-a");
    assert_eq!(ranked[0].1, 3);
    assert_eq!(ranked[1].0.id, "rt-b");
    assert_eq!(ranked[1].1, 1);
}

#[tokio::test]
async fn test_get_ranked_nodes_by_edge_kind_outgoing() {
    let (db, _dir) = setup_db().await;

    let caller = sample_node("ro-caller", "big_caller", "src/lib.rs");
    let target1 = sample_node("ro-t1", "t1", "src/lib.rs");
    let target2 = sample_node("ro-t2", "t2", "src/lib.rs");
    db.insert_nodes(&[caller, target1, target2])
        .await
        .expect("insert_nodes failed");

    let edges = vec![
        sample_edge("ro-caller", "ro-t1", EdgeKind::Calls),
        sample_edge("ro-caller", "ro-t2", EdgeKind::Calls),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let ranked = db
        .get_ranked_nodes_by_edge_kind(&EdgeKind::Calls, None, false, None, 10)
        .await
        .expect("get_ranked_nodes_by_edge_kind failed");

    assert!(!ranked.is_empty());
    assert_eq!(ranked[0].0.id, "ro-caller");
    assert_eq!(ranked[0].1, 2);
}

#[tokio::test]
async fn test_get_ranked_nodes_by_edge_kind_with_node_filter() {
    let (db, _dir) = setup_db().await;

    let mut func_node = sample_node("rnf-1", "func1", "src/lib.rs");
    func_node.kind = NodeKind::Function;

    let mut struct_node = sample_node("rnf-2", "MyStruct", "src/lib.rs");
    struct_node.kind = NodeKind::Struct;

    let caller = sample_node("rnf-c", "caller", "src/lib.rs");

    db.insert_nodes(&[func_node, struct_node, caller])
        .await
        .expect("insert_nodes failed");

    let edges = vec![
        sample_edge("rnf-c", "rnf-1", EdgeKind::Calls),
        sample_edge("rnf-c", "rnf-2", EdgeKind::Calls),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    // Filter to only Function nodes
    let ranked = db
        .get_ranked_nodes_by_edge_kind(&EdgeKind::Calls, Some(&NodeKind::Function), true, None, 10)
        .await
        .expect("get_ranked_nodes_by_edge_kind failed");

    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0.kind, NodeKind::Function);
}

// -------------------------------------------------------------------------
// get_largest_nodes
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_largest_nodes() {
    let (db, _dir) = setup_db().await;

    let mut small = sample_node("ln-small", "small_fn", "src/lib.rs");
    small.start_line = 1;
    small.end_line = 5; // 5 lines

    let mut medium = sample_node("ln-medium", "medium_fn", "src/lib.rs");
    medium.start_line = 10;
    medium.end_line = 30; // 21 lines

    let mut large = sample_node("ln-large", "large_fn", "src/lib.rs");
    large.start_line = 50;
    large.end_line = 150; // 101 lines

    db.insert_nodes(&[small, medium, large])
        .await
        .expect("insert_nodes failed");

    let largest = db
        .get_largest_nodes(None, None, 10)
        .await
        .expect("get_largest_nodes failed");

    assert_eq!(largest.len(), 3);
    // Largest first
    assert_eq!(largest[0].0.id, "ln-large");
    assert_eq!(largest[0].1, 101);
    assert_eq!(largest[1].0.id, "ln-medium");
    assert_eq!(largest[1].1, 21);
    assert_eq!(largest[2].0.id, "ln-small");
    assert_eq!(largest[2].1, 5);
}

#[tokio::test]
async fn test_get_largest_nodes_with_kind_filter() {
    let (db, _dir) = setup_db().await;

    let mut func = sample_node("lk-func", "big_fn", "src/lib.rs");
    func.kind = NodeKind::Function;
    func.start_line = 1;
    func.end_line = 100;

    let mut strct = sample_node("lk-struct", "BigStruct", "src/lib.rs");
    strct.kind = NodeKind::Struct;
    strct.start_line = 1;
    strct.end_line = 200;

    db.insert_nodes(&[func, strct])
        .await
        .expect("insert_nodes failed");

    let largest = db
        .get_largest_nodes(Some(&NodeKind::Function), None, 10)
        .await
        .expect("get_largest_nodes failed");

    assert_eq!(largest.len(), 1);
    assert_eq!(largest[0].0.id, "lk-func");
}

#[tokio::test]
async fn test_get_largest_nodes_respects_limit() {
    let (db, _dir) = setup_db().await;

    let nodes: Vec<Node> = (0..10)
        .map(|i| {
            let mut n = sample_node(&format!("ll-{i}"), &format!("f{i}"), "src/lib.rs");
            n.start_line = 1;
            n.end_line = (i + 1) * 10;
            n
        })
        .collect();
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let largest = db
        .get_largest_nodes(None, None, 3)
        .await
        .expect("get_largest_nodes failed");

    assert_eq!(largest.len(), 3);
}

// -------------------------------------------------------------------------
// get_file_coupling
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_file_coupling_fan_in() {
    let (db, _dir) = setup_db().await;

    // Nodes in different files
    let n1 = sample_node("fc-1", "f1", "src/a.rs");
    let n2 = sample_node("fc-2", "f2", "src/b.rs");
    let n3 = sample_node("fc-3", "f3", "src/c.rs");
    let n4 = sample_node("fc-4", "f4", "src/a.rs");
    db.insert_nodes(&[n1, n2, n3, n4])
        .await
        .expect("insert_nodes failed");

    // Cross-file edges: b -> a, c -> a (a has fan-in of 2)
    let edges = vec![
        sample_edge("fc-2", "fc-1", EdgeKind::Calls),
        sample_edge("fc-3", "fc-4", EdgeKind::Uses),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let coupling = db
        .get_file_coupling(true, None, 10)
        .await
        .expect("get_file_coupling failed");

    // src/a.rs should have fan-in of 2 (from b and c)
    assert!(!coupling.is_empty());
    assert_eq!(coupling[0].0, "src/a.rs");
    assert_eq!(coupling[0].1, 2);
}

#[tokio::test]
async fn test_get_file_coupling_fan_out() {
    let (db, _dir) = setup_db().await;

    let n1 = sample_node("fco-1", "f1", "src/a.rs");
    let n2 = sample_node("fco-2", "f2", "src/b.rs");
    let n3 = sample_node("fco-3", "f3", "src/c.rs");
    db.insert_nodes(&[n1, n2, n3])
        .await
        .expect("insert_nodes failed");

    // a calls b and c => a has fan-out of 2
    let edges = vec![
        sample_edge("fco-1", "fco-2", EdgeKind::Calls),
        sample_edge("fco-1", "fco-3", EdgeKind::Uses),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let coupling = db
        .get_file_coupling(false, None, 10)
        .await
        .expect("get_file_coupling failed");

    assert!(!coupling.is_empty());
    assert_eq!(coupling[0].0, "src/a.rs");
    assert_eq!(coupling[0].1, 2);
}

// -------------------------------------------------------------------------
// get_inheritance_depth
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_inheritance_depth() {
    let (db, _dir) = setup_db().await;

    // Create a chain: Child extends Parent extends GrandParent
    let mut grandparent = sample_node("ih-gp", "GrandParent", "src/lib.rs");
    grandparent.kind = NodeKind::Class;

    let mut parent = sample_node("ih-p", "Parent", "src/lib.rs");
    parent.kind = NodeKind::Class;

    let mut child = sample_node("ih-c", "Child", "src/lib.rs");
    child.kind = NodeKind::Class;

    db.insert_nodes(&[grandparent, parent, child])
        .await
        .expect("insert_nodes failed");

    let edges = vec![
        Edge {
            source: "ih-c".to_string(),
            target: "ih-p".to_string(),
            kind: EdgeKind::Extends,
            line: None,
        },
        Edge {
            source: "ih-p".to_string(),
            target: "ih-gp".to_string(),
            kind: EdgeKind::Extends,
            line: None,
        },
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let depths = db
        .get_inheritance_depth(None, 10)
        .await
        .expect("get_inheritance_depth failed");

    // Child has depth 2 (Child -> Parent -> GrandParent)
    // Parent has depth 1 (Parent -> GrandParent)
    assert_eq!(depths.len(), 2);
    assert_eq!(depths[0].0.id, "ih-c");
    assert_eq!(depths[0].1, 2);
    assert_eq!(depths[1].0.id, "ih-p");
    assert_eq!(depths[1].1, 1);
}

// -------------------------------------------------------------------------
// get_node_distribution
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_node_distribution_no_prefix() {
    let (db, _dir) = setup_db().await;

    let mut n1 = sample_node("nd-1", "f1", "src/a.rs");
    n1.kind = NodeKind::Function;

    let mut n2 = sample_node("nd-2", "f2", "src/a.rs");
    n2.kind = NodeKind::Function;

    let mut n3 = sample_node("nd-3", "S1", "src/a.rs");
    n3.kind = NodeKind::Struct;

    let mut n4 = sample_node("nd-4", "f3", "src/b.rs");
    n4.kind = NodeKind::Function;

    db.insert_nodes(&[n1, n2, n3, n4])
        .await
        .expect("insert_nodes failed");

    let dist = db
        .get_node_distribution(None)
        .await
        .expect("get_node_distribution failed");

    // Should have entries for (src/a.rs, function, 2), (src/a.rs, struct, 1), (src/b.rs, function, 1)
    assert_eq!(dist.len(), 3);
}

#[tokio::test]
async fn test_get_node_distribution_with_prefix() {
    let (db, _dir) = setup_db().await;

    let mut n1 = sample_node("ndp-1", "f1", "src/a/foo.rs");
    n1.kind = NodeKind::Function;

    let mut n2 = sample_node("ndp-2", "f2", "src/b/bar.rs");
    n2.kind = NodeKind::Function;

    db.insert_nodes(&[n1, n2])
        .await
        .expect("insert_nodes failed");

    let dist = db
        .get_node_distribution(Some("src/a/"))
        .await
        .expect("get_node_distribution failed");

    assert_eq!(dist.len(), 1);
    assert_eq!(dist[0].0, "src/a/foo.rs");
}

// -------------------------------------------------------------------------
// get_call_edges
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_call_edges() {
    let (db, _dir) = setup_db().await;

    let n1 = sample_node("ce-1", "f1", "src/lib.rs");
    let n2 = sample_node("ce-2", "f2", "src/lib.rs");
    let n3 = sample_node("ce-3", "f3", "src/lib.rs");
    db.insert_nodes(&[n1, n2, n3])
        .await
        .expect("insert_nodes failed");

    let edges = vec![
        sample_edge("ce-1", "ce-2", EdgeKind::Calls),
        sample_edge("ce-2", "ce-3", EdgeKind::Calls),
        sample_edge("ce-1", "ce-3", EdgeKind::Uses), // not a call edge
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let call_edges = db
        .get_call_edges(None)
        .await
        .expect("get_call_edges failed");

    assert_eq!(call_edges.len(), 2);
    // Should only return calls edges
    let sources: Vec<&str> = call_edges.iter().map(|(s, _)| s.as_str()).collect();
    assert!(sources.contains(&"ce-1"));
    assert!(sources.contains(&"ce-2"));
}

// -------------------------------------------------------------------------
// get_complexity_ranked
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_complexity_ranked_no_filter() {
    let (db, _dir) = setup_db().await;

    // Returns (Node, lines, fan_out, fan_in, score)
    // score = lines + fan_out*3 + fan_in
    let mut n1 = sample_node("cx-1", "complex_fn", "src/lib.rs");
    n1.kind = NodeKind::Function;
    n1.start_line = 1;
    n1.end_line = 50; // 50 lines

    let mut n2 = sample_node("cx-2", "simple_fn", "src/lib.rs");
    n2.kind = NodeKind::Method;
    n2.start_line = 1;
    n2.end_line = 5; // 5 lines

    let mut target = sample_node("cx-t", "target", "src/lib.rs");
    target.kind = NodeKind::Struct; // Not function/method so it's excluded from default filter

    db.insert_nodes(&[n1, n2, target])
        .await
        .expect("insert_nodes failed");

    // cx-1 calls cx-t (fan_out = 1)
    let edges = vec![sample_edge("cx-1", "cx-t", EdgeKind::Calls)];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    // No node_kind filter -> defaults to function + method
    let ranked = db
        .get_complexity_ranked(None, None, 10)
        .await
        .expect("get_complexity_ranked failed");

    assert_eq!(ranked.len(), 2);
    // cx-1: score = 50 + 1*3 + 0 = 53
    // cx-2: score = 5 + 0 + 0 = 5
    assert_eq!(ranked[0].0.id, "cx-1");
    assert_eq!(ranked[0].1, 50); // lines
    assert_eq!(ranked[0].2, 1); // fan_out
    assert_eq!(ranked[0].3, 0); // fan_in
    assert_eq!(ranked[0].4, 53); // score
}

#[tokio::test]
async fn test_get_complexity_ranked_with_filter() {
    let (db, _dir) = setup_db().await;

    let mut n1 = sample_node("cxf-1", "fn1", "src/lib.rs");
    n1.kind = NodeKind::Function;
    n1.start_line = 1;
    n1.end_line = 20;

    let mut n2 = sample_node("cxf-2", "method1", "src/lib.rs");
    n2.kind = NodeKind::Method;
    n2.start_line = 1;
    n2.end_line = 40;

    db.insert_nodes(&[n1, n2])
        .await
        .expect("insert_nodes failed");

    let ranked = db
        .get_complexity_ranked(Some(&NodeKind::Function), None, 10)
        .await
        .expect("get_complexity_ranked failed");

    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0.kind, NodeKind::Function);
}

// -------------------------------------------------------------------------
// get_undocumented_public_symbols
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_undocumented_public_symbols() {
    let (db, _dir) = setup_db().await;

    // Undocumented public function
    let mut undoc_pub = sample_node("udp-1", "undoc_fn", "src/lib.rs");
    undoc_pub.kind = NodeKind::Function;
    undoc_pub.visibility = Visibility::Pub;
    undoc_pub.docstring = None;

    // Documented public function
    let mut doc_pub = sample_node("udp-2", "doc_fn", "src/lib.rs");
    doc_pub.kind = NodeKind::Function;
    doc_pub.visibility = Visibility::Pub;
    doc_pub.docstring = Some("This is documented".to_string());

    // Undocumented private function (should not appear)
    let mut undoc_priv = sample_node("udp-3", "priv_fn", "src/lib.rs");
    undoc_priv.kind = NodeKind::Function;
    undoc_priv.visibility = Visibility::Private;
    undoc_priv.docstring = None;

    // Undocumented public struct
    let mut undoc_struct = sample_node("udp-4", "MyStruct", "src/lib.rs");
    undoc_struct.kind = NodeKind::Struct;
    undoc_struct.visibility = Visibility::Pub;
    undoc_struct.docstring = None;

    // Undocumented public with empty string docstring
    let mut undoc_empty = sample_node("udp-5", "empty_doc_fn", "src/lib.rs");
    undoc_empty.kind = NodeKind::Function;
    undoc_empty.visibility = Visibility::Pub;
    undoc_empty.docstring = Some(String::new());

    db.insert_nodes(&[undoc_pub, doc_pub, undoc_priv, undoc_struct, undoc_empty])
        .await
        .expect("insert_nodes failed");

    let undoc = db
        .get_undocumented_public_symbols(None, 100)
        .await
        .expect("get_undocumented_public_symbols failed");

    // Should include undoc_fn, MyStruct, empty_doc_fn but NOT doc_fn or priv_fn
    assert_eq!(undoc.len(), 3);
    let ids: Vec<&str> = undoc.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"udp-1"));
    assert!(ids.contains(&"udp-4"));
    assert!(ids.contains(&"udp-5"));
}

#[tokio::test]
async fn test_get_undocumented_public_symbols_with_prefix() {
    let (db, _dir) = setup_db().await;

    let mut n1 = sample_node("udpp-1", "f1", "src/a/foo.rs");
    n1.kind = NodeKind::Function;
    n1.visibility = Visibility::Pub;
    n1.docstring = None;

    let mut n2 = sample_node("udpp-2", "f2", "src/b/bar.rs");
    n2.kind = NodeKind::Function;
    n2.visibility = Visibility::Pub;
    n2.docstring = None;

    db.insert_nodes(&[n1, n2])
        .await
        .expect("insert_nodes failed");

    let undoc = db
        .get_undocumented_public_symbols(Some("src/a/"), 100)
        .await
        .expect("get_undocumented_public_symbols failed");

    assert_eq!(undoc.len(), 1);
    assert_eq!(undoc[0].file_path, "src/a/foo.rs");
}

// -------------------------------------------------------------------------
// get_god_classes
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_god_classes() {
    let (db, _dir) = setup_db().await;

    // A struct with many contained members
    let mut class_node = sample_node("gc-class", "GodClass", "src/lib.rs");
    class_node.kind = NodeKind::Class;

    let mut method1 = sample_node("gc-m1", "method1", "src/lib.rs");
    method1.kind = NodeKind::Method;

    let mut method2 = sample_node("gc-m2", "method2", "src/lib.rs");
    method2.kind = NodeKind::Method;

    let mut field1 = sample_node("gc-f1", "field1", "src/lib.rs");
    field1.kind = NodeKind::Field;

    let mut constructor = sample_node("gc-ctor", "new", "src/lib.rs");
    constructor.kind = NodeKind::Constructor;

    db.insert_nodes(&[class_node, method1, method2, field1, constructor])
        .await
        .expect("insert_nodes failed");

    // "contains" edges from class to its members
    let edges = vec![
        Edge {
            source: "gc-class".to_string(),
            target: "gc-m1".to_string(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gc-class".to_string(),
            target: "gc-m2".to_string(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gc-class".to_string(),
            target: "gc-f1".to_string(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gc-class".to_string(),
            target: "gc-ctor".to_string(),
            kind: EdgeKind::Contains,
            line: None,
        },
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let god_classes = db
        .get_god_classes(None, 10)
        .await
        .expect("get_god_classes failed");

    assert_eq!(god_classes.len(), 1);
    let (node, methods, fields, total) = &god_classes[0];
    assert_eq!(node.id, "gc-class");
    // methods: method1, method2, constructor = 3
    assert_eq!(*methods, 3);
    // fields: field1 = 1
    assert_eq!(*fields, 1);
    // total: 4
    assert_eq!(*total, 4);
}

// -------------------------------------------------------------------------
// upsert_files (batch)
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_upsert_files_batch() {
    let (db, _dir) = setup_db().await;

    let files = vec![
        sample_file("src/a.rs"),
        sample_file("src/b.rs"),
        sample_file("src/c.rs"),
    ];

    db.upsert_files(&files).await.expect("upsert_files failed");

    let all = db.get_all_files().await.expect("get_all_files failed");
    assert_eq!(all.len(), 3);

    // Verify upsert replaces existing
    let updated_files = vec![FileRecord {
        path: "src/a.rs".to_string(),
        content_hash: "new_hash".to_string(),
        size: 9999,
        modified_at: 5000,
        indexed_at: 6000,
        node_count: 99,
    }];

    db.upsert_files(&updated_files)
        .await
        .expect("upsert_files failed");

    let fetched = db
        .get_file("src/a.rs")
        .await
        .expect("get_file failed")
        .expect("file should exist");
    assert_eq!(fetched.content_hash, "new_hash");
    assert_eq!(fetched.size, 9999);
}

#[tokio::test]
async fn test_upsert_files_empty() {
    let (db, _dir) = setup_db().await;
    db.upsert_files(&[])
        .await
        .expect("upsert_files with empty slice should succeed");
}

// -------------------------------------------------------------------------
// delete_file
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_delete_file() {
    let (db, _dir) = setup_db().await;

    let file = sample_file("src/target.rs");
    db.upsert_file(&file).await.expect("upsert_file failed");

    // Also insert a node so we verify cascading
    let node = sample_node("df-1", "fn_in_target", "src/target.rs");
    db.insert_node(&node).await.expect("insert_node failed");

    // Verify file exists before delete
    let before = db.get_file("src/target.rs").await.expect("get_file failed");
    assert!(before.is_some());

    db.delete_file("src/target.rs")
        .await
        .expect("delete_file failed");

    // File record should be gone
    let after = db.get_file("src/target.rs").await.expect("get_file failed");
    assert!(after.is_none());

    // Associated nodes should also be gone
    let nodes = db
        .get_nodes_by_file("src/target.rs")
        .await
        .expect("get_nodes_by_file failed");
    assert!(nodes.is_empty());
}

// -------------------------------------------------------------------------
// get_all_files
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_all_files() {
    let (db, _dir) = setup_db().await;

    let files = vec![sample_file("src/a.rs"), sample_file("src/b.rs")];
    db.upsert_files(&files).await.expect("upsert_files failed");

    let all = db.get_all_files().await.expect("get_all_files failed");
    assert_eq!(all.len(), 2);
    let paths: Vec<&str> = all.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"src/a.rs"));
    assert!(paths.contains(&"src/b.rs"));
}

// -------------------------------------------------------------------------
// last_index_time
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_last_index_time_empty() {
    let (db, _dir) = setup_db().await;

    let time = db.last_index_time().await.expect("last_index_time failed");
    assert_eq!(time, 0);
}

#[tokio::test]
async fn test_last_index_time_with_files() {
    let (db, _dir) = setup_db().await;

    let mut f1 = sample_file("src/a.rs");
    f1.indexed_at = 1000;

    let mut f2 = sample_file("src/b.rs");
    f2.indexed_at = 5000;

    let mut f3 = sample_file("src/c.rs");
    f3.indexed_at = 3000;

    db.upsert_files(&[f1, f2, f3])
        .await
        .expect("upsert_files failed");

    let time = db.last_index_time().await.expect("last_index_time failed");
    assert_eq!(time, 5000);
}

// -------------------------------------------------------------------------
// metadata get/set
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_metadata_get_set() {
    let (db, _dir) = setup_db().await;

    // Non-existent key returns None
    let val = db
        .get_metadata("nonexistent")
        .await
        .expect("get_metadata failed");
    assert!(val.is_none());

    // Set and get
    db.set_metadata("my_key", "my_value")
        .await
        .expect("set_metadata failed");

    let val = db
        .get_metadata("my_key")
        .await
        .expect("get_metadata failed")
        .expect("metadata should exist");
    assert_eq!(val, "my_value");

    // Overwrite
    db.set_metadata("my_key", "updated_value")
        .await
        .expect("set_metadata failed");

    let val = db
        .get_metadata("my_key")
        .await
        .expect("get_metadata failed")
        .expect("metadata should exist");
    assert_eq!(val, "updated_value");
}

#[tokio::test]
async fn test_metadata_multiple_keys() {
    let (db, _dir) = setup_db().await;

    db.set_metadata("key1", "val1")
        .await
        .expect("set_metadata failed");
    db.set_metadata("key2", "val2")
        .await
        .expect("set_metadata failed");

    let v1 = db
        .get_metadata("key1")
        .await
        .expect("get_metadata failed")
        .expect("key1 should exist");
    let v2 = db
        .get_metadata("key2")
        .await
        .expect("get_metadata failed")
        .expect("key2 should exist");

    assert_eq!(v1, "val1");
    assert_eq!(v2, "val2");
}

// -------------------------------------------------------------------------
// get_nodes_by_dir
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_nodes_by_dir() {
    let (db, _dir) = setup_db().await;

    let mut n1 = sample_node("dir-1", "f1", "src/a/foo.rs");
    n1.kind = NodeKind::Function;

    let mut n2 = sample_node("dir-2", "f2", "src/a/bar.rs");
    n2.kind = NodeKind::Function;

    let mut n3 = sample_node("dir-3", "f3", "src/b/baz.rs");
    n3.kind = NodeKind::Function;

    let mut n4 = sample_node("dir-4", "S1", "src/a/foo.rs");
    n4.kind = NodeKind::Struct;

    db.insert_nodes(&[n1, n2, n3, n4])
        .await
        .expect("insert_nodes failed");

    // Query src/a/ with Function kind
    let results = db
        .get_nodes_by_dir("src/a/", &[NodeKind::Function])
        .await
        .expect("get_nodes_by_dir failed");

    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|n| n.file_path.starts_with("src/a/")));
    assert!(results.iter().all(|n| n.kind == NodeKind::Function));
}

#[tokio::test]
async fn test_get_nodes_by_dir_multiple_kinds() {
    let (db, _dir) = setup_db().await;

    let mut n1 = sample_node("dirk-1", "f1", "src/a/foo.rs");
    n1.kind = NodeKind::Function;

    let mut n2 = sample_node("dirk-2", "S1", "src/a/foo.rs");
    n2.kind = NodeKind::Struct;

    let mut n3 = sample_node("dirk-3", "m1", "src/a/foo.rs");
    n3.kind = NodeKind::Method;

    db.insert_nodes(&[n1, n2, n3])
        .await
        .expect("insert_nodes failed");

    let results = db
        .get_nodes_by_dir("src/a/", &[NodeKind::Function, NodeKind::Struct])
        .await
        .expect("get_nodes_by_dir failed");

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_get_nodes_by_dir_empty_kinds() {
    let (db, _dir) = setup_db().await;

    let n1 = sample_node("dire-1", "f1", "src/a/foo.rs");
    db.insert_node(&n1).await.expect("insert_node failed");

    // Empty kinds should return empty
    let results = db
        .get_nodes_by_dir("src/a/", &[])
        .await
        .expect("get_nodes_by_dir failed");

    assert!(results.is_empty());
}

// -------------------------------------------------------------------------
// get_internal_edges
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_internal_edges() {
    let (db, _dir) = setup_db().await;

    let n1 = sample_node("ie-1", "f1", "src/lib.rs");
    let n2 = sample_node("ie-2", "f2", "src/lib.rs");
    let n3 = sample_node("ie-3", "f3", "src/lib.rs");
    let n4 = sample_node("ie-4", "f4", "src/lib.rs"); // outside the subset
    db.insert_nodes(&[n1, n2, n3, n4])
        .await
        .expect("insert_nodes failed");

    let edges = vec![
        sample_edge("ie-1", "ie-2", EdgeKind::Calls), // internal
        sample_edge("ie-2", "ie-3", EdgeKind::Calls), // internal
        sample_edge("ie-1", "ie-4", EdgeKind::Calls), // external (target not in subset)
        sample_edge("ie-4", "ie-1", EdgeKind::Calls), // external (source not in subset)
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let subset = vec!["ie-1".to_string(), "ie-2".to_string(), "ie-3".to_string()];

    let internal = db
        .get_internal_edges(&subset)
        .await
        .expect("get_internal_edges failed");

    assert_eq!(internal.len(), 2);
    let pairs: Vec<(&str, &str)> = internal
        .iter()
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect();
    assert!(pairs.contains(&("ie-1", "ie-2")));
    assert!(pairs.contains(&("ie-2", "ie-3")));
}

#[tokio::test]
async fn test_get_internal_edges_empty() {
    let (db, _dir) = setup_db().await;

    let result = db
        .get_internal_edges(&[])
        .await
        .expect("get_internal_edges failed");
    assert!(result.is_empty());
}

// -------------------------------------------------------------------------
// insert_unresolved_refs (batch)
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_insert_unresolved_refs_batch() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("ur-node", "my_func", "src/lib.rs");
    db.insert_node(&node).await.expect("insert_node failed");

    let refs = vec![
        UnresolvedRef {
            from_node_id: "ur-node".to_string(),
            reference_name: "HashMap".to_string(),
            reference_kind: EdgeKind::Uses,
            line: 10,
            column: 5,
            file_path: "src/lib.rs".to_string(),
        },
        UnresolvedRef {
            from_node_id: "ur-node".to_string(),
            reference_name: "Vec".to_string(),
            reference_kind: EdgeKind::Uses,
            line: 15,
            column: 10,
            file_path: "src/lib.rs".to_string(),
        },
        UnresolvedRef {
            from_node_id: "ur-node".to_string(),
            reference_name: "other_fn".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 20,
            column: 0,
            file_path: "src/lib.rs".to_string(),
        },
    ];

    db.insert_unresolved_refs(&refs)
        .await
        .expect("insert_unresolved_refs failed");

    let fetched = db
        .get_unresolved_refs()
        .await
        .expect("get_unresolved_refs failed");
    assert_eq!(fetched.len(), 3);
}

#[tokio::test]
async fn test_insert_unresolved_refs_empty() {
    let (db, _dir) = setup_db().await;
    db.insert_unresolved_refs(&[])
        .await
        .expect("insert_unresolved_refs with empty slice should succeed");
}

// -------------------------------------------------------------------------
// search_nodes — FTS ranking order
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_search_nodes_ranking_order() {
    let (db, _dir) = setup_db().await;

    // Node whose name exactly contains the query term should rank higher
    let mut exact_match = sample_node("sr-1", "process_data", "src/lib.rs");
    exact_match.qualified_name = "crate::process_data".to_string();
    exact_match.signature = Some("fn process_data()".to_string());

    // Node with partial match in qualified_name only
    let mut partial_match = sample_node("sr-2", "helper", "src/lib.rs");
    partial_match.qualified_name = "crate::data_module::helper".to_string();
    partial_match.signature = Some("fn helper()".to_string());

    // Node with no match at all
    let mut no_match = sample_node("sr-3", "unrelated", "src/lib.rs");
    no_match.qualified_name = "crate::unrelated".to_string();
    no_match.docstring = None;

    db.insert_nodes(&[exact_match, partial_match, no_match])
        .await
        .expect("insert_nodes failed");

    let results = db
        .search_nodes("process_data", 10)
        .await
        .expect("search_nodes failed");

    // The exact name match should appear first (highest score)
    assert!(!results.is_empty());
    assert_eq!(results[0].node.id, "sr-1");
    // Score should be positive
    assert!(results[0].score > 0.0, "score should be positive");
}

// -------------------------------------------------------------------------
// insert_all — verify all data via get_all_*
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_insert_all_comprehensive() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("ia-1", "alpha", "src/a.rs"),
        sample_node("ia-2", "beta", "src/a.rs"),
        sample_node("ia-3", "gamma", "src/b.rs"),
        sample_node("ia-4", "delta", "src/c.rs"),
    ];

    let edges = vec![
        sample_edge("ia-1", "ia-2", EdgeKind::Calls),
        sample_edge("ia-2", "ia-3", EdgeKind::Uses),
        sample_edge("ia-3", "ia-4", EdgeKind::Contains),
    ];

    let files = vec![
        sample_file("src/a.rs"),
        sample_file("src/b.rs"),
        sample_file("src/c.rs"),
    ];

    db.insert_all(&nodes, &edges, &files)
        .await
        .expect("insert_all failed");

    // Verify nodes
    let all_nodes = db.get_all_nodes().await.expect("get_all_nodes failed");
    assert_eq!(all_nodes.len(), 4);
    let node_ids: Vec<&str> = all_nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"ia-1"));
    assert!(node_ids.contains(&"ia-4"));

    // Verify edges
    let all_edges = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all_edges.len(), 3);
    let edge_pairs: Vec<(&str, &str)> = all_edges
        .iter()
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect();
    assert!(edge_pairs.contains(&("ia-1", "ia-2")));
    assert!(edge_pairs.contains(&("ia-3", "ia-4")));

    // Verify files
    let all_files = db.get_all_files().await.expect("get_all_files failed");
    assert_eq!(all_files.len(), 3);
    let file_paths: Vec<&str> = all_files.iter().map(|f| f.path.as_str()).collect();
    assert!(file_paths.contains(&"src/a.rs"));
    assert!(file_paths.contains(&"src/c.rs"));

    // Verify individual node retrieval
    let node = db
        .get_node_by_id("ia-2")
        .await
        .expect("get_node_by_id failed")
        .expect("node should exist");
    assert_eq!(node.name, "beta");
}

// -------------------------------------------------------------------------
// get_nodes_by_kind — multiple kinds in same file
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_nodes_by_kind_same_file_multiple_kinds() {
    let (db, _dir) = setup_db().await;

    let mut func = sample_node("kmf-1", "func_a", "src/mixed.rs");
    func.kind = NodeKind::Function;

    let mut strct = sample_node("kmf-2", "StructA", "src/mixed.rs");
    strct.kind = NodeKind::Struct;

    let mut method = sample_node("kmf-3", "method_a", "src/mixed.rs");
    method.kind = NodeKind::Method;

    let mut trait_node = sample_node("kmf-4", "TraitA", "src/mixed.rs");
    trait_node.kind = NodeKind::Trait;

    let mut enum_node = sample_node("kmf-5", "EnumA", "src/mixed.rs");
    enum_node.kind = NodeKind::Enum;

    db.insert_nodes(&[func, strct, method, trait_node, enum_node])
        .await
        .expect("insert_nodes failed");

    // Verify each kind is returned correctly
    let functions = db.get_nodes_by_kind(NodeKind::Function).await.unwrap();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name, "func_a");

    let structs = db.get_nodes_by_kind(NodeKind::Struct).await.unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "StructA");

    let traits = db.get_nodes_by_kind(NodeKind::Trait).await.unwrap();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "TraitA");

    let enums = db.get_nodes_by_kind(NodeKind::Enum).await.unwrap();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "EnumA");

    // All in same file
    let all = db.get_nodes_by_file("src/mixed.rs").await.unwrap();
    assert_eq!(all.len(), 5);
}

// -------------------------------------------------------------------------
// get_complexity_ranked — verify ranking by branches/max_nesting
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_complexity_ranked_by_branches_and_nesting() {
    let (db, _dir) = setup_db().await;

    // High complexity: many branches + deep nesting + large body
    let mut complex = sample_node("cxb-1", "complex_fn", "src/lib.rs");
    complex.kind = NodeKind::Function;
    complex.start_line = 1;
    complex.end_line = 100;
    complex.branches = 15;
    complex.max_nesting = 5;
    complex.loops = 3;

    // Medium complexity
    let mut medium = sample_node("cxb-2", "medium_fn", "src/lib.rs");
    medium.kind = NodeKind::Function;
    medium.start_line = 200;
    medium.end_line = 230;
    medium.branches = 5;
    medium.max_nesting = 2;
    medium.loops = 1;

    // Simple: no branching
    let mut simple = sample_node("cxb-3", "simple_fn", "src/lib.rs");
    simple.kind = NodeKind::Function;
    simple.start_line = 300;
    simple.end_line = 305;
    simple.branches = 0;
    simple.max_nesting = 0;

    db.insert_nodes(&[complex, medium, simple])
        .await
        .expect("insert_nodes failed");

    let ranked = db
        .get_complexity_ranked(None, None, 10)
        .await
        .expect("get_complexity_ranked failed");

    assert_eq!(ranked.len(), 3);
    // Highest score first (100 lines > 31 lines > 6 lines, plus fan_out/fan_in = 0)
    assert_eq!(ranked[0].0.id, "cxb-1");
    assert_eq!(ranked[0].1, 100); // lines
    assert_eq!(ranked[1].0.id, "cxb-2");
    assert_eq!(ranked[2].0.id, "cxb-3");
}

// -------------------------------------------------------------------------
// get_god_classes — varying method/field counts
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_god_classes_multiple_classes() {
    let (db, _dir) = setup_db().await;

    // Big class with many members
    let mut big_class = sample_node("gcm-big", "BigClass", "src/lib.rs");
    big_class.kind = NodeKind::Class;

    let mut m1 = sample_node("gcm-m1", "m1", "src/lib.rs");
    m1.kind = NodeKind::Method;
    let mut m2 = sample_node("gcm-m2", "m2", "src/lib.rs");
    m2.kind = NodeKind::Method;
    let mut m3 = sample_node("gcm-m3", "m3", "src/lib.rs");
    m3.kind = NodeKind::Method;
    let mut f1 = sample_node("gcm-f1", "field1", "src/lib.rs");
    f1.kind = NodeKind::Field;
    let mut f2 = sample_node("gcm-f2", "field2", "src/lib.rs");
    f2.kind = NodeKind::Field;

    // Small class with one member
    let mut small_class = sample_node("gcm-small", "SmallClass", "src/lib.rs");
    small_class.kind = NodeKind::Struct;

    let mut sm1 = sample_node("gcm-sm1", "sm_method", "src/lib.rs");
    sm1.kind = NodeKind::Method;

    db.insert_nodes(&[big_class, m1, m2, m3, f1, f2, small_class, sm1])
        .await
        .expect("insert_nodes failed");

    let edges = vec![
        Edge {
            source: "gcm-big".into(),
            target: "gcm-m1".into(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gcm-big".into(),
            target: "gcm-m2".into(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gcm-big".into(),
            target: "gcm-m3".into(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gcm-big".into(),
            target: "gcm-f1".into(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gcm-big".into(),
            target: "gcm-f2".into(),
            kind: EdgeKind::Contains,
            line: None,
        },
        Edge {
            source: "gcm-small".into(),
            target: "gcm-sm1".into(),
            kind: EdgeKind::Contains,
            line: None,
        },
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let god = db
        .get_god_classes(None, 10)
        .await
        .expect("get_god_classes failed");

    // BigClass should be first (5 total), SmallClass second (1 total)
    assert_eq!(god.len(), 2);
    assert_eq!(god[0].0.id, "gcm-big");
    assert_eq!(god[0].0.name, "BigClass");
    assert_eq!(god[0].1, 3); // methods
    assert_eq!(god[0].2, 2); // fields
    assert_eq!(god[0].3, 5); // total

    assert_eq!(god[1].0.id, "gcm-small");
    assert_eq!(god[1].1, 1); // methods
    assert_eq!(god[1].2, 0); // fields
    assert_eq!(god[1].3, 1); // total
}

// -------------------------------------------------------------------------
// Edge insertion with line: None and line: Some(n)
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_edge_line_none_and_some() {
    let (db, _dir) = setup_db().await;

    let n1 = sample_node("eln-1", "f1", "src/lib.rs");
    let n2 = sample_node("eln-2", "f2", "src/lib.rs");
    db.insert_nodes(&[n1, n2])
        .await
        .expect("insert_nodes failed");

    // Insert edge with line = None
    let edge_no_line = Edge {
        source: "eln-1".to_string(),
        target: "eln-2".to_string(),
        kind: EdgeKind::Calls,
        line: None,
    };
    db.insert_edge(&edge_no_line)
        .await
        .expect("insert_edge failed");

    // Insert edge with line = Some(42)
    let edge_with_line = Edge {
        source: "eln-1".to_string(),
        target: "eln-2".to_string(),
        kind: EdgeKind::Calls,
        line: Some(42),
    };
    db.insert_edge(&edge_with_line)
        .await
        .expect("insert_edge failed");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    // Both should exist (unique constraint includes line)
    assert_eq!(all.len(), 2);

    let lines: Vec<Option<u32>> = all.iter().map(|e| e.line).collect();
    assert!(lines.contains(&None));
    assert!(lines.contains(&Some(42)));
}

#[tokio::test]
async fn test_edge_unique_constraint_dedup() {
    let (db, _dir) = setup_db().await;

    let n1 = sample_node("euc-1", "f1", "src/lib.rs");
    let n2 = sample_node("euc-2", "f2", "src/lib.rs");
    db.insert_nodes(&[n1, n2])
        .await
        .expect("insert_nodes failed");

    // Insert the exact same edge twice — should be deduplicated by INSERT OR IGNORE
    let edge = Edge {
        source: "euc-1".to_string(),
        target: "euc-2".to_string(),
        kind: EdgeKind::Calls,
        line: Some(10),
    };
    db.insert_edge(&edge).await.expect("insert_edge failed");
    db.insert_edge(&edge)
        .await
        .expect("second insert should not fail");

    let all = db.get_all_edges().await.expect("get_all_edges failed");
    assert_eq!(all.len(), 1, "duplicate edge should be ignored");
}

// -------------------------------------------------------------------------
// get_internal_edges — larger set
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_internal_edges_larger_set() {
    let (db, _dir) = setup_db().await;

    // Create 10 nodes
    let nodes: Vec<Node> = (0..10)
        .map(|i| sample_node(&format!("iel-{i}"), &format!("fn_{i}"), "src/lib.rs"))
        .collect();
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    // Create a chain of calls: 0->1->2->...->9, plus some edges to nodes outside subset
    let mut edges = Vec::new();
    for i in 0..9 {
        edges.push(sample_edge(
            &format!("iel-{i}"),
            &format!("iel-{}", i + 1),
            EdgeKind::Calls,
        ));
    }
    db.insert_edges(&edges).await.expect("insert_edges failed");

    // Subset: nodes 0-4 (5 nodes) — should have 4 internal edges (0->1, 1->2, 2->3, 3->4)
    let subset: Vec<String> = (0..5).map(|i| format!("iel-{i}")).collect();
    let internal = db
        .get_internal_edges(&subset)
        .await
        .expect("get_internal_edges failed");

    assert_eq!(internal.len(), 4);

    // Edge 4->5 should NOT be in internal because 5 is not in subset
    let has_external = internal.iter().any(|e| e.target == "iel-5");
    assert!(
        !has_external,
        "edge to node outside subset should be excluded"
    );
}

#[tokio::test]
async fn test_fts_name_match_outranks_docstring_match() {
    let (db, _dir) = setup_db().await;

    // Node A: search term in name
    let node_a = Node {
        id: "function:aaa".to_string(),
        kind: NodeKind::Function,
        name: "sync_data".to_string(),
        qualified_name: "src/lib.rs::sync_data".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("pub fn sync_data()".to_string()),
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
    db.insert_node(&node_a).await.unwrap();

    // Node B: search term only in docstring
    let node_b = Node {
        id: "function:bbb".to_string(),
        kind: NodeKind::Function,
        name: "upload_report".to_string(),
        qualified_name: "src/lib.rs::upload_report".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 10,
        attrs_start_line: 10,
        end_line: 15,
        start_column: 0,
        end_column: 1,
        signature: Some("pub fn upload_report()".to_string()),
        docstring: Some(
            "This function runs after sync completes to upload the sync report".to_string(),
        ),
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
    db.insert_node(&node_b).await.unwrap();

    let results = db.search_nodes("sync", 10).await.unwrap();
    assert!(results.len() >= 2, "both nodes should match 'sync'");
    assert_eq!(
        results[0].node.id, "function:aaa",
        "name match should rank first"
    );
}

#[tokio::test]
async fn test_batch_incoming_call_counts() {
    let dir = tempfile::TempDir::new().unwrap();
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .unwrap();

    for (id, name) in [("fn:a", "alpha"), ("fn:b", "beta"), ("fn:c", "gamma")] {
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
            signature: None,
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

    // alpha has 2 callers, beta has 1, gamma has 0
    for (src, tgt) in [("fn:b", "fn:a"), ("fn:c", "fn:a"), ("fn:c", "fn:b")] {
        db.insert_edge(&Edge {
            source: src.to_string(),
            target: tgt.to_string(),
            kind: EdgeKind::Calls,
            line: None,
        })
        .await
        .unwrap();
    }

    let counts = db
        .batch_incoming_call_counts(&["fn:a".to_string(), "fn:b".to_string(), "fn:c".to_string()])
        .await
        .unwrap();
    assert_eq!(*counts.get("fn:a").unwrap_or(&0), 2);
    assert_eq!(*counts.get("fn:b").unwrap_or(&0), 1);
    assert_eq!(
        counts.get("fn:c"),
        None,
        "gamma has 0 callers so should be absent"
    );
}

/// Regression test: a node whose signature contains non-UTF-8 bytes (e.g. from
/// a Latin-1 encoded source file) must not crash `row_to_node`. The lossy
/// fallback replaces invalid bytes with U+FFFD.
#[tokio::test]
async fn test_non_utf8_signature_does_not_crash() {
    let (db, _dir) = setup_db().await;

    // Insert a node with a BLOB signature containing 0xFF (invalid UTF-8)
    // via raw SQL — the Rust insert_node API only accepts valid Strings.
    db.conn()
        .execute(
            "INSERT INTO nodes (id, kind, name, qualified_name, file_path, \
             start_line, end_line, start_column, end_column, \
             docstring, signature, visibility, is_async, \
             branches, loops, returns, max_nesting, \
             unsafe_blocks, unchecked_calls, assertions, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, \
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            libsql::params![
                "function:bad_utf8",
                "function",
                "render",
                "src/view.cpp::render",
                "src/view.cpp",
                1i64,
                10i64,
                0i64,
                50i64,
                // docstring with a Latin-1 copyright symbol (0xA9) — invalid UTF-8
                libsql::Value::Blob(b"Renders the sc\xe8ne with \xa9 effects".to_vec()),
                // signature with 0xFF byte
                libsql::Value::Blob(b"void render(const std::string& sc\xe8ne)".to_vec()),
                "public",
                0i64,
                0i64,
                0i64,
                0i64,
                0i64,
                0i64,
                0i64,
                0i64,
                0i64,
            ],
        )
        .await
        .unwrap();

    // This used to fail with "invalid utf-8 sequence of 1 bytes from index N"
    let node = db.get_node_by_id("function:bad_utf8").await;
    assert!(
        node.is_ok(),
        "get_node_by_id should not fail on non-UTF-8: {:?}",
        node.err()
    );
    let node = node.unwrap();
    assert!(node.is_some(), "node should exist");
    let node = node.unwrap();
    assert_eq!(node.name, "render");
    // The invalid bytes are replaced with U+FFFD (replacement character)
    assert!(node.signature.is_some());
    assert!(node.docstring.is_some());
}

// -------------------------------------------------------------------------
// get_incoming_edges_bulk
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_incoming_edges_bulk_returns_all_targets() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("caller_a", "caller_a", "src/lib.rs"),
        sample_node("caller_b", "caller_b", "src/lib.rs"),
        sample_node("target_x", "target_x", "src/lib.rs"),
        sample_node("target_y", "target_y", "src/lib.rs"),
        sample_node("isolated", "isolated", "src/lib.rs"),
    ];
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edges = vec![
        sample_edge("caller_a", "target_x", EdgeKind::Calls),
        sample_edge("caller_b", "target_x", EdgeKind::Calls),
        sample_edge("caller_a", "target_y", EdgeKind::Calls),
        sample_edge("caller_a", "isolated", EdgeKind::Uses),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let target_ids = vec!["target_x".to_string(), "target_y".to_string()];
    let result = db
        .get_incoming_edges_bulk(&target_ids, &[EdgeKind::Calls])
        .await
        .expect("get_incoming_edges_bulk failed");

    // 2 callers of target_x + 1 caller of target_y = 3 edges via Calls.
    assert_eq!(result.len(), 3);
    assert!(result.iter().all(|e| e.kind == EdgeKind::Calls));
    assert!(result
        .iter()
        .any(|e| e.target == "target_x" && e.source == "caller_a"));
    assert!(result
        .iter()
        .any(|e| e.target == "target_x" && e.source == "caller_b"));
    assert!(result
        .iter()
        .any(|e| e.target == "target_y" && e.source == "caller_a"));
}

#[tokio::test]
async fn test_get_incoming_edges_bulk_empty_kinds_returns_all_kinds() {
    let (db, _dir) = setup_db().await;

    let nodes = vec![
        sample_node("a", "a", "src/lib.rs"),
        sample_node("b", "b", "src/lib.rs"),
    ];
    db.insert_nodes(&nodes).await.expect("insert_nodes failed");

    let edges = vec![
        sample_edge("a", "b", EdgeKind::Calls),
        sample_edge("a", "b", EdgeKind::Uses),
    ];
    db.insert_edges(&edges).await.expect("insert_edges failed");

    let result = db
        .get_incoming_edges_bulk(&["b".to_string()], &[])
        .await
        .expect("get_incoming_edges_bulk failed");

    // Empty kinds should return both edges.
    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_get_incoming_edges_bulk_empty_input() {
    let (db, _dir) = setup_db().await;
    let result = db
        .get_incoming_edges_bulk(&[], &[])
        .await
        .expect("should not fail on empty input");
    assert!(result.is_empty());
}

// -------------------------------------------------------------------------
// get_nodes_by_qualified_name
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_get_nodes_by_qualified_name_returns_all_matches() {
    let (db, _dir) = setup_db().await;

    // Two nodes with the same qualified name (e.g. overloaded methods or
    // multiple impl blocks). Both should come back.
    let mut a = sample_node("a", "render", "src/foo.rs");
    a.qualified_name = "crate::foo::render".to_string();
    let mut b = sample_node("b", "render", "src/bar.rs");
    b.qualified_name = "crate::foo::render".to_string();
    let mut c = sample_node("c", "other", "src/foo.rs");
    c.qualified_name = "crate::foo::other".to_string();

    db.insert_nodes(&[a, b, c])
        .await
        .expect("insert_nodes failed");

    let hits = db
        .get_nodes_by_qualified_name("crate::foo::render")
        .await
        .expect("query failed");
    assert_eq!(hits.len(), 2);
    assert!(hits
        .iter()
        .all(|n| n.qualified_name == "crate::foo::render"));

    let none = db
        .get_nodes_by_qualified_name("crate::missing")
        .await
        .expect("query failed");
    assert!(none.is_empty());
}

// -------------------------------------------------------------------------
// attrs_start_line round-trip + backfill
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_attrs_start_line_round_trips_through_db() {
    let (db, _dir) = setup_db().await;

    let mut n = sample_node("n", "documented_fn", "src/lib.rs");
    n.start_line = 10;
    // Doc-comment block starts 4 lines above the function signature.
    n.attrs_start_line = 6;
    db.insert_node(&n).await.expect("insert failed");

    let fetched = db
        .get_node_by_id("n")
        .await
        .expect("query failed")
        .expect("node missing");
    assert_eq!(fetched.start_line, 10);
    assert_eq!(fetched.attrs_start_line, 6);
}
