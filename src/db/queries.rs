// Rust guideline compliant 2025-10-17
use std::collections::HashMap;

use libsql::params;

use super::connection::Database;
use crate::errors::{TokenSaveError, Result};
use crate::types::*;

// ---------------------------------------------------------------------------
// Helper: map a libsql row to domain types (by column index)
// ---------------------------------------------------------------------------

/// Maps a row from the `nodes` table to a `Node`.
///
/// Expected column order: id(0), kind(1), name(2), qualified_name(3),
/// file_path(4), start_line(5), end_line(6), start_column(7), end_column(8),
/// docstring(9), signature(10), visibility(11), is_async(12),
/// branches(13), loops(14), returns(15), max_nesting(16),
/// unsafe_blocks(17), unchecked_calls(18), assertions(19), updated_at(20).
fn row_to_node(row: &libsql::Row) -> std::result::Result<Node, libsql::Error> {
    let kind_str = row.get::<String>(1)?;
    let vis_str = row.get::<String>(11)?;
    let is_async_int = row.get::<i64>(12)?;

    Ok(Node {
        id: row.get::<String>(0)?,
        kind: NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Function),
        name: row.get::<String>(2)?,
        qualified_name: row.get::<String>(3)?,
        file_path: row.get::<String>(4)?,
        start_line: row.get::<u32>(5)?,
        end_line: row.get::<u32>(6)?,
        start_column: row.get::<u32>(7)?,
        end_column: row.get::<u32>(8)?,
        signature: row.get::<Option<String>>(10)?,
        docstring: row.get::<Option<String>>(9)?,
        visibility: Visibility::from_str(&vis_str).unwrap_or_default(),
        is_async: is_async_int != 0,
        branches: row.get::<u32>(13)?,
        loops: row.get::<u32>(14)?,
        returns: row.get::<u32>(15)?,
        max_nesting: row.get::<u32>(16)?,
        unsafe_blocks: row.get::<u32>(17)?,
        unchecked_calls: row.get::<u32>(18)?,
        assertions: row.get::<u32>(19)?,
        updated_at: row.get::<u64>(20)?,
    })
}

/// Maps a row from the `edges` table to an `Edge`.
///
/// Expected column order: source(0), target(1), kind(2), line(3).
fn row_to_edge(row: &libsql::Row) -> std::result::Result<Edge, libsql::Error> {
    let kind_str = row.get::<String>(2)?;
    let line = row.get::<Option<u32>>(3)?;

    Ok(Edge {
        source: row.get::<String>(0)?,
        target: row.get::<String>(1)?,
        kind: EdgeKind::from_str(&kind_str).unwrap_or(EdgeKind::Uses),
        line,
    })
}

/// Maps a row from the `files` table to a `FileRecord`.
///
/// Expected column order: path(0), content_hash(1), size(2), modified_at(3),
/// indexed_at(4), node_count(5).
fn row_to_file(row: &libsql::Row) -> std::result::Result<FileRecord, libsql::Error> {
    Ok(FileRecord {
        path: row.get::<String>(0)?,
        content_hash: row.get::<String>(1)?,
        size: row.get::<u64>(2)?,
        modified_at: row.get::<i64>(3)?,
        indexed_at: row.get::<i64>(4)?,
        node_count: row.get::<u32>(5)?,
    })
}

