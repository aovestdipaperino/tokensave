use libsql::{Builder, Connection, Database as LibsqlDatabase};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokensave::db::migrations::{create_schema, latest_version, migrate};
use tokensave::db::Database;
use tokensave::errors::TokenSaveError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Creates a raw libsql database in a temp directory.
/// Returns (TempDir, Connection, Database) — all three must stay alive, and the
/// `TempDir` comes first so it is the last dropped: the connection must close
/// before the directory is removed, or Windows leaks it (#367).
async fn create_raw_db() -> (TempDir, Connection, LibsqlDatabase) {
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
    (dir, conn, db)
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

fn sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut path = db_path.as_os_str().to_os_string();
    path.push(suffix);
    path.into()
}

fn file_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("failed to read database bytes")
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

/// Checks whether a trigger exists in sqlite_master.
async fn trigger_exists(conn: &Connection, trigger_name: &str) -> bool {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='trigger' AND name=?1",
            libsql::params![trigger_name],
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

/// `create_schema` on a fresh database sets the latest version and creates all tables.
#[tokio::test]
async fn test_create_schema_fresh_db() {
    let (_dir, conn, _db) = create_raw_db().await;

    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    assert_eq!(get_user_version(&conn).await, latest_version());
    assert!(table_exists(&conn, "nodes").await);
    assert!(table_exists(&conn, "edges").await);
    assert!(table_exists(&conn, "files").await);
    assert!(table_exists(&conn, "unresolved_refs").await);
    assert!(table_exists(&conn, "vectors").await);
    assert!(table_exists(&conn, "metadata").await);
    assert!(table_exists(&conn, "nodes_fts").await);
    assert!(table_exists(&conn, "executable_body_fts").await);
}

/// create_schema is idempotent — calling it twice does not error.
#[tokio::test]
async fn test_create_schema_idempotent() {
    let (_dir, conn, _db) = create_raw_db().await;

    create_schema(&conn)
        .await
        .expect("first create_schema should succeed");
    create_schema(&conn)
        .await
        .expect("second create_schema should succeed");

    assert_eq!(get_user_version(&conn).await, latest_version());
}

/// migrate returns false when already at the latest version.
#[tokio::test]
async fn test_migrate_already_latest_returns_false() {
    let (_dir, conn, _db) = create_raw_db().await;

    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    let migrated = migrate(&conn).await.expect("migrate should succeed");

    assert!(
        !migrated,
        "migrate should return false when already at latest"
    );
    assert_eq!(get_user_version(&conn).await, latest_version());
}

/// migrate from v0 (completely empty database) applies all migrations to latest.
#[tokio::test]
async fn test_migrate_from_v0() {
    let (_dir, conn, _db) = create_raw_db().await;

    // user_version defaults to 0 on a fresh database
    assert_eq!(get_user_version(&conn).await, 0);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v0 should succeed");

    assert!(
        migrated,
        "migrate should return true when migrations were applied"
    );
    assert_eq!(get_user_version(&conn).await, latest_version());

    // All expected tables should exist
    assert!(table_exists(&conn, "nodes").await);
    assert!(table_exists(&conn, "edges").await);
    assert!(table_exists(&conn, "files").await);
    assert!(table_exists(&conn, "unresolved_refs").await);
    assert!(table_exists(&conn, "vectors").await);
    assert!(table_exists(&conn, "metadata").await);
    assert!(table_exists(&conn, "nodes_fts").await);
    assert!(table_exists(&conn, "executable_body_fts").await);

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
    let (_dir, conn, _db) = create_raw_db().await;
    create_v1_schema(&conn).await;

    assert_eq!(get_user_version(&conn).await, 1);
    assert!(!table_exists(&conn, "metadata").await);
    assert!(!column_exists(&conn, "nodes", "branches").await);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v1 should succeed");

    assert!(migrated);
    assert_eq!(get_user_version(&conn).await, latest_version());

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
    let (_dir, conn, _db) = create_raw_db().await;
    create_v1_schema(&conn).await;
    apply_v2(&conn).await;

    assert_eq!(get_user_version(&conn).await, 2);
    assert!(table_exists(&conn, "metadata").await);
    assert!(!column_exists(&conn, "nodes", "branches").await);

    let migrated = migrate(&conn)
        .await
        .expect("migrate from v2 should succeed");

    assert!(migrated);
    assert_eq!(get_user_version(&conn).await, latest_version());

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
    let (_dir, conn, _db) = create_raw_db().await;
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
    assert_eq!(get_user_version(&conn).await, latest_version());

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
    let (_dir, conn, _db) = create_raw_db().await;
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
    assert_eq!(get_user_version(&conn).await, latest_version());

    assert!(index_exists(&conn, "idx_edges_unique").await);
}

/// V5 migration actually deduplicates edge rows.
#[tokio::test]
async fn test_v5_deduplicates_edges() {
    let (_dir, conn, _db) = create_raw_db().await;
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
    let (_dir, conn, _db) = create_raw_db().await;

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
    assert_eq!(version as u32, latest_version());
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
    assert_eq!(version as u32, latest_version());
}

#[tokio::test]
async fn read_only_open_current_schema_queries_without_writes_or_byte_changes() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("read_only.db");
    let (db, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
    db.close();
    let before = file_bytes(&db_path);

    let db = Database::open_read_only(&db_path)
        .await
        .expect("read-only open should succeed");

    let mut rows = db
        .conn()
        .query("PRAGMA user_version", ())
        .await
        .expect("read-only query should succeed");
    let row = rows
        .next()
        .await
        .expect("failed to read result")
        .expect("query should return a row");
    assert_eq!(
        row.get::<i64>(0).expect("failed to read schema version") as u32,
        latest_version()
    );

    let write_error = db
        .conn()
        .execute(
            "INSERT INTO metadata (key, value) VALUES ('read_only', 'must fail')",
            (),
        )
        .await
        .expect_err("writes must fail on a read-only connection");
    assert!(
        write_error
            .to_string()
            .to_ascii_lowercase()
            .contains("readonly"),
        "unexpected write error: {write_error}"
    );

    db.close();
    assert_eq!(file_bytes(&db_path), before);
}

#[tokio::test]
async fn read_only_open_rejects_old_schema_without_migration_or_byte_changes() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("read_only_old.db");
    {
        let raw_db = Builder::new_local(&db_path)
            .build()
            .await
            .expect("failed to build libsql database");
        let conn = raw_db.connect().expect("failed to connect");
        create_v1_schema(&conn).await;
    }
    let before = file_bytes(&db_path);

    let error = Database::open_read_only(&db_path)
        .await
        .err()
        .expect("old schemas must be rejected");

    assert!(
        matches!(
            &error,
            TokenSaveError::Config { message } if message.contains("schema version")
        ),
        "unexpected schema error: {error}"
    );
    assert_eq!(file_bytes(&db_path), before);
}

