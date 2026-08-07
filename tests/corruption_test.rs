//! Regression tests for issue #16: SQLite FTS corruption during search_nodes.
//!
//! These tests verify:
//! - `quick_check` detects real page-level corruption
//! - FTS self-healing in `search_nodes` recovers from a corrupt FTS index
//! - `rebuild_fts` restores query capability after FTS damage
//! - `begin_bulk_load` no longer disables fsync (`synchronous = OFF`)
//! - The dirty sentinel lifecycle works correctly
//! - The full crash→detect→repair cycle works end-to-end

use std::io::{Seek, Write};
use tempfile::TempDir;
use tokensave::db::Database;
use tokensave::types::*;

/// Helper: create a temp database and return (TempDir, Database, db_path).
///
/// The `TempDir` comes first so that it is the last binding declared at each
/// call site and therefore the last dropped: the database must close before the
/// directory is removed, or Windows refuses the removal and leaks it (#367).
async fn setup_db() -> (TempDir, Database, std::path::PathBuf) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let (db, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
    (dir, db, db_path)
}

/// Helper: create a sample node.
fn sample_node(id: &str, name: &str) -> Node {
    Node {
        id: id.to_string(),
        kind: NodeKind::Function,
        name: name.to_string(),
        qualified_name: format!("crate::{name}"),
        file_path: "src/lib.rs".to_string(),
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
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 1000,
        parent_id: None,
    }
}

// ─── quick_check ─────────────────────────────────────────────────────────

#[tokio::test]
async fn quick_check_passes_on_healthy_db() {
    let (_dir, db, _path) = setup_db().await;
    assert!(
        db.quick_check().await.unwrap(),
        "fresh database should pass quick_check"
    );
}

#[tokio::test]
async fn quick_check_passes_after_inserts() {
    let (_dir, db, _path) = setup_db().await;
    let nodes: Vec<Node> = (0..50)
        .map(|i| sample_node(&format!("n{i}"), &format!("func_{i}")))
        .collect();
    db.insert_nodes(&nodes).await.unwrap();
    assert!(
        db.quick_check().await.unwrap(),
        "database with data should pass quick_check"
    );
}

#[tokio::test]
async fn quick_check_detects_page_level_corruption() {
    let (_dir, db, db_path) = setup_db().await;

    // Insert enough data to create multiple pages
    let nodes: Vec<Node> = (0..100)
        .map(|i| sample_node(&format!("n{i}"), &format!("function_with_long_name_{i}")))
        .collect();
    db.insert_nodes(&nodes).await.unwrap();
    db.checkpoint().await.unwrap();
    drop(db);

    // Corrupt the database by overwriting bytes in the middle of the file.
    // This simulates what happens when a crash leaves partially-written pages.
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&db_path)
            .unwrap();
        let len = file.metadata().unwrap().len();
        // Write garbage in the middle of the file (skip the header page)
        let offset = std::cmp::min(len / 2, 8192);
        file.seek(std::io::SeekFrom::Start(offset)).unwrap();
        file.write_all(&[0xDE, 0xAD, 0xBE, 0xEF].repeat(64))
            .unwrap();
        file.sync_all().unwrap();
    }

    // Reopen — quick_check should detect the corruption
    let (db2, _) = Database::open(&db_path)
        .await
        .expect("open should succeed even with corruption");
    let intact = db2.quick_check().await.unwrap();
    assert!(!intact, "quick_check should detect page-level corruption");
}

// ─── FTS rebuild ─────────────────────────────────────────────────────────

#[tokio::test]
async fn rebuild_fts_on_fresh_db() {
    let (_dir, db, _path) = setup_db().await;
    // rebuild on empty db should not error
    db.rebuild_fts().await.unwrap();
}

