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

    /// Returns all nodes under a directory prefix filtered by kinds.
    ///
    /// Uses `LIKE dir || '%'` for the path prefix and an `IN` clause for kinds.
    pub async fn get_nodes_by_dir(&self, dir: &str, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }

        let kind_placeholders: Vec<String> = kinds
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect();
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async,
                    branches, loops, returns, max_nesting,
                    unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes
             WHERE file_path LIKE ?1 || '%' AND kind IN ({})
             ORDER BY file_path, start_line",
            kind_placeholders.join(", ")
        );

        let mut param_values: Vec<libsql::Value> = Vec::new();
        param_values.push(libsql::Value::Text(dir.to_string()));
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
                "SELECT source, target, kind, line FROM edges WHERE source IN ({})",
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
    /// (push the ids into a TEMP table, then probe by id) so the LIKE never
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

    /// Drops, recreates, and bulk-inserts `ids` into `temp.test_markers`.
    ///
    /// The temp table has a `PRIMARY KEY` on `id` so `SQLite` builds a real
    /// rowid B-tree — `IN (SELECT id FROM temp.test_markers)` in downstream
    /// queries probes via that index, not a wildcard scan. Inserts are
    /// chunked under `SQLite`'s 999-parameter limit.
    ///
    /// Always drops first, so a previous call on the same connection
    /// (e.g. consecutive `find_dead_code` from the same MCP client) does
    /// not collide. The caller should also drop the table when done — see
    /// `find_dead_code` for the wrapping pattern.
    pub async fn populate_test_marker_temp_table(&self, ids: &[String]) -> Result<()> {
        // `SQLite`'s default parameter limit is 999. Chunk well under that.
        const CHUNK_SIZE: usize = 500;

        let op = "populate_test_marker_temp_table";
        let conn = self.conn();

        // Drop + recreate so we always start from an empty table.
        conn.execute("DROP TABLE IF EXISTS temp.test_markers", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to drop temp.test_markers: {e}"),
                operation: op.to_string(),
            })?;
        conn.execute("CREATE TEMP TABLE test_markers (id TEXT PRIMARY KEY)", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to create temp.test_markers: {e}"),
                operation: op.to_string(),
            })?;

        if ids.is_empty() {
            return Ok(());
        }

        for chunk in ids.chunks(CHUNK_SIZE) {
            let mut sql = String::from("INSERT INTO temp.test_markers (id) VALUES ");
            for i in 0..chunk.len() {
                if i > 0 {
                    sql.push_str(", ");
                }
                sql.push_str("(?)");
            }
            let params: Vec<libsql::Value> = chunk
                .iter()
                .map(|id| libsql::Value::Text(id.clone()))
                .collect();
            conn.execute(&sql, libsql::params_from_iter(params))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to bulk-insert test markers: {e}"),
                    operation: op.to_string(),
                })?;
        }
        Ok(())
    }

    /// Drops `temp.test_markers` if it exists. Used as cleanup by
    /// `find_dead_code` so the table does not leak to other queries on the
    /// same connection.
    ///
    /// Safe to call even if the table doesn't exist (uses `IF EXISTS`).
    pub async fn drop_test_marker_temp_table(&self) -> Result<()> {
        self.conn()
            .execute("DROP TABLE IF EXISTS temp.test_markers", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to drop temp.test_markers: {e}"),
                operation: "drop_test_marker_temp_table".to_string(),
            })?;
        Ok(())
    }

    /// Materialises the set of node ids that are targets of a test-marker
    /// `annotates` edge into `temp.test_annotated_targets`.
    ///
    /// This is the second step of the dead-code test-exclusion pipeline:
    /// 1. `populate_test_marker_temp_table` fills `temp.test_markers`.
    /// 2. THIS fn pre-resolves "which nodes are annotated by any test
    ///    marker" into a small lookup table with a PK on `target`.
    /// 3. `find_dead_code`'s outer SELECT then uses
    ///    `id NOT IN (SELECT target FROM temp.test_annotated_targets)` —
    ///    an indexed PK probe per candidate.
    ///
    /// Why two tables instead of `IN (SELECT id FROM temp.test_markers)`
    /// inside a correlated `NOT EXISTS`: on chromium (~13 K markers,
    /// ~134 K dead-code candidates, ~411 K annotates edges) `SQLite` picked
    /// `idx_edges_unique (source, target, kind)` for the correlated
    /// subquery, iterating every marker as the outer driver for every
    /// candidate. That's ~1.7 billion index probes and a >25 s timeout
    /// on the MCP probe. Pre-materialising the *target* set means the
    /// per-candidate probe becomes a single indexed lookup against a
    /// table with ~15 K rows. Real measurement on chromium 7.5 GB DB:
    /// 0.75 s end-to-end (vs. >60 s for the single-temp-table form).
    pub async fn populate_test_annotated_targets_temp_table(&self) -> Result<()> {
        let op = "populate_test_annotated_targets_temp_table";
        let conn = self.conn();

        // Drop + recreate so we always start from an empty table — same
        // hygiene as the test_markers temp table.
        conn.execute("DROP TABLE IF EXISTS temp.test_annotated_targets", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to drop temp.test_annotated_targets: {e}"),
                operation: op.to_string(),
            })?;
        conn.execute(
            "CREATE TEMP TABLE test_annotated_targets (target TEXT PRIMARY KEY)",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to create temp.test_annotated_targets: {e}"),
            operation: op.to_string(),
        })?;

        // `INSERT OR IGNORE` because a single function can have multiple
        // test markers (e.g. `#[test] #[cfg(target_os = "linux")]`) — one
        // row per target, not per (target, marker) pair.
        conn.execute(
            "INSERT OR IGNORE INTO temp.test_annotated_targets (target)
             SELECT e.target FROM edges e
             WHERE e.kind = 'annotates'
               AND e.source IN (SELECT id FROM temp.test_markers)",
            (),
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to populate temp.test_annotated_targets: {e}"),
            operation: op.to_string(),
        })?;
        Ok(())
    }

    /// Drops `temp.test_annotated_targets` if it exists. Cleanup pair for
    /// `populate_test_annotated_targets_temp_table`.
    pub async fn drop_test_annotated_targets_temp_table(&self) -> Result<()> {
        self.conn()
            .execute("DROP TABLE IF EXISTS temp.test_annotated_targets", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to drop temp.test_annotated_targets: {e}"),
                operation: "drop_test_annotated_targets_temp_table".to_string(),
            })?;
        Ok(())
    }
}