#[tokio::test]
async fn read_only_open_rejects_newer_schema_as_config_error_without_byte_changes() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("read_only_newer.db");
    let (db, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
    db.conn()
        .execute(
            &format!("PRAGMA user_version = {}", latest_version() + 1),
            (),
        )
        .await
        .expect("failed to set future schema version");
    db.checkpoint()
        .await
        .expect("failed to checkpoint database");
    db.close();
    let before = file_bytes(&db_path);

    let error = Database::open_read_only(&db_path)
        .await
        .err()
        .expect("newer schemas must be rejected");

    assert!(
        matches!(
            &error,
            TokenSaveError::Config { message } if message.contains("schema version")
        ),
        "unexpected schema error: {error}"
    );
    assert_eq!(file_bytes(&db_path), before);
}

#[tokio::test]
async fn read_only_open_reads_active_non_empty_wal() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("read_only_live_wal.db");
    let raw_db = Builder::new_local(&db_path)
        .build()
        .await
        .expect("failed to build libsql database");
    let conn = raw_db.connect().expect("failed to connect");
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
        .await
        .expect("failed to configure WAL");
    create_schema(&conn)
        .await
        .expect("failed to create current schema");
    conn.execute(
        "INSERT INTO metadata (key, value) VALUES ('active_wal', 'visible')",
        (),
    )
    .await
    .expect("failed to write active WAL row");

    let wal_path = sidecar_path(&db_path, "-wal");
    assert!(
        std::fs::metadata(&wal_path)
            .expect("live WAL should exist")
            .len()
            > 0,
        "live WAL should be non-empty"
    );

    let db = Database::open_read_only(&db_path)
        .await
        .expect("read-only open should coordinate with an active WAL");
    let mut rows = db
        .conn()
        .query("SELECT value FROM metadata WHERE key = 'active_wal'", ())
        .await
        .expect("failed to query active WAL row");
    let row = rows
        .next()
        .await
        .expect("failed to read active WAL row")
        .expect("active WAL row should be visible");
    assert_eq!(
        row.get_str(0).expect("failed to read active WAL value"),
        "visible"
    );
}