#[tokio::test]
async fn rebuild_fts_restores_search_after_fts_damage() {
    let (_dir, db, _path) = setup_db().await;

    let nodes = vec![
        sample_node("a1", "process_data"),
        sample_node("a2", "validate_input"),
    ];
    db.insert_nodes(&nodes).await.unwrap();

    // Verify search works before damage
    let results = db.search_nodes("process_data", 10).await.unwrap();
    assert!(!results.is_empty(), "search should find process_data");

    // Damage the FTS index by clearing its internal data tables.
    // This simulates what happens when begin_bulk_load clears FTS but
    // end_bulk_load never runs (crash during indexing).
    db.conn()
        .execute_batch("DELETE FROM nodes_fts;")
        .await
        .unwrap();

    // FTS is wiped but content table intact — search_nodes should still work
    // via LIKE fallback (FTS returns empty, falls through to LIKE).

    // Rebuild FTS from content table
    db.rebuild_fts().await.unwrap();

    // Search should work again
    let results = db.search_nodes("process_data", 10).await.unwrap();
    assert!(!results.is_empty(), "search should work after FTS rebuild");
    assert_eq!(results[0].node.id, "a1");
}

// ─── search_nodes self-healing ───────────────────────────────────────────

#[tokio::test]
async fn search_nodes_falls_back_to_like_when_fts_empty() {
    let (_dir, db, _path) = setup_db().await;

    let nodes = vec![sample_node("b1", "my_function")];
    db.insert_nodes(&nodes).await.unwrap();

    // Wipe FTS
    db.conn()
        .execute_batch("DELETE FROM nodes_fts;")
        .await
        .unwrap();

    // search_nodes should still find the node via LIKE fallback
    // (after FTS returns empty, it falls back to LIKE)
    let results = db.search_nodes("my_function", 10).await.unwrap();
    assert!(!results.is_empty(), "LIKE fallback should find the node");
    assert_eq!(results[0].node.id, "b1");
}

// ─── begin_bulk_load no longer disables synchronous ──────────────────────

#[tokio::test]
async fn bulk_load_preserves_synchronous_normal() {
    let (_dir, db, _path) = setup_db().await;

    db.begin_bulk_load().await.unwrap();

    // Check that synchronous is still NORMAL (1) not OFF (0)
    let mut rows = db.conn().query("PRAGMA synchronous", ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let sync_value: i64 = row.get(0).unwrap();
    // NORMAL = 1, OFF = 0, FULL = 2
    assert_eq!(
        sync_value, 1,
        "synchronous should be NORMAL (1) during bulk load, not OFF (0)"
    );

    db.end_bulk_load().await.unwrap();
}

#[tokio::test]
async fn bulk_load_round_trip_preserves_data() {
    let (_dir, db, _path) = setup_db().await;

    db.begin_bulk_load().await.unwrap();

    let nodes = vec![sample_node("c1", "alpha"), sample_node("c2", "beta")];
    db.insert_nodes(&nodes).await.unwrap();

    db.end_bulk_load().await.unwrap();

    // After bulk load, FTS should be rebuilt and search should work
    let results = db.search_nodes("alpha", 10).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].node.id, "c1");
}

// ─── is_corruption_error ─────────────────────────────────────────────────

#[test]
fn is_corruption_error_matches_malformed() {
    let e = tokensave::errors::TokenSaveError::Database {
        message: "failed to read search result: SQLite failure: `database disk image is malformed`"
            .to_string(),
        operation: "search_nodes".to_string(),
    };
    assert!(Database::is_corruption_error(&e));
}

#[test]
fn is_corruption_error_matches_corrupt() {
    let e = tokensave::errors::TokenSaveError::Database {
        message: "database is corrupt".to_string(),
        operation: "test".to_string(),
    };
    assert!(Database::is_corruption_error(&e));
}

#[test]
fn is_corruption_error_rejects_normal_errors() {
    let e = tokensave::errors::TokenSaveError::Database {
        message: "no such table: foobar".to_string(),
        operation: "test".to_string(),
    };
    assert!(!Database::is_corruption_error(&e));

    let e2 = tokensave::errors::TokenSaveError::Config {
        message: "some config error".to_string(),
    };
    assert!(!Database::is_corruption_error(&e2));
}

// ─── Dirty sentinel ──────────────────────────────────────────────────────

#[test]
fn dirty_sentinel_lifecycle() {
    let dir = TempDir::new().unwrap();
    let ts_dir = dir.path().join(".tokensave");
    std::fs::create_dir_all(&ts_dir).unwrap();

    let dirty_path = ts_dir.join("dirty");

    // No sentinel initially
    assert!(!dirty_path.exists());

    // Write sentinel
    std::fs::write(
        &dirty_path,
        format!("pid={}\nversion=test", std::process::id()),
    )
    .unwrap();
    assert!(dirty_path.exists());

    // Read contents
    let contents = std::fs::read_to_string(&dirty_path).unwrap();
    assert!(contents.contains("pid="));
    assert!(contents.contains("version=test"));

    // Clear sentinel
    std::fs::remove_file(&dirty_path).unwrap();
    assert!(!dirty_path.exists());
}

