//! Node CRUD queries.
use super::*;
use std::fmt::Write as _;

// ---------------------------------------------------------------------------
// Node operations
// ---------------------------------------------------------------------------

/// A predicate for [`Database::get_nodes_filtered`], built by the caller
/// instead of applied to a materialised `Vec<Node>` in Rust.
///
/// Six MCP handlers used to call `get_all_nodes()` and filter the result,
/// so a single tool call materialised the whole node table to keep a
/// fraction of it (#410). Each field here is one of the predicates they
/// applied; an empty filter is exactly `get_all_nodes()`.
#[derive(Debug, Clone, Default)]
pub struct NodeFilter {
    path_prefix: Option<String>,
    kinds: Option<Vec<&'static str>>,
    public_only: bool,
    min_lines: Option<u32>,
    name_contains: Option<String>,
}

/// Escapes a string for use inside a `LIKE` pattern.
///
/// Without this, a path containing `_` or `%` acts as a wildcard: a filter for
/// the directory `a_b` would also match `axb`. That is a wrong-results bug
/// rather than an error, so it fails silently — which is why the escaping is
/// paired with a test rather than left to review. `\` is the escape character,
/// declared with `ESCAPE` at each use site.
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

impl NodeFilter {
    /// An unconstrained filter, equivalent to selecting every node.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts to a file path, or to a directory containing it.
    ///
    /// Matches the exact path, or the path plus a `/` separator, mirroring the
    /// rule the handlers each wrote by hand — so `src` never matches `srcfoo`.
    #[must_use]
    pub fn path_prefix(mut self, prefix: &str) -> Self {
        self.path_prefix = Some(prefix.to_string());
        self
    }

    /// Restricts to a set of node kinds.
    #[must_use]
    pub fn kinds(mut self, kinds: &[NodeKind]) -> Self {
        self.kinds = Some(kinds.iter().map(NodeKind::as_str).collect());
        self
    }

    /// Restricts to publicly visible nodes.
    #[must_use]
    pub fn public_only(mut self) -> Self {
        self.public_only = true;
        self
    }

    /// Restricts to nodes spanning at least `lines` lines, inclusive of both
    /// endpoints — the same arithmetic the redundancy handler used.
    #[must_use]
    pub fn min_lines(mut self, lines: u32) -> Self {
        self.min_lines = Some(lines);
        self
    }

    /// Restricts to nodes whose name or qualified name contains `needle`,
    /// case-insensitively.
    #[must_use]
    pub fn name_contains(mut self, needle: &str) -> Self {
        self.name_contains = Some(needle.to_ascii_lowercase());
        self
    }

    /// Renders the `WHERE` clause, or an empty string when unconstrained.
    ///
    /// Values are inlined rather than bound because the kind list is variadic
    /// and libsql's `params!` is fixed-arity; every inlined value therefore
    /// goes through `push_quoted` or is an integer, and every `LIKE` pattern
    /// through [`escape_like`] first.
    fn where_clause(&self) -> String {
        let mut clauses: Vec<String> = Vec::new();

        if let Some(prefix) = &self.path_prefix {
            let exact = prefix.trim_end_matches('/');
            let mut sql = String::from("(file_path = ");
            push_quoted(&mut sql, exact);
            sql.push_str(" OR file_path LIKE ");
            push_quoted(&mut sql, &format!("{}/%", escape_like(exact)));
            sql.push_str(" ESCAPE '\\')");
            clauses.push(sql);
        }

        if let Some(kinds) = &self.kinds {
            if kinds.is_empty() {
                // An explicit empty set matches nothing; without this the
                // `IN ()` below would be a syntax error.
                return " WHERE 0".to_string();
            }
            let mut sql = String::from("kind IN (");
            for (i, kind) in kinds.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                push_quoted(&mut sql, kind);
            }
            sql.push(')');
            clauses.push(sql);
        }

        if self.public_only {
            let mut sql = String::from("visibility = ");
            push_quoted(&mut sql, Visibility::Pub.as_str());
            clauses.push(sql);
        }

