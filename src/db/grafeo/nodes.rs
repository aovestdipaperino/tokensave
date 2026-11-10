//! Code-graph nodes.
use std::collections::HashMap;

use grafeo::store::Direction;

use super::{db_error, ends_with_ci, get_str, node_from, node_props, under_path, Database, SYMBOL};
use crate::db::model::NodeFilter;
use crate::errors::Result;
use crate::resolution::TouchedNode;
use crate::types::{Edge, EdgeKind, FileRecord, Node, NodeKind, Visibility};

/// Kinds whose signature resolution reads in `.gd` files (#597).
const GDSCRIPT_TYPED_KINDS: &[NodeKind] = &[
    NodeKind::Class,
    NodeKind::InnerClass,
    NodeKind::Function,
    NodeKind::Method,
    NodeKind::Field,
];

/// Kinds whose signature resolution reads in `.cs` files (#642).
const CSHARP_TYPED_KINDS: &[NodeKind] = &[
    NodeKind::Class,
    NodeKind::InnerClass,
    NodeKind::Struct,
    NodeKind::Interface,
    NodeKind::Record,
    NodeKind::Function,
    NodeKind::Method,
    NodeKind::Field,
    NodeKind::CSharpProperty,
];

impl Database {
    /// Inserts or replaces a single node.
    pub async fn insert_node(&self, node: &Node) -> Result<()> {
        self.insert_nodes(std::slice::from_ref(node)).await
    }

    /// Inserts nodes, edges and file records, in that order.
    pub async fn insert_all(
        &self,
        nodes: &[Node],
        edges: &[Edge],
        files: &[FileRecord],
    ) -> Result<()> {
        self.insert_nodes(nodes).await?;
        self.insert_edges(edges).await?;
        self.upsert_files(files).await
    }