/// Maps a row from the `unresolved_refs` table to an `UnresolvedRef`.
///
/// Expected column order: from_node_id(0), reference_name(1),
/// reference_kind(2), line(3), col(4), file_path(5).
fn row_to_unresolved_ref(
    row: &libsql::Row,
) -> std::result::Result<UnresolvedRef, libsql::Error> {
    let kind_str = row.get::<String>(2)?;

    Ok(UnresolvedRef {
        from_node_id: row.get::<String>(0)?,
        reference_name: row.get::<String>(1)?,
        reference_kind: EdgeKind::from_str(&kind_str).unwrap_or(EdgeKind::Uses),
        line: row.get::<u32>(3)?,
        column: row.get::<u32>(4)?,
        file_path: row.get::<String>(5)?,
    })
}

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
                 unsafe_blocks, unchecked_calls, assertions, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                params![
                    node.id.as_str(),
                    node.kind.as_str(),
                    node.name.as_str(),
                    node.qualified_name.as_str(),
                    node.file_path.as_str(),
                    node.start_line as i64,
                    node.end_line as i64,
                    node.start_column as i64,
                    node.end_column as i64,
                    opt_str(&node.docstring),
                    opt_str(&node.signature),
                    node.visibility.as_str(),
                    node.is_async as i64,
                    node.branches as i64,
                    node.loops as i64,
                    node.returns as i64,
                    node.max_nesting as i64,
                    node.unsafe_blocks as i64,
                    node.unchecked_calls as i64,
                    node.assertions as i64,
                    node.updated_at as i64,
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert node: {e}"),
                operation: "insert_node".to_string(),
            })?;
        Ok(())
    }

    /// Inserts or replaces a batch of nodes inside a single transaction.
    pub async fn insert_nodes(&self, nodes: &[Node]) -> Result<()> {
        let tx = self
            .conn()
            .transaction()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "insert_nodes".to_string(),
            })?;

        for node in nodes {
            tx.execute(
                "INSERT OR REPLACE INTO nodes
                (id, kind, name, qualified_name, file_path,
                 start_line, end_line, start_column, end_column,
                 docstring, signature, visibility, is_async,
                 branches, loops, returns, max_nesting,
                 unsafe_blocks, unchecked_calls, assertions, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                params![
                    node.id.as_str(),
                    node.kind.as_str(),
                    node.name.as_str(),
                    node.qualified_name.as_str(),
                    node.file_path.as_str(),
                    node.start_line as i64,
                    node.end_line as i64,
                    node.start_column as i64,
                    node.end_column as i64,
                    opt_str(&node.docstring),
                    opt_str(&node.signature),
                    node.visibility.as_str(),
                    node.is_async as i64,
                    node.branches as i64,
                    node.loops as i64,
                    node.returns as i64,
                    node.max_nesting as i64,
                    node.unsafe_blocks as i64,
                    node.unchecked_calls as i64,
                    node.assertions as i64,
                    node.updated_at as i64,
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert node: {e}"),
                operation: "insert_nodes".to_string(),
            })?;
        }

        tx.commit().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "insert_nodes".to_string(),
        })
    }

    /// Retrieves a node by its unique ID, returning `None` if not found.
    pub async fn get_node_by_id(&self, id: &str) -> Result<Option<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at
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

    /// Returns all nodes for a given file, ordered by start line.
    pub async fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at
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

    /// Returns all nodes of a given kind.
    pub async fn get_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<Node>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at
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
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at
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
        // Gather node IDs for the file first.
        let node_ids: Vec<String> = {
            let mut rows = self
                .conn()
                .query("SELECT id FROM nodes WHERE file_path = ?1", params![file_path])
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

            tx.execute("DELETE FROM vectors WHERE node_id = ?1", params![id.as_str()])
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to delete vectors: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?;
        }

        tx.execute(
            "DELETE FROM nodes WHERE file_path = ?1",
            params![file_path],
        )
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

// ---------------------------------------------------------------------------
// Edge operations
// ---------------------------------------------------------------------------

