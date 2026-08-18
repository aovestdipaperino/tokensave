// Rust guideline compliant 2025-10-17
//! Sequential schema migrations for the tokensave database.
//!
//! Each migration is a function that takes a connection and applies DDL
//! statements. Migrations run inside an EXCLUSIVE transaction so that
//! concurrent processes (e.g. a post-commit hook and an MCP server)
//! cannot corrupt the schema.
//!
//! The current schema version is stored in `PRAGMA user_version`, which
//! is an atomic integer built into `SQLite`. No extra table is needed.

use libsql::{params, Connection};

use crate::errors::{Result, TokenSaveError};

/// The highest migration version defined in this file. Bump this and add a
/// new entry to `run_migration` whenever the schema changes.
const LATEST_VERSION: u32 = 17;

/// Schema for the ambiguity record added in v17 (#412). Used by both
/// `migrate_v17` and the fresh-schema path, which never replays migrations.
const AMBIGUOUS_CALLS_SQL: &str = "CREATE TABLE IF NOT EXISTS ambiguous_calls (
    from_node_id TEXT NOT NULL,
    reference_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line INTEGER NOT NULL,
    candidate_node_ids TEXT NOT NULL,
    PRIMARY KEY (from_node_id, reference_name, line)
);
CREATE INDEX IF NOT EXISTS idx_ambiguous_calls_file ON ambiguous_calls(file_path);";

pub(crate) const TRAIT_DISPATCH_TRIGGERS_SQL: &str = r"
CREATE TRIGGER IF NOT EXISTS trait_dispatch_call_insert
AFTER INSERT ON edges WHEN NEW.kind = 'calls' BEGIN
    INSERT OR IGNORE INTO trait_dispatch_callers
        (concrete_method_id, trait_method_id, caller_id, line)
    SELECT concrete.id, trait_method.id, NEW.source, COALESCE(NEW.line, -1)
      FROM nodes trait_method
      JOIN edges dispatch
        ON dispatch.target = trait_method.parent_id
       AND dispatch.kind = 'implements'
      JOIN nodes concrete
        ON concrete.parent_id = dispatch.source
       AND concrete.name = trait_method.name
       AND concrete.kind IN ('method', 'function')
     WHERE trait_method.id = NEW.target
       AND trait_method.kind IN ('method', 'function');
END;

CREATE TRIGGER IF NOT EXISTS trait_dispatch_implements_insert
AFTER INSERT ON edges WHEN NEW.kind = 'implements' BEGIN
    INSERT OR IGNORE INTO trait_dispatch_callers
        (concrete_method_id, trait_method_id, caller_id, line)
    SELECT concrete.id, trait_method.id, call.source, COALESCE(call.line, -1)
      FROM nodes trait_method
      JOIN nodes concrete
        ON concrete.parent_id = NEW.source
       AND concrete.name = trait_method.name
       AND concrete.kind IN ('method', 'function')
      JOIN edges call
        ON call.target = trait_method.id
       AND call.kind = 'calls'
     WHERE trait_method.parent_id = NEW.target
       AND trait_method.kind IN ('method', 'function');
END;

CREATE TRIGGER IF NOT EXISTS trait_dispatch_call_delete
AFTER DELETE ON edges WHEN OLD.kind = 'calls' BEGIN
    DELETE FROM trait_dispatch_callers
     WHERE caller_id = OLD.source
       AND trait_method_id = OLD.target
       AND line = COALESCE(OLD.line, -1);
END;

CREATE TRIGGER IF NOT EXISTS trait_dispatch_implements_delete
AFTER DELETE ON edges WHEN OLD.kind = 'implements' BEGIN
    DELETE FROM trait_dispatch_callers
     WHERE concrete_method_id IN (SELECT id FROM nodes WHERE parent_id = OLD.source)
       AND trait_method_id IN (SELECT id FROM nodes WHERE parent_id = OLD.target);
END;
";

/// Returns the highest schema version this build knows how to produce.
#[must_use]
pub const fn latest_version() -> u32 {
    LATEST_VERSION
}

/// Reads the current schema version stored in the database's `PRAGMA user_version`.
///
/// # Errors
/// Returns an error if the `PRAGMA user_version` query fails.
pub async fn read_schema_version(conn: &Connection) -> Result<u32> {
    get_version(conn).await
}

