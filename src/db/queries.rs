use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use rusqlite::OptionalExtension;

use super::connection::Database;
use crate::errors::{CodeGraphError, Result};
use crate::types::*;

// ---------------------------------------------------------------------------
// Helper: map a rusqlite row to domain types
// ---------------------------------------------------------------------------

/// Maps a row from the `nodes` table to a `Node`.
fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
    let kind_str: String = row.get("kind")?;
    let vis_str: String = row.get("visibility")?;
    let is_async_int: i32 = row.get("is_async")?;

    Ok(Node {
        id: row.get("id")?,
        kind: NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Function),
        name: row.get("name")?,
        qualified_name: row.get("qualified_name")?,
        file_path: row.get("file_path")?,
        start_line: row.get("start_line")?,
        end_line: row.get("end_line")?,
        start_column: row.get("start_column")?,
        end_column: row.get("end_column")?,
        signature: row.get("signature")?,
        docstring: row.get("docstring")?,
        visibility: Visibility::from_str(&vis_str).unwrap_or_default(),
        is_async: is_async_int != 0,
        updated_at: row.get::<_, i64>("updated_at")? as u64,
    })
}

/// Maps a row from the `edges` table to an `Edge`.
fn row_to_edge(row: &rusqlite::Row) -> rusqlite::Result<Edge> {
    let kind_str: String = row.get("kind")?;
    let line: Option<u32> = row.get("line")?;

    Ok(Edge {
        source: row.get("source")?,
        target: row.get("target")?,
        kind: EdgeKind::from_str(&kind_str).unwrap_or(EdgeKind::Uses),
        line,
    })
}

/// Maps a row from the `files` table to a `FileRecord`.
fn row_to_file(row: &rusqlite::Row) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        path: row.get("path")?,
        content_hash: row.get("content_hash")?,
        size: row.get::<_, i64>("size")? as u64,
        modified_at: row.get("modified_at")?,
        indexed_at: row.get("indexed_at")?,
        node_count: row.get::<_, i32>("node_count")? as u32,
    })
}

/// Maps a row from the `unresolved_refs` table to an `UnresolvedRef`.
fn row_to_unresolved_ref(row: &rusqlite::Row) -> rusqlite::Result<UnresolvedRef> {
    let kind_str: String = row.get("reference_kind")?;

    Ok(UnresolvedRef {
        from_node_id: row.get("from_node_id")?,
        reference_name: row.get("reference_name")?,
        reference_kind: EdgeKind::from_str(&kind_str).unwrap_or(EdgeKind::Uses),
        line: row.get("line")?,
        column: row.get::<_, u32>("col")?,
        file_path: row.get("file_path")?,
    })
}

// ---------------------------------------------------------------------------
// Node operations
// ---------------------------------------------------------------------------