impl Database {
    /// Inserts a single edge.
    pub async fn insert_edge(&self, edge: &Edge) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO edges (source, target, kind, line) VALUES (?1, ?2, ?3, ?4)",
                params![
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.kind.as_str(),
                    edge.line.map(|l| l as i64)
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert edge: {e}"),
                operation: "insert_edge".to_string(),
            })?;
        Ok(())
    }

    /// Inserts a batch of edges inside a single transaction.
    pub async fn insert_edges(&self, edges: &[Edge]) -> Result<()> {
        let tx = self
            .conn()
            .transaction()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "insert_edges".to_string(),
            })?;

        for edge in edges {
            tx.execute(
                "INSERT INTO edges (source, target, kind, line) VALUES (?1, ?2, ?3, ?4)",
                params![
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.kind.as_str(),
                    edge.line.map(|l| l as i64)
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert edge: {e}"),
                operation: "insert_edges".to_string(),
            })?;
        }

        tx.commit().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "insert_edges".to_string(),
        })
    }

    /// Returns outgoing edges from a source node, optionally filtered by edge kinds.
    ///
    /// If `kinds` is empty, all outgoing edges are returned.
    pub async fn get_outgoing_edges(
        &self,
        source_id: &str,
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut rows = self
                .conn()
                .query(
                    "SELECT source, target, kind, line FROM edges WHERE source = ?1",
                    params![source_id],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query outgoing edges: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_outgoing_edges").await
        } else {
            let placeholders: Vec<String> = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 2))
                .collect();
            let sql = format!(
                "SELECT source, target, kind, line FROM edges WHERE source = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );

            let mut param_values: Vec<libsql::Value> = Vec::new();
            param_values.push(libsql::Value::Text(source_id.to_string()));
            for k in kinds {
                param_values.push(libsql::Value::Text(k.as_str().to_string()));
            }

            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(param_values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query outgoing edges: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_outgoing_edges").await
        }
    }

    /// Returns incoming edges to a target node, optionally filtered by edge kinds.
    ///
    /// If `kinds` is empty, all incoming edges are returned.
    pub async fn get_incoming_edges(
        &self,
        target_id: &str,
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut rows = self
                .conn()
                .query(
                    "SELECT source, target, kind, line FROM edges WHERE target = ?1",
                    params![target_id],
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query incoming edges: {e}"),
                    operation: "get_incoming_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_incoming_edges").await
        } else {
            let placeholders: Vec<String> = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 2))
                .collect();
            let sql = format!(
                "SELECT source, target, kind, line FROM edges WHERE target = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );

            let mut param_values: Vec<libsql::Value> = Vec::new();
            param_values.push(libsql::Value::Text(target_id.to_string()));
            for k in kinds {
                param_values.push(libsql::Value::Text(k.as_str().to_string()));
            }

            let mut rows = self
                .conn()
                .query(&sql, libsql::params_from_iter(param_values))
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query incoming edges: {e}"),
                    operation: "get_incoming_edges".to_string(),
                })?;

            collect_rows(&mut rows, row_to_edge, "get_incoming_edges").await
        }
    }

    /// Returns nodes ranked by edge count for a given edge kind and direction,
    /// optionally filtered by node kind.
    ///
    /// When `incoming` is true, ranks target nodes by incoming edge count
    /// (e.g. "most implemented interface"). When false, ranks source nodes
    /// by outgoing edge count (e.g. "class that implements the most interfaces").
    ///
    /// The query is performed entirely in SQL for efficiency — no need to load
    /// all edges into memory. Results are ordered by count descending.
    pub async fn get_ranked_nodes_by_edge_kind(
        &self,
        edge_kind: &EdgeKind,
        node_kind: Option<&NodeKind>,
        incoming: bool,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        let (join_col, group_col) = if incoming {
            ("e.target", "e.target")
        } else {
            ("e.source", "e.source")
        };

        let (sql, param_values): (String, Vec<libsql::Value>) = match node_kind {
            Some(nk) => (
                format!(
                    "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                            n.start_line, n.end_line, n.start_column, n.end_column,
                            n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at,
                            COUNT(*) AS cnt
                     FROM edges e
                     JOIN nodes n ON {join_col} = n.id
                     WHERE e.kind = ?1 AND n.kind = ?2
                     GROUP BY {group_col}
                     ORDER BY cnt DESC
                     LIMIT ?3"
                ),
                vec![
                    libsql::Value::Text(edge_kind.as_str().to_string()),
                    libsql::Value::Text(nk.as_str().to_string()),
                    libsql::Value::Integer(limit as i64),
                ],
            ),
            None => (
                format!(
                    "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                            n.start_line, n.end_line, n.start_column, n.end_column,
                            n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at,
                            COUNT(*) AS cnt
                     FROM edges e
                     JOIN nodes n ON {join_col} = n.id
                     WHERE e.kind = ?1
                     GROUP BY {group_col}
                     ORDER BY cnt DESC
                     LIMIT ?2"
                ),
                vec![
                    libsql::Value::Text(edge_kind.as_str().to_string()),
                    libsql::Value::Integer(limit as i64),
                ],
            ),
        };

        let op = "get_ranked_nodes_by_edge_kind";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query ranked nodes: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let count = row.get::<u64>(21).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read count column: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, count));
        }

        Ok(items)
    }

    /// Returns nodes ranked by line span (end_line - start_line + 1), optionally
    /// filtered by node kind. Results are ordered by size descending.
    pub async fn get_largest_nodes(
        &self,
        node_kind: Option<&NodeKind>,
        limit: usize,
    ) -> Result<Vec<(Node, u32)>> {
        let (sql, param_values): (String, Vec<libsql::Value>) = match node_kind {
            Some(nk) => (
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at,
                        (end_line - start_line + 1) AS lines
                 FROM nodes
                 WHERE kind = ?1
                 ORDER BY lines DESC
                 LIMIT ?2"
                    .to_string(),
                vec![
                    libsql::Value::Text(nk.as_str().to_string()),
                    libsql::Value::Integer(limit as i64),
                ],
            ),
            None => (
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at,
                        (end_line - start_line + 1) AS lines
                 FROM nodes
                 ORDER BY lines DESC
                 LIMIT ?1"
                    .to_string(),
                vec![libsql::Value::Integer(limit as i64)],
            ),
        };

        let op = "get_largest_nodes";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query largest nodes: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let lines = row.get::<u32>(21).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read lines column: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, lines));
        }

        Ok(items)
    }

    /// Returns files ranked by coupling (number of distinct other files connected
    /// via cross-file edges). `fan_in` mode counts how many files depend on each
    /// file; `fan_out` counts how many files each file depends on.
    ///
    /// Only `calls`, `uses`, `implements`, and `extends` edges are considered.
    pub async fn get_file_coupling(
        &self,
        fan_in: bool,
        limit: usize,
    ) -> Result<Vec<(String, u64)>> {
        let sql = if fan_in {
            "SELECT n_tgt.file_path, COUNT(DISTINCT n_src.file_path) AS coupling
             FROM edges e
             JOIN nodes n_src ON e.source = n_src.id
             JOIN nodes n_tgt ON e.target = n_tgt.id
             WHERE e.kind IN ('calls', 'uses', 'implements', 'extends')
               AND n_src.file_path != n_tgt.file_path
             GROUP BY n_tgt.file_path
             ORDER BY coupling DESC
             LIMIT ?1"
        } else {
            "SELECT n_src.file_path, COUNT(DISTINCT n_tgt.file_path) AS coupling
             FROM edges e
             JOIN nodes n_src ON e.source = n_src.id
             JOIN nodes n_tgt ON e.target = n_tgt.id
             WHERE e.kind IN ('calls', 'uses', 'implements', 'extends')
               AND n_src.file_path != n_tgt.file_path
             GROUP BY n_src.file_path
             ORDER BY coupling DESC
             LIMIT ?1"
        };

        let op = "get_file_coupling";
        let mut rows = self
            .conn()
            .query(sql, params![limit as i64])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file coupling: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let file_path = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read file_path: {e}"),
                operation: op.to_string(),
            })?;
            let count = row.get::<u64>(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read coupling count: {e}"),
                operation: op.to_string(),
            })?;
            items.push((file_path, count));
        }

        Ok(items)
    }

    /// Returns the maximum inheritance depth for classes/interfaces reachable
    /// via `extends` edges. Uses a recursive CTE to walk the hierarchy.
    ///
    /// Each result is a (leaf_node, depth) pair where depth is the number of
    /// `extends` hops from the leaf to the root of its hierarchy.
    pub async fn get_inheritance_depth(&self, limit: usize) -> Result<Vec<(Node, u64)>> {
        let sql =
            "WITH RECURSIVE hierarchy(leaf_id, current_id, depth) AS (
                 SELECT e.source, e.target, 1
                 FROM edges e
                 WHERE e.kind = 'extends'
                 UNION ALL
                 SELECT h.leaf_id, e.target, h.depth + 1
                 FROM hierarchy h
                 JOIN edges e ON e.source = h.current_id AND e.kind = 'extends'
                 WHERE h.depth < 50
             )
             SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at,
                    MAX(h.depth) AS max_depth
             FROM hierarchy h
             JOIN nodes n ON h.leaf_id = n.id
             GROUP BY h.leaf_id
             ORDER BY max_depth DESC
             LIMIT ?1";

        let op = "get_inheritance_depth";
        let mut rows = self
            .conn()
            .query(sql, params![limit as i64])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query inheritance depth: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let depth = row.get::<u64>(21).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read depth column: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, depth));
        }

        Ok(items)
    }

    /// Returns node kind counts grouped by file or directory prefix.
    ///
    /// If `path_prefix` is provided, only files under that path are included.
    /// Results are grouped by (file_path, kind) and ordered by file then count.
    pub async fn get_node_distribution(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, u64)>> {
        let (sql, param_values): (&str, Vec<libsql::Value>) = match path_prefix {
            Some(prefix) => (
                "SELECT file_path, kind, COUNT(*) AS cnt
                 FROM nodes
                 WHERE file_path LIKE ?1
                 GROUP BY file_path, kind
                 ORDER BY file_path, cnt DESC",
                vec![libsql::Value::Text(format!("{}%", prefix))],
            ),
            None => (
                "SELECT file_path, kind, COUNT(*) AS cnt
                 FROM nodes
                 GROUP BY file_path, kind
                 ORDER BY file_path, cnt DESC",
                vec![],
            ),
        };

        let op = "get_node_distribution";
        let mut rows = self
            .conn()
            .query(sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query node distribution: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let file_path = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read file_path: {e}"),
                operation: op.to_string(),
            })?;
            let kind = row.get::<String>(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read kind: {e}"),
                operation: op.to_string(),
            })?;
            let count = row.get::<u64>(2).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read count: {e}"),
                operation: op.to_string(),
            })?;
            items.push((file_path, kind, count));
        }

        Ok(items)
    }

    /// Returns all `calls` edges for cycle detection in the call graph.
    ///
    /// Returns `(source_id, target_id)` pairs for every `calls` edge.
    pub async fn get_call_edges(&self) -> Result<Vec<(String, String)>> {
        let op = "get_call_edges";
        let mut rows = self
            .conn()
            .query(
                "SELECT source, target FROM edges WHERE kind = 'calls'",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query call edges: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let source = row.get::<String>(0).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read source: {e}"),
                operation: op.to_string(),
            })?;
            let target = row.get::<String>(1).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read target: {e}"),
                operation: op.to_string(),
            })?;
            items.push((source, target));
        }

        Ok(items)
    }

    /// Returns functions/methods ranked by a composite complexity score.
    ///
    /// Complexity = line_count + (call_fan_out * 3) + call_fan_in.
    /// Line count reflects size, fan-out reflects cognitive load, fan-in
    /// reflects coupling. Results are ordered by score descending.
    pub async fn get_complexity_ranked(
        &self,
        node_kind: Option<&NodeKind>,
        limit: usize,
    ) -> Result<Vec<(Node, u32, u64, u64, u64)>> {
        // line_count, fan_out, fan_in, score
        let (sql, param_values): (String, Vec<libsql::Value>) = match node_kind {
            Some(nk) => (
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                        n.start_line, n.end_line, n.start_column, n.end_column,
                        n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at,
                        (n.end_line - n.start_line + 1) AS lines,
                        COALESCE(out_calls.cnt, 0) AS fan_out,
                        COALESCE(in_calls.cnt, 0) AS fan_in,
                        ((n.end_line - n.start_line + 1) + COALESCE(out_calls.cnt, 0) * 3 + COALESCE(in_calls.cnt, 0)) AS score
                 FROM nodes n
                 LEFT JOIN (SELECT source, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY source) out_calls ON out_calls.source = n.id
                 LEFT JOIN (SELECT target, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY target) in_calls ON in_calls.target = n.id
                 WHERE n.kind = ?1
                 ORDER BY score DESC
                 LIMIT ?2"
                    .to_string(),
                vec![
                    libsql::Value::Text(nk.as_str().to_string()),
                    libsql::Value::Integer(limit as i64),
                ],
            ),
            None => (
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                        n.start_line, n.end_line, n.start_column, n.end_column,
                        n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at,
                        (n.end_line - n.start_line + 1) AS lines,
                        COALESCE(out_calls.cnt, 0) AS fan_out,
                        COALESCE(in_calls.cnt, 0) AS fan_in,
                        ((n.end_line - n.start_line + 1) + COALESCE(out_calls.cnt, 0) * 3 + COALESCE(in_calls.cnt, 0)) AS score
                 FROM nodes n
                 LEFT JOIN (SELECT source, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY source) out_calls ON out_calls.source = n.id
                 LEFT JOIN (SELECT target, COUNT(*) AS cnt FROM edges WHERE kind = 'calls' GROUP BY target) in_calls ON in_calls.target = n.id
                 WHERE n.kind IN ('function', 'method')
                 ORDER BY score DESC
                 LIMIT ?1"
                    .to_string(),
                vec![libsql::Value::Integer(limit as i64)],
            ),
        };

        let op = "get_complexity_ranked";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query complexity ranking: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let lines = row.get::<u32>(21).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read lines: {e}"),
                operation: op.to_string(),
            })?;
            let fan_out = row.get::<u64>(22).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read fan_out: {e}"),
                operation: op.to_string(),
            })?;
            let fan_in = row.get::<u64>(23).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read fan_in: {e}"),
                operation: op.to_string(),
            })?;
            let score = row.get::<u64>(24).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read score: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, lines, fan_out, fan_in, score));
        }

        Ok(items)
    }

    /// Returns public symbols that are missing docstrings.
    ///
    /// Filters to meaningful node kinds (function, method, class, interface,
    /// trait, struct, enum) and checks for NULL or empty docstring.
    pub async fn get_undocumented_public_symbols(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Node>> {
        let (sql, param_values): (String, Vec<libsql::Value>) = match path_prefix {
            Some(prefix) => (
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at
                 FROM nodes
                 WHERE visibility = 'public'
                   AND (docstring IS NULL OR docstring = '')
                   AND kind IN ('function', 'method', 'class', 'interface', 'trait', 'struct', 'enum', 'module')
                   AND file_path LIKE ?1
                 ORDER BY file_path, start_line
                 LIMIT ?2"
                    .to_string(),
                vec![
                    libsql::Value::Text(format!("{}%", prefix)),
                    libsql::Value::Integer(limit as i64),
                ],
            ),
            None => (
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at
                 FROM nodes
                 WHERE visibility = 'public'
                   AND (docstring IS NULL OR docstring = '')
                   AND kind IN ('function', 'method', 'class', 'interface', 'trait', 'struct', 'enum', 'module')
                 ORDER BY file_path, start_line
                 LIMIT ?1"
                    .to_string(),
                vec![libsql::Value::Integer(limit as i64)],
            ),
        };

        let op = "get_undocumented_public_symbols";
        let mut rows = self
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query undocumented symbols: {e}"),
                operation: op.to_string(),
            })?;

        collect_rows(&mut rows, row_to_node, op).await
    }

    /// Returns classes/structs ranked by number of contained members
    /// (methods, fields, constructors). Identifies "god classes" with
    /// excessive responsibility.
    pub async fn get_god_classes(&self, limit: usize) -> Result<Vec<(Node, u64, u64, u64)>> {
        // Returns (node, method_count, field_count, total_members)
        let sql =
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at,
                    SUM(CASE WHEN c.kind IN ('method', 'abstract_method', 'constructor') THEN 1 ELSE 0 END) AS methods,
                    SUM(CASE WHEN c.kind = 'field' THEN 1 ELSE 0 END) AS fields,
                    COUNT(*) AS total
             FROM edges e
             JOIN nodes n ON e.source = n.id
             JOIN nodes c ON e.target = c.id
             WHERE e.kind = 'contains'
               AND n.kind IN ('class', 'struct', 'inner_class', 'object')
             GROUP BY e.source
             ORDER BY total DESC
             LIMIT ?1";

        let op = "get_god_classes";
        let mut rows = self
            .conn()
            .query(sql, params![limit as i64])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query god classes: {e}"),
                operation: op.to_string(),
            })?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: op.to_string(),
        })? {
            let node = row_to_node(&row).map_err(|e| TokenSaveError::Database {
                message: format!("failed to map row: {e}"),
                operation: op.to_string(),
            })?;
            let methods = row.get::<u64>(21).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read methods: {e}"),
                operation: op.to_string(),
            })?;
            let fields = row.get::<u64>(22).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read fields: {e}"),
                operation: op.to_string(),
            })?;
            let total = row.get::<u64>(23).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read total: {e}"),
                operation: op.to_string(),
            })?;
            items.push((node, methods, fields, total));
        }

        Ok(items)
    }

    /// Returns every edge in the database.
    pub async fn get_all_edges(&self) -> Result<Vec<Edge>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT source, target, kind, line FROM edges",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all edges: {e}"),
                operation: "get_all_edges".to_string(),
            })?;

        collect_rows(&mut rows, row_to_edge, "get_all_edges").await
    }

    /// Deletes all edges originating from a given source node.
    pub async fn delete_edges_by_source(&self, source_id: &str) -> Result<()> {
        self.conn()
            .execute(
                "DELETE FROM edges WHERE source = ?1",
                params![source_id],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete edges by source: {e}"),
                operation: "delete_edges_by_source".to_string(),
            })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

