use libsql::{Builder, Connection, Database as LibsqlDatabase};
use tempfile::TempDir;
use tokensave::db::migrations::{create_schema, migrate};
use tokensave::db::Database;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Creates a raw libsql database in a temp directory.
/// Returns (Connection, Database, TempDir) — all three must stay alive.
async fn create_raw_db() -> (Connection, LibsqlDatabase, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let db = Builder::new_local(&db_path)
        .build()
        .await
        .expect("failed to build libsql database");
    let conn = db.connect().expect("failed to connect");
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .await
    .expect("failed to apply pragmas");
    (conn, db, dir)
}

/// Sets PRAGMA user_version on the connection.
async fn set_user_version(conn: &Connection, version: u32) {
    conn.execute(&format!("PRAGMA user_version = {version}"), ())
        .await
        .expect("failed to set user_version");
}

/// Reads PRAGMA user_version from the connection.
async fn get_user_version(conn: &Connection) -> u32 {
    let mut rows = conn
        .query("PRAGMA user_version", ())
        .await
        .expect("failed to query user_version");
    let row = rows
        .next()
        .await
        .expect("failed to read user_version row")
        .expect("user_version should return a row");
    let v: i64 = row.get(0).expect("failed to read user_version value");
    v as u32
}

/// Checks whether a table exists in sqlite_master.
async fn table_exists(conn: &Connection, table_name: &str) -> bool {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?1",
            libsql::params![table_name],
        )
        .await
        .expect("failed to query sqlite_master");
    rows.next()
        .await
        .expect("failed to read sqlite_master row")
        .is_some()
}

/// Checks whether an index exists in sqlite_master.
async fn index_exists(conn: &Connection, index_name: &str) -> bool {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='index' AND name=?1",
            libsql::params![index_name],
        )
        .await
        .expect("failed to query sqlite_master");
    rows.next()
        .await
        .expect("failed to read sqlite_master row")
        .is_some()
}

/// Checks whether a column exists on a table via PRAGMA table_info.
async fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let mut rows = conn
        .query(&format!("PRAGMA table_info({table})"), ())
        .await
        .expect("failed to query table_info");
    while let Some(row) = rows.next().await.expect("failed to read table_info row") {
        let name: String = row
            .get_str(1)
            .expect("failed to read column name")
            .to_string();
        if name == column {
            return true;
        }
    }
    false
}

/// Creates the V1 schema (tables, FTS, indexes — no metadata, no complexity columns).
async fn create_v1_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
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
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL,
            node_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS unresolved_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_node_id TEXT NOT NULL,
            reference_name TEXT NOT NULL,
            reference_kind TEXT NOT NULL,
            line INTEGER NOT NULL,
            col INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS vectors (
            node_id TEXT PRIMARY KEY,
            embedding BLOB NOT NULL,
            model TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name, qualified_name, docstring, signature,
            content='nodes', content_rowid='rowid'
        );

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;

        CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
        CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);
        CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
        CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
        CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
        CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);",
    )
    .await
    .expect("failed to create v1 schema");
    set_user_version(conn, 1).await;
}

/// Applies the V2 additions on top of V1 (metadata table).
async fn apply_v2(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .await
    .expect("failed to apply v2");
    set_user_version(conn, 2).await;
}

/// Applies the V3 additions on top of V2 (complexity columns).
async fn apply_v3(conn: &Connection) {
    conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN branches INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN loops INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN returns INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN max_nesting INTEGER NOT NULL DEFAULT 0;",
    )
    .await
    .expect("failed to apply v3");
    set_user_version(conn, 3).await;
}

/// Applies the V4 additions on top of V3 (safety metric columns).
async fn apply_v4(conn: &Connection) {
    conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN unsafe_blocks INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN unchecked_calls INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN assertions INTEGER NOT NULL DEFAULT 0;",
    )
    .await
    .expect("failed to apply v4");
    set_user_version(conn, 4).await;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// create_schema on a fresh database sets user_version to 5 and creates all tables.