        if let Some(lines) = self.min_lines {
            // `end_line - start_line + 1`, matching the handler. Written with
            // the subtraction on the right so a node whose end precedes its
            // start cannot underflow into a huge span the way saturating
            // integer arithmetic would.
            clauses.push(format!("(end_line + 1 - start_line) >= {lines}"));
        }

        if let Some(needle) = &self.name_contains {
            let pattern = format!("%{}%", escape_like(needle));
            let mut sql = String::from("(LOWER(name) LIKE ");
            push_quoted(&mut sql, &pattern);
            sql.push_str(" ESCAPE '\\' OR LOWER(qualified_name) LIKE ");
            push_quoted(&mut sql, &pattern);
            sql.push_str(" ESCAPE '\\')");
            clauses.push(sql);
        }

        if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        }
    }
}

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
                 unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands, search_terms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)",
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
                    crate::text::search_terms(&node.name, &node.qualified_name),
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
                 unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands,search_terms) VALUES ",
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
                sql.push(',');
                push_quoted(
                    &mut sql,
                    &crate::text::search_terms(&node.name, &node.qualified_name),
                );
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
                 unsafe_blocks,unchecked_calls,assertions,updated_at,attrs_start_line,parent_id,cognitive_complexity,distinct_operators,distinct_operands,total_operators,total_operands,search_terms) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29)"
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
                crate::text::search_terms(&node.name, &node.qualified_name),
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

    /// Returns children of many parent scopes in one query.
    pub async fn get_children_of_many(&self, parent_ids: &[String]) -> Result<Vec<Node>> {
        if parent_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = build_qmark_placeholders(parent_ids.len());
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes WHERE parent_id IN ({placeholders}) ORDER BY file_path, start_line"
        );
        let values = parent_ids
            .iter()
            .cloned()
            .map(libsql::Value::Text)
            .collect::<Vec<_>>();
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query children for parent set: {e}"),
                operation: "get_children_of_many".to_string(),
            })?;
        collect_rows(&mut rows, row_to_node, "get_children_of_many").await
    }

    /// Resolves a method in an impl block to same-named methods on the trait
    /// implemented by that block, using one indexed join.
    pub async fn get_trait_methods_for_impl_method(
        &self,
        impl_id: &str,
        method_name: &str,
    ) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT method.id, method.kind, method.name, method.qualified_name, method.file_path,
                        method.start_line, method.end_line, method.start_column, method.end_column,
                        method.docstring, method.signature, method.visibility, method.is_async,
                        method.branches, method.loops, method.returns, method.max_nesting,
                        method.unsafe_blocks, method.unchecked_calls, method.assertions,
                        method.updated_at, method.attrs_start_line, method.parent_id,
                        method.cognitive_complexity, method.distinct_operators,
                        method.distinct_operands, method.total_operators, method.total_operands
                 FROM edges dispatch
                 JOIN nodes trait ON trait.id = dispatch.target AND trait.kind = 'trait'
                 JOIN nodes method ON method.parent_id = trait.id
                 WHERE dispatch.source = ?1
                   AND dispatch.kind = 'implements'
                   AND method.name = ?2
                   AND method.kind IN ('method', 'function')
                 ORDER BY method.file_path, method.start_line",
                params![impl_id, method_name],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to resolve reverse trait dispatch: {e}"),
                operation: "get_trait_methods_for_impl_method".to_string(),
            })?;
        collect_rows(&mut rows, row_to_node, "get_trait_methods_for_impl_method").await
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

    /// Replaces the recorded ambiguities for a set of files (#412).
    ///
    /// Scoped by file so a sync refreshes only what it re-resolved rather than
    /// clearing the whole table, matching how nodes and edges are replaced per
    /// file. Correct only for a pass that re-resolved *every* reference in
    /// those files — see [`Self::replace_ambiguous_calls_for_refs`] for the
    /// incremental case, where it is not.
    pub async fn replace_ambiguous_calls(
        &self,
        files: &[String],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        for file in files {
            self.conn()
                .execute(
                    "DELETE FROM ambiguous_calls WHERE file_path = ?1",
                    params![file.as_str()],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to clear ambiguous calls: {e}"),
                    operation: "replace_ambiguous_calls".to_string(),
                })?;
        }
        self.insert_ambiguous_calls(calls).await
    }

    /// Replaces the recorded ambiguities for a set of *references* (#484).
    ///
    /// An incremental resolution pass re-attempts a subset of a file's
    /// references, so it cannot clear that file: `replace_ambiguous_calls`
    /// would delete the records of every reference in the file and put back
    /// only the ones this pass looked at. That is not hypothetical — it is what
    /// `tests/incremental_resolution_test.rs` caught, an `area` ambiguity in an
    /// untouched caller vanishing because a sibling reference in the same file
    /// was re-resolved.
    ///
    /// So the delete is keyed the same way the insert is: by the reference's
    /// own identity. Every re-attempted reference has its record cleared,
    /// whether or not it produced a new one, and only those.
    pub async fn replace_ambiguous_calls_for_refs(
        &self,
        refs: &[AmbiguityRefKey],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        if refs.is_empty() {
            return self.insert_ambiguous_calls(calls).await;
        }

        // One prepared statement inside one transaction, as `insert_unresolved_refs`
        // does: this loop is per re-attempted reference rather than per file, so
        // a sync that touches a lot of names would otherwise pay a round trip
        // and an implicit transaction for each one.
        let err = |e: libsql::Error, what: &str| TokenSaveError::Database {
            message: format!("failed to {what}: {e}"),
            operation: "replace_ambiguous_calls_for_refs".to_string(),
        };
        self.conn()
            .execute("BEGIN", ())
            .await
            .map_err(|e| err(e, "begin"))?;
        let stmt = self
            .conn()
            .prepare(
                "DELETE FROM ambiguous_calls
                 WHERE from_node_id = ?1 AND reference_name = ?2
                   AND file_path = ?3 AND line = ?4",
            )
            .await
            .map_err(|e| err(e, "prepare"))?;
        for key in refs {
            stmt.execute(params![
                key.from_node_id.as_str(),
                key.reference_name.as_str(),
                key.file_path.as_str(),
                i64::from(key.line)
            ])
            .await
            .map_err(|e| err(e, "clear ambiguous call"))?;
            stmt.reset();
        }
        self.conn()
            .execute("COMMIT", ())
            .await
            .map_err(|e| err(e, "commit"))?;

        self.insert_ambiguous_calls(calls).await
    }

    async fn insert_ambiguous_calls(&self, calls: &[AmbiguousCall]) -> Result<()> {
        for call in calls {
            let encoded = serde_json::to_string(&call.candidate_node_ids).map_err(|e| {
                TokenSaveError::Database {
                    message: format!("failed to encode ambiguity candidates: {e}"),
                    operation: "replace_ambiguous_calls".to_string(),
                }
            })?;
            self.conn()
                .execute(
                    "INSERT OR REPLACE INTO ambiguous_calls
                     (from_node_id, reference_name, file_path, line, candidate_node_ids)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        call.from_node_id.as_str(),
                        call.reference_name.as_str(),
                        call.file_path.as_str(),
                        i64::from(call.line),
                        encoded.as_str()
                    ],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to record ambiguous call: {e}"),
                    operation: "replace_ambiguous_calls".to_string(),
                })?;
        }
        Ok(())
    }

    /// Recorded ambiguous calls, optionally scoped to a path prefix.
    ///
    /// `limit` is a hard cap rather than a page: a large codebase can carry
    /// thousands of ambiguous sites, and handing all of them back would swamp
    /// the caller this feature exists to help.
    pub async fn get_ambiguous_calls(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AmbiguousCall>> {
        let mut sql = String::from(
            "SELECT from_node_id, reference_name, file_path, line, candidate_node_ids
             FROM ambiguous_calls",
        );
        if let Some(prefix) = path_prefix {
            let exact = prefix.trim_end_matches('/');
            sql.push_str(" WHERE (file_path = ");
            push_quoted(&mut sql, exact);
            sql.push_str(" OR file_path LIKE ");
            push_quoted(
                &mut sql,
                &format!(
                    "{}/%",
                    exact
                        .replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                ),
            );
            sql.push_str(" ESCAPE '\\')");
        }
        let _ = write!(sql, " ORDER BY file_path, line LIMIT {limit}");

        let mut rows = self
            .conn()
            .query(&sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query ambiguous calls: {e}"),
                operation: "get_ambiguous_calls".to_string(),
            })?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read ambiguous call row: {e}"),
            operation: "get_ambiguous_calls".to_string(),
        })? {
            let encoded = get_string_lossy(&row, 4).unwrap_or_default();
            out.push(AmbiguousCall {
                from_node_id: get_string_lossy(&row, 0).unwrap_or_default(),
                reference_name: get_string_lossy(&row, 1).unwrap_or_default(),
                file_path: get_string_lossy(&row, 2).unwrap_or_default(),
                line: row.get::<i64>(3).unwrap_or(0) as u32,
                candidate_node_ids: serde_json::from_str(&encoded).unwrap_or_default(),
            });
        }
        Ok(out)
    }

    /// Node ids named as a candidate by any recorded ambiguity (#412).
    ///
    /// `dead_code` uses this: having refused to fabricate an edge for an
    /// ambiguous call, reporting its candidates as uncalled would trade a
    /// fabricated edge for a fabricated finding — and a finding reads as more
    /// actionable than an edge. "Referenced, target unknown" is not "dead".
    pub async fn ambiguous_candidate_ids(&self) -> Result<HashSet<String>> {
        let mut rows = self
            .conn()
            .query("SELECT candidate_node_ids FROM ambiguous_calls", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query ambiguity candidates: {e}"),
                operation: "ambiguous_candidate_ids".to_string(),
            })?;
        let mut out = HashSet::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read ambiguity candidate row: {e}"),
            operation: "ambiguous_candidate_ids".to_string(),
        })? {
            let encoded = get_string_lossy(&row, 0).unwrap_or_default();
            let ids: Vec<String> = serde_json::from_str(&encoded).unwrap_or_default();
            out.extend(ids);
        }
        Ok(out)
    }

    /// Every node's id paired with its file path, without building a `Node`.
    ///
    /// `handle_test_risk` needs this mapping for the *whole* graph — it walks
    /// every edge, and an edge can point anywhere, so a test in `tests/`
    /// calling a function in `src/` is exactly what it is looking for. The map
    /// therefore cannot be scoped the way the predicates in
    /// [`Self::get_nodes_filtered`] can (#411).
    ///
    /// What it can avoid is the other twenty-six columns: materialising a
    /// `Node` per row costs 248 bytes of struct plus its unbounded `signature`
    /// and `docstring` strings, to keep two short ones. Asserted equal to the
    /// map built from `get_all_nodes` in `tests/node_filter_test.rs`.
    pub async fn get_node_paths(&self) -> Result<HashMap<String, String>> {
        let mut rows = self
            .conn()
            .query("SELECT id, file_path FROM nodes", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node paths: {e}"),
                operation: "get_node_paths".to_string(),
            })?;

        let mut map = HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read node path row: {e}"),
            operation: "get_node_paths".to_string(),
        })? {
            let id = get_string_lossy(&row, 0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read node id: {e}"),
                operation: "get_node_paths".to_string(),
            })?;
            let path = get_string_lossy(&row, 1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read node file_path: {e}"),
                operation: "get_node_paths".to_string(),
            })?;
            map.insert(id, path);
        }
        Ok(map)
    }

    /// Every node matching `filter`, evaluated in SQL rather than in Rust.
    ///
    /// The handlers this replaces each loaded the whole node table and kept a
    /// fraction of it, so one tool call on a large project re-triggered a
    /// graph-sized allocation on a *read* (#410). An empty [`NodeFilter`] is
    /// equivalent to [`Self::get_all_nodes`], so a handler with nothing to
    /// scope by keeps working.
    ///
    /// Behaviour is asserted equal to the in-Rust filters in
    /// `tests/node_filter_test.rs` — these predicates are user-visible through
    /// `redundancy`, `module_api`, `unused_imports`, `gini`, `health` and
    /// `literal_search`, so a subtly different one would silently change what
    /// those tools report rather than fail.
    pub async fn get_nodes_filtered(&self, filter: &NodeFilter) -> Result<Vec<Node>> {
        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path,
                start_line, end_line, start_column, end_column,
                docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
             FROM nodes{}",
            filter.where_clause()
        );
        let mut rows = self
            .conn()
            .query(&sql, ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query filtered nodes: {e}"),
                operation: "get_nodes_filtered".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_nodes_filtered").await
    }

    /// Every node, without the two columns resolution never reads (#306).
    ///
    /// `docstring` and `signature` are unbounded TEXT — an extractor stores a
    /// `const`'s whole initializer in `signature`, which #362 found reaching
    /// roughly 43 KB for one node — and they dominate a `Node`'s heap
    /// footprint. `ReferenceResolver` reads only `id`, `kind`, `name`,
    /// `qualified_name`, `file_path`, `start_line`, `visibility` and
    /// `parent_id`, yet the whole `Vec<Node>` stays resident for the entire
    /// resolution pass, so those two columns were pure peak.
    ///
    /// The resolver borrows from that slice for its lifetime and needs a
    /// global name index to resolve cross-file references, so the pass cannot
    /// be chunked or streamed without a redesign. Narrowing what each node
    /// carries is the part that can be done without changing behaviour.
    ///
    /// `NULL` placeholders keep the column *positions* identical so
    /// [`row_to_node`] is shared rather than duplicated; the two fields come
    /// back as `None`. Use [`Self::get_all_nodes`] anywhere the text is
    /// actually needed — `tests/resolution_slim_nodes_test.rs` asserts the two
    /// loads resolve identically, so a future resolver change that starts
    /// reading either field fails loudly instead of silently seeing `None`.
    pub async fn get_all_nodes_for_resolution(&self) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    NULL AS docstring, NULL AS signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at, attrs_start_line, parent_id, cognitive_complexity, distinct_operators, distinct_operands, total_operators, total_operands
                 FROM nodes",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all nodes for resolution: {e}"),
                operation: "get_all_nodes_for_resolution".to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, "get_all_nodes_for_resolution").await
    }

    /// The identifying fields of a file's nodes, for the incremental-resolution
    /// touched-name set (#484).
    ///
    /// Called immediately before `delete_nodes_by_file` re-extracts the file:
    /// the nodes about to disappear take their in-edges with them, so their
    /// name-index keys have to be recorded while the rows still exist. Three
    /// columns rather than the whole row, because that is all
    /// `resolution::touched::index_keys` reads.
    pub async fn touched_nodes_by_file(&self, file_path: &str) -> Result<Vec<TouchedNode>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT kind, name, qualified_name FROM nodes WHERE file_path = ?1",
                params![file_path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node names: {e}"),
                operation: "touched_nodes_by_file".to_string(),
            })?;

        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let kind = NodeKind::from_str(&row.get::<String>(0).unwrap_or_default())
                .unwrap_or(NodeKind::Function);
            out.push(TouchedNode {
                kind,
                name: row.get::<String>(1).unwrap_or_default(),
                qualified_name: row.get::<String>(2).unwrap_or_default(),
            });
        }
        Ok(out)
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
        self.conn()
            .execute(
                "DELETE FROM executable_body_fts WHERE file_path = ?1",
                params![file_path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete executable body documents: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

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
