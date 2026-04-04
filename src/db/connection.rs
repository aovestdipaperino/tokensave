// Rust guideline compliant 2025-10-17
use std::path::Path;

use libsql::{Builder, Connection, Database as LibsqlDatabase};

use crate::errors::{TokenSaveError, Result};

use super::migrations;

/// SQLite database backing the code graph, powered by libsql.
pub struct Database {
    conn: Connection,
    /// Kept alive so the underlying database is not dropped.
    _db: LibsqlDatabase,
}

impl Database {
    /// Creates a new database at `db_path`, creating parent directories if needed.
    ///
    /// Opens a libsql connection, applies performance pragmas, and runs all
    /// schema migrations up to the latest version.
    /// Returns `(Self, migrated)` where `migrated` is `true` if schema
    /// migrations were applied during initialization.
    pub async fn initialize(db_path: &Path) -> Result<(Self, bool)> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TokenSaveError::Database {
                message: format!("failed to create database directory: {e}"),
                operation: "initialize".to_string(),
            })?;
        }

        let db = Builder::new_local(db_path)
            .build()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to open database: {e}"),
                operation: "initialize".to_string(),
            })?;

        let conn = db.connect().map_err(|e| TokenSaveError::Database {
            message: format!("failed to connect to database: {e}"),
            operation: "initialize".to_string(),
        })?;

        Self::apply_pragmas(&conn).await?;
        let migrated = migrations::migrate(&conn).await?;

        Ok((Self { conn, _db: db }, migrated))
    }

    /// Opens an existing database at `db_path`, applies performance pragmas,
    /// and runs any pending schema migrations.
    /// Returns `(Self, migrated)` where `migrated` is `true` if schema
    /// migrations were applied during open.
    pub async fn open(db_path: &Path) -> Result<(Self, bool)> {
        let db = Builder::new_local(db_path)
            .build()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to open database: {e}"),
                operation: "open".to_string(),
            })?;

        let conn = db.connect().map_err(|e| TokenSaveError::Database {
            message: format!("failed to connect to database: {e}"),
            operation: "open".to_string(),
        })?;

        Self::apply_pragmas(&conn).await?;
        let migrated = migrations::migrate(&conn).await?;

        Ok((Self { conn, _db: db }, migrated))
    }

    /// Returns a reference to the underlying libsql connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Consumes the `Database`, closing the underlying connection.
    pub fn close(self) {
        drop(self.conn);
    }

    /// Checkpoints the WAL back into the main database file.
    ///
    /// This ensures all committed transactions are merged into the main DB
    /// before the process exits, preventing a stale WAL file on next startup.
    pub async fn checkpoint(&self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to checkpoint WAL: {e}"),
                operation: "checkpoint".to_string(),
            })?;
        Ok(())
    }

    /// Runs VACUUM and ANALYZE to reclaim space and update query planner statistics.
    pub async fn optimize(&self) -> Result<()> {
        self.conn
            .execute_batch("VACUUM; ANALYZE;")
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to optimize database: {e}"),
                operation: "optimize".to_string(),
            })?;
        Ok(())
    }

    /// Returns the on-disk size of the database file in bytes.
    pub async fn size(&self) -> Result<u64> {
        let mut rows = self
            .conn
            .query(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to get database size: {e}"),
                operation: "size".to_string(),
            })?;

        let row = rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read database size row: {e}"),
                operation: "size".to_string(),
            })?
            .ok_or_else(|| TokenSaveError::Database {
                message: "no result from page size query".to_string(),
                operation: "size".to_string(),
            })?;

        let size = row.get::<i64>(0).map_err(|e| TokenSaveError::Database {
            message: format!("failed to read size value: {e}"),
            operation: "size".to_string(),
        })?;

        Ok(size as u64)
    }

    /// Applies performance-oriented SQLite pragmas.
    async fn apply_pragmas(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "PRAGMA page_size = 8192;
             PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 120000;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -65536;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 268435456;",
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to apply pragmas: {e}"),
            operation: "apply_pragmas".to_string(),
        })?;
        Ok(())
    }

    /// Drops secondary indexes, disables fsync/FK, and clears FTS for fast
    /// bulk loading. Callers should insert data sorted by PK so the primary
    /// B-tree gets sequential appends. Call `end_bulk_load` afterwards to
    /// rebuild indexes in one optimized pass.
    pub async fn begin_bulk_load(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA synchronous = OFF;
             PRAGMA foreign_keys = OFF;
             DROP INDEX IF EXISTS idx_nodes_kind;
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
             DELETE FROM nodes_fts;",
        ).await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to begin bulk load: {e}"),
            operation: "begin_bulk_load".to_string(),
        })?;
        Ok(())
    }

    /// Recreates secondary indexes (benefiting from sorted row order),
    /// restores FTS triggers and content, and re-enables normal durability.
    pub async fn end_bulk_load(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
             CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
             CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
             CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
             CREATE INDEX IF NOT EXISTS idx_nodes_file_path_start_line ON nodes(file_path, start_line);
             CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
             CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);
             CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique ON edges(source, target, kind, COALESCE(line, -1));
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_from_node_id ON unresolved_refs(from_node_id);
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_reference_name ON unresolved_refs(reference_name);
             CREATE INDEX IF NOT EXISTS idx_unresolved_refs_file_path ON unresolved_refs(file_path);
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
             INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
                 SELECT rowid, name, qualified_name, docstring, signature FROM nodes;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        ).await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to end bulk load: {e}"),
            operation: "end_bulk_load".to_string(),
        })?;
        Ok(())
    }
}
