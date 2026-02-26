use codegraph::db::Database;
use codegraph::types::*;
use tempfile::TempDir;

/// Helper: create an in-memory-style temp database and return (Database, TempDir).
/// The TempDir is returned so that it stays alive for the duration of the test.
fn setup_db() -> (Database, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let db = Database::initialize(&db_path).expect("failed to initialize database");
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
        end_line: 10,
        start_column: 0,
        end_column: 1,
        signature: Some(format!("fn {name}()")),
        docstring: Some(format!("Documentation for {name}")),
        visibility: Visibility::Pub,
        is_async: false,
        updated_at: 1000,
    }
}

#[test]
fn test_initialize_creates_database() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("subdir").join("code_graph.db");
    let _db = Database::initialize(&db_path).expect("failed to initialize database");
    assert!(
        db_path.exists(),
        "database file should exist after initialize"
    );
}

#[test]
fn test_insert_and_get_node() {
    let (db, _dir) = setup_db();
    let node = sample_node("node-1", "process_data", "src/main.rs");

    db.insert_node(&node).expect("failed to insert node");

    let fetched = db
        .get_node_by_id("node-1")
        .expect("failed to get node")
        .expect("node should exist");

    assert_eq!(fetched.id, "node-1");
    assert_eq!(fetched.name, "process_data");
    assert_eq!(fetched.kind, NodeKind::Function);
    assert_eq!(fetched.qualified_name, "crate::process_data");
    assert_eq!(fetched.file_path, "src/main.rs");
    assert_eq!(fetched.start_line, 1);
    assert_eq!(fetched.end_line, 10);
    assert_eq!(fetched.signature, Some("fn process_data()".to_string()));
    assert_eq!(
        fetched.docstring,
        Some("Documentation for process_data".to_string())
    );
    assert_eq!(fetched.visibility, Visibility::Pub);
    assert!(!fetched.is_async);
    assert_eq!(fetched.updated_at, 1000);
}

#[test]
fn test_insert_and_get_edge() {
    let (db, _dir) = setup_db();
    let node_a = sample_node("node-a", "caller", "src/lib.rs");
    let node_b = sample_node("node-b", "callee", "src/lib.rs");

    db.insert_node(&node_a).expect("failed to insert node a");
    db.insert_node(&node_b).expect("failed to insert node b");

    let edge = Edge {
        source: "node-a".to_string(),
        target: "node-b".to_string(),
        kind: EdgeKind::Calls,
        line: Some(5),
    };
    db.insert_edge(&edge).expect("failed to insert edge");

    // Outgoing from node-a
    let outgoing = db
        .get_outgoing_edges("node-a", &[])
        .expect("failed to get outgoing edges");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].source, "node-a");
    assert_eq!(outgoing[0].target, "node-b");
    assert_eq!(outgoing[0].kind, EdgeKind::Calls);
    assert_eq!(outgoing[0].line, Some(5));

    // Incoming to node-b
    let incoming = db
        .get_incoming_edges("node-b", &[])
        .expect("failed to get incoming edges");
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].source, "node-a");

    // Filter by kind — should match
    let filtered = db
        .get_outgoing_edges("node-a", &[EdgeKind::Calls])
        .expect("failed to get filtered edges");
    assert_eq!(filtered.len(), 1);

    // Filter by wrong kind — should be empty
    let empty = db
        .get_outgoing_edges("node-a", &[EdgeKind::Uses])
        .expect("failed to get filtered edges");
    assert!(empty.is_empty());
}

#[test]
fn test_upsert_file() {
    let (db, _dir) = setup_db();

    let file = FileRecord {
        path: "src/main.rs".to_string(),
        content_hash: "abc123".to_string(),
        size: 4096,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 5,
    };

    db.upsert_file(&file).expect("failed to upsert file");

    let fetched = db
        .get_file("src/main.rs")
        .expect("failed to get file")
        .expect("file should exist");

    assert_eq!(fetched.path, "src/main.rs");
    assert_eq!(fetched.content_hash, "abc123");
    assert_eq!(fetched.size, 4096);
    assert_eq!(fetched.modified_at, 1000);
    assert_eq!(fetched.indexed_at, 2000);
    assert_eq!(fetched.node_count, 5);

    // Upsert again with different hash — should replace
    let updated_file = FileRecord {
        path: "src/main.rs".to_string(),
        content_hash: "def456".to_string(),
        size: 8192,
        modified_at: 3000,
        indexed_at: 4000,
        node_count: 10,
    };
    db.upsert_file(&updated_file)
        .expect("failed to upsert file");

    let fetched2 = db
        .get_file("src/main.rs")
        .expect("failed to get file")
        .expect("file should exist");
    assert_eq!(fetched2.content_hash, "def456");
    assert_eq!(fetched2.size, 8192);
}

#[test]
fn test_fts_search() {
    let (db, _dir) = setup_db();

    let node = sample_node("fts-node", "process_request", "src/handler.rs");
    db.insert_node(&node).expect("failed to insert node");

    let results = db
        .search_nodes("process", 10)
        .expect("failed to search nodes");
    assert!(
        !results.is_empty(),
        "FTS search for 'process' should find 'process_request'"
    );
    assert_eq!(results[0].node.id, "fts-node");
    assert!(results[0].score > 0.0);
}