/// Reads the current schema version from `PRAGMA user_version`.
async fn get_version(conn: &Connection) -> Result<u32> {
    let mut rows =
        conn.query("PRAGMA user_version", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read user_version: {e}"),
                operation: "get_version".to_string(),
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

/// Creates the complete latest schema from scratch for a brand-new database.
/// This avoids running v0→v1→…→v6 migrations sequentially.
pub async fn create_schema(conn: &Connection) -> Result<()> {
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
            branches INTEGER NOT NULL DEFAULT 0,
            loops INTEGER NOT NULL DEFAULT 0,
            returns INTEGER NOT NULL DEFAULT 0,
            max_nesting INTEGER NOT NULL DEFAULT 0,
            unsafe_blocks INTEGER NOT NULL DEFAULT 0,
            unchecked_calls INTEGER NOT NULL DEFAULT 0,
            assertions INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL,
            attrs_start_line INTEGER NOT NULL DEFAULT 0,
            parent_id TEXT,
            cognitive_complexity INTEGER NOT NULL DEFAULT 0,
            distinct_operators INTEGER NOT NULL DEFAULT 0,
            distinct_operands INTEGER NOT NULL DEFAULT 0,
            total_operators INTEGER NOT NULL DEFAULT 0,
            total_operands INTEGER NOT NULL DEFAULT 0,
            search_terms TEXT NOT NULL DEFAULT ''
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
            node_count INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'code'
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

        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            name, qualified_name, docstring, signature, search_terms,
            content='nodes', content_rowid='rowid',
            tokenize='porter unicode61'
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS executable_body_fts USING fts5(
            node_id UNINDEXED,
            file_path UNINDEXED,
            body,
            tokenize='unicode61'
        );

        CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );

        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
        CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);

        CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique
            ON edges(source, target, kind, COALESCE(line, -1));

        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
        CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);

        CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name));
        CREATE INDEX IF NOT EXISTS idx_nodes_parent_id ON nodes(parent_id);

        CREATE TABLE IF NOT EXISTS node_fingerprints (
            node_id TEXT PRIMARY KEY,
            ast_hash TEXT NOT NULL,
            cfg_hash TEXT NOT NULL,
            call_seq_hash TEXT NOT NULL,
            shingles TEXT NOT NULL,
            body_tokens INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_ast ON node_fingerprints(ast_hash);
        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_size ON node_fingerprints(body_tokens);

        CREATE TABLE IF NOT EXISTS memory_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reason TEXT,
            created_at INTEGER NOT NULL,
            files TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS memory_code_areas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            description TEXT,
            last_touched_at INTEGER NOT NULL,
            touch_count INTEGER NOT NULL DEFAULT 1
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_code_areas_path ON memory_code_areas(path);
        CREATE INDEX IF NOT EXISTS idx_memory_decisions_created_at ON memory_decisions(created_at);

        CREATE TABLE IF NOT EXISTS read_cache (
            project_id   TEXT NOT NULL,
            session_id   TEXT NOT NULL,
            file_path    TEXT NOT NULL,
            mtime_ns     INTEGER NOT NULL,
            mode         TEXT NOT NULL,
            args_hash    TEXT NOT NULL,
            digest       TEXT NOT NULL,
            body         BLOB NOT NULL,
            token_count  INTEGER NOT NULL,
            created_at   INTEGER NOT NULL,
            PRIMARY KEY (project_id, session_id, file_path, mode, args_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_read_cache_session
            ON read_cache(session_id, created_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_decisions_fts USING fts5(
            text, reason,
            content='memory_decisions', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_insert
            AFTER INSERT ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_delete
            AFTER DELETE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_update
            AFTER UPDATE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("failed to create schema: {e}"),
        operation: "create_schema".to_string(),
    })?;

    conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to create trait dispatch triggers: {e}"),
            operation: "create_schema".to_string(),
        })?;

    // A fresh database is stamped at LATEST_VERSION and never replays
    // migrations, so anything a migration adds has to be created here too.
    // Sharing one statement keeps the two definitions from drifting.
    conn.execute_batch(AMBIGUOUS_CALLS_SQL)
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to create ambiguous_calls: {e}"),
            operation: "create_schema".to_string(),
        })?;

    set_version(conn, LATEST_VERSION).await?;
    Ok(())
}

