//! Search queries.
use super::*;

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

impl Database {
    /// Replaces executable-body FTS documents in one prepared-statement
    /// transaction. Callers delete stale documents by file before supplying
    /// replacements for incremental indexing; full indexing starts from an
    /// empty table.
    pub async fn insert_executable_body_documents(
        &self,
        documents: &[ExecutableBodyDocument],
    ) -> Result<()> {
        if documents.is_empty() {
            return Ok(());
        }
        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin executable body insert: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
        let stmt = self
            .conn()
            .prepare(
                "INSERT INTO executable_body_fts (node_id, file_path, body) VALUES (?1, ?2, ?3)",
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare executable body insert: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
        for document in documents {
            stmt.execute(params![
                document.node_id.as_str(),
                document.file_path.as_str(),
                document.body.as_str(),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert executable body: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
            stmt.reset();
        }
        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit executable body insert: {e}"),
                operation: "insert_executable_body_documents".to_string(),
            })?;
        Ok(())
    }

    /// Searches the persistent executable-body index and returns nodes matched
    /// by at least two distinct conceptual terms. Each term uses a conservative
    /// five-character prefix so prose such as `preconditioner` can retrieve the
    /// conventional local abbreviation `precond`.
    pub async fn search_executable_bodies(
        &self,
        terms: &[String],
        candidate_limit: usize,
    ) -> Result<Vec<(Node, usize)>> {
        if terms.len() < 2 || candidate_limit == 0 {
            return Ok(Vec::new());
        }
        let mut seen = std::collections::HashSet::new();
        let prefixes: Vec<String> = terms
            .iter()
            .filter_map(|term| {
                let prefix = term
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(5)
                    .collect::<String>()
                    .to_lowercase();
                (prefix.len() >= 4 && seen.insert(prefix.clone())).then_some(prefix)
            })
            .collect();
        if prefixes.len() < 2 {
            return Ok(Vec::new());
        }

        // Ask FTS for documents containing any pair of distinct concepts. This
        // guarantees useful co-occurrence candidates while letting LIMIT stop
        // the index walk early; a BM25 ORDER BY would rank every matching body
        // in a large project before applying the bound.
        let fts_query = (0..prefixes.len())
            .flat_map(|left| {
                let prefixes = &prefixes;
                ((left + 1)..prefixes.len()).map(move |right| {
                    format!("(\"{}\"* AND \"{}\"*)", prefixes[left], prefixes[right])
                })
            })
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut rows = self
            .conn()
            .query(
                "SELECT node_id, body FROM executable_body_fts
                 WHERE executable_body_fts MATCH ?1
                 LIMIT ?2",
                params![fts_query, candidate_limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to search executable bodies: {e}"),
                operation: "search_executable_bodies".to_string(),
            })?;
        let mut hits = std::collections::HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read executable body hit: {e}"),
            operation: "search_executable_bodies".to_string(),
        })? {
            let node_id: String = row.get(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read executable body node id: {e}"),
                operation: "search_executable_bodies".to_string(),
            })?;
            let body: String = row.get(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read executable body hit: {e}"),
                operation: "search_executable_bodies".to_string(),
            })?;
            let body = body.to_lowercase();
            let matched = prefixes
                .iter()
                .filter(|prefix| {
                    body.split(|c: char| !c.is_alphanumeric())
                        .any(|token| token.starts_with(prefix.as_str()))
                })
                .count();
            if matched >= 2 {
                hits.insert(node_id, matched);
            }
        }

        let ids: Vec<String> = hits.keys().cloned().collect();
        let mut nodes: Vec<(Node, usize)> = self
            .get_nodes_by_ids(&ids)
            .await?
            .into_iter()
            .filter_map(|node| hits.get(&node.id).copied().map(|count| (node, count)))
            .collect();
        nodes.sort_by(|(a, a_hits), (b, b_hits)| {
            b_hits.cmp(a_hits).then_with(|| {
                a.end_line
                    .saturating_sub(a.start_line)
                    .cmp(&b.end_line.saturating_sub(b.start_line))
            })
        });
        Ok(nodes)
    }

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

    /// Returns a bounded FTS candidate set without globally BM25-ranking every
    /// match. Context building applies its own multi-signal ranking after
    /// merging several search channels, so paying for a repository-wide sort
    /// here is both redundant and pathological for common conceptual terms.
    pub async fn search_nodes_bounded(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        debug_assert!(
            !query.is_empty(),
            "search_nodes_bounded called with empty query"
        );
        debug_assert!(limit > 0, "search_nodes_bounded limit must be positive");
        let sanitized: String = query.chars().filter(|c| *c != '"').collect();
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }
        let fts_query = format!("\"{sanitized}\"*");
        let result = self.search_nodes_bounded_fts(&fts_query, limit).await;
        if result.as_ref().is_err_and(Self::is_corruption_error) {
            eprintln!("[tokensave] FTS index corruption detected — rebuilding…");
            self.rebuild_fts().await?;
            return self.search_nodes_bounded_fts(&fts_query, limit).await;
        }
        result
    }

    async fn search_nodes_bounded_fts(
        &self,
        fts_query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at, n.attrs_start_line, n.parent_id, n.cognitive_complexity, n.distinct_operators, n.distinct_operands, n.total_operators, n.total_operands
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.rowid
                 WHERE nodes_fts MATCH ?1
                 LIMIT ?2",
                params![fts_query, limit as i64],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to execute bounded FTS query: {e}"),
                operation: "search_nodes_bounded".to_string(),
            })?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read bounded search result: {e}"),
            operation: "search_nodes_bounded".to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map bounded search result: {e}"),
                operation: "search_nodes_bounded".to_string(),
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