impl Database {
    /// Inserts or replaces a file record.
    pub async fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO files
                (path, content_hash, size, modified_at, indexed_at, node_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    file.path.as_str(),
                    file.content_hash.as_str(),
                    file.size as i64,
                    file.modified_at,
                    file.indexed_at,
                    file.node_count as i64,
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to upsert file: {e}"),
                operation: "upsert_file".to_string(),
            })?;
        Ok(())
    }

    /// Retrieves a file record by path, returning `None` if not found.
    pub async fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count
                 FROM files WHERE path = ?1",
                params![path],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file: {e}"),
                operation: "get_file".to_string(),
            })?;

        match rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read file row: {e}"),
            operation: "get_file".to_string(),
        })? {
            Some(row) => {
                let file = row_to_file(&row).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to map file row: {e}"),
                    operation: "get_file".to_string(),
                })?;
                Ok(Some(file))
            }
            None => Ok(None),
        }
    }

    /// Returns all file records.
    pub async fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count FROM files",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query all files: {e}"),
                operation: "get_all_files".to_string(),
            })?;

        collect_rows(&mut rows, row_to_file, "get_all_files").await
    }

    /// Deletes a file record and cascades to delete its nodes first.
    pub async fn delete_file(&self, path: &str) -> Result<()> {
        self.delete_nodes_by_file(path).await?;
        self.conn()
            .execute("DELETE FROM files WHERE path = ?1", params![path])
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to delete file: {e}"),
                operation: "delete_file".to_string(),
            })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Unresolved reference operations
