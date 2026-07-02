//! Search queries.
use super::*;

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

impl Database {
    /// Searches nodes by name, qualified name, docstring, or signature.
    ///
    /// Attempts an FTS5 prefix match first. If the FTS index is corrupted,
    /// it is automatically rebuilt and the query retried. If FTS returns no
    /// results, falls back to a `LIKE` query.
    pub async fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        debug_assert!(!query.is_empty(), "search_nodes called with empty query");
        debug_assert!(limit > 0, "search_nodes limit must be positive");
        // Sanitize query for FTS5: wrap each word in double quotes to escape
        // special characters (*, ?, :, etc.) and join with spaces (implicit OR).
        let fts_query: String = query
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .map(|w| {
                let sanitized: String = w.chars().filter(|c| *c != '"').collect();
                format!("\"{sanitized}\"*")
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        if fts_query.is_empty() {
            return Ok(Vec::new());
        }

        // Try FTS search, with one self-healing retry on corruption.
        let fts_result = self.search_nodes_fts(&fts_query, limit).await;
        match fts_result {
            Ok(ref results) if !results.is_empty() => return fts_result,
            Ok(_) => {} // empty — fall through to LIKE
            Err(ref e) if Self::is_corruption_error(e) => {
                eprintln!("[tokensave] FTS index corruption detected — rebuilding…");
                if self.rebuild_fts().await.is_ok() {
                    match self.search_nodes_fts(&fts_query, limit).await {
                        Ok(results) if !results.is_empty() => return Ok(results),
                        Ok(_) => {} // fall through to LIKE
                        Err(e) => return Err(e),
                    }
                }
                // rebuild_fts failed — fall through to LIKE as last resort
            }
            Err(e) => return Err(e),
        }

        // Fallback: LIKE query
        let like_pattern = format!("%{query}%");
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes
                 WHERE name LIKE ?1 OR qualified_name LIKE ?1 OR docstring LIKE ?1 OR signature LIKE ?1
                 LIMIT ?2",
                params![like_pattern.as_str(), limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to execute LIKE query: {e}"),
                operation: "search_nodes".to_string(),
            })?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read search result: {e}"),
            operation: "search_nodes".to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map search result: {e}"),
                operation: "search_nodes".to_string(),
            })?;
            results.push(SearchResult { node, score: 1.0 });
        }
        Ok(results)
    }

    /// Executes the FTS5 query and returns ranked results.
    pub(crate) async fn search_nodes_fts(
        &self,
        fts_query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands,
                    bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0) AS rank
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.rowid
                 WHERE nodes_fts MATCH ?1
                 ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0)
                 LIMIT ?2",
                params![fts_query, limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to execute FTS query: {e}"),
                operation: "search_nodes".to_string(),
            })?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read search result: {e}"),
            operation: "search_nodes".to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map search result: {e}"),
                operation: "search_nodes".to_string(),
            })?;
            let rank: f64 = row.get::<f64>(28).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read rank: {e}"),
                operation: "search_nodes".to_string(),
            })?;
            results.push(SearchResult { node, score: -rank });
        }
        Ok(results)
    }

    /// Returns a map of `node_id` → incoming "calls" edge count for the given IDs.
    /// IDs not found in any edge target are omitted from the result.
    pub async fn batch_incoming_call_counts(
        &self,
        node_ids: &[String],
    ) -> Result<std::collections::HashMap<String, u64>> {
        let mut counts = std::collections::HashMap::new();
        if node_ids.is_empty() {
            return Ok(counts);
        }
        let placeholders = build_qmark_placeholders(node_ids.len());
        let sql = format!(
            "SELECT target, COUNT(*) AS cnt FROM edges WHERE target IN ({placeholders}) AND kind = 'calls' GROUP BY target",
        );
        let param_values: Vec<libsql::Value> = node_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to batch count incoming calls: {e}"),
                operation: "batch_incoming_call_counts".to_string(),
            })?;
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read batch call count row: {e}"),
            operation: "batch_incoming_call_counts".to_string(),
        })? {
            let id: String = row.get(0).unwrap_or_default();
            let cnt: u64 = row.get::<u64>(1).unwrap_or(0);
            counts.insert(id, cnt);
        }
        Ok(counts)
    }

    /// Finds nodes whose `name` column exactly matches one of the given names
    /// (case-insensitive). Used to supplement FTS results so that perfect
    /// matches are never buried by BM25 noise.
    pub async fn search_nodes_by_exact_name(
        &self,
        names: &[String],
        limit: usize,
    ) -> Result<Vec<Node>> {
        if names.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let placeholders = build_qmark_placeholders(names.len());
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async,
                    branches, loops, returns, max_nesting,
                    unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes
             WHERE LOWER(name) IN ({placeholders})
             LIMIT ?",
        );
        let mut param_values: Vec<libsql::Value> = names
            .iter()
            .map(|n| libsql::Value::Text(n.to_lowercase()))
            .collect();
        param_values.push(libsql::Value::Integer(limit as i64));

        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to search by exact name: {e}"),
                operation: "search_nodes_by_exact_name".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "search_nodes_by_exact_name").await
    }

    /// Returns `true` if the error indicates `SQLite` database corruption.
    pub fn is_corruption_error(e: &TokenSaveError) -> bool {
        match e {
            TokenSaveError::Database { message, .. } => {
                message.contains("malformed")
                    || message.contains("corrupt")
                    || message.contains("disk image")
            }
            _ => false,
        }
    }
}