/// Runs all pending migrations up to `LATEST_VERSION`.
///
/// Acquires an EXCLUSIVE transaction to prevent concurrent writers from
/// interleaving schema changes. Each migration is applied and the version
/// is bumped inside the same transaction.
/// Returns `true` if any migrations were applied, `false` if already up-to-date.
pub async fn migrate(conn: &Connection) -> Result<bool> {
    let current = get_version(conn).await?;
    debug_assert!(
        current <= LATEST_VERSION,
        "database version {current} is ahead of code version {LATEST_VERSION}"
    );
    if current >= LATEST_VERSION {
        return Ok(false);
    }

    eprintln!("[tokensave] migrating database schema v{current} → v{LATEST_VERSION}…");

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
    debug_assert!(
        current < LATEST_VERSION,
        "run_migrations called when already at latest version"
    );
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
        5 => migrate_v5(conn).await,
        6 => migrate_v6(conn).await,
        7 => migrate_v7(conn).await,
        8 => migrate_v8(conn).await,
        9 => migrate_v9(conn).await,
        10 => migrate_v10(conn).await,
        11 => migrate_v11(conn).await,
        12 => migrate_v12(conn).await,
        13 => migrate_v13(conn).await,
        14 => migrate_v14(conn).await,
        15 => migrate_v15(conn).await,
        16 => migrate_v16(conn).await,
        17 => migrate_v17(conn).await,
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
            node_count INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'code'
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

/// Adds branches, loops, returns, and `max_nesting` columns to the nodes table.
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

/// Adds `unsafe_blocks`, `unchecked_calls`, and assertions columns to the nodes table.
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

// ---------------------------------------------------------------------------
// Migration V5: deduplicate edges and add UNIQUE index
// ---------------------------------------------------------------------------

/// Removes duplicate edges accumulated by repeated reference resolution
/// during incremental syncs, then adds a UNIQUE index to prevent future
/// duplicates. See: <https://github.com/…/issues/5>
async fn migrate_v5(conn: &Connection) -> Result<()> {
    // Rebuild the edges table keeping only distinct rows. We use a temp
    // table + swap because DELETE with a self-join subquery can be very
    // slow on large tables (the reporter had 13.9 M edges).
    conn.execute_batch(
        "CREATE TABLE edges_dedup (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );

        INSERT INTO edges_dedup (source, target, kind, line)
        SELECT DISTINCT source, target, kind, line FROM edges;

        DROP TABLE edges;
        ALTER TABLE edges_dedup RENAME TO edges;

        CREATE INDEX idx_edges_source ON edges(source);
        CREATE INDEX idx_edges_target ON edges(target);
        CREATE INDEX idx_edges_kind ON edges(kind);
        CREATE INDEX idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX idx_edges_target_kind ON edges(target, kind);
        CREATE UNIQUE INDEX idx_edges_unique
            ON edges(source, target, kind, COALESCE(line, -1));",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v5: failed to deduplicate edges: {e}"),
        operation: "migrate_v5".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V6: expression index on lower(name) for case-insensitive lookups
// ---------------------------------------------------------------------------

/// Adds an expression index on `lower(name)` so that case-insensitive queries
/// and LIKE fallbacks avoid full table scans on large codebases.
async fn migrate_v6(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name))",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v6: failed to create lower(name) index: {e}"),
        operation: "migrate_v6".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V7: attrs_start_line column for full-span item lookups
// ---------------------------------------------------------------------------

