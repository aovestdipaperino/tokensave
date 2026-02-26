use std::collections::{HashSet, VecDeque};

use crate::db::Database;
use crate::errors::Result;
use crate::types::*;

/// A path through the graph: a sequence of nodes, each paired with the
/// optional edge used to reach it (the first node has `None`).
pub type GraphPath = Vec<(Node, Option<Edge>)>;

/// Performs graph traversal operations on the code graph.
pub struct GraphTraverser<'a> {
    db: &'a Database,
}

impl<'a> GraphTraverser<'a> {
    /// Creates a new `GraphTraverser` backed by the given database.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Performs a breadth-first traversal starting from `start_id`.
    ///
    /// Respects the traversal options including max depth, edge kind filter,
    /// node kind filter, direction, and result limit. Returns a `Subgraph`
    /// containing the discovered nodes and the edges used to reach them.
    pub fn traverse_bfs(&self, start_id: &str, opts: &TraversalOptions) -> Result<Subgraph> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut result_nodes: Vec<Node> = Vec::new();
        let mut result_edges: Vec<Edge> = Vec::new();
        let mut roots: Vec<String> = Vec::new();

        // Queue holds (node_id, current_depth).
        let mut queue: VecDeque<(String, u32)> = VecDeque::new();

        // Optionally include the start node.
        if let Some(start_node) = self.db.get_node_by_id(start_id)? {
            visited.insert(start_id.to_string());
            if opts.include_start && self.node_matches_filter(&start_node, opts) {
                roots.push(start_id.to_string());
                result_nodes.push(start_node);
            }
            queue.push_back((start_id.to_string(), 0));
        } else {
            return Ok(Subgraph {
                nodes: Vec::new(),
                edges: Vec::new(),
                roots: Vec::new(),
            });
        }

        let edge_filter = opts.edge_kinds.as_deref().unwrap_or(&[]);

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= opts.max_depth {
                continue;
            }

            if result_nodes.len() >= opts.limit as usize {
                break;
            }

            let edges = self.get_edges_for_direction(&current_id, edge_filter, &opts.direction)?;