#[tokio::test]
async fn read_only_open_remains_valid_while_writer_writes_and_checkpoints() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let db_path = dir.path().join("read_only_concurrent.db");
    let (initialized, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
    initialized.close();

    let writer_db = Builder::new_local(&db_path)
        .build()
        .await
        .expect("failed to build writer database");
    let writer = writer_db.connect().expect("failed to connect writer");
    writer
        .execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
        .await
        .expect("failed to configure writer WAL");

    let reader = Database::open_read_only(&db_path)
        .await
        .expect("failed to open reader");
    writer
        .execute(
            "INSERT INTO metadata (key, value) VALUES ('concurrent', 'visible')",
            (),
        )
        .await
        .expect("writer insert should succeed");
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .await
        .expect("writer checkpoint should succeed");

    let mut rows = reader
        .conn()
        .query("SELECT value FROM metadata WHERE key = 'concurrent'", ())
        .await
        .expect("reader should remain valid after checkpoint");
    let row = rows
        .next()
        .await
        .expect("failed to read concurrent row")
        .expect("reader should see the checkpointed write");
    assert_eq!(
        row.get_str(0).expect("failed to read concurrent value"),
        "visible"
    );
}

#[tokio::test]
async fn read_only_open_accepts_relative_database_path() {
    let cwd = std::env::current_dir().expect("failed to read current directory");
    let dir = TempDir::new_in(&cwd).expect("failed to create temp dir under current directory");
    let db_path = dir.path().join("relative.db");
    let relative_path = db_path
        .strip_prefix(&cwd)
        .expect("temporary database should be under current directory");
    let (db, _) = Database::initialize(&db_path)
        .await
        .expect("failed to initialize database");
    db.close();

    let db = Database::open_read_only(relative_path)
        .await
        .expect("read-only open should accept a relative path");
    assert_eq!(get_user_version(db.conn()).await, latest_version());
}

/// V13 repairs v12 databases missing the trait-dispatch cache table.
#[tokio::test]
async fn test_migrate_v13_repairs_missing_trait_dispatch_cache() {
    let (_dir, conn, _db) = create_raw_db().await;
    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    conn.execute("DROP TABLE trait_dispatch_callers", ())
        .await
        .expect("failed to remove trait dispatch cache table");
    set_user_version(&conn, 12).await;

    assert!(migrate(&conn).await.expect("v13 migration should succeed"));
    assert_eq!(get_user_version(&conn).await, latest_version());
    assert!(table_exists(&conn, "trait_dispatch_callers").await);
    assert!(index_exists(&conn, "idx_trait_dispatch_callers_concrete").await);
}