/// Adds `attrs_start_line` to the nodes table. This column captures the first
/// line of an item's leading doc-comment / attribute block, so that consumers
/// (refactoring tools, code movers) can select an item's full span including
/// its documentation rather than guessing where the leading attrs start.
///
/// Existing rows are backfilled with `start_line` so behaviour is preserved
/// for nodes indexed before this migration.
async fn migrate_v7(conn: &Connection) -> Result<()> {
    conn.execute(
        "ALTER TABLE nodes ADD COLUMN attrs_start_line INTEGER NOT NULL DEFAULT 0",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v7: failed to add attrs_start_line column: {e}"),
        operation: "migrate_v7".to_string(),
    })?;

    conn.execute(
        "UPDATE nodes SET attrs_start_line = start_line WHERE attrs_start_line = 0",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v7: failed to backfill attrs_start_line: {e}"),
        operation: "migrate_v7".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V8: cross-session memory tables (decisions, code areas)
// ---------------------------------------------------------------------------

/// Adds tables for persistent agent memory: `memory_decisions` records
/// architecture / design choices with optional reason and tags;
/// `memory_code_areas` tracks paths the agent has worked in. An FTS5 mirror
/// over `memory_decisions.text` and `memory_decisions.reason` enables
/// fuzzy recall via `tokensave_session_recall`.
async fn migrate_v8(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reason TEXT,
            created_at INTEGER NOT NULL,
            files TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS memory_code_areas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            description TEXT,
            last_touched_at INTEGER NOT NULL,
            touch_count INTEGER NOT NULL DEFAULT 1
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_code_areas_path
            ON memory_code_areas(path);
        CREATE INDEX IF NOT EXISTS idx_memory_decisions_created_at
            ON memory_decisions(created_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_decisions_fts USING fts5(
            text, reason,
            content='memory_decisions', content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_insert
            AFTER INSERT ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_delete
            AFTER DELETE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
            END;

        CREATE TRIGGER IF NOT EXISTS memory_decisions_fts_update
            AFTER UPDATE ON memory_decisions BEGIN
                INSERT INTO memory_decisions_fts(memory_decisions_fts, rowid, text, reason)
                VALUES ('delete', OLD.id, OLD.text, OLD.reason);
                INSERT INTO memory_decisions_fts(rowid, text, reason)
                VALUES (NEW.id, NEW.text, NEW.reason);
            END;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v8: failed to create memory tables: {e}"),
        operation: "migrate_v8".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V9: read cache + parent_id denormalization
// ---------------------------------------------------------------------------

/// Two changes:
///
/// 1. Creates the `read_cache` table used by `tokensave_read` to serve
///    unchanged files as a tiny stub across sessions.
/// 2. Denormalizes `Contains` edges onto a new `nodes.parent_id` column.
///    The column is backfilled from existing `Contains` rows, then those
///    rows are deleted. After v9, the truth for "who contains node X" is
///    `nodes.parent_id`, not the edges table — readers should prefer it.
async fn migrate_v9(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS read_cache (
            project_id   TEXT NOT NULL,
            session_id   TEXT NOT NULL,
            file_path    TEXT NOT NULL,
            mtime_ns     INTEGER NOT NULL,
            mode         TEXT NOT NULL,
            args_hash    TEXT NOT NULL,
            digest       TEXT NOT NULL,
            body         BLOB NOT NULL,
            token_count  INTEGER NOT NULL,
            created_at   INTEGER NOT NULL,
            PRIMARY KEY (project_id, session_id, file_path, mode, args_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_read_cache_session
            ON read_cache(session_id, created_at);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v9: failed to create read_cache table: {e}"),
        operation: "migrate_v9".to_string(),
    })?;

    // ALTER TABLE has no IF NOT EXISTS for columns in SQLite. Probe
    // PRAGMA table_info first — fresh installs already include parent_id
    // from create_schema, and the test harness exercises that path by
    // resetting user_version to a pre-v9 value.
    let has_parent_id = {
        let mut rows = conn
            .query("PRAGMA table_info(nodes)", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to probe nodes columns: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
        let mut found = false;
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("v9: failed to read table_info row: {e}"),
            operation: "migrate_v9".to_string(),
        })? {
            if let Ok(name) = row.get::<String>(1) {
                if name == "parent_id" {
                    found = true;
                    break;
                }
            }
        }
        found
    };

    if !has_parent_id {
        conn.execute("ALTER TABLE nodes ADD COLUMN parent_id TEXT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to add parent_id column: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
    }

    // Backfill parent_id from existing Contains edges, then drop those
    // rows. Gate on the edges table actually existing — tests seed
    // partial schemas and a real install always has it (migrate_v1).
    let has_edges_table = {
        let mut rows = conn
            .query(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='edges'",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to probe sqlite_master: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
        rows.next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to read sqlite_master row: {e}"),
                operation: "migrate_v9".to_string(),
            })?
            .is_some()
    };

    if has_edges_table {
        // When a node has multiple incoming Contains rows (legacy data
        // anomaly), the first matching row wins — subsequent rows are
        // noise the new schema does not preserve.
        conn.execute(
            "UPDATE nodes SET parent_id = (
                SELECT source FROM edges
                WHERE edges.target = nodes.id AND edges.kind = 'contains'
                LIMIT 1
            )",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v9: failed to backfill parent_id from contains edges: {e}"),
            operation: "migrate_v9".to_string(),
        })?;

        conn.execute("DELETE FROM edges WHERE kind = 'contains'", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v9: failed to drop contains edges: {e}"),
                operation: "migrate_v9".to_string(),
            })?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_nodes_parent_id ON nodes(parent_id)",
        (),
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v9: failed to create idx_nodes_parent_id: {e}"),
        operation: "migrate_v9".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V10: node_fingerprints (issue #83 — tokensave_redundancy)
// ---------------------------------------------------------------------------

/// Creates the `node_fingerprints` table used by `tokensave_redundancy` to
/// detect AST-isomorphic, control-flow-equivalent, and token-similar
/// function/method duplicates. Populated lazily on first redundancy query
/// and invalidated by `source_hash` mismatch.
async fn migrate_v10(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS node_fingerprints (
            node_id TEXT PRIMARY KEY,
            ast_hash TEXT NOT NULL,
            cfg_hash TEXT NOT NULL,
            call_seq_hash TEXT NOT NULL,
            shingles TEXT NOT NULL,
            body_tokens INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_ast ON node_fingerprints(ast_hash);
        CREATE INDEX IF NOT EXISTS idx_node_fingerprints_size ON node_fingerprints(body_tokens);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v10: failed to create node_fingerprints table: {e}"),
        operation: "migrate_v10".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V11: code-health metric columns (issue #150)
// ---------------------------------------------------------------------------

/// Adds the issue #150 code-health columns to the nodes table:
/// `cognitive_complexity` plus the raw Halstead token counts
/// (`distinct_operators`, `distinct_operands`, `total_operators`,
/// `total_operands`). Derived metrics (Halstead volume/difficulty/effort and
/// the maintainability index) are computed on demand in the complexity handler
/// and are not stored.
///
/// Existing rows default to 0; they are repopulated on the next re-index.
///
/// `SQLite` has no `ADD COLUMN IF NOT EXISTS`, so each column is added only
/// when absent. This keeps the migration idempotent — important because the
/// v7→latest upgrade path may re-run v11 against a schema that `create_schema`
/// already provisioned with these columns.
async fn migrate_v11(conn: &Connection) -> Result<()> {
    let existing = node_columns(conn).await?;
    for col in [
        "cognitive_complexity",
        "distinct_operators",
        "distinct_operands",
        "total_operators",
        "total_operands",
    ] {
        if existing.iter().any(|c| c == col) {
            continue;
        }
        conn.execute(
            &format!("ALTER TABLE nodes ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0"),
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v11: failed to add column {col}: {e}"),
            operation: "migrate_v11".to_string(),
        })?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V12: persistent executable-body search
// ---------------------------------------------------------------------------

/// Adds persistent indexes used by conceptual context retrieval and resolved
/// reverse trait dispatch.
/// Existing projects are fully re-indexed by `TokenSave::open` after this
/// schema migration, which populates the table from source exactly once.
async fn migrate_v12(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS executable_body_fts USING fts5(
            node_id UNINDEXED,
            file_path UNINDEXED,
            body,
            tokenize='unicode61'
        );
        CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );
        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v12: failed to create executable body FTS table: {e}"),
        operation: "migrate_v12".to_string(),
    })?;
    let mut edge_table = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v12: failed to inspect edges table: {e}"),
            operation: "migrate_v12".to_string(),
        })?;
    let has_edges = edge_table
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v12: failed to read edges table status: {e}"),
            operation: "migrate_v12".to_string(),
        })?
        .is_some();
    drop(edge_table);
    if has_edges {
        conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v12: failed to create trait dispatch triggers: {e}"),
                operation: "migrate_v12".to_string(),
            })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V13: repair incomplete trait-dispatch cache schema
