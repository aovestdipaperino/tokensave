use tempfile::TempDir;
use tokensave::db::Database;
use tokensave::types::*;

/// Helper: create an in-memory-style temp database and return (Database, TempDir).
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

#[tokio::test]
async fn test_initialize_creates_database() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("subdir").join("code_graph.db");
    let (_db, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
    assert!(
        db_path.exists(),
        "database file should exist after initialize"
    );
}

#[tokio::test]
async fn test_insert_and_get_node() {
    let (db, _dir) = setup_db().await;
    let node = sample_node("node-1", "process_data", "src/main.rs");

    db.insert_node(&node).await.expect("failed to insert node");

    let fetched = db
        .get_node_by_id("node-1")
        .await
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

#[tokio::test]
async fn test_insert_and_get_edge() {
    let (db, _dir) = setup_db().await;
    let node_a = sample_node("node-a", "caller", "src/lib.rs");
    let node_b = sample_node("node-b", "callee", "src/lib.rs");

    db.insert_node(&node_a)
        .await
        .expect("failed to insert node a");
    db.insert_node(&node_b)
        .await
        .expect("failed to insert node b");

    let edge = Edge {
        source: "node-a".to_string(),
        target: "node-b".to_string(),
        kind: EdgeKind::Calls,
        line: Some(5),
    };
    db.insert_edge(&edge).await.expect("failed to insert edge");

    // Outgoing from node-a
    let outgoing = db
        .get_outgoing_edges("node-a", &[])
        .await
        .expect("failed to get outgoing edges");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].source, "node-a");
    assert_eq!(outgoing[0].target, "node-b");
    assert_eq!(outgoing[0].kind, EdgeKind::Calls);
    assert_eq!(outgoing[0].line, Some(5));

    // Incoming to node-b
    let incoming = db
        .get_incoming_edges("node-b", &[])
        .await
        .expect("failed to get incoming edges");
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].source, "node-a");

    // Filter by kind — should match
    let filtered = db
        .get_outgoing_edges("node-a", &[EdgeKind::Calls])
        .await
        .expect("failed to get filtered edges");
    assert_eq!(filtered.len(), 1);

    // Filter by wrong kind — should be empty
    let empty = db
        .get_outgoing_edges("node-a", &[EdgeKind::Uses])
        .await
        .expect("failed to get filtered edges");
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_upsert_file() {
    let (db, _dir) = setup_db().await;

    let file = FileRecord {
        path: "src/main.rs".to_string(),
        content_hash: "abc123".to_string(),
        size: 4096,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 5,
    };

    db.upsert_file(&file).await.expect("failed to upsert file");

    let fetched = db
        .get_file("src/main.rs")
        .await
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
        .await
        .expect("failed to upsert file");

    let fetched2 = db
        .get_file("src/main.rs")
        .await
        .expect("failed to get file")
        .expect("file should exist");
    assert_eq!(fetched2.content_hash, "def456");
    assert_eq!(fetched2.size, 8192);
}

#[tokio::test]
async fn test_fts_search() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("fts-node", "process_request", "src/handler.rs");
    db.insert_node(&node).await.expect("failed to insert node");

    let results = db
        .search_nodes("process", 10)
        .await
        .expect("failed to search nodes");
    assert!(
        !results.is_empty(),
        "FTS search for 'process' should find 'process_request'"
    );
    assert_eq!(results[0].node.id, "fts-node");
    assert!(results[0].score > 0.0);
}

#[tokio::test]
async fn test_get_stats() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("stats-node", "my_func", "src/lib.rs");
    db.insert_node(&node).await.expect("failed to insert node");

    let stats = db.get_stats().await.expect("failed to get stats");
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

#[tokio::test]
async fn test_delete_nodes_by_file() {
    let (db, _dir) = setup_db().await;

    let node1 = sample_node("del-1", "func_a", "src/target.rs");
    let node2 = sample_node("del-2", "func_b", "src/target.rs");
    let node_other = sample_node("del-3", "func_c", "src/other.rs");

    db.insert_nodes(&[node1, node2, node_other])
        .await
        .expect("failed to insert nodes");

    // Insert an edge between the target nodes
    let edge = Edge {
        source: "del-1".to_string(),
        target: "del-2".to_string(),
        kind: EdgeKind::Calls,
        line: None,
    };
    db.insert_edge(&edge).await.expect("failed to insert edge");

    // Delete nodes for src/target.rs
    db.delete_nodes_by_file("src/target.rs")
        .await
        .expect("failed to delete nodes by file");

    // Verify they are gone
    let nodes = db
        .get_nodes_by_file("src/target.rs")
        .await
        .expect("failed to get nodes by file");
    assert!(nodes.is_empty(), "nodes for target.rs should be deleted");

    // Verify the other file's node is still there
    let other_nodes = db
        .get_nodes_by_file("src/other.rs")
        .await
        .expect("failed to get nodes by file");
    assert_eq!(other_nodes.len(), 1);
    assert_eq!(other_nodes[0].id, "del-3");
}

