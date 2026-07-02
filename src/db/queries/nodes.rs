//! Node CRUD queries.
use super::*;

// ---------------------------------------------------------------------------
// Node operations
// ---------------------------------------------------------------------------

impl Database {
    /// Inserts or replaces a single node.
    pub async fn insert_node(&self, node: &Node) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO nodes
                (id, kind, name, qualified_name, file_path,
                 start_line, end_line, start_column, end_column,
                 docstring, signature, visibility, is_async,
                 branches, loops, returns, max_nesting,
                 unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
                params![
                    node.id.as_str(),
                    node.kind.as_str(),
                    node.name.as_str(),
                    node.qualified_name.as_str(),
                    node.file_path.as_str(),
                    i64::from(node.start_line),
                    i64::from(node.end_line),
                    i64::from(node.start_column),
                    i64::from(node.end_column),
                    opt_str(node.docstring.as_deref()),
                    opt_str(node.signature.as_deref()),
                    node.visibility.as_str(),
                    i64::from(node.is_async),
                    i64::from(node.branches),
                    i64::from(node.loops),
                    i64::from(node.returns),
                    i64::from(node.max_nesting),
                    i64::from(node.unsafe_blocks),
                    i64::from(node.unchecked_calls),
                    i64::from(node.assertions),
                    node.updated_at as i64,
                    i64::from(node.attrs_start_line),
                    opt_str(node.parent_id.as_deref()),
                    i64::from(node.cognitive_complexity),
                    i64::from(node.distinct_operators),
                    i64::from(node.distinct_operands),
                    i64::from(node.total_operators),
                    i64::from(node.total_operands),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert node: {e}"),
                operation: "insert_node".to_string(),
            })?;
        Ok(())
    }

    /// Inserts all nodes, edges, and file records in a single `execute_batch` call.
    /// This minimizes transaction overhead by combining everything into one SQL string.
    ///
    /// `Contains` edges are denormalized at insert time: their `(source, target)`
    /// pair is folded into the target node's `parent_id` column, and the edge
    /// itself is not persisted. Extractors keep emitting `Contains` edges as
    /// before; the conversion happens here, in one place.
    pub async fn insert_all(
        &self,
        nodes: &[Node],
        edges: &[Edge],
        files: &[FileRecord],
    ) -> Result<()> {
        // Pull every Contains edge out: build target_id -> parent_id map, then
        // filter the surviving edges list. When a node has multiple incoming
        // Contains rows (extractor anomaly), the first one wins — matching
        // the migration's `LIMIT 1` backfill behavior.
        let mut parent_map: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::new();
        let mut surviving_edges: Vec<&Edge> = Vec::with_capacity(edges.len());
        for edge in edges {
            if edge.kind == crate::types::EdgeKind::Contains {
                parent_map
                    .entry(edge.target.as_str())
                    .or_insert(edge.source.as_str());
            } else {
                surviving_edges.push(edge);
            }
        }
        // Apply the hoisted parents to the node slice without cloning every
        // node: we materialize only when parent_map has something to say.
        let nodes_owned: Vec<Node>;
        let nodes_ref: &[Node] = if parent_map.is_empty() {
            nodes
        } else {
            nodes_owned = nodes
                .iter()
                .map(|n| {
                    if let Some(parent) = parent_map.get(n.id.as_str()) {
                        let mut copy = n.clone();
                        copy.parent_id = Some((*parent).to_string());
                        copy
                    } else {
                        n.clone()
                    }
                })
                .collect();
            &nodes_owned
        };

        let mut sql = String::with_capacity(
            nodes_ref.len() * 400 + surviving_edges.len() * 120 + files.len() * 120,
        );
        sql.push_str("BEGIN;\n");

        // Nodes
        for chunk in nodes_ref.chunks(200) {
            sql.push_str(
                "INSERT OR REPLACE INTO nodes \
                 (id,kind,name,qualified_name,file_path,\
                 start_line,end_line,start_column,end_column,\
                 docstring,signature,visibility,is_async,\
                 branches,loops,returns,max_nesting,\
                 unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands) VALUES ",
            );
            for (i, node) in chunk.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('(');
                push_quoted(&mut sql, &node.id);
                sql.push(',');
                push_quoted(&mut sql, node.kind.as_str());
                sql.push(',');
                push_quoted(&mut sql, &node.name);
                sql.push(',');
                push_quoted(&mut sql, &node.qualified_name);
                sql.push(',');
                push_quoted(&mut sql, &node.file_path);
                sql.push(',');
                push_int(&mut sql, i64::from(node.start_line));
                sql.push(',');
                push_int(&mut sql, i64::from(node.end_line));
                sql.push(',');
                push_int(&mut sql, i64::from(node.start_column));
                sql.push(',');
                push_int(&mut sql, i64::from(node.end_column));
                sql.push(',');
                push_opt_quoted(&mut sql, node.docstring.as_deref());
                sql.push(',');
                push_opt_quoted(&mut sql, node.signature.as_deref());
                sql.push(',');
                push_quoted(&mut sql, node.visibility.as_str());
                sql.push(',');
                push_int(&mut sql, i64::from(node.is_async));
                sql.push(',');
                push_int(&mut sql, i64::from(node.branches));
                sql.push(',');
                push_int(&mut sql, i64::from(node.loops));
                sql.push(',');
                push_int(&mut sql, i64::from(node.returns));
                sql.push(',');
                push_int(&mut sql, i64::from(node.max_nesting));
                sql.push(',');
                push_int(&mut sql, i64::from(node.unsafe_blocks));
                sql.push(',');
                push_int(&mut sql, i64::from(node.unchecked_calls));
                sql.push(',');
                push_int(&mut sql, i64::from(node.assertions));
                sql.push(',');
                push_int(&mut sql, node.updated_at as i64);
                sql.push(',');
                push_int(&mut sql, i64::from(node.attrs_start_line));
                sql.push(',');
                push_opt_quoted(&mut sql, node.parent_id.as_deref());
                sql.push(',');
                push_int(&mut sql, i64::from(node.cognitive_complexity));
                sql.push(',');
                push_int(&mut sql, i64::from(node.distinct_operators));
                sql.push(',');
                push_int(&mut sql, i64::from(node.distinct_operands));
                sql.push(',');
                push_int(&mut sql, i64::from(node.total_operators));
                sql.push(',');
                push_int(&mut sql, i64::from(node.total_operands));
                sql.push(')');
            }
            sql.push_str(";\n");
        }

        // Edges (Contains has already been hoisted out into parent_id)
        for chunk in surviving_edges.chunks(500) {
            sql.push_str("INSERT OR IGNORE INTO edges (source,target,kind,line) VALUES ");
            for (i, edge) in chunk.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('(');
                push_quoted(&mut sql, &edge.source);
                sql.push(',');
                push_quoted(&mut sql, &edge.target);
                sql.push(',');
                push_quoted(&mut sql, edge.kind.as_str());
                sql.push(',');
                match edge.line {
                    Some(l) => push_int(&mut sql, i64::from(l)),
                    None => sql.push_str("NULL"),
                }
                sql.push(')');
            }
            sql.push_str(";\n");
        }

        // Files
        for chunk in files.chunks(500) {
            sql.push_str(
                "INSERT OR REPLACE INTO files \
                 (path,content_hash,size,modified_at,indexed_at,node_count) VALUES ",
            );
            for (i, file) in chunk.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('(');
                push_quoted(&mut sql, &file.path);
                sql.push(',');
                push_quoted(&mut sql, &file.content_hash);
                sql.push(',');
                push_int(&mut sql, file.size as i64);
                sql.push(',');
                push_int(&mut sql, file.modified_at);
                sql.push(',');
                push_int(&mut sql, file.indexed_at);
                sql.push(',');
                push_int(&mut sql, i64::from(file.node_count));
                sql.push(')');
            }
            sql.push_str(";\n");
        }

        sql.push_str("COMMIT;\n");

        self.conn()
            .execute_batch(&sql)
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to bulk insert: {e}"),
                operation: "insert_all".to_string(),
            })?;
        Ok(())
    }

    /// Inserts nodes using a prepared statement: parse SQL once, then
    /// bind+execute+reset for each row — zero SQL parsing after the first call.
    pub async fn insert_nodes(&self, nodes: &[Node]) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin: {e}"),
                operation: "insert_nodes".to_string(),
            })?;

        let stmt = self.conn()
            .prepare(
                "INSERT OR REPLACE INTO nodes \
                 (id,kind,name,qualified_name,file_path,\
                 start_line,end_line,start_column,end_column,\
                 docstring,signature,visibility,is_async,\
                 branches,loops,returns,max_nesting,\
                 unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28)"
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to prepare: {e}"),
                operation: "insert_nodes".to_string(),
            })?;

        for node in nodes {
            stmt.execute(params![
                node.id.as_str(),
                node.kind.as_str(),
                node.name.as_str(),
                node.qualified_name.as_str(),
                node.file_path.as_str(),
                i64::from(node.start_line),
                i64::from(node.end_line),
                i64::from(node.start_column),
                i64::from(node.end_column),
                opt_str(node.docstring.as_deref()),
                opt_str(node.signature.as_deref()),
                node.visibility.as_str(),
                i64::from(node.is_async),
                i64::from(node.branches),
                i64::from(node.loops),
                i64::from(node.returns),
                i64::from(node.max_nesting),
                i64::from(node.unsafe_blocks),
                i64::from(node.unchecked_calls),
                i64::from(node.assertions),
                node.updated_at as i64,
                i64::from(node.attrs_start_line),
                opt_str(node.parent_id.as_deref()),
                i64::from(node.cognitive_complexity),
                i64::from(node.distinct_operators),
                i64::from(node.distinct_operands),
                i64::from(node.total_operators),
                i64::from(node.total_operands),
            ])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert node: {e}"),
                operation: "insert_nodes".to_string(),
            })?;
            stmt.reset();
        }

        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to commit: {e}"),
                operation: "insert_nodes".to_string(),
            })?;
        Ok(())
    }

    /// Retrieves a node by its unique ID, returning `None` if not found.
    pub async fn get_node_by_id(&self, id: &str) -> Result<Option<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE id = ?1",
                params![id],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node by id: {e}"),
                operation: "get_node_by_id".to_string(),
            })?;

        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read node row: {e}"),
            operation: "get_node_by_id".to_string(),
        })? {
            Some(row) => {
                let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to map node row: {e}"),
                    operation: "get_node_by_id".to_string(),
                })?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    /// Returns nodes by their IDs in a single batch query.
    /// IDs not found are silently omitted. Results are returned in arbitrary order.
    pub async fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<Node>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Build `?, ?, ?, …` in one allocation instead of `Vec<String>` of
        // `?1`/`?2`/`?N`. libsql binds anonymous `?` parameters in order, so
        // dropping the numbered form changes nothing for the driver. Large
        // BFS frontiers (`traverse_bfs` calls this once per level) hit this
        // path often enough that the per-id `format!` allocations showed up
        // on profiles.
        let placeholders = build_qmark_placeholders(ids.len());
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes WHERE id IN ({placeholders})",
        );
        let param_values: Vec<libsql::Value> = ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to batch query nodes: {e}"),
                operation: "get_nodes_by_ids".to_string(),
            })?;
        collect_rows(&mut rows, row_to_node, "get_nodes_by_ids").await
    }

    /// Returns all nodes for a given file, ordered by start line.
    pub async fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE file_path = ?1 ORDER BY start_line",
                params![file_path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query nodes by file: {e}"),
                operation: "get_nodes_by_file".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_by_file").await
    }

    /// Returns every node whose `parent_id` matches `parent_id`. Replaces
    /// the v8 pattern of querying outgoing `Contains` edges; after v9 the
    /// edges table no longer carries that information.
    pub async fn get_children_of(&self, parent_id: &str) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE parent_id = ?1 ORDER BY start_line",
                params![parent_id],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query children: {e}"),
                operation: "get_children_of".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_children_of").await
    }

    /// Returns all nodes of a given kind.
    pub async fn get_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes WHERE kind = ?1",
                params![kind.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query nodes by kind: {e}"),
                operation: "get_nodes_by_kind".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_by_kind").await
    }

    /// Returns every node in the database.
    pub async fn get_all_nodes(&self) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all nodes: {e}"),
                operation: "get_all_nodes".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_all_nodes").await
    }

    /// Deletes all nodes (and cascading edges, unresolved refs, vectors) for a file.
    pub async fn delete_nodes_by_file(&self, file_path: &str) -> Result<()> {
        debug_assert!(
            !file_path.is_empty(),
            "delete_nodes_by_file called with empty file_path"
        );
        debug_assert!(
            !file_path.starts_with('/'),
            "delete_nodes_by_file expects relative path, got absolute"
        );
        // Gather node IDs for the file first.
        let node_ids: Vec<String> = {
            let mut rows = self
                .conn()
                .query(
                    "SELECT id FROM nodes WHERE file_path = ?1",
                    params![file_path],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query node ids: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?;

            let mut ids = Vec::new();
            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read node id: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })? {
                ids.push(row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read node id value: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?);
            }
            ids
        };

        if node_ids.is_empty() {
            return Ok(());
        }

        let tx = self
            .conn()
            .transaction()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

        for id in &node_ids {
            tx.execute(
                "DELETE FROM edges WHERE source = ?1 OR target = ?1",
                params![id.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete edges: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

            tx.execute(
                "DELETE FROM unresolved_refs WHERE from_node_id = ?1",
                params![id.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete unresolved refs: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

            tx.execute(
                "DELETE FROM vectors WHERE node_id = ?1",
                params![id.as_str()],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete vectors: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;
        }

        tx.execute("DELETE FROM nodes WHERE file_path = ?1", params![file_path])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete nodes: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

        tx.commit().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "delete_nodes_by_file".to_string(),
        })
    }
}