// ---------------------------------------------------------------------------

/// Recreates the trait-dispatch cache objects for databases that recorded V12
/// before all of its DDL was durable.  The statements are idempotent so this
/// also leaves correctly migrated V12 databases unchanged.
async fn migrate_v13(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trait_dispatch_callers (
            concrete_method_id TEXT NOT NULL,
            trait_method_id TEXT NOT NULL,
            caller_id TEXT NOT NULL,
            line INTEGER NOT NULL DEFAULT -1,
            PRIMARY KEY (concrete_method_id, trait_method_id, caller_id, line)
        );
        CREATE INDEX IF NOT EXISTS idx_trait_dispatch_callers_concrete
            ON trait_dispatch_callers(concrete_method_id);",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v13: failed to recreate trait dispatch cache table: {e}"),
        operation: "migrate_v13".to_string(),
    })?;

    let mut edge_table = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v13: failed to inspect edges table: {e}"),
            operation: "migrate_v13".to_string(),
        })?;
    let has_edges = edge_table
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v13: failed to read edges table status: {e}"),
            operation: "migrate_v13".to_string(),
        })?
        .is_some();
    drop(edge_table);
    if has_edges {
        conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v13: failed to recreate trait dispatch triggers: {e}"),
                operation: "migrate_v13".to_string(),
            })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V14: two independent one-shot repairs bundled into one version.
