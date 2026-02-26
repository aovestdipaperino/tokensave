use std::path::Path;

use rusqlite::Connection;

use crate::errors::{CodeGraphError, Result};

/// The embedded SQL schema applied when initializing a new database.
const SCHEMA_SQL: &str = include_str!("schema.sql");

/// SQLite database backing the code graph.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Creates a new database at `db_path`, creating parent directories if needed.
    ///
    /// Opens a SQLite connection, applies performance pragmas, and executes the
    /// full schema (tables, indexes, triggers, FTS).
    pub fn initialize(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CodeGraphError::Database {
                message: format!("failed to create database directory: {e}"),
                operation: "initialize".to_string(),
            })?;
        }

        let conn = Connection::open(db_path).map_err(|e| CodeGraphError::Database {
            message: format!("failed to open database: {e}"),
            operation: "initialize".to_string(),
        })?;

        Self::apply_pragmas(&conn)?;

        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to apply schema: {e}"),
                operation: "initialize".to_string(),
            })?;

        Ok(Self { conn })
    }

    /// Opens an existing database at `db_path` and applies performance pragmas.
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path).map_err(|e| CodeGraphError::Database {
            message: format!("failed to open database: {e}"),
            operation: "open".to_string(),
        })?;

        Self::apply_pragmas(&conn)?;

        Ok(Self { conn })
    }

    /// Returns a reference to the underlying SQLite connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Consumes the `Database`, closing the underlying connection.
    pub fn close(self) {
        drop(self.conn);
    }

    /// Runs VACUUM and ANALYZE to reclaim space and update query planner statistics.
    pub fn optimize(&self) -> Result<()> {
        self.conn
            .execute_batch("VACUUM; ANALYZE;")
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to optimize database: {e}"),
                operation: "optimize".to_string(),
            })
    }

    /// Returns the on-disk size of the database file in bytes.
    pub fn size(&self) -> Result<u64> {
        let size: i64 = self
            .conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| row.get(0),
            )
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to get database size: {e}"),
                operation: "size".to_string(),
            })?;
        Ok(size as u64)
    }

    /// Applies performance-oriented SQLite pragmas.
    fn apply_pragmas(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 120000;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -65536;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 268435456;",
        )
        .map_err(|e| CodeGraphError::Database {
            message: format!("failed to apply pragmas: {e}"),
            operation: "apply_pragmas".to_string(),
        })
    }
}