// ---------------------------------------------------------------------------

impl Database {
    /// Inserts a single unresolved reference.
    pub async fn insert_unresolved_ref(&self, uref: &UnresolvedRef) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO unresolved_refs
                (from_node_id, reference_name, reference_kind, line, col, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    uref.from_node_id.as_str(),
                    uref.reference_name.as_str(),
                    uref.reference_kind.as_str(),
                    uref.line as i64,
                    uref.column as i64,
                    uref.file_path.as_str(),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert unresolved ref: {e}"),
                operation: "insert_unresolved_ref".to_string(),
            })?;
        Ok(())
    }

    /// Inserts a batch of unresolved references inside a single transaction.
    pub async fn insert_unresolved_refs(&self, refs: &[UnresolvedRef]) -> Result<()> {
        let tx = self
            .conn()
            .transaction()
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;

        for uref in refs {
            tx.execute(
                "INSERT INTO unresolved_refs
                    (from_node_id, reference_name, reference_kind, line, col, file_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    uref.from_node_id.as_str(),
                    uref.reference_name.as_str(),
                    uref.reference_kind.as_str(),
                    uref.line as i64,
                    uref.column as i64,
                    uref.file_path.as_str(),
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to insert unresolved ref: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            })?;
        }

        tx.commit().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "insert_unresolved_refs".to_string(),
        })
    }

    /// Returns all unresolved references.
    pub async fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
                 FROM unresolved_refs",
                (),
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query unresolved refs: {e}"),
                operation: "get_unresolved_refs".to_string(),
            })?;

        collect_rows(&mut rows, row_to_unresolved_ref, "get_unresolved_refs").await
    }

    /// Removes all unresolved references.
    pub async fn clear_unresolved_refs(&self) -> Result<()> {
        self.conn()
            .execute("DELETE FROM unresolved_refs", ())
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to clear unresolved refs: {e}"),
                operation: "clear_unresolved_refs".to_string(),
            })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

