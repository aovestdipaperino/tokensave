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
    pub async fn initialize(db_path: &Path) -> Result<Self> {
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
        migrations::migrate(&conn).await?;

        Ok(Self { conn, _db: db })
    }

    /// Opens an existing database at `db_path`, applies performance pragmas,
    /// and runs any pending schema migrations.
    pub async fn open(db_path: &Path) -> Result<Self> {
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
        migrations::migrate(&conn).await?;

        Ok(Self { conn, _db: db })
    }

    /// Returns a reference to the underlying libsql connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Consumes the `Database`, closing the underlying connection.
    pub fn close(self) {
        drop(self.conn);
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
            "PRAGMA journal_mode = WAL;
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
}
