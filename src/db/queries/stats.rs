//! Aggregate statistics queries.
use super::*;

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

impl Database {
    /// Returns aggregate statistics about the code graph.
    pub async fn get_stats(&self) -> Result<GraphStats> {
        // Single query for all scalar counts: nodes, edges, files, last_updated, total_source_bytes
        let mut counts_rows = self
            .conn()
            .query(
                "SELECT \
                   (SELECT COUNT(*) FROM nodes), \
                   (SELECT COUNT(*) FROM edges), \
                   (SELECT COUNT(*) FROM files), \
                   (SELECT COALESCE(MAX(indexed_at), 0) FROM files), \
                   (SELECT COALESCE(SUM(size), 0) FROM files)",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query counts: {e}"),
                operation: "get_stats".to_string(),
            })?;
        let counts_row = counts_rows
            .next()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to read counts row: {e}"),
                operation: "get_stats".to_string(),
            })?;
        let (node_count, edge_count, file_count, last_updated, total_source_bytes) =
            match counts_row {
                Some(r) => {
                    let nc: i64 = r.get(0).unwrap_or(0);
                    let ec: i64 = r.get(1).unwrap_or(0);
                    let fc: i64 = r.get(2).unwrap_or(0);
                    let lu: i64 = r.get(3).unwrap_or(0);
                    let ts: i64 = r.get(4).unwrap_or(0);
                    (nc as u64, ec as u64, fc as u64, lu as u64, ts as u64)
                }
                None => (0, 0, 0, 0, 0),
            };

        // Nodes grouped by kind
        let nodes_by_kind = query_kind_counts(
            self.conn(),
            "SELECT kind, COUNT(*) FROM nodes GROUP BY kind",
        )
        .await?;

        // Edges grouped by kind
        let edges_by_kind = query_kind_counts(
            self.conn(),
            "SELECT kind, COUNT(*) FROM edges GROUP BY kind",
        )
        .await?;

        let db_size_bytes = self.size().await.unwrap_or(0);

        // Files grouped by language. Done in Rust (not SQL) so the label set
        // stays in sync with the extractor registry without an ever-growing
        // CASE expression. See `display_language_for_path`.
        let files_by_language = {
            let mut rows = self
                .conn()
                .query("SELECT path FROM files", ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query files for language stats: {e}"),
                    operation: "get_stats".to_string(),
                })?;
            let mut map: HashMap<String, u64> = HashMap::new();
            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read file row: {e}"),
                operation: "get_stats".to_string(),
            })? {
                let path: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read file path: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                *map.entry(display_language_for_path(&path).to_string())
                    .or_insert(0) += 1;
            }
            map
        };

        let last_sync_at = self
            .get_metadata("last_sync_at")
            .await?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let last_full_sync_at = self
            .get_metadata("last_full_sync_at")
            .await?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let last_sync_duration_ms = self
            .get_metadata("last_sync_duration_ms")
            .await?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        Ok(GraphStats {
            node_count,
            edge_count,
            file_count,
            nodes_by_kind,
            edges_by_kind,
            db_size_bytes,
            last_updated,
            total_source_bytes,
            files_by_language,
            last_sync_at,
            last_full_sync_at,
            last_sync_duration_ms,
        })
    }

    /// Returns the most recent `indexed_at` timestamp across all files,
    /// or 0 if the files table is empty.
    pub async fn last_index_time(&self) -> Result<i64> {
        query_scalar_i64(
            self.conn(),
            "SELECT COALESCE(MAX(indexed_at), 0) FROM files",
            "last_index_time",
        )
        .await
    }
}
