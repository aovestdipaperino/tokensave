//! Code-graph edges.
use std::collections::{HashMap, HashSet};

use grafeo::database::BatchEdge;
use grafeo::store::Direction;
use grafeo::NodeId;

use super::{db_error, edge_from, edge_props, get_i64, get_str, int, Database};
use crate::errors::Result;
use crate::types::{Edge, EdgeKind};

impl Database {
    /// Inserts a single edge.
    pub async fn insert_edge(&self, edge: &Edge) -> Result<()> {
        self.insert_edges(std::slice::from_ref(edge)).await
    }

    /// Inserts edges, keeping one edge per `(source, target, kind, line)`.
    ///
    /// A duplicate only fills in a missing `resolved_by`, as the libSQL
    /// upsert does. An edge whose source or target node does not exist is
    /// skipped: a property graph cannot hold a dangling edge.
    pub async fn insert_edges(&self, edges: &[Edge]) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }
        let mut ids: HashMap<&str, Option<NodeId>> = HashMap::new();
        let mut seen: HashSet<(NodeId, NodeId, &str, Option<u32>)> = HashSet::new();
        let mut batch = Vec::with_capacity(edges.len());
        for edge in edges {
            let (Some(src), Some(dst)) = (
                self.cached_id(&mut ids, &edge.source)?,
                self.cached_id(&mut ids, &edge.target)?,
            ) else {
                continue;
            };
            let kind = edge.kind.as_str();
            if !seen.insert((src, dst, kind, edge.line)) {
                continue;
            }
            if let Some(existing) = self.find_edge(src, dst, kind, edge.line)? {
                if let (None, Some(resolved_by)) = (existing.1, edge.resolved_by) {
                    self.db
                        .set_edge_property(existing.0, "resolved_by", int(resolved_by.code()))
                        .map_err(|e| db_error("insert_edges", e))?;
                }
                continue;
            }
            batch.push(BatchEdge::new(src, dst, kind).with_properties(edge_props(edge)));
        }
        if !batch.is_empty() {
            self.db
                .batch_create_edges(batch)
                .map_err(|e| db_error("insert_edges", e))?;
        }
        Ok(())
    }

    /// Returns the edges leaving `source_id`, optionally of the given kinds.
    pub async fn get_outgoing_edges(
        &self,
        source_id: &str,
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        self.edges_of(source_id, Direction::Outgoing, kinds, "get_outgoing_edges")
    }

    /// Returns the edges entering `target_id`, optionally of the given kinds.
    pub async fn get_incoming_edges(
        &self,
        target_id: &str,
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        self.edges_of(target_id, Direction::Incoming, kinds, "get_incoming_edges")
    }

    fn edges_of(
        &self,
        id: &str,
        direction: Direction,
        kinds: &[EdgeKind],
        operation: &str,
    ) -> Result<Vec<Edge>> {
        let Some(gid) = self.symbol_node_id(id, operation)? else {
            return Ok(Vec::new());
        };
        let types: Vec<&str> = kinds.iter().map(EdgeKind::as_str).collect();
        let adjacent = self
            .db
            .get_node_edges(gid, direction, &types)
            .map_err(|e| db_error(operation, e))?;
        let mut out = Vec::with_capacity(adjacent.len());
        for (other, eid) in adjacent {
            let Some(edge) = self.db.get_edge(eid).map_err(|e| db_error(operation, e))? else {
                continue;
            };
            let Some(other) = self.graph_node(other, operation)? else {
                continue;
            };
            let other_id = get_str(&other, "id");
            out.push(match direction {
                Direction::Incoming => edge_from(other_id, id.to_string(), &edge),
                _ => edge_from(id.to_string(), other_id, &edge),
            });
        }
        Ok(out)
    }

    fn cached_id<'a>(
        &self,
        cache: &mut HashMap<&'a str, Option<NodeId>>,
        id: &'a str,
    ) -> Result<Option<NodeId>> {
        if let Some(found) = cache.get(id) {
            return Ok(*found);
        }
        let found = self.symbol_node_id(id, "insert_edges")?;
        cache.insert(id, found);
        Ok(found)
    }

    /// An existing edge with the same identity, with its `resolved_by`.
    fn find_edge(
        &self,
        src: NodeId,
        dst: NodeId,
        kind: &str,
        line: Option<u32>,
    ) -> Result<Option<(grafeo::store::EdgeId, Option<i64>)>> {
        let adjacent = self
            .db
            .get_node_edges(src, Direction::Outgoing, &[kind])
            .map_err(|e| db_error("insert_edges", e))?;
        for (other, eid) in adjacent {
            if other != dst {
                continue;
            }
            let Some(edge) = self
                .db
                .get_edge(eid)
                .map_err(|e| db_error("insert_edges", e))?
            else {
                continue;
            };
            let edge_line = get_i64(&edge, "line").and_then(|v| u32::try_from(v).ok());
            if edge_line == line {
                return Ok(Some((eid, get_i64(&edge, "resolved_by"))));
            }
        }
        Ok(None)
    }
}