//   1. Remove phantom annotation-usage-to-annotation-usage `annotates` edges.
//   2. Add the `search_terms` column and rebuild `nodes_fts` with porter
//      stemming so inflected prose queries match indexed identifiers.
// The two operations touch disjoint tables (`edges` vs `nodes`/`nodes_fts`)
// and are ordered edges-first, FTS-second; neither depends on the other.
// ---------------------------------------------------------------------------

/// One-shot data repair for the resolver bug fixed alongside this migration:
/// `kind_compatible` used to accept `NodeKind::AnnotationUsage` and
/// `NodeKind::Decorator` as valid targets for `EdgeKind::Annotates`, which let
/// an annotation reference bind to a sibling usage in the same file instead of
/// staying unresolved. The resolver now never produces `annotates` edges at
/// all (`kind_compatible` returns `false` for the whole edge kind); the only
/// `annotates` edges left are the ones extractors emit directly.
///
/// The delete targets both `annotation_usage` and `decorator` nodes:
/// - `annotation_usage` is always a usage site, never a legitimate target of
///   an `annotates` edge.
/// - `decorator` is emitted at the `@foo(...)` application site (Python,
///   TypeScript), never at the declaration it decorates, so it can never be a
///   legitimate target either — only ever a source, including for stacked
///   decorators (`@a @b def f`).
///
/// This does *not* widen to `annotation` nodes: `@Retention @interface Foo {}`
/// is a legitimate direct edge to a `NodeKind::Annotation` node, so deleting
/// by that target kind would remove real data. Any stale resolver-produced
/// edges that happened to target an `annotation` node are cleared by the full
/// reindex `TokenSave::open` already forces on any migration.
///
/// The second half is a retrieval graft from codebase-memory — it adds the
/// `search_terms` column (camelCase word segments computed at write time),
/// rebuilds `nodes_fts` with that column and the `porter unicode61` tokenizer
/// so inflected prose queries ("ranking", "candidates") match indexed
/// identifiers, and backfills existing rows.
///
/// New databases (and any file re-synced with the fixed resolver) never
/// produce the phantom edges in the first place; that half of this migration
/// only repairs rows already stored in databases indexed before the fix.
///
/// The edge repair is gated on the `edges` table actually existing — some
/// pre-v9 migration tests seed a nodes-only schema and drive `migrate` all the
/// way to `LATEST_VERSION`, and a real install always has the table by v1.
async fn migrate_v14(conn: &Connection) -> Result<()> {
    // --- Part 1: remove phantom `annotates` edges (see #326). -------------
    let mut edge_table = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'edges'",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to inspect edges table: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    let has_edges = edge_table
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read edges table status: {e}"),
            operation: "migrate_v14".to_string(),
        })?
        .is_some();
    drop(edge_table);

    if has_edges {
        conn.execute(
            "DELETE FROM edges
             WHERE kind = 'annotates'
               AND target IN (
                   SELECT id FROM nodes WHERE kind IN ('annotation_usage', 'decorator')
               )",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to delete phantom annotates edges: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    }

    // --- Part 2: search_terms column + porter FTS rebuild (see #316). -----
    let cols = node_columns(conn).await?;
    if !cols.iter().any(|c| c == "search_terms") {
        conn.execute_batch("ALTER TABLE nodes ADD COLUMN search_terms TEXT NOT NULL DEFAULT ''")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v14: failed to add search_terms column: {e}"),
                operation: "migrate_v14".to_string(),
            })?;
    }

    // Drop the old FTS table and its triggers before the backfill so the
    // UPDATEs below don't fire FTS sync triggers against a table that is
    // about to be rebuilt anyway.
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS nodes_fts_insert;
         DROP TRIGGER IF EXISTS nodes_fts_delete;
         DROP TRIGGER IF EXISTS nodes_fts_update;
         DROP TABLE IF EXISTS nodes_fts;",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v14: failed to drop old FTS table: {e}"),
        operation: "migrate_v14".to_string(),
    })?;

    // Backfill search_terms for existing nodes. Only rows that actually have
    // camelCase-derived segments need a write; snake_case names produce an
    // empty string, which the column already defaults to.
    let mut rows = conn
        .query("SELECT rowid, name, qualified_name FROM nodes", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read nodes for backfill: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    let mut backfill: Vec<(i64, String)> = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("v14: failed to read backfill row: {e}"),
        operation: "migrate_v14".to_string(),
    })? {
        let rowid: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read rowid: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
        let name: String = row.get(1).map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read name: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
        let qualified_name: String = row.get(2).map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to read qualified_name: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
        let terms = crate::text::search_terms(&name, &qualified_name);
        if !terms.is_empty() {
            backfill.push((rowid, terms));
        }
    }
    drop(rows);
    for (rowid, terms) in backfill {
        conn.execute(
            "UPDATE nodes SET search_terms = ?1 WHERE rowid = ?2",
            params![terms, rowid],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v14: failed to backfill search_terms: {e}"),
            operation: "migrate_v14".to_string(),
        })?;
    }

    // Recreate the FTS table with the new column + porter stemming, its sync
    // triggers, and rebuild the index from the content table.
    conn.execute_batch(
        "CREATE VIRTUAL TABLE nodes_fts USING fts5(
            name, qualified_name, docstring, signature, search_terms,
            content='nodes', content_rowid='rowid',
            tokenize='porter unicode61'
        );

        CREATE TRIGGER nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild');",
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("v14: failed to recreate FTS table: {e}"),
        operation: "migrate_v14".to_string(),
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Migration V15: restore indexes and FTS triggers lost by an interrupted
// bulk load (#358).  `begin_bulk_load` drops all secondary indexes and FTS
// triggers; `end_bulk_load` recreates them — but only if it runs.  If the
// process is killed mid-`index_all` and the `.tokensave/dirty` sentinel is
// absent, the #318 healing path in `TokenSave::open` never triggers, and
// no earlier migration checks for or restores the missing objects.  This
// migration recreates every index and FTS trigger with `IF NOT EXISTS`, so
// it is instant on a healthy DB and repairs a damaged one.
//
// It also fixes a second root cause: `end_bulk_load` recreates the pre-v14
// FTS triggers (without the `search_terms` column) after every `index_all`
// on a v14 DB, silently downgrading the FTS sync.  This migration drops
// and recreates the triggers with the v14-era bodies, and `end_bulk_load`
// is updated in the same change to match.
// ---------------------------------------------------------------------------