/// V14 removes phantom `annotates` edges that target an `annotation_usage`
/// node (the bug fixed alongside this migration), but leaves every
/// legitimate `annotates` edge — direct extractor-emitted and resolver-
/// resolved alike — untouched.
#[tokio::test]
async fn test_v14_removes_phantom_annotates_edges() {
    let (_dir, conn, _db) = create_raw_db().await;
    create_schema(&conn)
        .await
        .expect("create_schema should succeed");
    set_user_version(&conn, 13).await;

    let insert_node = |id: &str, kind: &str| {
        format!(
            "INSERT INTO nodes (id, kind, name, qualified_name, file_path, start_line, end_line, start_column, end_column, updated_at) \
             VALUES ('{id}', '{kind}', '{id}', '{id}', 'src/lib.rs', 1, 1, 0, 1, 1000)"
        )
    };

    // Two annotation_usage nodes in the same file, with a phantom
    // usage-to-usage `annotates` edge between them plus a phantom self-edge.
    conn.execute(&insert_node("usage1", "annotation_usage"), ())
        .await
        .expect("failed to insert usage1");
    conn.execute(&insert_node("usage2", "annotation_usage"), ())
        .await
        .expect("failed to insert usage2");

    // A legitimate direct edge: usage -> the item it annotates.
    conn.execute(&insert_node("fn1", "function"), ())
        .await
        .expect("failed to insert fn1");

    // The `@Retention @interface Foo {}` direct-edge case: a usage naming a
    // real Annotation declaration is a legitimate extractor-emitted edge, not
    // a resolver-produced usage-to-usage phantom.
    conn.execute(&insert_node("usage3", "annotation_usage"), ())
        .await
        .expect("failed to insert usage3");
    conn.execute(&insert_node("decl1", "annotation"), ())
        .await
        .expect("failed to insert decl1");

    // A phantom edge targeting a `decorator` node: decorator nodes are only
    // ever emitted at the application site, never the declaration, so a
    // usage -> decorator edge is the same phantom class as usage -> usage.
    conn.execute(&insert_node("usage4", "annotation_usage"), ())
        .await
        .expect("failed to insert usage4");
    conn.execute(&insert_node("dec1", "decorator"), ())
        .await
        .expect("failed to insert dec1");

    // A legitimate direct edge with a decorator as *source*: proves the
    // delete keys on target kind, not on `decorator` appearing anywhere in
    // the edge.
    conn.execute(&insert_node("fn2", "function"), ())
        .await
        .expect("failed to insert fn2");

    conn.execute(
        "INSERT INTO edges (source, target, kind, line) VALUES ('usage1', 'usage2', 'annotates', 1)",
        (),
    )
    .await
    .expect("failed to insert phantom cross-node edge");
    conn.execute(
        "INSERT INTO edges (source, target, kind, line) VALUES ('usage1', 'usage1', 'annotates', 1)",
        (),
    )
    .await
    .expect("failed to insert phantom self-edge");
    conn.execute(
        "INSERT INTO edges (source, target, kind, line) VALUES ('usage2', 'fn1', 'annotates', 1)",
        (),
    )
    .await
    .expect("failed to insert legitimate direct edge");
    conn.execute(
        "INSERT INTO edges (source, target, kind, line) VALUES ('usage3', 'decl1', 'annotates', 1)",
        (),
    )
    .await
    .expect("failed to insert direct annotation-usage-to-declaration edge");
    conn.execute(
        "INSERT INTO edges (source, target, kind, line) VALUES ('usage4', 'dec1', 'annotates', 1)",
        (),
    )
    .await
    .expect("failed to insert phantom usage-to-decorator edge");
    conn.execute(
        "INSERT INTO edges (source, target, kind, line) VALUES ('dec1', 'fn2', 'annotates', 1)",
        (),
    )
    .await
    .expect("failed to insert legitimate decorator-as-source edge");

    assert!(migrate(&conn).await.expect("v14 migration should succeed"));
    assert_eq!(get_user_version(&conn).await, latest_version());

    let mut rows = conn
        .query(
            "SELECT source, target FROM edges WHERE kind = 'annotates' ORDER BY source",
            (),
        )
        .await
        .expect("failed to query surviving annotates edges");
    let mut surviving = Vec::new();
    while let Some(row) = rows.next().await.expect("failed to read row") {
        let source: String = row.get(0).expect("failed to read source");
        let target: String = row.get(1).expect("failed to read target");
        surviving.push((source, target));
    }

    assert_eq!(
        surviving,
        vec![
            ("dec1".to_string(), "fn2".to_string()),
            ("usage2".to_string(), "fn1".to_string()),
            ("usage3".to_string(), "decl1".to_string()),
        ],
        "only edges not targeting an annotation_usage or decorator node should survive the v14 repair"
    );
}

/// After create_schema, all v5 columns on nodes exist.
#[tokio::test]
async fn test_create_schema_has_all_node_columns() {
    let (_dir, conn, _db) = create_raw_db().await;
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
    let (_dir, conn, _db) = create_raw_db().await;
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
    let (_dir, conn, _db) = create_raw_db().await;

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

#[tokio::test]
async fn test_v8_creates_memory_tables() {
    let (_dir, conn, _db) = create_raw_db().await;
    create_schema(&conn).await.unwrap();

    // memory_decisions table exists with expected columns
    let mut rows = conn
        .query(
            "SELECT name FROM pragma_table_info('memory_decisions') ORDER BY cid",
            (),
        )
        .await
        .unwrap();
    let mut cols = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        cols.push(row.get::<String>(0).unwrap());
    }
    assert_eq!(
        cols,
        vec!["id", "text", "reason", "created_at", "files", "tags"]
    );

    // memory_code_areas table exists
    let mut rows = conn
        .query(
            "SELECT name FROM pragma_table_info('memory_code_areas') ORDER BY cid",
            (),
        )
        .await
        .unwrap();
    let mut cols = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        cols.push(row.get::<String>(0).unwrap());
    }
    assert_eq!(
        cols,
        vec![
            "id",
            "path",
            "description",
            "last_touched_at",
            "touch_count"
        ]
    );

    // FTS table exists
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='memory_decisions_fts'",
            (),
        )
        .await
        .unwrap();
    assert!(
        rows.next().await.unwrap().is_some(),
        "memory_decisions_fts missing"
    );

    // All three FTS triggers exist
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='trigger' \
             AND name IN ('memory_decisions_fts_insert', 'memory_decisions_fts_delete', 'memory_decisions_fts_update') \
             ORDER BY name",
            (),
        )
        .await
        .unwrap();
    let mut trigger_names = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        trigger_names.push(row.get::<String>(0).unwrap());
    }
    assert_eq!(
        trigger_names,
        vec![
            "memory_decisions_fts_delete",
            "memory_decisions_fts_insert",
            "memory_decisions_fts_update",
        ]
    );
}