            for edge in edges {
                let neighbor_id = self.neighbor_id(&edge, &current_id, &opts.direction);

                if visited.contains(&neighbor_id) {
                    continue;
                }
                visited.insert(neighbor_id.clone());

                if let Some(neighbor_node) = self.db.get_node_by_id(&neighbor_id)? {
                    if self.node_matches_filter(&neighbor_node, opts) {
                        result_nodes.push(neighbor_node);
                        if result_nodes.len() >= opts.limit as usize {
                            result_edges.push(edge);
                            break;
                        }
                    }
                    result_edges.push(edge);
                    queue.push_back((neighbor_id, depth + 1));
                }
            }
        }

        Ok(Subgraph {
            nodes: result_nodes,
            edges: result_edges,
            roots,
        })
    }

    /// Performs a depth-first traversal starting from `start_id`.
    ///
    /// Respects the traversal options including max depth, edge kind filter,
    /// node kind filter, direction, and result limit. Returns a `Subgraph`
    /// containing the discovered nodes and edges.
    pub fn traverse_dfs(&self, start_id: &str, opts: &TraversalOptions) -> Result<Subgraph> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut result_nodes: Vec<Node> = Vec::new();
        let mut result_edges: Vec<Edge> = Vec::new();
        let mut roots: Vec<String> = Vec::new();

        if let Some(start_node) = self.db.get_node_by_id(start_id)? {
            visited.insert(start_id.to_string());
            if opts.include_start && self.node_matches_filter(&start_node, opts) {
                roots.push(start_id.to_string());
                result_nodes.push(start_node);
            }
            self.dfs_recursive(
                start_id,
                0,
                opts,
                &mut visited,
                &mut result_nodes,
                &mut result_edges,
            )?;
        }

        Ok(Subgraph {
            nodes: result_nodes,
            edges: result_edges,
            roots,
        })
    }

    /// Gets all nodes that call the given node, up to `max_depth` levels.
    ///
    /// Follows incoming `Calls` edges to find callers transitively.
    pub fn get_callers(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>> {
        let mut results: Vec<(Node, Edge)> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(node_id.to_string());

        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((node_id.to_string(), 0));

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let edges = self
                .db
                .get_incoming_edges(&current_id, &[EdgeKind::Calls])?;

            for edge in edges {
                let caller_id = &edge.source;
                if visited.contains(caller_id) {
                    continue;
                }
                visited.insert(caller_id.clone());

                if let Some(caller_node) = self.db.get_node_by_id(caller_id)? {
                    queue.push_back((caller_id.clone(), depth + 1));
                    results.push((caller_node, edge));
                }
            }
        }

        Ok(results)
    }

    /// Gets all nodes that the given node calls, up to `max_depth` levels.
    ///
    /// Follows outgoing `Calls` edges to find callees transitively.
    pub fn get_callees(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>> {
        let mut results: Vec<(Node, Edge)> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(node_id.to_string());

        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((node_id.to_string(), 0));

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let edges = self
                .db
                .get_outgoing_edges(&current_id, &[EdgeKind::Calls])?;

            for edge in edges {
                let callee_id = &edge.target;
                if visited.contains(callee_id) {
                    continue;
                }
                visited.insert(callee_id.clone());

                if let Some(callee_node) = self.db.get_node_by_id(callee_id)? {
                    queue.push_back((callee_id.clone(), depth + 1));
                    results.push((callee_node, edge));
                }
            }
        }

        Ok(results)
    }

    /// Computes the impact radius of a node: all nodes that directly or
    /// indirectly reference or call this node.
    ///
    /// Performs a BFS over incoming edges of all kinds up to `max_depth`.
    pub fn get_impact_radius(&self, node_id: &str, max_depth: usize) -> Result<Subgraph> {
        let opts = TraversalOptions {
            max_depth: max_depth as u32,
            edge_kinds: None,
            node_kinds: None,
            direction: TraversalDirection::Incoming,
            limit: u32::MAX,
            include_start: true,
        };
        self.traverse_bfs(node_id, &opts)
    }

    /// Builds a bidirectional call graph around a node.
    ///
    /// Combines BFS over outgoing `Calls` edges (callees) and BFS over
    /// incoming `Calls` edges (callers) up to the specified `depth`.
    pub fn get_call_graph(&self, node_id: &str, depth: usize) -> Result<Subgraph> {
        // Outgoing (callees)
        let outgoing_opts = TraversalOptions {
            max_depth: depth as u32,
            edge_kinds: Some(vec![EdgeKind::Calls]),
            node_kinds: None,
            direction: TraversalDirection::Outgoing,
            limit: u32::MAX,
            include_start: true,
        };
        let outgoing_sub = self.traverse_bfs(node_id, &outgoing_opts)?;

        // Incoming (callers)
        let incoming_opts = TraversalOptions {
            max_depth: depth as u32,
            edge_kinds: Some(vec![EdgeKind::Calls]),
            node_kinds: None,
            direction: TraversalDirection::Incoming,
            limit: u32::MAX,
            include_start: false,
        };
        let incoming_sub = self.traverse_bfs(node_id, &incoming_opts)?;

        // Merge the two subgraphs, deduplicating nodes by ID.
        let mut seen_nodes: HashSet<String> = HashSet::new();
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        let roots = outgoing_sub.roots;

        for node in outgoing_sub.nodes {
            if seen_nodes.insert(node.id.clone()) {
                nodes.push(node);
            }
        }
        for node in incoming_sub.nodes {
            if seen_nodes.insert(node.id.clone()) {
                nodes.push(node);
            }
        }

        // Deduplicate edges by (source, target, kind).
        let mut seen_edges: HashSet<(String, String, String)> = HashSet::new();
        for edge in outgoing_sub
            .edges
            .into_iter()
            .chain(incoming_sub.edges.into_iter())
        {
            let key = (
                edge.source.clone(),
                edge.target.clone(),
                edge.kind.as_str().to_string(),
            );
            if seen_edges.insert(key) {
                edges.push(edge);
            }
        }

        Ok(Subgraph {
            nodes,
            edges,
            roots,
        })
    }

    /// Discovers the type hierarchy around a node by following `Implements` edges.
    ///
    /// Follows both outgoing (traits this node implements) and incoming
    /// (nodes that implement this trait) `Implements` edges.
    pub fn get_type_hierarchy(&self, node_id: &str) -> Result<Subgraph> {
        let opts = TraversalOptions {
            max_depth: 10,
            edge_kinds: Some(vec![EdgeKind::Implements]),
            node_kinds: None,
            direction: TraversalDirection::Both,
            limit: u32::MAX,
            include_start: true,
        };
        self.traverse_bfs(node_id, &opts)
    }

    /// Finds the shortest path between two nodes using BFS.
    ///
    /// If `edge_kinds` is empty, all edge types are followed. Returns `None`
    /// if no path exists. The returned path includes the start and end nodes
    /// with the edges connecting them.
    pub fn find_path(
        &self,
        from_id: &str,
        to_id: &str,
        edge_kinds: &[EdgeKind],
    ) -> Result<Option<GraphPath>> {
        if from_id == to_id {
            if let Some(node) = self.db.get_node_by_id(from_id)? {
                return Ok(Some(vec![(node, None)]));
            }
            return Ok(None);
        }

        // BFS: track parent info for path reconstruction.
        // parent_map: child_id -> (parent_id, edge_used)
        let mut parent_map: std::collections::HashMap<String, (String, Edge)> =
            std::collections::HashMap::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        visited.insert(from_id.to_string());
        queue.push_back(from_id.to_string());

        let mut found = false;

        while let Some(current_id) = queue.pop_front() {
            // Get outgoing edges.
            let outgoing = self.db.get_outgoing_edges(&current_id, edge_kinds)?;
            for edge in outgoing {
                let neighbor = edge.target.clone();
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    let is_target = neighbor == to_id;
                    parent_map.insert(neighbor.clone(), (current_id.clone(), edge));

                    if is_target {
                        found = true;
                        break;
                    }
                    queue.push_back(neighbor);
                }
            }

            if found {
                break;
            }

            // Also get incoming edges (traverse bidirectionally for path finding).
            let incoming = self.db.get_incoming_edges(&current_id, edge_kinds)?;
            for edge in incoming {
                let neighbor = edge.source.clone();
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    let is_target = neighbor == to_id;
                    parent_map.insert(neighbor.clone(), (current_id.clone(), edge));

                    if is_target {
                        found = true;
                        break;
                    }
                    queue.push_back(neighbor);
                }
            }

            if found {
                break;
            }
        }

        if !found {
            return Ok(None);
        }

        // Reconstruct path from to_id back to from_id.
        let mut path_ids: Vec<(String, Option<Edge>)> = Vec::new();
        let mut current = to_id.to_string();
        while current != from_id {
            if let Some((parent, edge)) = parent_map.remove(&current) {
                path_ids.push((current, Some(edge)));
                current = parent;
            } else {
                return Ok(None);
            }
        }
        path_ids.push((from_id.to_string(), None));
        path_ids.reverse();

        // Resolve node IDs to actual Node objects.
        let mut path: Vec<(Node, Option<Edge>)> = Vec::new();
        for (id, edge) in path_ids {
            if let Some(node) = self.db.get_node_by_id(&id)? {
                path.push((node, edge));
            }
        }

        Ok(Some(path))
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Recursively performs DFS, collecting nodes and edges.
    fn dfs_recursive(
        &self,
        current_id: &str,
        depth: u32,
        opts: &TraversalOptions,
        visited: &mut HashSet<String>,
        result_nodes: &mut Vec<Node>,
        result_edges: &mut Vec<Edge>,
    ) -> Result<()> {
        if depth >= opts.max_depth {
            return Ok(());
        }

        if result_nodes.len() >= opts.limit as usize {
            return Ok(());
        }

        let edge_filter = opts.edge_kinds.as_deref().unwrap_or(&[]);
        let edges = self.get_edges_for_direction(current_id, edge_filter, &opts.direction)?;

        for edge in edges {
            let neighbor_id = self.neighbor_id(&edge, current_id, &opts.direction);

            if visited.contains(&neighbor_id) {
                continue;
            }
            visited.insert(neighbor_id.clone());

            if let Some(neighbor_node) = self.db.get_node_by_id(&neighbor_id)? {
                if self.node_matches_filter(&neighbor_node, opts) {
                    result_nodes.push(neighbor_node);
                    if result_nodes.len() >= opts.limit as usize {
                        result_edges.push(edge);
                        return Ok(());
                    }
                }
                result_edges.push(edge);
                self.dfs_recursive(
                    &neighbor_id,
                    depth + 1,
                    opts,
                    visited,
                    result_nodes,
                    result_edges,
                )?;
            }
        }

        Ok(())
    }

    /// Gets edges from the database according to the traversal direction.
    fn get_edges_for_direction(
        &self,
        node_id: &str,
        edge_kinds: &[EdgeKind],
        direction: &TraversalDirection,
    ) -> Result<Vec<Edge>> {
        match direction {
            TraversalDirection::Outgoing => self.db.get_outgoing_edges(node_id, edge_kinds),
            TraversalDirection::Incoming => self.db.get_incoming_edges(node_id, edge_kinds),
            TraversalDirection::Both => {
                let mut edges = self.db.get_outgoing_edges(node_id, edge_kinds)?;
                edges.extend(self.db.get_incoming_edges(node_id, edge_kinds)?);
                Ok(edges)
            }
        }
    }

    /// Returns the neighbor node ID from an edge, depending on direction.
    ///
    /// For outgoing: the neighbor is `edge.target`.
    /// For incoming: the neighbor is `edge.source`.
    /// For both: whichever end is not `current_id`.
    fn neighbor_id(&self, edge: &Edge, current_id: &str, direction: &TraversalDirection) -> String {
        match direction {
            TraversalDirection::Outgoing => edge.target.clone(),
            TraversalDirection::Incoming => edge.source.clone(),
            TraversalDirection::Both => {
                if edge.source == current_id {
                    edge.target.clone()
                } else {
                    edge.source.clone()
                }
            }
        }
    }

    /// Checks whether a node passes the optional `node_kinds` filter.
    fn node_matches_filter(&self, node: &Node, opts: &TraversalOptions) -> bool {
        if let Some(ref kinds) = opts.node_kinds {
            if !kinds.is_empty() {
                return kinds.contains(&node.kind);
            }
        }
        true
    }
}