#[tokio::test]
async fn test_create_schema_fresh_db() {
    let (conn, _db, _dir) = create_raw_db().await;

    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    assert_eq!(get_user_version(&conn).await, 7);
    assert!(table_exists(&conn, "nodes").await);
    assert!(table_exists(&conn, "edges").await);
    assert!(table_exists(&conn, "files").await);
    assert!(table_exists(&conn, "unresolved_refs").await);
    assert!(table_exists(&conn, "vectors").await);
    assert!(table_exists(&conn, "metadata").await);
    assert!(table_exists(&conn, "nodes_fts").await);
}

/// create_schema is idempotent — calling it twice does not error.
#[tokio::test]
async fn test_create_schema_idempotent() {
    let (conn, _db, _dir) = create_raw_db().await;

    create_schema(&conn)
        .await
        .expect("first create_schema should succeed");
    create_schema(&conn)
        .await
        .expect("second create_schema should succeed");

    assert_eq!(get_user_version(&conn).await, 7);
}

/// migrate returns false when already at the latest version.
#[tokio::test]
async fn test_migrate_already_latest_returns_false() {
    let (conn, _db, _dir) = create_raw_db().await;

    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    let migrated = migrate(&conn).await.expect("migrate should succeed");

    assert!(
        !migrated,
        "migrate should return false when already at latest"
    );
    assert_eq!(get_user_version(&conn).await, 7);
}

/// migrate from v0 (completely empty database) applies all migrations to latest.
#[tokio::test]
async fn test_migrate_from_v0() {
    let (conn, _db, _dir) = create_raw_db().await;

    // user_version defaults to 0 on a fresh database
    assert_eq!(get_user_version(&conn).await, 0);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v0 should succeed");

    assert!(
        migrated,
        "migrate should return true when migrations were applied"
    );
    assert_eq!(get_user_version(&conn).await, 7);

    // All expected tables should exist
    assert!(table_exists(&conn, "nodes").await);
    assert!(table_exists(&conn, "edges").await);
    assert!(table_exists(&conn, "files").await);
    assert!(table_exists(&conn, "unresolved_refs").await);
    assert!(table_exists(&conn, "vectors").await);
    assert!(table_exists(&conn, "metadata").await);
    assert!(table_exists(&conn, "nodes_fts").await);

    // V3 complexity columns should exist
    assert!(column_exists(&conn, "nodes", "branches").await);
    assert!(column_exists(&conn, "nodes", "loops").await);
    assert!(column_exists(&conn, "nodes", "returns").await);
    assert!(column_exists(&conn, "nodes", "max_nesting").await);

    // V4 safety columns should exist
    assert!(column_exists(&conn, "nodes", "unsafe_blocks").await);
    assert!(column_exists(&conn, "nodes", "unchecked_calls").await);
    assert!(column_exists(&conn, "nodes", "assertions").await);

    // V5 unique index should exist
    assert!(index_exists(&conn, "idx_edges_unique").await);
}

/// migrate from v1 (tables exist, no metadata, no complexity columns) to v5.
#[tokio::test]
async fn test_migrate_from_v1() {
    let (conn, _db, _dir) = create_raw_db().await;
    create_v1_schema(&conn).await;

    assert_eq!(get_user_version(&conn).await, 1);
    assert!(!table_exists(&conn, "metadata").await);
    assert!(!column_exists(&conn, "nodes", "branches").await);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v1 should succeed");

    assert!(migrated);
    assert_eq!(get_user_version(&conn).await, 7);

    // V2: metadata table
    assert!(table_exists(&conn, "metadata").await);

    // V3: complexity columns
    assert!(column_exists(&conn, "nodes", "branches").await);
    assert!(column_exists(&conn, "nodes", "loops").await);
    assert!(column_exists(&conn, "nodes", "returns").await);
    assert!(column_exists(&conn, "nodes", "max_nesting").await);

    // V4: safety columns
    assert!(column_exists(&conn, "nodes", "unsafe_blocks").await);
    assert!(column_exists(&conn, "nodes", "unchecked_calls").await);
    assert!(column_exists(&conn, "nodes", "assertions").await);

    // V5: unique index
    assert!(index_exists(&conn, "idx_edges_unique").await);
}