impl Database {
    /// Searches nodes by name, qualified name, docstring, or signature.
    ///
    /// Attempts an FTS5 prefix match first. If no results are found, falls back
    /// to a `LIKE` query.
    pub async fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
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

        let mut rows = self
            .conn()
            .query(
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.branches, n.loops, n.returns, n.max_nesting, n.unsafe_blocks, n.unchecked_calls, n.assertions, n.updated_at,
                    rank
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.rowid
                 WHERE nodes_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
                params![fts_query.as_str(), limit as i64],
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
            let rank: f64 = row.get::<f64>(21).map_err(|e| TokenSaveError::Database {
                message: format!("failed to read rank: {e}"),
                operation: "search_nodes".to_string(),
            })?;
            // FTS5 rank is negative (lower = better match). Convert to positive score.
            results.push(SearchResult {
                node,
                score: -rank,
            });
        }

        if !results.is_empty() {
            return Ok(results);
        }

        // Fallback: LIKE query
        let like_pattern = format!("%{query}%");
        let mut rows = self
            .conn()
            .query(
                "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, branches, loops, returns, max_nesting, unsafe_blocks, unchecked_calls, assertions, updated_at
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
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

impl Database {
    /// Returns aggregate statistics about the code graph.
    pub async fn get_stats(&self) -> Result<GraphStats> {
        let node_count = query_scalar_i64(self.conn(), "SELECT COUNT(*) FROM nodes", "get_stats")
            .await? as u64;

        let edge_count = query_scalar_i64(self.conn(), "SELECT COUNT(*) FROM edges", "get_stats")
            .await? as u64;

        let file_count = query_scalar_i64(self.conn(), "SELECT COUNT(*) FROM files", "get_stats")
            .await? as u64;

        // Nodes grouped by kind
        let mut nodes_by_kind = HashMap::new();
        {
            let mut rows = self
                .conn()
                .query("SELECT kind, COUNT(*) FROM nodes GROUP BY kind", ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query nodes by kind: {e}"),
                    operation: "get_stats".to_string(),
                })?;

            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read stats row: {e}"),
                operation: "get_stats".to_string(),
            })? {
                let kind: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read kind: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                let count: i64 = row.get(1).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read count: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                nodes_by_kind.insert(kind, count as u64);
            }
        }