/// Returns `true` if a table named `name` exists in the current database.
///
/// Repair migrations use this to stay no-ops on a partial schema: a DB that
/// predates a given table, or a minimal fixture, must not make an
/// unconditional `CREATE INDEX … ON <table>` fail with "no such table".
async fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to probe sqlite_master for table '{name}': {e}"),
            operation: "table_exists".to_string(),
        })?;
    let exists = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read sqlite_master row for table '{name}': {e}"),
            operation: "table_exists".to_string(),
        })?
        .is_some();
    Ok(exists)
}

/// Recreates any missing secondary indexes and FTS triggers that an
/// interrupted `begin_bulk_load` may have dropped and no prior migration
/// restored.  Idempotent: `CREATE … IF NOT EXISTS` is a no-op when the
/// object already exists.
///
/// Each object group is gated on its base table existing. On a healthy DB
/// (the #358 repair target) every table is present and this changes nothing;
/// the guards keep the migration a safe no-op on a partial schema — a DB
/// migrated up from a version that predates a table, or a minimal test
/// fixture — instead of erroring with "no such table: edges" (#359).
async fn migrate_v15(conn: &Connection) -> Result<()> {
    // --- Node indexes --------------------------------------------------
    // Exactly the set `begin_bulk_load` drops and `end_bulk_load` restores —
    // NOT the full `create_schema` set. `idx_nodes_lower_name` and
    // `idx_nodes_parent_id` are deliberately omitted: bulk load never drops
    // them, so they are never lost, and `idx_nodes_parent_id` would reference a
    // `parent_id` column that only exists from v10 onward (#359).
    if table_exists(conn, "nodes").await? {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
            CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
            CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
            CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
            CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);",
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate node indexes: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- Edge indexes --------------------------------------------------
    if table_exists(conn, "edges").await? {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
            CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
            CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique
                ON edges(source, target, kind, COALESCE(line, -1));",
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate edge indexes: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- Unresolved-ref indexes ----------------------------------------
    if table_exists(conn, "unresolved_refs").await? {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
            CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
            CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);",
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate unresolved_refs indexes: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- FTS sync triggers (v14-era bodies with `search_terms`) ---------
    // `end_bulk_load` may have recreated the pre-v14 triggers that omit
    // `search_terms`.  Drop and recreate to ensure the current bodies. The
    // triggers fire on `nodes` and write to `nodes_fts`, so both must exist;
    // by this point `migrate_v14` has already given both the `search_terms`
    // column.
    if table_exists(conn, "nodes").await? && table_exists(conn, "nodes_fts").await? {
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS nodes_fts_insert;
        DROP TRIGGER IF EXISTS nodes_fts_delete;
        DROP TRIGGER IF EXISTS nodes_fts_update;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES ('delete', OLD.rowid, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature, OLD.search_terms);
            INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature, search_terms)
            VALUES (NEW.rowid, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature, NEW.search_terms);
        END;",
    )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v15: failed to recreate FTS triggers: {e}"),
            operation: "migrate_v15".to_string(),
        })?;
    }

    // --- Trait-dispatch triggers ---------------------------------------
    // Fire on `edges` and write to `trait_dispatch_callers`; both must exist.
    if table_exists(conn, "edges").await? && table_exists(conn, "trait_dispatch_callers").await? {
        conn.execute_batch(TRAIT_DISPATCH_TRIGGERS_SQL)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("v15: failed to recreate trait dispatch triggers: {e}"),
                operation: "migrate_v15".to_string(),
            })?;
    }

    Ok(())
}