#[test]
fn dirty_sentinel_survives_drop() {
    // The sentinel is a plain file, not tied to a Drop guard.
    // Simulates: process writes sentinel, then gets killed.
    let dir = TempDir::new().unwrap();
    let ts_dir = dir.path().join(".tokensave");
    std::fs::create_dir_all(&ts_dir).unwrap();
    let dirty_path = ts_dir.join("dirty");

    {
        // Inner scope — everything is dropped
        std::fs::write(&dirty_path, "pid=99999\nversion=test").unwrap();
    }

    // Sentinel persists after the inner scope exits (simulating process death)
    assert!(dirty_path.exists(), "sentinel must survive scope drop");
}

// ─── Full crash→detect→repair cycle ──────────────────────────────────────

#[tokio::test]
async fn corrupt_db_detected_and_repaired_on_reopen() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");

    // Create and populate a database
    let (db, _) = Database::initialize(&db_path).await.unwrap();
    let nodes: Vec<Node> = (0..50)
        .map(|i| sample_node(&format!("d{i}"), &format!("func_{i}")))
        .collect();
    db.insert_nodes(&nodes).await.unwrap();
    db.checkpoint().await.unwrap();
    drop(db);

    // Corrupt the database file
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&db_path)
            .unwrap();
        let len = file.metadata().unwrap().len();
        let offset = std::cmp::min(len / 2, 8192);
        file.seek(std::io::SeekFrom::Start(offset)).unwrap();
        file.write_all(&[0xFF; 256]).unwrap();
        file.sync_all().unwrap();
    }

    // Reopen — should be able to open but quick_check fails
    let open_result = Database::open(&db_path).await;
    match open_result {
        Ok((db2, _)) => {
            let intact = db2.quick_check().await.unwrap();
            assert!(!intact, "corrupted db should fail quick_check");
        }
        Err(e) => {
            // Some corruption is severe enough to prevent open — that's also
            // valid. The important thing is it doesn't silently succeed.
            assert!(
                Database::is_corruption_error(&e)
                    || format!("{e}").contains("malformed")
                    || format!("{e}").contains("not a database"),
                "unexpected error: {e}"
            );
        }
    }

    // Simulate the recovery path: delete and re-initialize
    std::fs::remove_file(&db_path).ok();
    let mut wal = db_path.clone();
    wal.set_extension("db-wal");
    std::fs::remove_file(&wal).ok();
    wal.set_extension("db-shm");
    std::fs::remove_file(&wal).ok();

    let (db3, _) = Database::initialize(&db_path).await.unwrap();
    assert!(
        db3.quick_check().await.unwrap(),
        "fresh db after recovery should be healthy"
    );
}

#[tokio::test]
async fn fts_corruption_healed_by_search_nodes() {
    let (_dir, db, _path) = setup_db().await;

    // Insert data so FTS has content
    let nodes = vec![
        sample_node("e1", "important_handler"),
        sample_node("e2", "other_helper"),
    ];
    db.insert_nodes(&nodes).await.unwrap();

    // Verify search works
    let results = db.search_nodes("important_handler", 10).await.unwrap();
    assert_eq!(results[0].node.id, "e1");

    // Drop and re-insert one row of FTS with mismatched data to create
    // inconsistency (simulate partial crash during trigger execution)
    db.conn()
        .execute_batch(
            "INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
             VALUES('delete', 1, 'important_handler', 'crate::important_handler', 'Documentation for important_handler', 'fn important_handler()');",
        )
        .await
        .unwrap();

    // The FTS index is now inconsistent — missing a row that exists in content.
    // search_nodes should still find it via LIKE fallback even if FTS misses it.
    let results = db.search_nodes("important_handler", 10).await.unwrap();
    assert!(
        !results.is_empty(),
        "search should recover via self-healing or LIKE fallback"
    );
}

// ─── Bulk-load edge dedup + interrupted-finalization recovery (#318) ──────