        // Edges grouped by kind
        let mut edges_by_kind = HashMap::new();
        {
            let mut rows = self
                .conn()
                .query("SELECT kind, COUNT(*) FROM edges GROUP BY kind", ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query edges by kind: {e}"),
                    operation: "get_stats".to_string(),
                })?;

            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read stats row: {e}"),
                operation: "get_stats".to_string(),
            })? {
                let kind: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read kind: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                let count: i64 = row.get(1).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read count: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                edges_by_kind.insert(kind, count as u64);
            }
        }

        let db_size_bytes = self.size().await.unwrap_or(0);

        let last_updated =
            query_scalar_i64(self.conn(), "SELECT COALESCE(MAX(indexed_at), 0) FROM files", "get_stats")
                .await
                .unwrap_or(0) as u64;

        let total_source_bytes =
            query_scalar_i64(self.conn(), "SELECT COALESCE(SUM(size), 0) FROM files", "get_stats")
                .await
                .unwrap_or(0) as u64;

        // Files grouped by language (derived from file extension)
        let mut files_by_language = HashMap::new();
        {
            let mut rows = self
                .conn()
                .query(
                    "SELECT \
                       CASE \
                         WHEN path LIKE '%.rs' THEN 'Rust' \
                         WHEN path LIKE '%.go' THEN 'Go' \
                         WHEN path LIKE '%.java' THEN 'Java' \
                         WHEN path LIKE '%.scala' OR path LIKE '%.sc' THEN 'Scala' \
                         ELSE 'Other' \
                       END AS lang, \
                       COUNT(*) \
                     FROM files GROUP BY lang",
                    (),
                )
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query files by language: {e}"),
                    operation: "get_stats".to_string(),
                })?;

            while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
                message: format!("failed to read stats row: {e}"),
                operation: "get_stats".to_string(),
            })? {
                let lang: String = row.get(0).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read language: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                let count: i64 = row.get(1).map_err(|e| TokenSaveError::Database {
                    message: format!("failed to read count: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                if count > 0 {
                    files_by_language.insert(lang, count as u64);
                }
            }
        }

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
        })
    }
}