/// migrate from v2 (has metadata, no complexity columns) to v5.
#[tokio::test]
async fn test_migrate_from_v2() {
    let (conn, _db, _dir) = create_raw_db().await;
    create_v1_schema(&conn).await;
    apply_v2(&conn).await;

    assert_eq!(get_user_version(&conn).await, 2);
    assert!(table_exists(&conn, "metadata").await);
    assert!(!column_exists(&conn, "nodes", "branches").await);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v2 should succeed");

    assert!(migrated);
    assert_eq!(get_user_version(&conn).await, 7);

    // V3 columns
    assert!(column_exists(&conn, "nodes", "branches").await);
    assert!(column_exists(&conn, "nodes", "max_nesting").await);

    // V4 columns
    assert!(column_exists(&conn, "nodes", "unsafe_blocks").await);

    // V5 unique index
    assert!(index_exists(&conn, "idx_edges_unique").await);
}

/// migrate from v3 (has complexity columns, no safety columns) to v5.
#[tokio::test]
async fn test_migrate_from_v3() {
    let (conn, _db, _dir) = create_raw_db().await;
    create_v1_schema(&conn).await;
    apply_v2(&conn).await;
    apply_v3(&conn).await;

    assert_eq!(get_user_version(&conn).await, 3);
    assert!(column_exists(&conn, "nodes", "branches").await);
    assert!(!column_exists(&conn, "nodes", "unsafe_blocks").await);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v3 should succeed");

    assert!(migrated);
    assert_eq!(get_user_version(&conn).await, 7);

    // V4 columns
    assert!(column_exists(&conn, "nodes", "unsafe_blocks").await);
    assert!(column_exists(&conn, "nodes", "unchecked_calls").await);
    assert!(column_exists(&conn, "nodes", "assertions").await);

    // V5 unique index
    assert!(index_exists(&conn, "idx_edges_unique").await);
}

/// migrate from v4 (has all columns, no edge dedup) to v5.
#[tokio::test]
async fn test_migrate_from_v4() {
    let (conn, _db, _dir) = create_raw_db().await;
    create_v1_schema(&conn).await;
    apply_v2(&conn).await;
    apply_v3(&conn).await;
    apply_v4(&conn).await;

    assert_eq!(get_user_version(&conn).await, 4);
    assert!(!index_exists(&conn, "idx_edges_unique").await);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v4 should succeed");

    assert!(migrated);
    assert_eq!(get_user_version(&conn).await, 7);

    assert!(index_exists(&conn, "idx_edges_unique").await);
}

/// V5 migration actually deduplicates edge rows.
#[tokio::test]
async fn test_v5_deduplicates_edges() {
    let (conn, _db, _dir) = create_raw_db().await;
    create_v1_schema(&conn).await;
    apply_v2(&conn).await;
    apply_v3(&conn).await;
    apply_v4(&conn).await;

    // Insert a node so foreign keys are satisfied
    conn.execute(
        "INSERT INTO nodes (id, kind, name, qualified_name, file_path, start_line, end_line, start_column, end_column, visibility, updated_at, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions) VALUES ('n1', 'function', 'foo', 'crate::foo', 'src/lib.rs', 1, 10, 0, 1, 'pub', 1000, 0, 0, 0, 0, 0, 0, 0)",
        (),
    )
    .await
    .expect("failed to insert node n1");

    conn.execute(
        "INSERT INTO nodes (id, kind, name, qualified_name, file_path, start_line, end_line, start_column, end_column, visibility, updated_at, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions) VALUES ('n2', 'function', 'bar', 'crate::bar', 'src/lib.rs', 11, 20, 0, 1, 'pub', 1000, 0, 0, 0, 0, 0, 0, 0)",
        (),
    )
    .await
    .expect("failed to insert node n2");

    // Insert duplicate edges (same source, target, kind, line)
    for _ in 0..5 {
        conn.execute(
            "INSERT INTO edges (source, target, kind, line) VALUES ('n1', 'n2', 'calls', 5)",
            (),
        )
        .await
        .expect("failed to insert duplicate edge");
    }

    // Also insert an edge with NULL line (duplicated)
    for _ in 0..3 {
        conn.execute(
            "INSERT INTO edges (source, target, kind, line) VALUES ('n1', 'n2', 'uses', NULL)",
            (),
        )
        .await
        .expect("failed to insert duplicate NULL-line edge");
    }

    // Verify duplicates exist before migration
    {
        let mut rows = conn
            .query("SELECT COUNT(*) FROM edges", ())
            .await
            .expect("failed to count edges");
        let row = rows
            .next()
            .await
            .expect("failed to read row")
            .expect("should have row");
        let count_before: i64 = row.get(0).expect("failed to read count");
        assert_eq!(
            count_before, 8,
            "should have 8 rows (5 + 3 duplicates) before migration"
        );
    }

    // Run migration (v4 -> v5)
    let migrated = migrate(&conn)
        .await
        .expect("migrate from v4 should succeed");
    assert!(migrated);

    // After dedup, should have exactly 2 distinct edges
    let mut rows = conn
        .query("SELECT COUNT(*) FROM edges", ())
        .await
        .expect("failed to count edges after migration");
    let row = rows
        .next()
        .await
        .expect("failed to read row")
        .expect("should have row");
    let count_after: i64 = row.get(0).expect("failed to read count");
    assert_eq!(
        count_after, 2,
        "v5 migration should deduplicate to 2 distinct edges"
    );
}

