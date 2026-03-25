// Rust guideline compliant 2025-10-17
//! Sequential schema migrations for the tokensave database.
//!
//! Each migration is a function that takes a connection and applies DDL
//! statements. Migrations run inside an EXCLUSIVE transaction so that
//! concurrent processes (e.g. a post-commit hook and an MCP server)
//! cannot corrupt the schema.
//!
//! The current schema version is stored in `PRAGMA user_version`, which
//! is an atomic integer built into SQLite. No extra table is needed.

use libsql::Connection;

use crate::errors::{TokenSaveError, Result};

/// The highest migration version defined in this file. Bump this and add a
/// new entry to `run_migration` whenever the schema changes.
const LATEST_VERSION: u32 = 4;

/// Reads the current schema version from `PRAGMA user_version`.
async fn get_version(conn: &Connection) -> Result<u32> {
    let mut rows = conn.query("PRAGMA user_version", ()).await.map_err(|e| {
        TokenSaveError::Database {
            message: format!("failed to read user_version: {e}"),
            operation: "get_version".to_string(),
        }
    })?;
    let row = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read user_version row: {e}"),
        operation: "get_version".to_string(),
    })?;
    match row {
        Some(r) => {
            let v: i64 = r.get(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read user_version value: {e}"),
                operation: "get_version".to_string(),
            })?;
            Ok(v as u32)
        }
        None => Ok(0),
    }
}

/// Sets the schema version via `PRAGMA user_version`.
///
/// PRAGMA statements cannot be parameterised, so we format the value
/// directly. This is safe because `version` is a u32.
async fn set_version(conn: &Connection, version: u32) -> Result<()> {
    conn.execute(&format!("PRAGMA user_version = {version}"), ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to set user_version: {e}"),
            operation: "set_version".to_string(),
        })?;
    Ok(())
}

/// Runs all pending migrations up to `LATEST_VERSION`.
///
/// Acquires an EXCLUSIVE transaction to prevent concurrent writers from
/// interleaving schema changes. Each migration is applied and the version
/// is bumped inside the same transaction.
/// Returns `true` if any migrations were applied, `false` if already up-to-date.
pub async fn migrate(conn: &Connection) -> Result<bool> {
    debug_assert!(LATEST_VERSION > 0, "LATEST_VERSION must be positive");
    let current = get_version(conn).await?;
    debug_assert!(current <= LATEST_VERSION, "database version {} is ahead of code version {}", current, LATEST_VERSION);
    if current >= LATEST_VERSION {
        return Ok(false);
    }

    eprintln!(
        "[tokensave] migrating database schema v{current} → v{LATEST_VERSION}…"
    );

    // BEGIN EXCLUSIVE blocks other writers (including other MCP servers or
    // post-commit hooks) until we COMMIT. Readers using WAL mode are not
    // blocked.
    conn.execute("BEGIN EXCLUSIVE", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to acquire exclusive lock: {e}"),
            operation: "migrate".to_string(),
        })?;

    // Re-read inside the lock in case another process migrated between our
    // check and the lock acquisition.
    let current = get_version(conn).await?;

    let result = run_migrations(conn, current).await;

    match result {
        Ok(()) => {
            conn.execute("COMMIT", ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to commit migrations: {e}"),
                    operation: "migrate".to_string(),
                })?;
            Ok(true)
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", ()).await;
            Err(e)
        }
    }
}

/// Applies migrations sequentially from `current` up to `LATEST_VERSION`.
async fn run_migrations(conn: &Connection, current: u32) -> Result<()> {
    debug_assert!(current < LATEST_VERSION, "run_migrations called when already at latest version");
    for version in (current + 1)..=LATEST_VERSION {
        run_migration(conn, version).await?;
        set_version(conn, version).await?;
    }
    Ok(())
}

/// Dispatches a single migration by version number.
async fn run_migration(conn: &Connection, version: u32) -> Result<()> {
    match version {
        1 => migrate_v1(conn).await,
        2 => migrate_v2(conn).await,
        3 => migrate_v3(conn).await,
        4 => migrate_v4(conn).await,
        _ => Err(TokenSaveError::Database {
            message: format!("unknown migration version: {version}"),
            operation: "run_migration".to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Migration V1: initial schema
// ---------------------------------------------------------------------------

/// Creates all core tables, FTS index, triggers, and indexes.
async fn migrate_v1(conn: &Connection) -> Result<()> {
    // Tables
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
        );",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v1: failed to create tables: {e}"),
        operation: "migrate_v1".to_string(),
    })?;

    // FTS5
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name,
            qualified_name,
            docstring,
            signature,
            content='nodes',
            content_rowid='rowid'
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
        END;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v1: failed to create FTS: {e}"),
        operation: "migrate_v1".to_string(),
    })?;

    // Indexes
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
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
    .map_err(|e| TokenSaveError::Database {
        message: format!("v1: failed to create indexes: {e}"),
        operation: "migrate_v1".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V2: metadata table
// ---------------------------------------------------------------------------

/// Adds the key-value metadata table for persistent counters.
async fn migrate_v2(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v2: failed to create metadata table: {e}"),
        operation: "migrate_v2".to_string(),
    })?;

    // Drop the legacy schema_versions table if it exists.
    conn.execute("DROP TABLE IF EXISTS schema_versions", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v2: failed to drop schema_versions: {e}"),
            operation: "migrate_v2".to_string(),
        })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V3: complexity metric columns on nodes
// ---------------------------------------------------------------------------

/// Adds branches, loops, returns, and max_nesting columns to the nodes table.
async fn migrate_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN branches INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN loops INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN returns INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN max_nesting INTEGER NOT NULL DEFAULT 0;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v3: failed to add complexity columns: {e}"),
        operation: "migrate_v3".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V4: unsafe_blocks, unchecked_calls, assertions columns on nodes
// ---------------------------------------------------------------------------

/// Adds unsafe_blocks, unchecked_calls, and assertions columns to the nodes table.
async fn migrate_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN unsafe_blocks INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN unchecked_calls INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE nodes ADD COLUMN assertions INTEGER NOT NULL DEFAULT 0;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v4: failed to add safety metric columns: {e}"),
        operation: "migrate_v4".to_string(),
    })?;

    Ok(())
}