    /// Inserts nodes; a node whose id already exists is replaced, keeping
    /// its edges, as `INSERT OR REPLACE` does in the libSQL backend.
    pub async fn insert_nodes(&self, nodes: &[Node]) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }
        self.graph_changed();
        let mut new = Vec::with_capacity(nodes.len());
        for node in nodes {
            match self.symbol_node_id(&node.id, "insert_nodes")? {
                Some(existing) => {
                    for (key, value) in node_props(node) {
                        self.db
                            .set_node_property(existing, key.as_str(), value)
                            .map_err(|e| db_error("insert_nodes", &e))?;
                    }
                    // Optional fields absent from the new version must not
                    // survive from the old one.
                    for (key, missing) in [
                        ("signature", node.signature.is_none()),
                        ("docstring", node.docstring.is_none()),
                        ("parent_id", node.parent_id.is_none()),
                    ] {
                        if missing {
                            self.db
                                .remove_node_property(existing, key)
                                .map_err(|e| db_error("insert_nodes", &e))?;
                        }
                    }
                }
                None => new.push(node_props(node)),
            }
        }
        if !new.is_empty() {
            self.db
                .batch_create_nodes_with_props(SYMBOL, new)
                .map_err(|e| db_error("insert_nodes", &e))?;
        }
        Ok(())
    }

    /// Returns the node with the given id.
    pub async fn get_node_by_id(&self, id: &str) -> Result<Option<Node>> {
        self.node_by_id(id, "get_node_by_id")
    }

    /// Returns the nodes with the given ids that exist, in no defined order.
    pub async fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<Node>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(node) = self.node_by_id(id, "get_nodes_by_ids")? {
                out.push(node);
            }
        }
        Ok(out)
    }

    /// Returns the nodes of one file, ordered by start line.
    pub async fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        let mut out = self.symbols_where("file_path", file_path, "get_nodes_by_file")?;
        out.sort_by_key(|n| n.start_line);
        Ok(out)
    }

    /// Returns the direct children of a node, ordered by start line.
    pub async fn get_children_of(&self, parent_id: &str) -> Result<Vec<Node>> {
        let mut out = self.symbols_where("parent_id", parent_id, "get_children_of")?;
        out.sort_by_key(|n| n.start_line);
        Ok(out)
    }

    /// Returns the children of many parent scopes, ordered by file and line.
    pub async fn get_children_of_many(&self, parent_ids: &[String]) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        for parent in parent_ids {
            out.extend(self.symbols_where("parent_id", parent, "get_children_of_many")?);
        }
        sort_by_location(&mut out);
        Ok(out)
    }

    /// Resolves a method in an impl block to same-named methods on the trait
    /// implemented by that block.
    pub async fn get_trait_methods_for_impl_method(
        &self,
        impl_id: &str,
        method_name: &str,
    ) -> Result<Vec<Node>> {
        let op = "get_trait_methods_for_impl_method";
        let Some(impl_gid) = self.symbol_node_id(impl_id, op)? else {
            return Ok(Vec::new());
        };
        let targets = self
            .db
            .get_node_edges(
                impl_gid,
                Direction::Outgoing,
                &[EdgeKind::Implements.as_str()],
            )
            .map_err(|e| db_error(op, &e))?;
        let mut out = Vec::new();
        for (target, _) in targets {
            let Some(trait_node) = self.graph_node(target, op)? else {
                continue;
            };
            if get_str(&trait_node, "kind") != NodeKind::Trait.as_str() {
                continue;
            }
            for method in self.symbols_where("parent_id", &get_str(&trait_node, "id"), op)? {
                if method.name == method_name
                    && matches!(method.kind, NodeKind::Method | NodeKind::Function)
                {
                    out.push(method);
                }
            }
        }
        sort_by_location(&mut out);
        Ok(out)
    }

    /// Returns all nodes of a given kind.
    pub async fn get_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<Node>> {
        self.symbols_where("kind", kind.as_str(), "get_nodes_by_kind")
    }

    /// Returns every node in the database.
    pub async fn get_all_nodes(&self) -> Result<Vec<Node>> {
        self.all_symbols("get_all_nodes")
    }

    /// Returns all nodes whose `name` equals `name`, at most 200.
    pub async fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>> {
        let mut out = self.symbols_where("name", name, "get_nodes_by_name")?;
        out.truncate(200);
        Ok(out)
    }

    /// Returns the nodes whose qualified name is `qname`. Without an exact
    /// match, a `::`-qualified name falls back to a suffix match and a bare
    /// name to a name match, each capped at 50 rows.
    pub async fn get_nodes_by_qualified_name(&self, qname: &str) -> Result<Vec<Node>> {
        let op = "get_nodes_by_qualified_name";
        let exact = self.symbols_where("qualified_name", qname, op)?;
        if !exact.is_empty() {
            return Ok(exact);
        }
        let mut out = if qname.contains("::") {
            let suffix = format!("::{qname}");
            self.all_symbols(op)?
                .into_iter()
                .filter(|n| ends_with_ci(&n.qualified_name, &suffix))
                .take(50)
                .collect()
        } else {
            self.symbols_where("name", qname, op)?
        };
        out.truncate(50);
        Ok(out)
    }

    /// Every node's id paired with its file path.
    pub async fn get_node_paths(&self) -> Result<HashMap<String, String>> {
        Ok(self
            .scan_label(SYMBOL, "get_node_paths")?
            .iter()
            .map(|p| (get_str(p.as_ref(), "id"), get_str(p.as_ref(), "file_path")))
            .collect())
    }

    /// Every node matching `filter`.
    pub async fn get_nodes_filtered(&self, filter: &NodeFilter) -> Result<Vec<Node>> {
        if filter.kinds.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        Ok(self
            .all_symbols("get_nodes_filtered")?
            .into_iter()
            .filter(|n| matches_filter(n, filter))
            .collect())
    }

    /// Every node, without the docstrings and signatures resolution never
    /// reads. `GDScript` and C# declarations keep their signatures, which
    /// carry the static types the resolver reads (#597, #642).
    pub async fn get_all_nodes_for_resolution(&self) -> Result<Vec<Node>> {
        let mut nodes = self.all_symbols("get_all_nodes_for_resolution")?;
        for node in &mut nodes {
            node.docstring = None;
            let keep = (ends_with_ci(&node.file_path, ".gd")
                && GDSCRIPT_TYPED_KINDS.contains(&node.kind))
                || (ends_with_ci(&node.file_path, ".cs")
                    && CSHARP_TYPED_KINDS.contains(&node.kind));
            if !keep {
                node.signature = None;
            }
        }
        Ok(nodes)
    }

    /// The identifying fields of a file's nodes, for the incremental
    /// resolution touched-name set (#484).
    pub async fn touched_nodes_by_file(&self, file_path: &str) -> Result<Vec<TouchedNode>> {
        Ok(self
            .symbols_where("file_path", file_path, "touched_nodes_by_file")?
            .into_iter()
            .map(|n| TouchedNode {
                kind: n.kind,
                name: n.name,
                qualified_name: n.qualified_name,
            })
            .collect())
    }

    /// Returns all nodes in a file or under a directory with one of `kinds`,
    /// ordered by file and line.
    pub async fn get_nodes_by_dir(&self, dir: &str, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<Node> = self
            .all_symbols("get_nodes_by_dir")?
            .into_iter()
            .filter(|n| under_path(&n.file_path, dir) && kinds.contains(&n.kind))
            .collect();
        sort_by_location(&mut out);
        Ok(out)
    }

    /// Deletes a file's nodes with their edges, unresolved references,
    /// fingerprints and executable-body documents.
    pub async fn delete_nodes_by_file(&self, file_path: &str) -> Result<()> {
        let op = "delete_nodes_by_file";
        self.graph_changed();
        for gid in self.find("file_path", file_path, op)? {
            if let Some(node) = self.graph_node(gid, op)? {
                let fingerprints = self.find("fp_node", &get_str(&node, "id"), op)?;
                self.delete_graph_nodes(&fingerprints, op)?;
            }
            self.db
                .detach_delete_node(gid)
                .map_err(|e| db_error(op, &e))?;
        }
        let refs = self.find("ref_file_path", file_path, op)?;
        self.delete_graph_nodes(&refs, op)?;
        let bodies = self.find("body_file", file_path, op)?;
        self.delete_graph_nodes(&bodies, op)
    }

    /// The node `id`, read through the `id` index.
    pub(super) fn node_by_id(&self, id: &str, operation: &str) -> Result<Option<Node>> {
        let Some(gid) = self.symbol_node_id(id, operation)? else {
            return Ok(None);
        };
        Ok(self.graph_node(gid, operation)?.map(|n| node_from(&n)))
    }

    /// Symbols whose indexed `property` equals `value`, in index order.
    pub(super) fn symbols_where(
        &self,
        property: &str,
        value: &str,
        operation: &str,
    ) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        for gid in self.find(property, value, operation)? {
            if let Some(n) = self.graph_node(gid, operation)? {
                out.push(node_from(&n));
            }
        }
        Ok(out)
    }
}

/// Orders nodes by file, then start line, as `ORDER BY file_path,
/// start_line` does.
pub(super) fn sort_by_location(nodes: &mut [Node]) {
    nodes.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.start_line.cmp(&b.start_line))
    });
}

/// Whether `node` satisfies every predicate of `filter`, as the libSQL
/// `NodeFilter::where_clause` does.
fn matches_filter(node: &Node, filter: &NodeFilter) -> bool {
    if let Some(prefix) = &filter.path_prefix {
        if !under_path(&node.file_path, prefix) {
            return false;
        }
    }
    if let Some(kinds) = &filter.kinds {
        if !kinds.contains(&node.kind.as_str()) {
            return false;
        }
    }
    if filter.public_only && node.visibility != Visibility::Pub {
        return false;
    }
    if let Some(lines) = filter.min_lines {
        let span = i64::from(node.end_line) + 1 - i64::from(node.start_line);
        if span < i64::from(lines) {
            return false;
        }
    }
    if let Some(needle) = &filter.name_contains {
        let needle = needle.as_str();
        if !node.name.to_ascii_lowercase().contains(needle)
            && !node.qualified_name.to_ascii_lowercase().contains(needle)
        {
            return false;
        }
    }
    true
}