#[tokio::test]
async fn test_unresolved_refs() {
    let (db, _dir) = setup_db().await;

    // Insert a node first (FK constraint)
    let node = sample_node("ref-node", "my_func", "src/lib.rs");
    db.insert_node(&node).await.expect("failed to insert node");

    let uref = UnresolvedRef {
        from_node_id: "ref-node".to_string(),
        reference_name: "HashMap".to_string(),
        reference_kind: EdgeKind::Uses,
        line: 10,
        column: 5,
        file_path: "src/lib.rs".to_string(),
    };

    db.insert_unresolved_ref(&uref)
        .await
        .expect("failed to insert unresolved ref");

    let refs = db
        .get_unresolved_refs()
        .await
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
        .await
        .expect("failed to clear unresolved refs");
    let refs_after = db
        .get_unresolved_refs()
        .await
        .expect("failed to get unresolved refs");
    assert!(refs_after.is_empty());
}

#[tokio::test]
async fn test_batch_insert_nodes() {
    let (db, _dir) = setup_db().await;

    let nodes: Vec<Node> = (0..10)
        .map(|i| sample_node(&format!("batch-{i}"), &format!("func_{i}"), "src/batch.rs"))
        .collect();

    db.insert_nodes(&nodes)
        .await
        .expect("failed to batch insert nodes");

    let fetched = db
        .get_nodes_by_file("src/batch.rs")
        .await
        .expect("failed to get nodes by file");
    assert_eq!(fetched.len(), 10);
}

#[tokio::test]
async fn test_clear() {
    let (db, _dir) = setup_db().await;

    let node = sample_node("clear-1", "func", "src/lib.rs");
    db.insert_node(&node).await.expect("failed to insert node");

    let file = FileRecord {
        path: "src/lib.rs".to_string(),
        content_hash: "hash".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 2000,
        node_count: 1,
    };
    db.upsert_file(&file).await.expect("failed to upsert file");

    db.clear().await.expect("failed to clear database");

    let stats = db.get_stats().await.expect("failed to get stats");
    assert_eq!(stats.node_count, 0);
    assert_eq!(stats.edge_count, 0);
    assert_eq!(stats.file_count, 0);
}

