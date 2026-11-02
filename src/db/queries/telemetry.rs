//! Index facts sampled by runtime telemetry (`runtime_telemetry`).
use crate::db::Database;
use crate::errors::{Result, TokenSaveError};

impl Database {
    /// Returns the connection's `PRAGMA journal_mode` (for example `wal`).
    pub async fn read_journal_mode(&self) -> Result<String> {
        let mut rows = self
            .conn()
            .query("PRAGMA journal_mode", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read journal_mode: {e}"),
                operation: "read_journal_mode".to_string(),
            })?;
        let row = rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read journal_mode row: {e}"),
                operation: "read_journal_mode".to_string(),
            })?
            .ok_or_else(|| TokenSaveError::Database {
                message: "no journal_mode row returned".to_string(),
                operation: "read_journal_mode".to_string(),
            })?;
        row.get::<String>(0).map_err(|e| TokenSaveError::Database {
            message: format!("failed to decode journal_mode: {e}"),
            operation: "read_journal_mode".to_string(),
        })
    }

    /// Returns the summed recorded size of all indexed source files.
    pub async fn read_source_total_bytes(&self) -> Result<u64> {
        let mut rows = self
            .conn()
            .query("SELECT COALESCE(SUM(size), 0) FROM files", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to sum source bytes: {e}"),
                operation: "read_source_total_bytes".to_string(),
            })?;
        let row = rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read source-sum row: {e}"),
                operation: "read_source_total_bytes".to_string(),
            })?
            .ok_or_else(|| TokenSaveError::Database {
                message: "no source-sum row returned".to_string(),
                operation: "read_source_total_bytes".to_string(),
            })?;
        let v: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
            message: format!("failed to decode source-sum: {e}"),
            operation: "read_source_total_bytes".to_string(),
        })?;
        Ok(u64::try_from(v).unwrap_or(0))
    }

    /// Returns the `(node, edge)` row counts.
    pub async fn read_graph_counts(&self) -> Result<(u64, u64)> {
        let nodes = self.scalar_count("SELECT COUNT(*) FROM nodes").await?;
        let edges = self.scalar_count("SELECT COUNT(*) FROM edges").await?;
        Ok((nodes, edges))
    }

    async fn scalar_count(&self, sql: &str) -> Result<u64> {
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("scalar query failed: {e}"),
                operation: "scalar_count".to_string(),
            })?;
        let row = rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("scalar row read failed: {e}"),
                operation: "scalar_count".to_string(),
            })?
            .ok_or_else(|| TokenSaveError::Database {
                message: "no scalar row".to_string(),
                operation: "scalar_count".to_string(),
            })?;
        let v: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
            message: format!("scalar decode failed: {e}"),
            operation: "scalar_count".to_string(),
        })?;
        Ok(u64::try_from(v).unwrap_or(0))
    }
}
