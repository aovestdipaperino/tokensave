//! Single-value reads for the runtime telemetry snapshot.
use super::*;

/// Error wording for one telemetry scalar read: the operation name and the
/// subject used in each failure message.
struct ScalarRead {
    operation: &'static str,
    query_failed: &'static str,
    row_failed: &'static str,
    no_row: &'static str,
    decode_failed: &'static str,
}

impl ScalarRead {
    fn error(&self, message: String) -> TokenSaveError {
        TokenSaveError::Database {
            message,
            operation: self.operation.to_string(),
        }
    }
}

impl Database {
    /// Runs `sql` and returns its first row.
    async fn telemetry_row(&self, sql: &str, w: &ScalarRead) -> Result<libsql::Row> {
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| w.error(format!("{}: {e}", w.query_failed)))?;
        rows.next()
            .await
            .map_err(|e| w.error(format!("{}: {e}", w.row_failed)))?
            .ok_or_else(|| w.error(w.no_row.to_string()))
    }

    /// Runs `sql` and returns column 0 of its first row as an `i64`.
    async fn telemetry_i64(&self, sql: &str, w: &ScalarRead) -> Result<i64> {
        self.telemetry_row(sql, w)
            .await?
            .get::<i64>(0)
            .map_err(|e| w.error(format!("{}: {e}", w.decode_failed)))
    }

    /// Returns the connection's `journal_mode` (`wal`, `delete`, …).
    pub async fn journal_mode(&self) -> Result<String> {
        let w = ScalarRead {
            operation: "read_journal_mode",
            query_failed: "failed to read journal_mode",
            row_failed: "failed to read journal_mode row",
            no_row: "no journal_mode row returned",
            decode_failed: "failed to decode journal_mode",
        };
        self.telemetry_row("PRAGMA journal_mode", &w)
            .await?
            .get::<String>(0)
            .map_err(|e| w.error(format!("{}: {e}", w.decode_failed)))
    }

    /// Returns the summed size in bytes of every indexed source file.
    pub async fn source_total_bytes(&self) -> Result<u64> {
        let v = self
            .telemetry_i64(
                "SELECT COALESCE(SUM(size), 0) FROM files",
                &ScalarRead {
                    operation: "read_source_total_bytes",
                    query_failed: "failed to sum source bytes",
                    row_failed: "failed to read source-sum row",
                    no_row: "no source-sum row returned",
                    decode_failed: "failed to decode source-sum",
                },
            )
            .await?;
        Ok(u64::try_from(v).unwrap_or(0))
    }

    /// Returns the `(nodes, edges)` row counts.
    pub async fn graph_counts(&self) -> Result<(u64, u64)> {
        const COUNT: ScalarRead = ScalarRead {
            operation: "scalar_count",
            query_failed: "scalar query failed",
            row_failed: "scalar row read failed",
            no_row: "no scalar row",
            decode_failed: "scalar decode failed",
        };
        let nodes = self
            .telemetry_i64("SELECT COUNT(*) FROM nodes", &COUNT)
            .await?;
        let edges = self
            .telemetry_i64("SELECT COUNT(*) FROM edges", &COUNT)
            .await?;
        Ok((
            u64::try_from(nodes).unwrap_or(0),
            u64::try_from(edges).unwrap_or(0),
        ))
    }
}