#[tokio::test]
async fn test_get_node_not_found() {
    let (db, _dir) = setup_db().await;
    let result = db
        .get_node_by_id("nonexistent")
        .await
        .expect("query should not fail");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_optimize() {
    let (db, _dir) = setup_db().await;
    db.optimize().await.expect("optimize should not fail");
}

#[tokio::test]
async fn test_database_size() {
    let (db, _dir) = setup_db().await;
    let size = db.size().await.expect("size should not fail");
    assert!(size > 0, "database should have non-zero size");
}

// ---------------------------------------------------------------------------
// Migration v7: attrs_start_line column add + backfill
// ---------------------------------------------------------------------------
//
// Builds a v6-shaped nodes table directly (no attrs_start_line column), inserts
// rows with various start_line values, runs the migration runner, and verifies
// the column now exists with values backfilled from start_line.

#[tokio::test]
async fn test_migrate_v7_adds_and_backfills_attrs_start_line() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("v6.db");

    // Open the DB directly so we can build a v6-shaped schema (no
    // attrs_start_line column) before running the migration.
    let lib_db = libsql::Builder::new_local(&db_path)
        .build()
        .await
        .expect("build db");
    let conn = lib_db.connect().expect("connect");

    conn.execute_batch(
        "CREATE TABLE nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            start_column INTEGER NOT NULL,
            end_column INTEGER NOT NULL,
            docstring TEXT,
            signature TEXT,
            visibility TEXT NOT NULL DEFAULT 'private',
            is_async INTEGER NOT NULL DEFAULT 0,
            branches INTEGER NOT NULL DEFAULT 0,
            loops INTEGER NOT NULL DEFAULT 0,
            returns INTEGER NOT NULL DEFAULT 0,
            max_nesting INTEGER NOT NULL DEFAULT 0,
            unsafe_blocks INTEGER NOT NULL DEFAULT 0,
            unchecked_calls INTEGER NOT NULL DEFAULT 0,
            assertions INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        );
         PRAGMA user_version = 6;",
    )
    .await
    .expect("v6 schema setup");

    // Two rows: one with a normal start_line, one a file root with start_line=0.
    conn.execute(
        "INSERT INTO nodes (id, kind, name, qualified_name, file_path,
                            start_line, end_line, start_column, end_column, updated_at)
         VALUES ('a', 'function', 'foo', 'crate::foo', 'src/lib.rs', 42, 50, 0, 1, 1000)",
        (),
    )
    .await
    .expect("insert row a");
    conn.execute(
        "INSERT INTO nodes (id, kind, name, qualified_name, file_path,
                            start_line, end_line, start_column, end_column, updated_at)
         VALUES ('b', 'file', 'src/lib.rs', 'src/lib.rs', 'src/lib.rs', 0, 100, 0, 0, 1000)",
        (),
    )
    .await
    .expect("insert row b");

    // Run pending migrations — should apply v7.
    let migrated = tokensave::db::migrations::migrate(&conn)
        .await
        .expect("migrate failed");
    assert!(migrated, "expected v7 migration to run");

    // user_version is now LATEST (= 7).
    let mut rows = conn
        .query("PRAGMA user_version", ())
        .await
        .expect("read version");
    let row = rows.next().await.expect("row").expect("some row");
    let version: i64 = row.get(0).expect("version");
    assert_eq!(version, 7);

    // attrs_start_line is backfilled from start_line for both rows.
    // Row a: start_line=42 -> attrs_start_line=42.
    // Row b: start_line=0  -> attrs_start_line stays 0 (file root, consistent).
    let mut rows = conn
        .query(
            "SELECT id, start_line, attrs_start_line FROM nodes ORDER BY id",
            (),
        )
        .await
        .expect("select");
    let r1 = rows.next().await.expect("row").expect("row a missing");
    assert_eq!(r1.get::<String>(0).expect("id"), "a");
    assert_eq!(r1.get::<u32>(1).expect("start_line"), 42);
    assert_eq!(
        r1.get::<u32>(2).expect("attrs_start_line"),
        42,
        "attrs_start_line should backfill from start_line"
    );

    let r2 = rows.next().await.expect("row").expect("row b missing");
    assert_eq!(r2.get::<String>(0).expect("id"), "b");
    assert_eq!(r2.get::<u32>(1).expect("start_line"), 0);
    assert_eq!(r2.get::<u32>(2).expect("attrs_start_line"), 0);

    // Inserting a fresh row with an explicit attrs_start_line works post-migration.
    conn.execute(
        "INSERT INTO nodes (id, kind, name, qualified_name, file_path,
                            start_line, end_line, start_column, end_column, updated_at,
                            attrs_start_line)
         VALUES ('c', 'function', 'bar', 'crate::bar', 'src/lib.rs', 60, 70, 0, 1, 2000, 55)",
        (),
    )
    .await
    .expect("insert row c");
    let mut rows = conn
        .query("SELECT attrs_start_line FROM nodes WHERE id = 'c'", ())
        .await
        .expect("select c");
    let r = rows.next().await.expect("row").expect("row c missing");
    assert_eq!(r.get::<u32>(0).expect("attrs"), 55);
}

#[tokio::test]
async fn test_migrate_is_idempotent_at_latest() {
    // After Database::initialize creates the latest schema, calling migrate
    // again must be a no-op (returns false) — guards against accidental
    // re-runs of v7's ALTER TABLE on an already-migrated DB.
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("idem.db");
    let (db, _) = Database::initialize(&db_path).await.expect("initialize");
    drop(db);

    let lib_db = libsql::Builder::new_local(&db_path)
        .build()
        .await
        .expect("build db");
    let conn = lib_db.connect().expect("connect");

    let migrated = tokensave::db::migrations::migrate(&conn)
        .await
        .expect("migrate");
    assert!(!migrated, "second migrate should be a no-op");
}