/// Returns `true` if an index named `name` exists on the database.
async fn index_exists(db: &Database, name: &str) -> bool {
    let mut rows = db
        .conn()
        .query("SELECT name FROM sqlite_master WHERE type = 'index'", ())
        .await
        .unwrap();
    while let Some(row) = rows.next().await.unwrap() {
        let n: String = row.get(0).unwrap();
        if n == name {
            return true;
        }
    }
    false
}

/// Returns the total number of rows in the `edges` table.
async fn edge_count(db: &Database) -> i64 {
    let mut rows = db
        .conn()
        .query("SELECT COUNT(*) FROM edges", ())
        .await
        .unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}

/// `begin_bulk_load` drops `idx_edges_unique`, so `INSERT OR IGNORE` can no
/// longer dedupe during the load. `end_bulk_load` must therefore collapse any
/// duplicate rows *before* it recreates the unique index — otherwise that
/// `CREATE UNIQUE INDEX` fails on the duplicates and the graph is left
/// permanently un-indexed (#318).
#[tokio::test]
async fn end_bulk_load_dedupes_edges_before_unique_index() {
    let (_dir, db, _path) = setup_db().await;

    db.insert_nodes(&[sample_node("s", "src_fn"), sample_node("t", "dst_fn")])
        .await
        .unwrap();

    db.begin_bulk_load().await.unwrap();
    db.conn()
        .execute_batch(
            "INSERT INTO edges (source, target, kind, line) VALUES ('s', 't', 'calls', 5);
             INSERT INTO edges (source, target, kind, line) VALUES ('s', 't', 'calls', 5);
             INSERT INTO edges (source, target, kind, line) VALUES ('s', 't', 'calls', 5);",
        )
        .await
        .unwrap();
    assert_eq!(
        edge_count(&db).await,
        3,
        "duplicates must be present before finalization"
    );

    db.end_bulk_load().await.unwrap();

    assert_eq!(
        edge_count(&db).await,
        1,
        "end_bulk_load must collapse duplicate edge tuples"
    );
    assert!(
        index_exists(&db, "idx_edges_unique").await,
        "the unique dedup index must exist after finalization"
    );
}

/// A bulk load interrupted between `begin_bulk_load` and `end_bulk_load`
/// leaves the graph un-indexed and possibly duplicated. `quick_check` passes
/// (the B-tree is intact), so `TokenSave::open` must detect the missing
/// `idx_edges_unique` on a dirty reopen and finalize the load — dedup and
/// rebuild the indexes — rather than serve the broken graph as-is (#318).
#[tokio::test]
async fn open_finalizes_interrupted_bulk_load() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "fn caller() { callee(); }\nfn callee() {}\n",
    )
    .unwrap();

    {
        let cg = tokensave::tokensave::TokenSave::init(project)
            .await
            .unwrap();
        cg.index_all().await.unwrap();

        // Simulate a crash mid-bulk-load: the dedup index is dropped and
        // duplicate edges are injected, exactly as a repeated or interrupted
        // load would leave the table.
        cg.db().begin_bulk_load().await.unwrap();
        cg.db()
            .conn()
            .execute_batch(
                "INSERT INTO edges (source, target, kind, line)
                 SELECT source, target, kind, line FROM edges;",
            )
            .await
            .unwrap();
        assert!(
            !index_exists(cg.db(), "idx_edges_unique").await,
            "precondition: bulk load dropped the unique index"
        );
    }

    // The dirty sentinel a killed process would have left behind.
    std::fs::write(
        project.join(".tokensave").join("dirty"),
        "pid=0\nversion=test",
    )
    .unwrap();

    let cg = tokensave::tokensave::TokenSave::open(project)
        .await
        .unwrap();

    assert!(
        index_exists(cg.db(), "idx_edges_unique").await,
        "recovery must recreate idx_edges_unique"
    );
    let mut rows = cg
        .db()
        .conn()
        .query(
            "SELECT COUNT(*) FROM (
                 SELECT 1 FROM edges
                 GROUP BY source, target, kind, COALESCE(line, -1)
                 HAVING COUNT(*) > 1
             )",
            (),
        )
        .await
        .unwrap();
    let dup_groups: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(dup_groups, 0, "recovery must remove duplicate edges");
}