// ---------------------------------------------------------------------------
// Clear
// ---------------------------------------------------------------------------

impl Database {
    /// Removes all data from every table.
    pub async fn clear(&self) -> Result<()> {
        self.conn()
            .execute_batch(
                "DELETE FROM vectors;
                 DELETE FROM unresolved_refs;
                 DELETE FROM edges;
                 DELETE FROM nodes;
                 DELETE FROM files;",
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to clear database: {e}"),
                operation: "clear".to_string(),
            })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

impl Database {
    /// Reads a metadata value by key, returning `None` if not set.
    pub async fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT value FROM metadata WHERE key = ?1",
                params![key],
            )
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
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Converts `Option<String>` to a `libsql::Value` for use in params.
fn opt_str(opt: &Option<String>) -> libsql::Value {
    match opt {
        Some(s) => libsql::Value::Text(s.clone()),
        None => libsql::Value::Null,
    }
}

/// Collects all rows from a `Rows` iterator into a `Vec<T>` using the given
/// row-mapping function.
async fn collect_rows<T>(
    rows: &mut libsql::Rows,
    map_fn: fn(&libsql::Row) -> std::result::Result<T, libsql::Error>,
    operation: &str,
) -> Result<Vec<T>> {
    let mut items = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("failed to read row: {e}"),
        operation: operation.to_string(),
    })? {
        items.push(map_fn(&row).map_err(|e| TokenSaveError::Database {
            message: format!("failed to map row: {e}"),
            operation: operation.to_string(),
        })?);
    }
    Ok(items)
}

/// Executes a scalar query returning a single `i64` value.
async fn query_scalar_i64(
    conn: &libsql::Connection,
    sql: &str,
    operation: &str,
) -> Result<i64> {
    let mut rows = conn
        .query(sql, ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to execute scalar query: {e}"),
            operation: operation.to_string(),
        })?;

    let row = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read scalar row: {e}"),
            operation: operation.to_string(),
        })?
        .ok_or_else(|| TokenSaveError::Database {
            message: "no result from scalar query".to_string(),
            operation: operation.to_string(),
        })?;

    row.get::<i64>(0).map_err(|e| TokenSaveError::Database {
        message: format!("failed to read scalar value: {e}"),
        operation: operation.to_string(),
    })
}