#[tokio::test]
async fn test_v7_to_latest_upgrade_path() {
    let (_dir, conn, _db) = create_raw_db().await;

    create_schema(&conn).await.unwrap();
    conn.execute("PRAGMA user_version = 7", ()).await.unwrap();
    // Drop the v8+ tables to simulate a true v7 starting state
    conn.execute("DROP TABLE IF EXISTS memory_decisions_fts", ())
        .await
        .unwrap();
    conn.execute("DROP TABLE IF EXISTS memory_decisions", ())
        .await
        .unwrap();
    conn.execute("DROP TABLE IF EXISTS memory_code_areas", ())
        .await
        .unwrap();
    conn.execute("DROP TABLE IF EXISTS read_cache", ())
        .await
        .unwrap();

    let did_migrate = migrate(&conn).await.unwrap();
    assert!(did_migrate, "expected migrate() to return true");

    let mut rows = conn.query("PRAGMA user_version", ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let v: i64 = row.get(0).unwrap();
    assert_eq!(v as u32, latest_version());

    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN \
             ('memory_decisions','memory_code_areas','memory_decisions_fts','read_cache') ORDER BY name",
            (),
        )
        .await
        .unwrap();
    let mut names = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        names.push(row.get::<String>(0).unwrap());
    }
    assert_eq!(
        names,
        vec![
            "memory_code_areas",
            "memory_decisions",
            "memory_decisions_fts",
            "read_cache",
        ]
    );
}

/// V9 adds the `read_cache` table used by `tokensave_read`.
#[tokio::test]
async fn test_migrate_v9_adds_read_cache() {
    let (_dir, conn, _db) = create_raw_db().await;
    migrate(&conn).await.expect("migrate should succeed");

    assert!(
        table_exists(&conn, "read_cache").await,
        "v9 migration should create the read_cache table"
    );
    assert!(
        index_exists(&conn, "idx_read_cache_session").await,
        "v9 migration should create idx_read_cache_session"
    );
}

