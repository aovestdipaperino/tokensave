//! Metadata key/value storage.
use super::*;

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

impl Database {
    /// Reads a metadata value by key, returning `None` if not set.
    pub async fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let mut rows = self
            .conn()
            .query("SELECT value FROM metadata WHERE key = ?1", params![key])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query metadata: {e}"),
                operation: "get_metadata".to_string(),
            })?;

        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read metadata row: {e}"),
            operation: "get_metadata".to_string(),
        })? {
            Some(row) => {
                let value: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read metadata value: {e}"),
                    operation: "get_metadata".to_string(),
                })?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Sets a metadata value, creating or replacing the entry.
    pub async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to set metadata: {e}"),
                operation: "set_metadata".to_string(),
            })?;
        Ok(())
    }

    /// Returns all nodes in a file or under a directory, filtered by kinds.
    ///
    /// Uses the shared escaped path filter and an `IN` clause for kinds.
    pub async fn get_nodes_by_dir(&self, dir: &str, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }

        let kind_placeholders: Vec<String> = kinds
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let mut path_filter = String::new();
        push_path_prefix_filter(&mut path_filter, "", dir);
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async,
                    branches, loops, returns, max_nesting,
                    unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes
             WHERE {path_filter} AND kind IN ({})
             ORDER BY file_path, start_line",
            kind_placeholders.join(", ")
        );

        let mut param_values: Vec<libsql::Value> = Vec::new();
        for k in kinds {
            param_values.push(libsql::Value::Text(k.as_str().to_string()));
        }

        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query nodes by dir: {e}"),
                operation: "get_nodes_by_dir".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_by_dir").await
    }

    /// Returns edges where both source and target are in the given node ID set.
    ///
    /// Batches queries in groups of 500 IDs to avoid SQL parameter limits.
    pub async fn get_internal_edges(&self, node_ids: &[String]) -> Result<Vec<Edge>> {
        const BATCH_SIZE: usize = 500;
        if node_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Build a set of IDs for filtering targets in memory, then query
        // edges from each batch of sources.
        let id_set: std::collections::HashSet<&str> =
            node_ids.iter().map(std::string::String::as_str).collect();
        let mut all_edges = Vec::new();
        let mut offset = 0;
        while offset < node_ids.len() {
            let end = (offset + BATCH_SIZE).min(node_ids.len());
            let batch = &node_ids[offset..end];

            let placeholders: Vec<String> = batch
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect();
            let sql = format!(
                "SELECT source, target, kind, line, resolved_by FROM edges WHERE source IN ({})",
                placeholders.join(", ")
            );

            let param_values: Vec<libsql::Value> = batch
                .iter()
                .map(|id| libsql::Value::Text(id.clone()))
                .collect();

            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(param_values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query internal edges: {e}"),
                    operation: "get_internal_edges".to_string(),
                })?;

            let batch_edges: Vec<Edge> =
                collect_rows(&mut rows, row_to_edge, "get_internal_edges").await?;

            // Keep only edges whose target is also in the node set.
            for edge in batch_edges {
                if id_set.contains(edge.target.as_str()) {
                    all_edges.push(edge);
                }
            }

            offset = end;
        }

        Ok(all_edges)
    }

    /// Resolves the set of `annotation_usage` node ids whose name marks a
    /// function as a test (`#[test]`, `#[tokio::test]`, `#[async_std::test]`,
    /// `#[wasm_bindgen_test]`, …). Runs the leading-wildcard `LIKE` scan
    /// exactly once over the `kind = 'annotation_usage'` partition.
    ///
    /// `find_dead_code` uses this in a two-step "resolve + use" pattern
    /// (resolve the annotated targets by id, then filter in memory) so the LIKE never
    /// runs in a correlated subquery — the per-row degenerate plan from the
    /// reverted 4.14.8 CTE attempt timed out at >60s on scirs; the original
    /// JOIN+LIKE form times out at >25s on chromium. Both pathologies stem
    /// from re-running the wildcard scan per candidate row.
    pub async fn collect_test_marker_ids(&self) -> Result<Vec<String>> {
        let op = "collect_test_marker_ids";
        let sql = "SELECT id FROM nodes
                   WHERE kind = 'annotation_usage'
                     AND (
                         name = 'test'
                         OR name LIKE '%::test'
                         OR name = 'wasm_bindgen_test'
                         OR name LIKE '%::wasm_bindgen_test'
                     )";
        let mut rows = self
            .conn()
            .query(sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query test marker ids: {e}"),
                operation: op.to_string(),
            })?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read marker id row: {e}"),
            operation: op.to_string(),
        })? {
            let id: String = row.get(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read marker id column: {e}"),
                operation: op.to_string(),
            })?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Resolves the set of node ids that are targets of an `annotates` edge
    /// whose source is one of `marker_ids` (see `collect_test_marker_ids`),
    /// i.e. "which nodes carry a test marker".
    ///
    /// The lookup runs in bounded chunks of marker ids, each probing
    /// `idx_edges_source_kind`, and the result is kept in memory. No TEMP
    /// table is created, so this works on connections opened with
    /// `PRAGMA query_only = 1` (selected graphs) and cannot race with another
    /// concurrent read-only call that shares the same connection.
    ///
    /// The set holds one id per test-annotated node (~15 K on chromium) and
    /// is dropped when `find_dead_code` returns.
    pub async fn collect_test_annotated_targets(
        &self,
        marker_ids: &[String],
    ) -> Result<HashSet<String>> {
        // `SQLite`'s default parameter limit is 999. Chunk well under that.
        const CHUNK_SIZE: usize = 500;

        let op = "collect_test_annotated_targets";
        let conn = self.conn();
        let mut targets = HashSet::new();
        for chunk in marker_ids.chunks(CHUNK_SIZE) {
            let placeholders = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT target FROM edges
                 WHERE kind = 'annotates' AND source IN ({placeholders})"
            );
            let params: Vec<libsql::Value> = chunk
                .iter()
                .map(|id| libsql::Value::Text(id.clone()))
                .collect();
            let mut rows = conn
                .query(&sql, libsql::params_from_iter(params))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query test-annotated targets: {e}"),
                    operation: op.to_string(),
                })?;
            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read test-annotated target row: {e}"),
                operation: op.to_string(),
            })? {
                let target: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read test-annotated target column: {e}"),
                    operation: op.to_string(),
                })?;
                targets.insert(target);
            }
        }
        Ok(targets)
    }
}
