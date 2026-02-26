use std::collections::{HashMap, HashSet};

use crate::db::Database;
use crate::errors::Result;
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
    pub fn find_dead_code(&self, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        let nodes = if kinds.is_empty() {
            self.db.get_all_nodes()?
        } else {
            let mut all = Vec::new();
            for kind in kinds {
                all.extend(self.db.get_nodes_by_kind(kind.clone())?);
            }
            all
        };

        let mut dead: Vec<Node> = Vec::new();

        for node in nodes {
            // Exclude entry points and tests.
            if node.name == "main" {
                continue;
            }
            if node.name.starts_with("test") {
                continue;
            }
            // Exclude pub items (potential public API).
            if node.visibility == Visibility::Pub {
                continue;
            }

            let incoming = self.db.get_incoming_edges(&node.id, &[])?;
            if incoming.is_empty() {
                dead.push(node);
            }
        }

        Ok(dead)
    }

    /// Computes metrics for a single node describing its graph connectivity.
    pub fn get_node_metrics(&self, node_id: &str) -> Result<NodeMetrics> {
        let incoming = self.db.get_incoming_edges(node_id, &[])?;
        let outgoing = self.db.get_outgoing_edges(node_id, &[])?;

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
        let depth = self.compute_depth(node_id)?;

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
    pub fn get_file_dependencies(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path)?;
        let mut dep_files: HashSet<String> = HashSet::new();

        for node in &nodes {
            let edges = self
                .db
                .get_outgoing_edges(&node.id, &[EdgeKind::Uses, EdgeKind::Calls])?;

            for edge in &edges {
                if let Some(target_node) = self.db.get_node_by_id(&edge.target)? {
                    if target_node.file_path != file_path {
                        dep_files.insert(target_node.file_path);
                    }
                }
            }
        }

        let mut result: Vec<String> = dep_files.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Gets the file paths that depend on the given file.
    ///
    /// Examines incoming `Uses` and `Calls` edges to all nodes in the
    /// specified file. Returns the deduplicated set of source file paths,
    /// excluding the target file itself.
    pub fn get_file_dependents(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path)?;
        let mut dependent_files: HashSet<String> = HashSet::new();

        for node in &nodes {
            let edges = self
                .db
                .get_incoming_edges(&node.id, &[EdgeKind::Uses, EdgeKind::Calls])?;

            for edge in &edges {
                if let Some(source_node) = self.db.get_node_by_id(&edge.source)? {
                    if source_node.file_path != file_path {
                        dependent_files.insert(source_node.file_path);
                    }
                }
            }
        }

        let mut result: Vec<String> = dependent_files.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Detects circular dependencies at the file level.
    ///
    /// Builds a file-level dependency graph and runs DFS-based cycle detection.
    /// Returns all cycles found, where each cycle is a vector of file paths.
    pub fn find_circular_dependencies(&self) -> Result<Vec<Vec<String>>> {
        // Build file-level adjacency list.
        let all_files = self.db.get_all_files()?;
        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();

        for file in &all_files {
            let deps = self.get_file_dependencies(&file.path)?;
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

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Computes the depth of a node in the containment hierarchy by walking
    /// up incoming `Contains` edges.
    fn compute_depth(&self, node_id: &str) -> Result<usize> {
        let mut depth: usize = 0;
        let mut current_id = node_id.to_string();
        let mut visited: HashSet<String> = HashSet::new();

        loop {
            if visited.contains(&current_id) {
                break;
            }
            visited.insert(current_id.clone());

            let incoming = self
                .db
                .get_incoming_edges(&current_id, &[EdgeKind::Contains])?;

            if incoming.is_empty() {
                break;
            }

            // Take the first parent in the containment hierarchy.
            current_id = incoming[0].source.clone();
            depth += 1;
        }

        Ok(depth)
    }
}

/// Recursive DFS for cycle detection on the file dependency graph.
fn dfs_cycle_detect(
    node: &str,
    adj: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    on_stack: &mut HashSet<String>,
    stack: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(node.to_string());
    on_stack.insert(node.to_string());
    stack.push(node.to_string());

    if let Some(neighbors) = adj.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                dfs_cycle_detect(neighbor, adj, visited, on_stack, stack, cycles);
            } else if on_stack.contains(neighbor) {
                // Found a cycle. Extract it from the stack.
                let mut cycle = Vec::new();
                let mut found_start = false;
                for item in stack.iter() {
                    if item == neighbor {
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

    stack.pop();
    on_stack.remove(node);
}