/// V15 restores secondary indexes and FTS triggers dropped by an interrupted
/// bulk load, even when the dirty sentinel is absent (#358).
#[tokio::test]
async fn test_migrate_v15_restores_missing_indexes_and_triggers() {
    let (_dir, conn, _db) = create_raw_db().await;
    create_schema(&conn)
        .await
        .expect("create_schema should succeed");

    // Simulate an interrupted bulk load: drop all indexes and FTS triggers
    // exactly as `begin_bulk_load` does, leaving the DB unindexed.
    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_nodes_kind;
         DROP INDEX IF EXISTS idx_nodes_name;
         DROP INDEX IF EXISTS idx_nodes_qualified_name;
         DROP INDEX IF EXISTS idx_nodes_file_path;
         DROP INDEX IF EXISTS idx_nodes_file_path_start_line;
         DROP INDEX IF EXISTS idx_edges_source;
         DROP INDEX IF EXISTS idx_edges_target;
         DROP INDEX IF EXISTS idx_edges_kind;
         DROP INDEX IF EXISTS idx_edges_source_kind;
         DROP INDEX IF EXISTS idx_edges_target_kind;
         DROP INDEX IF EXISTS idx_edges_unique;
         DROP INDEX IF EXISTS idx_unresolved_refs_from_node_id;
         DROP INDEX IF EXISTS idx_unresolved_refs_reference_name;
         DROP INDEX IF EXISTS idx_unresolved_refs_file_path;
         DROP TRIGGER IF EXISTS nodes_fts_insert;
         DROP TRIGGER IF EXISTS nodes_fts_delete;
         DROP TRIGGER IF EXISTS nodes_fts_update;
         DROP TRIGGER IF EXISTS trait_dispatch_call_insert;
         DROP TRIGGER IF EXISTS trait_dispatch_implements_insert;
         DROP TRIGGER IF EXISTS trait_dispatch_call_delete;
         DROP TRIGGER IF EXISTS trait_dispatch_implements_delete;",
    )
    .await
    .expect("failed to simulate interrupted bulk load");
    set_user_version(&conn, 14).await;

    // Run migrations - v15 should recreate everything.
    assert!(migrate(&conn).await.expect("v15 migration should succeed"));
    assert_eq!(get_user_version(&conn).await, latest_version());

    // Node indexes
    assert!(index_exists(&conn, "idx_nodes_kind").await);
    assert!(index_exists(&conn, "idx_nodes_name").await);
    assert!(index_exists(&conn, "idx_nodes_qualified_name").await);
    assert!(index_exists(&conn, "idx_nodes_file_path").await);
    assert!(index_exists(&conn, "idx_nodes_file_path_start_line").await);

    // Edge indexes
    assert!(index_exists(&conn, "idx_edges_source_kind").await);
    assert!(index_exists(&conn, "idx_edges_target_kind").await);
    assert!(index_exists(&conn, "idx_edges_kind").await);
    assert!(index_exists(&conn, "idx_edges_unique").await);

    // Unresolved refs indexes
    assert!(index_exists(&conn, "idx_unresolved_refs_from_node_id").await);
    assert!(index_exists(&conn, "idx_unresolved_refs_reference_name").await);
    assert!(index_exists(&conn, "idx_unresolved_refs_file_path").await);

    // FTS triggers exist (the core heal - without these the FTS index
    // goes stale on every later sync).
    assert!(trigger_exists(&conn, "nodes_fts_insert").await);
    assert!(trigger_exists(&conn, "nodes_fts_delete").await);
    assert!(trigger_exists(&conn, "nodes_fts_update").await);

    // Trait-dispatch triggers (also dropped by begin_bulk_load).
    assert!(trigger_exists(&conn, "trait_dispatch_call_insert").await);
    assert!(trigger_exists(&conn, "trait_dispatch_implements_insert").await);
    assert!(trigger_exists(&conn, "trait_dispatch_call_delete").await);
    assert!(trigger_exists(&conn, "trait_dispatch_implements_delete").await);
}

/// V15 is idempotent - running it on an already-healthy DB is a no-op.
#[tokio::test]
async fn test_migrate_v15_idempotent_on_healthy_db() {
    let (_dir, conn, _db) = create_raw_db().await;
    create_schema(&conn)
        .await
        .expect("create_schema should succeed");
    set_user_version(&conn, 14).await;

    assert!(migrate(&conn).await.expect("migrate should succeed"));
    assert_eq!(get_user_version(&conn).await, latest_version());

    // All indexes still present - no error, no data loss.
    assert!(index_exists(&conn, "idx_edges_unique").await);
    assert!(index_exists(&conn, "idx_nodes_file_path").await);
    assert!(index_exists(&conn, "idx_unresolved_refs_from_node_id").await);
}

/// #359: `migrate_v15` must be a safe no-op on a partial schema. A DB whose
/// `nodes` table predates `parent_id` and that has no `edges` /
/// `unresolved_refs` / `nodes_fts` (an old or hand-rolled schema) must migrate
/// without erroring on `no such table: edges` or `no such column: parent_id`.
/// It still restores the node indexes it can, and never touches `parent_id`
/// (which bulk load does not drop) or the absent tables.
#[tokio::test]
async fn test_migrate_v15_partial_schema_does_not_error() {
    let (_dir, conn, _db) = create_raw_db().await;
    // A minimal, pre-`parent_id` nodes table and nothing else.
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
            updated_at INTEGER NOT NULL
        );",
    )
    .await
    .expect("create minimal nodes table");
    // Only v15 runs.
    set_user_version(&conn, 14).await;

    assert!(
        migrate(&conn)
            .await
            .expect("v15 must not error on a partial schema"),
        "a pending v15 migration should report as run"
    );
    assert_eq!(get_user_version(&conn).await, latest_version());

    // Restores the node indexes it can…
    assert!(index_exists(&conn, "idx_nodes_kind").await);
    assert!(index_exists(&conn, "idx_nodes_file_path_start_line").await);
    // …without touching parent_id (never dropped by bulk load) or the absent
    // edges table.
    assert!(!column_exists(&conn, "nodes", "parent_id").await);
    assert!(!index_exists(&conn, "idx_nodes_parent_id").await);
    assert!(!index_exists(&conn, "idx_edges_source_kind").await);
}
