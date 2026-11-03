//! Single-value reads for the runtime telemetry snapshot.
use grafeo::Value;

use super::{db_error, Database, SYMBOL};
use crate::errors::Result;

impl Database {
    /// Grafeo has no journal modes; report its write-ahead log.
    pub async fn journal_mode(&self) -> Result<String> {
        Ok("grafeo-wal".to_string())
    }

    /// Returns the summed size in bytes of every indexed source file.
    pub async fn source_total_bytes(&self) -> Result<u64> {
        Ok(self.get_all_files().await?.iter().map(|f| f.size).sum())
    }

    /// Returns the `(nodes, edges)` counts of the code graph.
    pub async fn graph_counts(&self) -> Result<(u64, u64)> {
        let nodes = self.count(&format!("MATCH (n:{SYMBOL}) RETURN count(n)"))?;
        let edges = self.count(&format!(
            "MATCH (:{SYMBOL})-[e]->(:{SYMBOL}) RETURN count(e)"
        ))?;
        Ok((nodes, edges))
    }

    fn count(&self, query: &str) -> Result<u64> {
        let result = self
            .db
            .execute(query)
            .map_err(|e| db_error("graph_counts", e))?;
        Ok(result
            .rows()
            .first()
            .and_then(|row| row.first())
            .and_then(Value::as_int64)
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or(0))
    }
}
