// Rust guideline compliant 2025-10-17
use std::path::Path;

use libsql::{Builder, Connection, Database as LibsqlDatabase};

use crate::errors::{TokenSaveError, Result};

use super::migrations;

/// Computes adaptive `(cache_size_kb, mmap_size)` based on the DB file size.
///
/// - **cache_size**: 25% of DB size, clamped to \[2 MB, 64 MB\] (in KiB).
/// - **mmap_size**: 2× DB size, clamped to \[0, 256 MB\].
///
/// This avoids the fixed 320 MB memory baseline for small/medium projects.
pub(crate) fn adaptive_cache_sizes(db_file_size: u64) -> (u64, u64) {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;

    // cache_size: 25% of DB, clamped [2 MB .. 64 MB], expressed in KiB
    let cache_bytes = (db_file_size / 4).clamp(2 * MB, 64 * MB);
    let cache_kb = cache_bytes / KB;

    // mmap_size: 2× DB, clamped [0 .. 256 MB]
    let mmap = db_file_size.saturating_mul(2).min(256 * MB);

    (cache_kb, mmap)
}

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

        Self::apply_pragmas(&conn, 0).await?;
        migrations::create_schema(&conn).await?;

        Ok((Self { conn, _db: db }, false))
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

        let file_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
        Self::apply_pragmas(&conn, file_size).await?;
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

    /// Runs `PRAGMA quick_check` and returns `true` if the database is intact.
    ///
    /// This is faster than `integrity_check` — it verifies B-tree structure
    /// without cross-checking index contents against table data.
    pub async fn quick_check(&self) -> Result<bool> {
        let mut rows = self
            .conn
            .query("PRAGMA quick_check", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to run quick_check: {e}"),
                operation: "quick_check".to_string(),
            })?;

        if let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read quick_check result: {e}"),
            operation: "quick_check".to_string(),
        })? {
            let result: String = row.get::<String>(0).unwrap_or_default();
            Ok(result == "ok")
        } else {
            Ok(false)
        }
    }

    /// Rebuilds the FTS5 index from the content table.
    ///
    /// This fixes FTS-only corruption (e.g. from an interrupted bulk load)
    /// without requiring a full re-index of the codebase.
    pub async fn rebuild_fts(&self) -> Result<()> {
        self.conn
            .execute("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to rebuild FTS index: {e}"),
                operation: "rebuild_fts".to_string(),
            })?;
        Ok(())
    }

    /// Applies performance-oriented SQLite pragmas.
    ///
    /// `cache_size` and `mmap_size` are scaled to the on-disk DB size so
    /// small projects don't pay the 320 MB baseline of a large project.
    async fn apply_pragmas(conn: &Connection, db_file_size: u64) -> Result<()> {
        let (cache_kb, mmap) = adaptive_cache_sizes(db_file_size);
        conn.execute_batch(&format!(
            "PRAGMA page_size = 8192;
             PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 120000;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -{cache_kb};
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = {mmap};",
        ))
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
            "PRAGMA foreign_keys = OFF;
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
             PRAGMA foreign_keys = ON;",
        ).await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to end bulk load: {e}"),
            operation: "end_bulk_load".to_string(),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;

    #[test]
    fn adaptive_new_db_gets_minimum() {
        let (cache_kb, mmap) = adaptive_cache_sizes(0);
        assert_eq!(cache_kb, 2 * MB / KB); // 2 MB in KiB = 2048
        assert_eq!(mmap, 0);
    }

    #[test]
    fn adaptive_small_db() {
        // 5 MB DB → cache = 2 MB (floor), mmap = 10 MB
        let (cache_kb, mmap) = adaptive_cache_sizes(5 * MB);
        assert_eq!(cache_kb, 2 * MB / KB);
        assert_eq!(mmap, 10 * MB);
    }

    #[test]
    fn adaptive_medium_db() {
        // 100 MB DB → cache = 25 MB, mmap = 200 MB
        let (cache_kb, mmap) = adaptive_cache_sizes(100 * MB);
        assert_eq!(cache_kb, 25 * MB / KB);
        assert_eq!(mmap, 200 * MB);
    }

    #[test]
    fn adaptive_large_db() {
        // 500 MB DB → cache = 64 MB (cap), mmap = 256 MB (cap)
        let (cache_kb, mmap) = adaptive_cache_sizes(500 * MB);
        assert_eq!(cache_kb, 64 * MB / KB);
        assert_eq!(mmap, 256 * MB);
    }

    #[test]
    fn adaptive_very_large_db() {
        // 2 GB DB → both capped at max
        let (cache_kb, mmap) = adaptive_cache_sizes(2 * 1024 * MB);
        assert_eq!(cache_kb, 64 * MB / KB);
        assert_eq!(mmap, 256 * MB);
    }
}