#[test]
fn test_get_stats() {
    let (db, _dir) = setup_db();

    let node = sample_node("stats-node", "my_func", "src/lib.rs");
    db.insert_node(&node).expect("failed to insert node");

    let stats = db.get_stats().expect("failed to get stats");
    assert_eq!(stats.node_count, 1);
    assert_eq!(stats.edge_count, 0);
    assert_eq!(stats.file_count, 0);
    assert_eq!(
        stats.nodes_by_kind.get("function"),
        Some(&1),
        "should have 1 function node"
    );
    assert!(stats.db_size_bytes > 0);
}

#[test]
fn test_delete_nodes_by_file() {
    let (db, _dir) = setup_db();

    let node1 = sample_node("del-1", "func_a", "src/target.rs");
    let node2 = sample_node("del-2", "func_b", "src/target.rs");
    let node_other = sample_node("del-3", "func_c", "src/other.rs");

    db.insert_nodes(&[node1, node2, node_other])
        .expect("failed to insert nodes");

    // Insert an edge between the target nodes
    let edge = Edge {
        source: "del-1".to_string(),
        target: "del-2".to_string(),
        kind: EdgeKind::Calls,
        line: None,
    };
    db.insert_edge(&edge).expect("failed to insert edge");

    // Delete nodes for src/target.rs
    db.delete_nodes_by_file("src/target.rs")
        .expect("failed to delete nodes by file");

    // Verify they are gone
    let nodes = db
        .get_nodes_by_file("src/target.rs")
        .expect("failed to get nodes by file");
    assert!(nodes.is_empty(), "nodes for target.rs should be deleted");

    // Verify the other file's node is still there
    let other_nodes = db
        .get_nodes_by_file("src/other.rs")
        .expect("failed to get nodes by file");
    assert_eq!(other_nodes.len(), 1);
    assert_eq!(other_nodes[0].id, "del-3");
}

#[test]
fn test_unresolved_refs() {
    let (db, _dir) = setup_db();

    // Insert a node first (FK constraint)
    let node = sample_node("ref-node", "my_func", "src/lib.rs");
    db.insert_node(&node).expect("failed to insert node");

    let uref = UnresolvedRef {
        from_node_id: "ref-node".to_string(),
        reference_name: "HashMap".to_string(),
        reference_kind: EdgeKind::Uses,
        line: 10,
        column: 5,
        file_path: "src/lib.rs".to_string(),
    };

    db.insert_unresolved_ref(&uref)
        .expect("failed to insert unresolved ref");

    let refs = db
        .get_unresolved_refs()
        .expect("failed to get unresolved refs");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].from_node_id, "ref-node");
    assert_eq!(refs[0].reference_name, "HashMap");
    assert_eq!(refs[0].reference_kind, EdgeKind::Uses);
    assert_eq!(refs[0].line, 10);
    assert_eq!(refs[0].column, 5);
    assert_eq!(refs[0].file_path, "src/lib.rs");

    // Clear and verify
    db.clear_unresolved_refs()
        .expect("failed to clear unresolved refs");
    let refs_after = db
        .get_unresolved_refs()
        .expect("failed to get unresolved refs");
    assert!(refs_after.is_empty());
}

#[test]
fn test_batch_insert_nodes() {
    let (db, _dir) = setup_db();

    let nodes: Vec<Node> = (0..10)
        .map(|i| sample_node(&format!("batch-{i}"), &format!("func_{i}"), "src/batch.rs"))
        .collect();

    db.insert_nodes(&nodes)
        .expect("failed to batch insert nodes");

    let fetched = db
        .get_nodes_by_file("src/batch.rs")
        .expect("failed to get nodes by file");
    assert_eq!(fetched.len(), 10);
}

#[test]
fn test_clear() {
    let (db, _dir) = setup_db();

    let node = sample_node("clear-1", "func", "src/lib.rs");
    db.insert_node(&node).expect("failed to insert node");

    let file = FileRecord {
        path: "src/lib.rs".to_string(),
        content_hash: "hash".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 1,
    };
    db.upsert_file(&file).expect("failed to upsert file");

    db.clear().expect("failed to clear database");

    let stats = db.get_stats().expect("failed to get stats");
    assert_eq!(stats.node_count, 0);
    assert_eq!(stats.edge_count, 0);
    assert_eq!(stats.file_count, 0);
}

#[test]
fn test_get_node_not_found() {
    let (db, _dir) = setup_db();
    let result = db
        .get_node_by_id("nonexistent")
        .expect("query should not fail");
    assert!(result.is_none());
}

#[test]
fn test_optimize() {
    let (db, _dir) = setup_db();
    db.optimize().expect("optimize should not fail");
}

#[test]
fn test_database_size() {
    let (db, _dir) = setup_db();
    let size = db.size().expect("size should not fail");
    assert!(size > 0, "database should have non-zero size");
}