/// Returns the column names of the `nodes` table via `PRAGMA table_info`.
async fn node_columns(conn: &Connection) -> Result<Vec<String>> {
    let mut rows = conn
        .query("PRAGMA table_info(nodes)", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read nodes table_info: {e}"),
            operation: "node_columns".to_string(),
        })?;
    let mut cols = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read table_info row: {e}"),
        operation: "node_columns".to_string(),
    })? {
        // PRAGMA table_info columns: cid(0), name(1), type(2), ...
        let name: String = row.get(1).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read column name: {e}"),
            operation: "node_columns".to_string(),
        })?;
        cols.push(name);
    }
    Ok(cols)
}

// ---------------------------------------------------------------------------
// Migration V16: file kind column
// ---------------------------------------------------------------------------

/// Adds the `kind` column to `files`, distinguishing source from artifacts.
///
/// Tracking non-code artifacts (#323) needs a way to tell a source file that
/// happens to yield no symbols from a file that was never parsed at all, so
/// analyses meaning "code" can exclude the latter. Existing rows are all
/// source, which the column default already states.
///
/// `migrate_v1` creates `files` with the column, so a database migrated all
/// the way up from v0 arrives here already holding it; the probe keeps that
/// path a no-op rather than failing on a duplicate column.
async fn migrate_v16(conn: &Connection) -> Result<()> {
    if !table_exists(conn, "files").await? {
        return Ok(());
    }
    if table_columns(conn, "files")
        .await?
        .iter()
        .any(|c| c == "kind")
    {
        return Ok(());
    }

    conn.execute_batch("ALTER TABLE files ADD COLUMN kind TEXT NOT NULL DEFAULT 'code';")
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v16: failed to add files.kind: {e}"),
            operation: "migrate_v16".to_string(),
        })?;
    Ok(())
}

/// v17: records calls the resolver could not disambiguate (#412).
///
/// When several candidates tie, no edge is created — an edge is an assertion
/// and "one of these" is not one — but the alternatives are worth keeping. A
/// model reading the source can pick the right one; a scoring heuristic that
/// cannot see the receiver's type cannot.
///
/// Keyed by call site rather than by target, because the ambiguity belongs to
/// the call: the same method name called from two places may be resolvable at
/// one and not the other. `candidate_node_ids` is a JSON array; the set is
/// small by construction (a handful of same-named methods) and is never
/// joined against, only read back whole.
async fn migrate_v17(conn: &Connection) -> Result<()> {
    conn.execute_batch(AMBIGUOUS_CALLS_SQL)
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("v17: failed to create ambiguous_calls: {e}"),
            operation: "migrate_v17".to_string(),
        })?;
    Ok(())
}

/// Returns the column names of `table` via `PRAGMA table_info`.
async fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    // PRAGMA does not accept a bound parameter for the table name, and this is
    // only ever called with literals from this module.
    let mut rows = conn
        .query(&format!("PRAGMA table_info({table})"), ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read {table} table_info: {e}"),
            operation: "table_columns".to_string(),
        })?;
    let mut cols = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read table_info row: {e}"),
        operation: "table_columns".to_string(),
    })? {
        // PRAGMA table_info columns: cid(0), name(1), type(2), ...
        let name: String = row.get(1).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read column name: {e}"),
            operation: "table_columns".to_string(),
        })?;
        cols.push(name);
    }
    Ok(cols)
}
