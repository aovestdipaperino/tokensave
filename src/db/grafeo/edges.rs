//! Code-graph edges and the reverse trait-dispatch caller map.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::PoisonError;

use grafeo::database::BatchEdge;
use grafeo::store::{Direction, EdgeId};
use grafeo::NodeId;

use super::{
    db_error, edge_from, edge_props, ends_with_ci, get_i64, get_str, int, text, Database,
    DispatchMap, SYMBOL,
};
use crate::db::model::CachedTraitDispatchCaller;
use crate::errors::Result;
use crate::types::{Edge, EdgeKind, Node, NodeKind};

/// Extensions of the files whose classes and modules can be reopened.
const RUBY_EXTENSIONS: &[&str] = &[".rb", ".rake", ".erb", ".slim"];

impl Database {
    /// Inserts a single edge.
    pub async fn insert_edge(&self, edge: &Edge) -> Result<()> {
        self.insert_edges(std::slice::from_ref(edge)).await
    }

    /// Inserts edges, keeping one edge per `(source, target, kind, line)`.
    ///
    /// A duplicate only fills in a missing `resolved_by`, as the libSQL
    /// upsert does. An edge whose source or target node does not exist is
    /// skipped: a property graph cannot hold a dangling edge. A `contains`
    /// edge is stored as its target's `parent_id`, as in the libSQL backend.
    pub async fn insert_edges(&self, edges: &[Edge]) -> Result<()> {
        let op = "insert_edges";
        if edges.is_empty() {
            return Ok(());
        }
        self.graph_changed();
        let mut ids: HashMap<&str, Option<NodeId>> = HashMap::new();
        let mut seen: HashSet<(NodeId, NodeId, &str, Option<u32>)> = HashSet::new();
        let mut batch = Vec::with_capacity(edges.len());
        for edge in edges {
            if edge.kind == EdgeKind::Contains {
                if let Some(child) = self.cached_id(&mut ids, &edge.target)? {
                    self.db
                        .set_node_property(child, "parent_id", text(&edge.source))
                        .map_err(|e| db_error(op, &e))?;
                }
                continue;
            }
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
                        .map_err(|e| db_error(op, &e))?;
                }
                continue;
            }
            batch.push(BatchEdge::new(src, dst, kind).with_properties(edge_props(edge)));
        }
        if !batch.is_empty() {
            self.db
                .batch_create_edges(batch)
                .map_err(|e| db_error(op, &e))?;
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

    /// Returns the incoming edges of many targets, optionally of the given
    /// kinds.
    pub async fn get_incoming_edges_bulk(
        &self,
        target_ids: &[String],
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        let mut out = Vec::new();
        for id in target_ids {
            out.extend(self.edges_of(id, Direction::Incoming, kinds, "get_incoming_edges_bulk")?);
        }
        Ok(out)
    }

    /// Returns every edge in the database.
    pub async fn get_all_edges(&self) -> Result<Vec<Edge>> {
        self.scan_edges(&[], "get_all_edges")
    }

    /// Returns every edge of one kind.
    pub async fn get_edges_by_kind(&self, kind: EdgeKind) -> Result<Vec<Edge>> {
        self.scan_edges(&[kind], "get_edges_by_kind")
    }

    /// Returns every edge whose kind is in `kinds`; nothing when `kinds` is
    /// empty.
    pub async fn get_edges_by_kinds(&self, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        self.scan_edges(kinds, "get_edges_by_kinds")
    }

    /// Every `calls` edge whose target is one of `targets` (#481).
    pub async fn calls_edges_into(&self, targets: &[String]) -> Result<Vec<Edge>> {
        let mut out = Vec::new();
        for target in targets {
            out.extend(self.edges_of(
                target,
                Direction::Incoming,
                &[EdgeKind::Calls],
                "calls_edges_into",
            )?);
        }
        Ok(out)
    }

    /// Returns edges whose source and target are both in `node_ids`.
    pub async fn get_internal_edges(&self, node_ids: &[String]) -> Result<Vec<Edge>> {
        let ids: HashSet<&str> = node_ids.iter().map(String::as_str).collect();
        let mut out = Vec::new();
        for id in node_ids {
            out.extend(
                self.edges_of(id, Direction::Outgoing, &[], "get_internal_edges")?
                    .into_iter()
                    .filter(|e| ids.contains(e.target.as_str())),
            );
        }
        Ok(out)
    }

    /// Deletes all edges originating from a given source node.
    pub async fn delete_edges_by_source(&self, source_id: &str) -> Result<()> {
        let op = "delete_edges_by_source";
        let Some(gid) = self.symbol_node_id(source_id, op)? else {
            return Ok(());
        };
        self.graph_changed();
        let adjacent = self
            .db
            .get_node_edges(gid, Direction::Outgoing, &[])
            .map_err(|e| db_error(op, &e))?;
        for (_, eid) in adjacent {
            self.db.delete_edge(eid).map_err(|e| db_error(op, &e))?;
        }
        Ok(())
    }

    /// Deletes the `calls` edges that call sites may have produced, each site
    /// given as (caller id, line, callee bare name) (#597).
    pub async fn delete_call_edges_at_sites(&self, sites: &[(String, u32, String)]) -> Result<()> {
        let op = "delete_call_edges_at_sites";
        if sites.is_empty() {
            return Ok(());
        }
        self.graph_changed();
        for (source, line, name) in sites {
            let Some(gid) = self.symbol_node_id(source, op)? else {
                continue;
            };
            let adjacent = self
                .db
                .get_node_edges(gid, Direction::Outgoing, &[EdgeKind::Calls.as_str()])
                .map_err(|e| db_error(op, &e))?;
            for (target, eid) in adjacent {
                let Some(edge) = self.db.get_edge(eid).map_err(|e| db_error(op, &e))? else {
                    continue;
                };
                if get_i64(&edge, "line") != Some(i64::from(*line)) {
                    continue;
                }
                let Some(target) = self.graph_node(target, op)? else {
                    continue;
                };
                if get_str(&target, "name") == *name {
                    self.db.delete_edge(eid).map_err(|e| db_error(op, &e))?;
                }
            }
        }
        Ok(())
    }

    /// Links later Ruby declarations of a class or module to the first
    /// declaration of the same constant with a `reopens` edge, recomputing
    /// the whole relation.
    pub async fn rebuild_ruby_reopenings(&self) -> Result<()> {
        let op = "rebuild_ruby_reopenings";
        self.graph_changed();
        let rows = self.query_rows(
            &format!(
                "MATCH (:{SYMBOL})-[e:{}]->(:{SYMBOL}) RETURN id(e)",
                EdgeKind::Reopens.as_str()
            ),
            op,
        )?;
        for row in rows {
            let Some(id) = row
                .first()
                .and_then(grafeo::Value::as_int64)
                .and_then(|v| u64::try_from(v).ok())
            else {
                continue;
            };
            self.db
                .delete_edge(EdgeId::new(id))
                .map_err(|e| db_error(op, &e))?;
        }

        let mut declarations: HashMap<(NodeKind, String), Vec<Node>> = HashMap::new();
        for node in self.all_symbols(op)? {
            if !matches!(node.kind, NodeKind::Class | NodeKind::Module)
                || !RUBY_EXTENSIONS
                    .iter()
                    .any(|ext| ends_with_ci(&node.file_path, ext))
                || node.qualified_name.contains("<anonymous>")
            {
                continue;
            }
            let Some(constant) = reopening_constant(&node.qualified_name, &node.file_path) else {
                continue;
            };
            declarations
                .entry((node.kind.clone(), constant))
                .or_default()
                .push(node);
        }
        let mut reopenings = Vec::new();
        for nodes in declarations.values_mut() {
            nodes.sort_by(|a, b| {
                a.file_path
                    .cmp(&b.file_path)
                    .then(a.start_line.cmp(&b.start_line))
                    .then(a.start_column.cmp(&b.start_column))
                    .then(a.id.cmp(&b.id))
            });
            let Some((canonical, later)) = nodes.split_first() else {
                continue;
            };
            reopenings.extend(later.iter().map(|node| Edge {
                source: node.id.clone(),
                target: canonical.id.clone(),
                kind: EdgeKind::Reopens,
                line: Some(node.start_line),
                resolved_by: None,
            }));
        }
        self.insert_edges(&reopenings).await
    }

    /// Recomputes the reverse trait-dispatch caller map.
    pub async fn rebuild_trait_dispatch_callers(&self) -> Result<()> {
        self.refresh_trait_dispatch_callers().await
    }

    /// Recomputes the reverse trait-dispatch caller map from the graph.
    pub async fn refresh_trait_dispatch_callers(&self) -> Result<()> {
        self.dispatch_dirty.store(false, Ordering::SeqCst);
        match self.compute_trait_dispatch() {
            Ok(map) => {
                *self
                    .trait_dispatch
                    .write()
                    .unwrap_or_else(PoisonError::into_inner) = map;
                Ok(())
            }
            Err(e) => {
                self.dispatch_dirty.store(true, Ordering::SeqCst);
                Err(e)
            }
        }
    }

    /// Whether any caller reaches `node_id` through trait dispatch.
    #[must_use]
    pub fn has_trait_dispatch_callers(&self, node_id: &str) -> bool {
        self.ensure_trait_dispatch();
        self.trait_dispatch
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(node_id)
    }

    /// The callers that reach `node_id` through trait dispatch.
    #[must_use]
    pub fn cached_trait_dispatch_callers(&self, node_id: &str) -> Vec<CachedTraitDispatchCaller> {
        self.ensure_trait_dispatch();
        self.trait_dispatch
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Recomputes the caller map if the graph changed since it was built.
    /// A failure keeps the previous map and retries on the next read.
    fn ensure_trait_dispatch(&self) {
        if !self.dispatch_dirty.swap(false, Ordering::SeqCst) {
            return;
        }
        match self.compute_trait_dispatch() {
            Ok(map) => {
                *self
                    .trait_dispatch
                    .write()
                    .unwrap_or_else(PoisonError::into_inner) = map;
            }
            Err(_) => self.dispatch_dirty.store(true, Ordering::SeqCst),
        }
    }

    /// For every `implements` edge, pairs each method of the implementing
    /// block with the same-named method of the trait, and records the
    /// callers of the trait method against the concrete method.
    fn compute_trait_dispatch(&self) -> Result<DispatchMap> {
        let op = "refresh_trait_dispatch_callers";
        let is_callable = |n: &Node| matches!(n.kind, NodeKind::Method | NodeKind::Function);
        let mut seen: HashSet<(String, String, String, i64)> = HashSet::new();
        let mut map: DispatchMap = HashMap::new();
        for dispatch in self.scan_edges(&[EdgeKind::Implements], op)? {
            let trait_methods: Vec<Node> = self
                .symbols_where("parent_id", &dispatch.target, op)?
                .into_iter()
                .filter(is_callable)
                .collect();
            if trait_methods.is_empty() {
                continue;
            }
            let concretes: Vec<Node> = self
                .symbols_where("parent_id", &dispatch.source, op)?
                .into_iter()
                .filter(is_callable)
                .collect();
            for trait_method in &trait_methods {
                let matching: Vec<&Node> = concretes
                    .iter()
                    .filter(|c| c.name == trait_method.name)
                    .collect();
                if matching.is_empty() {
                    continue;
                }
                let calls = self.edges_of(
                    &trait_method.id,
                    Direction::Incoming,
                    &[EdgeKind::Calls],
                    op,
                )?;
                for concrete in &matching {
                    for call in &calls {
                        let line = call.line.map_or(-1, i64::from);
                        let key = (
                            concrete.id.clone(),
                            trait_method.id.clone(),
                            call.source.clone(),
                            line,
                        );
                        if !seen.insert(key) {
                            continue;
                        }
                        let Some(caller) = self.node_by_id(&call.source, op)? else {
                            continue;
                        };
                        let has_upstream = !self
                            .edges_of(&caller.id, Direction::Incoming, &[EdgeKind::Calls], op)?
                            .is_empty();
                        let edge = Edge {
                            source: caller.id.clone(),
                            target: trait_method.id.clone(),
                            kind: EdgeKind::Calls,
                            line: call.line,
                            resolved_by: None,
                        };
                        map.entry(concrete.id.clone()).or_default().push((
                            caller,
                            edge,
                            trait_method.id.clone(),
                            has_upstream,
                        ));
                    }
                }
            }
        }
        for callers in map.values_mut() {
            callers.sort_by(|a, b| {
                a.0.file_path
                    .cmp(&b.0.file_path)
                    .then(a.0.start_line.cmp(&b.0.start_line))
            });
        }
        Ok(map)
    }

    /// The edges of `id` in `direction`, of the given kinds or all kinds.
    pub(super) fn edges_of(
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
            .map_err(|e| db_error(operation, &e))?;
        let mut out = Vec::with_capacity(adjacent.len());
        for (other, eid) in adjacent {
            let Some(edge) = self.db.get_edge(eid).map_err(|e| db_error(operation, &e))? else {
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
    ) -> Result<Option<(EdgeId, Option<i64>)>> {
        let adjacent = self
            .db
            .get_node_edges(src, Direction::Outgoing, &[kind])
            .map_err(|e| db_error("insert_edges", &e))?;
        for (other, eid) in adjacent {
            if other != dst {
                continue;
            }
            let Some(edge) = self
                .db
                .get_edge(eid)
                .map_err(|e| db_error("insert_edges", &e))?
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

/// The constant a Ruby class or module declares, from its qualified name:
/// `file::file::Name` (current extraction) or `file::Name`, with a leading
/// `::` removed. `None` when the name does not start with `file::`.
fn reopening_constant(qualified_name: &str, file_path: &str) -> Option<String> {
    let single = format!("{file_path}::");
    let rest = qualified_name.strip_prefix(&single)?;
    let rest = rest.strip_prefix(&single).unwrap_or(rest);
    Some(rest.trim_start_matches(':').to_string())
}

#[cfg(test)]
mod tests {
    use super::reopening_constant;

    #[test]
    fn reopening_constant_strips_one_or_two_file_prefixes() {
        assert_eq!(
            reopening_constant("a.rb::a.rb::Admin::Notes", "a.rb").as_deref(),
            Some("Admin::Notes")
        );
        assert_eq!(
            reopening_constant("a.rb::Notes", "a.rb").as_deref(),
            Some("Notes")
        );
        assert_eq!(
            reopening_constant("a.rb::::Entry", "a.rb").as_deref(),
            Some("Entry")
        );
        assert_eq!(reopening_constant("b.rb::Notes", "a.rb"), None);
    }
}