impl Database {
    /// Inserts or replaces a single node.
    pub fn insert_node(&self, node: &Node) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO nodes
                (id, kind, name, qualified_name, file_path,
                 start_line, end_line, start_column, end_column,
                 docstring, signature, visibility, is_async, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                node.id,
                node.kind.as_str(),
                node.name,
                node.qualified_name,
                node.file_path,
                node.start_line,
                node.end_line,
                node.start_column,
                node.end_column,
                node.docstring,
                node.signature,
                node.visibility.as_str(),
                node.is_async as i32,
                node.updated_at as i64,
            ],
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to insert node: {e}"),
            operation: "insert_node".to_string(),
        })?;
        Ok(())
    }

    /// Inserts or replaces a batch of nodes inside a single transaction.
    pub fn insert_nodes(&self, nodes: &[Node]) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(|e| {
            CodeGraphError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "insert_nodes".to_string(),
            }
        })?;

        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO nodes
                    (id, kind, name, qualified_name, file_path,
                     start_line, end_line, start_column, end_column,
                     docstring, signature, visibility, is_async, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            ).map_err(|e| CodeGraphError::Database {
                message: format!("failed to prepare statement: {e}"),
                operation: "insert_nodes".to_string(),
            })?;

            for node in nodes {
                stmt.execute(params![
                    node.id,
                    node.kind.as_str(),
                    node.name,
                    node.qualified_name,
                    node.file_path,
                    node.start_line,
                    node.end_line,
                    node.start_column,
                    node.end_column,
                    node.docstring,
                    node.signature,
                    node.visibility.as_str(),
                    node.is_async as i32,
                    node.updated_at as i64,
                ]).map_err(|e| CodeGraphError::Database {
                    message: format!("failed to insert node: {e}"),
                    operation: "insert_nodes".to_string(),
                })?;
            }
        }

        tx.commit().map_err(|e| CodeGraphError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "insert_nodes".to_string(),
        })
    }

    /// Retrieves a node by its unique ID, returning `None` if not found.
    pub fn get_node_by_id(&self, id: &str) -> Result<Option<Node>> {
        self.conn()
            .query_row(
                "SELECT id, kind, name, qualified_name, file_path,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility, is_async, updated_at
                 FROM nodes WHERE id = ?1",
                params![id],
                row_to_node,
            )
            .optional()
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to get node by id: {e}"),
                operation: "get_node_by_id".to_string(),
            })
    }

    /// Returns all nodes for a given file, ordered by start line.
    pub fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes WHERE file_path = ?1 ORDER BY start_line",
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to prepare query: {e}"),
            operation: "get_nodes_by_file".to_string(),
        })?;

        let rows = stmt.query_map(params![file_path], row_to_node).map_err(|e| {
            CodeGraphError::Database {
                message: format!("failed to query nodes by file: {e}"),
                operation: "get_nodes_by_file".to_string(),
            }
        })?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row.map_err(|e| CodeGraphError::Database {
                message: format!("failed to read node row: {e}"),
                operation: "get_nodes_by_file".to_string(),
            })?);
        }
        Ok(nodes)
    }

    /// Returns all nodes of a given kind.
    pub fn get_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<Node>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes WHERE kind = ?1",
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to prepare query: {e}"),
            operation: "get_nodes_by_kind".to_string(),
        })?;

        let rows = stmt
            .query_map(params![kind.as_str()], row_to_node)
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to query nodes by kind: {e}"),
                operation: "get_nodes_by_kind".to_string(),
            })?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row.map_err(|e| CodeGraphError::Database {
                message: format!("failed to read node row: {e}"),
                operation: "get_nodes_by_kind".to_string(),
            })?);
        }
        Ok(nodes)
    }

    /// Returns every node in the database.
    pub fn get_all_nodes(&self) -> Result<Vec<Node>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes",
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to prepare query: {e}"),
            operation: "get_all_nodes".to_string(),
        })?;

        let rows = stmt.query_map([], row_to_node).map_err(|e| {
            CodeGraphError::Database {
                message: format!("failed to query all nodes: {e}"),
                operation: "get_all_nodes".to_string(),
            }
        })?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row.map_err(|e| CodeGraphError::Database {
                message: format!("failed to read node row: {e}"),
                operation: "get_all_nodes".to_string(),
            })?);
        }
        Ok(nodes)
    }

    /// Deletes all nodes (and cascading edges, unresolved refs, vectors) for a file.
    pub fn delete_nodes_by_file(&self, file_path: &str) -> Result<()> {
        // Gather node IDs for the file first.
        let node_ids: Vec<String> = {
            let mut stmt = self
                .conn()
                .prepare("SELECT id FROM nodes WHERE file_path = ?1")
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to prepare query: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?;

            let rows = stmt
                .query_map(params![file_path], |row| row.get::<_, String>(0))
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to query node ids: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?;

            let mut ids = Vec::new();
            for row in rows {
                ids.push(row.map_err(|e| CodeGraphError::Database {
                    message: format!("failed to read node id: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?);
            }
            ids
        };

        if node_ids.is_empty() {
            return Ok(());
        }

        let tx = self.conn().unchecked_transaction().map_err(|e| {
            CodeGraphError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            }
        })?;

        for id in &node_ids {
            tx.execute(
                "DELETE FROM edges WHERE source = ?1 OR target = ?1",
                params![id],
            )
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to delete edges: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

            tx.execute(
                "DELETE FROM unresolved_refs WHERE from_node_id = ?1",
                params![id],
            )
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to delete unresolved refs: {e}"),
                operation: "delete_nodes_by_file".to_string(),
            })?;

            tx.execute("DELETE FROM vectors WHERE node_id = ?1", params![id])
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to delete vectors: {e}"),
                    operation: "delete_nodes_by_file".to_string(),
                })?;
        }

        tx.execute(
            "DELETE FROM nodes WHERE file_path = ?1",
            params![file_path],
        )
        .map_err(|e| CodeGraphError::Database {
            message: format!("failed to delete nodes: {e}"),
            operation: "delete_nodes_by_file".to_string(),
        })?;

        tx.commit().map_err(|e| CodeGraphError::Database {
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
    pub fn insert_edge(&self, edge: &Edge) -> Result<()> {
        self.conn().execute(
            "INSERT INTO edges (source, target, kind, line) VALUES (?1, ?2, ?3, ?4)",
            params![edge.source, edge.target, edge.kind.as_str(), edge.line],
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to insert edge: {e}"),
            operation: "insert_edge".to_string(),
        })?;
        Ok(())
    }

    /// Inserts a batch of edges inside a single transaction.
    pub fn insert_edges(&self, edges: &[Edge]) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(|e| {
            CodeGraphError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "insert_edges".to_string(),
            }
        })?;

        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO edges (source, target, kind, line) VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to prepare statement: {e}"),
                    operation: "insert_edges".to_string(),
                })?;

            for edge in edges {
                stmt.execute(params![
                    edge.source,
                    edge.target,
                    edge.kind.as_str(),
                    edge.line,
                ])
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to insert edge: {e}"),
                    operation: "insert_edges".to_string(),
                })?;
            }
        }

        tx.commit().map_err(|e| CodeGraphError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "insert_edges".to_string(),
        })
    }

    /// Returns outgoing edges from a source node, optionally filtered by edge kinds.
    ///
    /// If `kinds` is empty, all outgoing edges are returned.
    pub fn get_outgoing_edges(&self, source_id: &str, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut stmt = self.conn().prepare(
                "SELECT source, target, kind, line FROM edges WHERE source = ?1",
            ).map_err(|e| CodeGraphError::Database {
                message: format!("failed to prepare query: {e}"),
                operation: "get_outgoing_edges".to_string(),
            })?;

            let rows = stmt.query_map(params![source_id], row_to_edge).map_err(|e| {
                CodeGraphError::Database {
                    message: format!("failed to query outgoing edges: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                }
            })?;

            let mut edges = Vec::new();
            for row in rows {
                edges.push(row.map_err(|e| CodeGraphError::Database {
                    message: format!("failed to read edge row: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                })?);
            }
            Ok(edges)
        } else {
            let placeholders: Vec<String> = kinds.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
            let sql = format!(
                "SELECT source, target, kind, line FROM edges WHERE source = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );

            let mut stmt = self.conn().prepare(&sql).map_err(|e| CodeGraphError::Database {
                message: format!("failed to prepare query: {e}"),
                operation: "get_outgoing_edges".to_string(),
            })?;

            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            param_values.push(Box::new(source_id.to_string()));
            for k in kinds {
                param_values.push(Box::new(k.as_str().to_string()));
            }
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|b| b.as_ref()).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), row_to_edge)
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to query outgoing edges: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                })?;

            let mut edges = Vec::new();
            for row in rows {
                edges.push(row.map_err(|e| CodeGraphError::Database {
                    message: format!("failed to read edge row: {e}"),
                    operation: "get_outgoing_edges".to_string(),
                })?);
            }
            Ok(edges)
        }
    }

    /// Returns incoming edges to a target node, optionally filtered by edge kinds.
    ///
    /// If `kinds` is empty, all incoming edges are returned.
    pub fn get_incoming_edges(&self, target_id: &str, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut stmt = self.conn().prepare(
                "SELECT source, target, kind, line FROM edges WHERE target = ?1",
            ).map_err(|e| CodeGraphError::Database {
                message: format!("failed to prepare query: {e}"),
                operation: "get_incoming_edges".to_string(),
            })?;

            let rows = stmt.query_map(params![target_id], row_to_edge).map_err(|e| {
                CodeGraphError::Database {
                    message: format!("failed to query incoming edges: {e}"),
                    operation: "get_incoming_edges".to_string(),
                }
            })?;

            let mut edges = Vec::new();
            for row in rows {
                edges.push(row.map_err(|e| CodeGraphError::Database {
                    message: format!("failed to read edge row: {e}"),
                    operation: "get_incoming_edges".to_string(),
                })?);
            }
            Ok(edges)
        } else {
            let placeholders: Vec<String> = kinds.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
            let sql = format!(
                "SELECT source, target, kind, line FROM edges WHERE target = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );

            let mut stmt = self.conn().prepare(&sql).map_err(|e| CodeGraphError::Database {
                message: format!("failed to prepare query: {e}"),
                operation: "get_incoming_edges".to_string(),
            })?;

            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            param_values.push(Box::new(target_id.to_string()));
            for k in kinds {
                param_values.push(Box::new(k.as_str().to_string()));
            }
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|b| b.as_ref()).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), row_to_edge)
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to query incoming edges: {e}"),
                    operation: "get_incoming_edges".to_string(),
                })?;

            let mut edges = Vec::new();
            for row in rows {
                edges.push(row.map_err(|e| CodeGraphError::Database {
                    message: format!("failed to read edge row: {e}"),
                    operation: "get_incoming_edges".to_string(),
                })?);
            }
            Ok(edges)
        }
    }

    /// Deletes all edges originating from a given source node.
    pub fn delete_edges_by_source(&self, source_id: &str) -> Result<()> {
        self.conn()
            .execute(
                "DELETE FROM edges WHERE source = ?1",
                params![source_id],
            )
            .map_err(|e| CodeGraphError::Database {
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
    pub fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO files
                (path, content_hash, size, modified_at, indexed_at, node_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                file.path,
                file.content_hash,
                file.size as i64,
                file.modified_at,
                file.indexed_at,
                file.node_count as i32,
            ],
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to upsert file: {e}"),
            operation: "upsert_file".to_string(),
        })?;
        Ok(())
    }

    /// Retrieves a file record by path, returning `None` if not found.
    pub fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        self.conn()
            .query_row(
                "SELECT path, content_hash, size, modified_at, indexed_at, node_count
                 FROM files WHERE path = ?1",
                params![path],
                row_to_file,
            )
            .optional()
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to get file: {e}"),
                operation: "get_file".to_string(),
            })
    }

    /// Returns all file records.
    pub fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn().prepare(
            "SELECT path, content_hash, size, modified_at, indexed_at, node_count FROM files",
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to prepare query: {e}"),
            operation: "get_all_files".to_string(),
        })?;

        let rows = stmt.query_map([], row_to_file).map_err(|e| {
            CodeGraphError::Database {
                message: format!("failed to query all files: {e}"),
                operation: "get_all_files".to_string(),
            }
        })?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row.map_err(|e| CodeGraphError::Database {
                message: format!("failed to read file row: {e}"),
                operation: "get_all_files".to_string(),
            })?);
        }
        Ok(files)
    }

    /// Deletes a file record and cascades to delete its nodes first.
    pub fn delete_file(&self, path: &str) -> Result<()> {
        self.delete_nodes_by_file(path)?;
        self.conn()
            .execute("DELETE FROM files WHERE path = ?1", params![path])
            .map_err(|e| CodeGraphError::Database {
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
    pub fn insert_unresolved_ref(&self, uref: &UnresolvedRef) -> Result<()> {
        self.conn().execute(
            "INSERT INTO unresolved_refs
                (from_node_id, reference_name, reference_kind, line, col, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                uref.from_node_id,
                uref.reference_name,
                uref.reference_kind.as_str(),
                uref.line,
                uref.column,
                uref.file_path,
            ],
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to insert unresolved ref: {e}"),
            operation: "insert_unresolved_ref".to_string(),
        })?;
        Ok(())
    }

    /// Inserts a batch of unresolved references inside a single transaction.
    pub fn insert_unresolved_refs(&self, refs: &[UnresolvedRef]) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(|e| {
            CodeGraphError::Database {
                message: format!("failed to begin transaction: {e}"),
                operation: "insert_unresolved_refs".to_string(),
            }
        })?;

        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO unresolved_refs
                        (from_node_id, reference_name, reference_kind, line, col, file_path)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to prepare statement: {e}"),
                    operation: "insert_unresolved_refs".to_string(),
                })?;

            for uref in refs {
                stmt.execute(params![
                    uref.from_node_id,
                    uref.reference_name,
                    uref.reference_kind.as_str(),
                    uref.line,
                    uref.column,
                    uref.file_path,
                ])
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to insert unresolved ref: {e}"),
                    operation: "insert_unresolved_refs".to_string(),
                })?;
            }
        }

        tx.commit().map_err(|e| CodeGraphError::Database {
            message: format!("failed to commit transaction: {e}"),
            operation: "insert_unresolved_refs".to_string(),
        })
    }

    /// Returns all unresolved references.
    pub fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>> {
        let mut stmt = self.conn().prepare(
            "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
             FROM unresolved_refs",
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to prepare query: {e}"),
            operation: "get_unresolved_refs".to_string(),
        })?;

        let rows = stmt
            .query_map([], row_to_unresolved_ref)
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to query unresolved refs: {e}"),
                operation: "get_unresolved_refs".to_string(),
            })?;

        let mut refs = Vec::new();
        for row in rows {
            refs.push(row.map_err(|e| CodeGraphError::Database {
                message: format!("failed to read unresolved ref row: {e}"),
                operation: "get_unresolved_refs".to_string(),
            })?);
        }
        Ok(refs)
    }

    /// Removes all unresolved references.
    pub fn clear_unresolved_refs(&self) -> Result<()> {
        self.conn()
            .execute("DELETE FROM unresolved_refs", [])
            .map_err(|e| CodeGraphError::Database {
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
    pub fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        // Try FTS5 first (prefix match)
        let fts_query = format!("{query}*");
        let mut stmt = self.conn().prepare(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.updated_at,
                    rank
             FROM nodes_fts
             JOIN nodes n ON nodes_fts.rowid = n.rowid
             WHERE nodes_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to prepare FTS query: {e}"),
            operation: "search_nodes".to_string(),
        })?;

        let rows = stmt
            .query_map(params![fts_query, limit as i64], |row| {
                let node = row_to_node(row)?;
                let rank: f64 = row.get("rank")?;
                // FTS5 rank is negative (lower = better match). Convert to positive score.
                Ok(SearchResult {
                    node,
                    score: -rank,
                })
            })
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to execute FTS query: {e}"),
                operation: "search_nodes".to_string(),
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| CodeGraphError::Database {
                message: format!("failed to read search result: {e}"),
                operation: "search_nodes".to_string(),
            })?);
        }

        if !results.is_empty() {
            return Ok(results);
        }

        // Fallback: LIKE query
        let like_pattern = format!("%{query}%");
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes
             WHERE name LIKE ?1 OR qualified_name LIKE ?1 OR docstring LIKE ?1 OR signature LIKE ?1
             LIMIT ?2",
        ).map_err(|e| CodeGraphError::Database {
            message: format!("failed to prepare LIKE query: {e}"),
            operation: "search_nodes".to_string(),
        })?;

        let rows = stmt
            .query_map(params![like_pattern, limit as i64], |row| {
                let node = row_to_node(row)?;
                Ok(SearchResult { node, score: 1.0 })
            })
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to execute LIKE query: {e}"),
                operation: "search_nodes".to_string(),
            })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| CodeGraphError::Database {
                message: format!("failed to read search result: {e}"),
                operation: "search_nodes".to_string(),
            })?);
        }
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