/// After full migration from v0, all expected indexes exist.
#[tokio::test]
async fn test_indexes_exist_after_full_migration() {
    let (conn, _db, _dir) = create_raw_db().await;

    migrate(&conn)
        .await
        .expect("migrate from v0 should succeed");

    // Node indexes
    assert!(index_exists(&conn, "idx_nodes_kind").await);
    assert!(index_exists(&conn, "idx_nodes_name").await);
    assert!(index_exists(&conn, "idx_nodes_qualified_name").await);
    assert!(index_exists(&conn, "idx_nodes_file_path").await);
    assert!(index_exists(&conn, "idx_nodes_file_path_start_line").await);

    // Edge indexes
    assert!(index_exists(&conn, "idx_edges_source").await);
    assert!(index_exists(&conn, "idx_edges_target").await);
    assert!(index_exists(&conn, "idx_edges_kind").await);
    assert!(index_exists(&conn, "idx_edges_source_kind").await);
    assert!(index_exists(&conn, "idx_edges_target_kind").await);
    assert!(index_exists(&conn, "idx_edges_unique").await);

    // Unresolved refs indexes
    assert!(index_exists(&conn, "idx_unresolved_refs_from_node_id").await);
    assert!(index_exists(&conn, "idx_unresolved_refs_reference_name").await);
    assert!(index_exists(&conn, "idx_unresolved_refs_file_path").await);
}

/// Database::initialize creates a database at the latest schema version.
#[tokio::test]
async fn test_database_initialize_creates_latest_version() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("init_test.db");

    let (db, _migrated) = Database::initialize(&db_path)
        .await
        .expect("Database::initialize should succeed");

    // Query user_version through the public conn
    let mut rows = db
        .conn()
        .query("PRAGMA user_version", ())
        .await
        .expect("failed to query user_version");
    let row = rows
        .next()
        .await
        .expect("failed to read row")
        .expect("should have row");
    let version: i64 = row.get(0).expect("failed to read version");
    assert_eq!(version, 7);
}

/// Database::open on an already-current database does not re-migrate.
#[tokio::test]
async fn test_database_open_no_migration_needed() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("open_test.db");

    // Initialize creates a database at the latest schema version
    let (db, _) = Database::initialize(&db_path)
        .await
        .expect("Database::initialize should succeed");
    db.close();

    // Open the same database — should not migrate
    let (_db2, migrated) = Database::open(&db_path)
        .await
        .expect("Database::open should succeed");

    assert!(
        !migrated,
        "opening an already-current database should not trigger migration"
    );
}

