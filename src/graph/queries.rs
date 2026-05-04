// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};

use crate::db::Database;
use crate::errors::{Result, TokenSaveError};
use crate::types::*;

/// Metrics describing the connectivity and structure around a single node.
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    /// Number of incoming edges (all kinds).
    pub incoming_edge_count: usize,
    /// Number of outgoing edges (all kinds).
    pub outgoing_edge_count: usize,
    /// Number of outgoing `Calls` edges (functions this node calls).
    pub call_count: usize,
    /// Number of incoming `Calls` edges (functions that call this node).
    pub caller_count: usize,
    /// Number of outgoing `Contains` edges (direct children).
    pub child_count: usize,
    /// Depth of the node in the containment hierarchy.
    pub depth: usize,
}

/// Provides analytical query operations over the code graph.
pub struct GraphQueryManager<'a> {
    db: &'a Database,
}

fn row_to_node_dead_code(row: &libsql::Row) -> std::result::Result<Node, libsql::Error> {
    let kind_str = get_string_lossy(row, 1)?;
    let vis_str = get_string_lossy(row, 11)?;
    let is_async_int = row.get::<i64>(12)?;
    let start_line = row.get::<u32>(5)?;
    // Pre-v7 rows lack attrs_start_line; fall back to start_line. The dead-code
    // SELECT below does not include the new column, so attempts to read at 21
    // will always error here — keep the fallback path explicit.
    let attrs_raw = row.get::<u32>(21).unwrap_or(0);
    let attrs_start_line = if attrs_raw == 0 {
        start_line
    } else {
        attrs_raw
    };

    Ok(Node {
        id: get_string_lossy(row, 0)?,
        kind: NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Function),
        name: get_string_lossy(row, 2)?,
        qualified_name: get_string_lossy(row, 3)?,
        file_path: get_string_lossy(row, 4)?,
        start_line,
        attrs_start_line,
        end_line: row.get::<u32>(6)?,
        start_column: row.get::<u32>(7)?,
        end_column: row.get::<u32>(8)?,
        signature: get_opt_string_lossy(row, 10)?,
        docstring: get_opt_string_lossy(row, 9)?,
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

fn get_string_lossy(row: &libsql::Row, idx: i32) -> std::result::Result<String, libsql::Error> {
    let val = row.get::<libsql::Value>(idx)?;
    match val {
        libsql::Value::Text(s) => Ok(s),
        libsql::Value::Blob(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        libsql::Value::Null => Ok(String::new()),
        libsql::Value::Integer(i) => Ok(i.to_string()),
        libsql::Value::Real(f) => Ok(f.to_string()),
    }
}

fn get_opt_string_lossy(
    row: &libsql::Row,
    idx: i32,
) -> std::result::Result<Option<String>, libsql::Error> {
    let val = row.get::<libsql::Value>(idx)?;
    match val {
        libsql::Value::Text(s) => Ok(Some(s)),
        libsql::Value::Blob(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        libsql::Value::Null => Ok(None),
        libsql::Value::Integer(i) => Ok(Some(i.to_string())),
        libsql::Value::Real(f) => Ok(Some(f.to_string())),
    }
}

impl<'a> GraphQueryManager<'a> {
    /// Creates a new `GraphQueryManager` backed by the given database.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Finds nodes with zero incoming edges, indicating potentially dead code.
    ///
    /// Excludes:
    /// - Nodes named `"main"` (program entry points).
    /// - Nodes whose name starts with `"test"` (likely test functions).
    /// - `pub` items at file level (they may be part of a public API).
    ///
    /// If `kinds` is non-empty, only nodes of the specified kinds are checked.
    pub async fn find_dead_code(&self, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        let kind_filter = if kinds.is_empty() {
            String::new()
        } else {
            let kind_strs: Vec<String> =
                kinds.iter().map(|k| format!("'{}'", k.as_str())).collect();
            format!(" AND kind IN ({})", kind_strs.join(", "))
        };

        let sql = format!(
            "SELECT id, kind, name, qualified_name, file_path, start_line, end_line,
                    start_column, end_column, docstring, signature, visibility,
                    is_async, branches, loops, returns, max_nesting, unsafe_blocks,
                    unchecked_calls, assertions, updated_at, attrs_start_line
             FROM nodes
             WHERE name != 'main'
             AND name NOT LIKE 'test%'
             AND visibility != 'public'
             {kind_filter}
             AND NOT EXISTS (SELECT 1 FROM edges WHERE target = nodes.id)"
        );

        let mut rows =
            self.db
                .conn()
                .query(&sql, ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to find dead code: {e}"),
                    operation: "find_dead_code".to_string(),
                })?;

        let mut dead = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read row: {e}"),
            operation: "find_dead_code".to_string(),
        })? {
            let node = row_to_node_dead_code(&row)?;
            dead.push(node);
        }

        Ok(dead)
    }

    /// Computes metrics for a single node describing its graph connectivity.
    pub async fn get_node_metrics(&self, node_id: &str) -> Result<NodeMetrics> {
        let incoming = self.db.get_incoming_edges(node_id, &[]).await?;
        let outgoing = self.db.get_outgoing_edges(node_id, &[]).await?;

        let caller_count = incoming
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count();
        let call_count = outgoing
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count();
        let child_count = outgoing
            .iter()
            .filter(|e| e.kind == EdgeKind::Contains)
            .count();

        // Compute depth by walking up the containment hierarchy.
        let depth = self.compute_depth(node_id).await?;

        Ok(NodeMetrics {
            incoming_edge_count: incoming.len(),
            outgoing_edge_count: outgoing.len(),
            call_count,
            caller_count,
            child_count,
            depth,
        })
    }

    /// Gets the file paths that the given file depends on.
    ///
    /// Examines outgoing `Uses` and `Calls` edges from all nodes in the
    /// specified file. Returns the deduplicated set of target file paths,
    /// excluding the source file itself.
    pub async fn get_file_dependencies(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path).await?;
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let placeholders: Vec<String> = (1..=node_ids.len()).map(|i| format!("?{i}")).collect();
        let kind_filter = "('uses', 'calls')";

        let sql = format!(
            "SELECT DISTINCT e.target FROM edges e \
             WHERE e.source IN ({}) AND e.kind IN {kind_filter}",
            placeholders.join(", ")
        );

        let param_values: Vec<libsql::Value> = node_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();

        let mut rows = self
            .db
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file dependencies: {e}"),
                operation: "get_file_dependencies".to_string(),
            })?;

        let mut target_ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read target id: {e}"),
            operation: "get_file_dependencies".to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                target_ids.push(id);
            }
        }

        if target_ids.is_empty() {
            return Ok(Vec::new());
        }

        let target_nodes = self.db.get_nodes_by_ids(&target_ids).await?;
        let dep_files: HashSet<String> = target_nodes
            .into_iter()
            .filter(|n| n.file_path != file_path)
            .map(|n| n.file_path)
            .collect();

        let mut result: Vec<String> = dep_files.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Gets the file paths that depend on the given file.
    ///
    /// Examines incoming `Uses` and `Calls` edges to all nodes in the
    /// specified file. Returns the deduplicated set of source file paths,
    /// excluding the target file itself.
    pub async fn get_file_dependents(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path).await?;
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let placeholders: Vec<String> = (1..=node_ids.len()).map(|i| format!("?{i}")).collect();
        let kind_filter = "('uses', 'calls')";

        let sql = format!(
            "SELECT DISTINCT e.source FROM edges e \
             WHERE e.target IN ({}) AND e.kind IN {kind_filter}",
            placeholders.join(", ")
        );

        let param_values: Vec<libsql::Value> = node_ids
            .iter()
            .map(|id| libsql::Value::Text(id.clone()))
            .collect();

        let mut rows = self
            .db
            .conn()
            .query(&sql, libsql::params_from_iter(param_values))
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("failed to query file dependents: {e}"),
                operation: "get_file_dependents".to_string(),
            })?;

        let mut source_ids: Vec<String> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read source id: {e}"),
            operation: "get_file_dependents".to_string(),
        })? {
            if let Ok(id) = row.get::<String>(0) {
                source_ids.push(id);
            }
        }

        if source_ids.is_empty() {
            return Ok(Vec::new());
        }

        let source_nodes = self.db.get_nodes_by_ids(&source_ids).await?;
        let dependent_files: HashSet<String> = source_nodes
            .into_iter()
            .filter(|n| n.file_path != file_path)
            .map(|n| n.file_path)
            .collect();

        let mut result: Vec<String> = dependent_files.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Detects circular dependencies at the file level.
    ///
    /// Builds a file-level dependency graph and runs DFS-based cycle detection.
    /// Returns all cycles found, where each cycle is a vector of file paths.
    pub async fn find_circular_dependencies(&self) -> Result<Vec<Vec<String>>> {
        // Build file-level adjacency list.
        let all_files = self.db.get_all_files().await?;
        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();

        for file in &all_files {
            let deps = self.get_file_dependencies(&file.path).await?;
            adj.insert(file.path.clone(), deps.into_iter().collect());
        }

        // DFS-based cycle detection.
        let mut cycles: Vec<Vec<String>> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut on_stack: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = Vec::new();

        let file_paths: Vec<String> = adj.keys().cloned().collect();

        for file_path in &file_paths {
            if !visited.contains(file_path) {
                dfs_cycle_detect(
                    file_path,
                    &adj,
                    &mut visited,
                    &mut on_stack,
                    &mut stack,
                    &mut cycles,
                );
            }
        }

        Ok(cycles)
    }

    /// Builds a file-level directed adjacency map from the code graph.
    ///
    /// For each file, collects all files it depends on via `calls`, `uses`,
    /// `extends`, or `implements` edges. Self-edges are excluded.
    ///
    /// When `path_prefix` is `Some`, only files under that prefix are included
    /// (both as sources and targets).
    pub async fn build_file_adjacency(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<HashMap<String, HashSet<String>>> {
        let sql = "SELECT DISTINCT n1.file_path AS src_file, n2.file_path AS tgt_file \
                   FROM edges e \
                   JOIN nodes n1 ON e.source = n1.id \
                   JOIN nodes n2 ON e.target = n2.id \
                   WHERE e.kind IN ('calls', 'uses', 'extends', 'implements') \
                   AND n1.file_path != n2.file_path";

        let mut rows =
            self.db
                .conn()
                .query(sql, ())
                .await
                .map_err(|e| TokenSaveError::Database {
                    message: format!("failed to query file adjacency: {e}"),
                    operation: "build_file_adjacency".to_string(),
                })?;

        // Normalise the prefix once: ensure it ends with '/'.
        let prefix: Option<String> = path_prefix.map(|p| {
            if p.ends_with('/') {
                p.to_string()
            } else {
                format!("{p}/")
            }
        });

        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();

        while let Some(row) = rows.next().await.map_err(|e| TokenSaveError::Database {
            message: format!("failed to read adjacency row: {e}"),
            operation: "build_file_adjacency".to_string(),
        })? {
            let src: String = row.get(0).unwrap_or_default();
            let tgt: String = row.get(1).unwrap_or_default();

            if let Some(ref pfx) = prefix {
                if !src.starts_with(pfx.as_str()) || !tgt.starts_with(pfx.as_str()) {
                    continue;
                }
            }

            adj.entry(src).or_default().insert(tgt);
        }

        // Ensure every known file appears as a key (even leaf nodes with no deps).
        let all_files = self.db.get_all_files().await?;
        for file in all_files {
            if let Some(ref pfx) = prefix {
                if !file.path.starts_with(pfx.as_str()) {
                    continue;
                }
            }
            adj.entry(file.path).or_default();
        }

        Ok(adj)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Computes the depth of a node in the containment hierarchy by walking
    /// up incoming `Contains` edges.
    async fn compute_depth(&self, node_id: &str) -> Result<usize> {
        const MAX_DEPTH: usize = 100;
        let mut depth: usize = 0;
        let mut current_id = node_id.to_string();
        let mut visited: HashSet<String> = HashSet::new();

        while depth < MAX_DEPTH {
            if visited.contains(&current_id) {
                break;
            }
            visited.insert(current_id.clone());

            let incoming = self
                .db
                .get_incoming_edges(&current_id, &[EdgeKind::Contains])
                .await?;

            // Take the first parent in the containment hierarchy.
            match incoming.first() {
                Some(edge) => {
                    current_id.clone_from(&edge.source);
                    depth += 1;
                }
                None => break,
            }
        }

        Ok(depth)
    }
}

/// Iterative DFS for cycle detection on the file dependency graph.
///
/// Uses an explicit stack instead of recursion to comply with the
/// "no recursion" rule (NASA Power of 10, Rule 1).
fn dfs_cycle_detect(
    start: &str,
    adj: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    on_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    // Each frame: (node, neighbor_index). We materialise the neighbor list
    // so we can index into it across iterations.
    let mut call_stack: Vec<(String, Vec<String>, usize)> = Vec::new();

    // Push the initial frame.
    let neighbors: Vec<String> = adj
        .get(start)
        .map(|s| s.iter().cloned().collect())
        .unwrap_or_default();
    visited.insert(start.to_string());
    on_stack.insert(start.to_string());
    path.push(start.to_string());
    call_stack.push((start.to_string(), neighbors, 0));

    while let Some(frame) = call_stack.last_mut() {
        let idx = frame.2;
        if idx >= frame.1.len() {
            // All neighbors explored — backtrack.
            // Safety: we are inside `while let Some(_) = call_stack.last_mut()`,
            // so pop() is guaranteed to return Some.
            let Some((node, _, _)) = call_stack.pop() else {
                break;
            };
            path.pop();
            on_stack.remove(&node);
            continue;
        }

        // Advance the iterator for this frame.
        frame.2 += 1;
        let neighbor = frame.1[idx].clone();

        if !visited.contains(&neighbor) {
            // Descend into the neighbor.
            let nb_neighbors: Vec<String> = adj
                .get(&neighbor)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            visited.insert(neighbor.clone());
            on_stack.insert(neighbor.clone());
            path.push(neighbor.clone());
            call_stack.push((neighbor, nb_neighbors, 0));
        } else if on_stack.contains(&neighbor) {
            // Found a cycle — extract it from the current path.
            let mut cycle = Vec::new();
            let mut found_start = false;
            for item in path.iter() {
                if item == &neighbor {
                    found_start = true;
                }
                if found_start {
                    cycle.push(item.clone());
                }
            }
            cycle.push(neighbor.clone());
            cycles.push(cycle);
        }
    }
}