impl Database {
    /// Returns aggregate statistics about the code graph.
    pub fn get_stats(&self) -> Result<GraphStats> {
        let node_count: u64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to count nodes: {e}"),
                operation: "get_stats".to_string(),
            })? as u64;

        let edge_count: u64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM edges", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to count edges: {e}"),
                operation: "get_stats".to_string(),
            })? as u64;

        let file_count: u64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM files", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to count files: {e}"),
                operation: "get_stats".to_string(),
            })? as u64;

        // Nodes grouped by kind
        let mut nodes_by_kind = HashMap::new();
        {
            let mut stmt = self
                .conn()
                .prepare("SELECT kind, COUNT(*) FROM nodes GROUP BY kind")
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to prepare query: {e}"),
                    operation: "get_stats".to_string(),
                })?;

            let rows = stmt
                .query_map([], |row| {
                    let kind: String = row.get(0)?;
                    let count: i64 = row.get(1)?;
                    Ok((kind, count as u64))
                })
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to query nodes by kind: {e}"),
                    operation: "get_stats".to_string(),
                })?;

            for row in rows {
                let (kind, count) = row.map_err(|e| CodeGraphError::Database {
                    message: format!("failed to read stats row: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                nodes_by_kind.insert(kind, count);
            }
        }

        // Edges grouped by kind
        let mut edges_by_kind = HashMap::new();
        {
            let mut stmt = self
                .conn()
                .prepare("SELECT kind, COUNT(*) FROM edges GROUP BY kind")
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to prepare query: {e}"),
                    operation: "get_stats".to_string(),
                })?;

            let rows = stmt
                .query_map([], |row| {
                    let kind: String = row.get(0)?;
                    let count: i64 = row.get(1)?;
                    Ok((kind, count as u64))
                })
                .map_err(|e| CodeGraphError::Database {
                    message: format!("failed to query edges by kind: {e}"),
                    operation: "get_stats".to_string(),
                })?;

            for row in rows {
                let (kind, count) = row.map_err(|e| CodeGraphError::Database {
                    message: format!("failed to read stats row: {e}"),
                    operation: "get_stats".to_string(),
                })?;
                edges_by_kind.insert(kind, count);
            }
        }

        let db_size_bytes = self.size().unwrap_or(0);

        let last_updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(GraphStats {
            node_count,
            edge_count,
            file_count,
            nodes_by_kind,
            edges_by_kind,
            db_size_bytes,
            last_updated,
        })
    }
}

// ---------------------------------------------------------------------------
// Clear
// ---------------------------------------------------------------------------

impl Database {
    /// Removes all data from every table.
    pub fn clear(&self) -> Result<()> {
        self.conn()
            .execute_batch(
                "DELETE FROM vectors;
                 DELETE FROM unresolved_refs;
                 DELETE FROM edges;
                 DELETE FROM nodes;
                 DELETE FROM files;",
            )
            .map_err(|e| CodeGraphError::Database {
                message: format!("failed to clear database: {e}"),
                operation: "clear".to_string(),
            })
    }
}