/// Database::open on a v1 database migrates to the latest schema version.
#[tokio::test]
async fn test_database_open_migrates_v1_to_latest() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("open_v1_test.db");

    // Create a raw v1 database
    {
        let raw_db = Builder::new_local(&db_path)
            .build()
            .await
            .expect("failed to build libsql database");
        let conn = raw_db.connect().expect("failed to connect");
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )
        .await
        .expect("failed to apply pragmas");
        create_v1_schema(&conn).await;
    }

    // Open via Database::open — should detect v1 and migrate to latest
    let (db, migrated) = Database::open(&db_path)
        .await
        .expect("Database::open should succeed");

    assert!(migrated, "opening a v1 database should trigger migration");

    // Verify the schema is now at latest
    let mut rows = db
        .conn()
        .query("PRAGMA user_version", ())
        .await
        .expect("failed to query user_version");
    let row = rows
        .next()
        .await
        .expect("failed to read row")
        .expect("should have row");
    let version: i64 = row.get(0).expect("failed to read version");
    assert_eq!(version, 7);
}

/// After create_schema, all v5 columns on nodes exist.
#[tokio::test]
async fn test_create_schema_has_all_node_columns() {
    let (conn, _db, _dir) = create_raw_db().await;
    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    let expected_columns = [
        "id",
        "kind",
        "name",
        "qualified_name",
        "file_path",
        "start_line",
        "end_line",
        "start_column",
        "end_column",
        "docstring",
        "signature",
        "visibility",
        "is_async",
        "branches",
        "loops",
        "returns",
        "max_nesting",
        "unsafe_blocks",
        "unchecked_calls",
        "assertions",
        "updated_at",
        "attrs_start_line",
    ];
    for col in &expected_columns {
        assert!(
            column_exists(&conn, "nodes", col).await,
            "nodes table should have column '{col}' after create_schema"
        );
    }
}

/// V5 unique index prevents duplicate edge insertion.
#[tokio::test]
async fn test_v5_unique_index_prevents_duplicates() {
    let (conn, _db, _dir) = create_raw_db().await;
    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    // Insert nodes for FK
    conn.execute(
        "INSERT INTO nodes (id, kind, name, qualified_name, file_path, start_line, end_line, start_column, end_column, visibility, updated_at, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions) VALUES ('a', 'function', 'a', 'crate::a', 'src/lib.rs', 1, 5, 0, 1, 'pub', 1000, 0, 0, 0, 0, 0, 0, 0)",
        (),
    )
    .await
    .expect("failed to insert node a");

    conn.execute(
        "INSERT INTO nodes (id, kind, name, qualified_name, file_path, start_line, end_line, start_column, end_column, visibility, updated_at, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions) VALUES ('b', 'function', 'b', 'crate::b', 'src/lib.rs', 6, 10, 0, 1, 'pub', 1000, 0, 0, 0, 0, 0, 0, 0)",
        (),
    )
    .await
    .expect("failed to insert node b");

    // First edge insertion should succeed
    conn.execute(
        "INSERT INTO edges (source, target, kind, line) VALUES ('a', 'b', 'calls', 3)",
        (),
    )
    .await
    .expect("first edge insert should succeed");

    // Duplicate insertion should fail due to unique index
    let result = conn
        .execute(
            "INSERT INTO edges (source, target, kind, line) VALUES ('a', 'b', 'calls', 3)",
            (),
        )
        .await;

    assert!(
        result.is_err(),
        "inserting a duplicate edge should fail with the v5 unique index"
    );
}

/// FTS triggers exist after migration from v0.
#[tokio::test]
async fn test_fts_triggers_exist_after_migration() {
    let (conn, _db, _dir) = create_raw_db().await;

    migrate(&conn)
        .await
        .expect("migrate from v0 should succeed");

    let triggers = ["nodes_fts_insert", "nodes_fts_delete", "nodes_fts_update"];
    for trigger in &triggers {
        let mut rows = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='trigger' AND name=?1",
                libsql::params![*trigger],
            )
            .await
            .expect("failed to query sqlite_master for trigger");
        assert!(
            rows.next()
                .await
                .expect("failed to read trigger row")
                .is_some(),
            "trigger '{trigger}' should exist after migration"
        );
    }
}
