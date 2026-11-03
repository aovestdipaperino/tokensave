//! Code-graph nodes.
use super::{db_error, node_from, node_props, Database, SYMBOL};
use crate::errors::Result;
use crate::types::{Edge, FileRecord, Node};

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
        let mut new = Vec::with_capacity(nodes.len());
        for node in nodes {
            match self.symbol_node_id(&node.id, "insert_nodes")? {
                Some(existing) => {
                    for (key, value) in node_props(node) {
                        self.db
                            .set_node_property(existing, key.as_str(), value)
                            .map_err(|e| db_error("insert_nodes", e))?;
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
                                .map_err(|e| db_error("insert_nodes", e))?;
                        }
                    }
                }
                None => new.push(node_props(node)),
            }
        }
        if !new.is_empty() {
            self.db
                .batch_create_nodes_with_props(SYMBOL, new)
                .map_err(|e| db_error("insert_nodes", e))?;
        }
        Ok(())
    }

    /// Returns the node with the given id.
    pub async fn get_node_by_id(&self, id: &str) -> Result<Option<Node>> {
        let Some(gid) = self.symbol_node_id(id, "get_node_by_id")? else {
            return Ok(None);
        };
        Ok(self
            .graph_node(gid, "get_node_by_id")?
            .map(|n| node_from(&n)))
    }

    /// Returns the nodes with the given ids that exist, in no defined order.
    pub async fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<Node>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(node) = self.get_node_by_id(id).await? {
                out.push(node);
            }
        }
        Ok(out)
    }

    /// Returns the nodes of one file, ordered by start line.
    pub async fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        self.symbols_where("file_path", file_path, "get_nodes_by_file")
    }

    /// Returns the direct children of a node, ordered by start line.
    pub async fn get_children_of(&self, parent_id: &str) -> Result<Vec<Node>> {
        self.symbols_where("parent_id", parent_id, "get_children_of")
    }

    /// Deletes a file's nodes with their edges and unresolved references.
    pub async fn delete_nodes_by_file(&self, file_path: &str) -> Result<()> {
        for gid in self.find("file_path", file_path, "delete_nodes_by_file")? {
            self.db
                .detach_delete_node(gid)
                .map_err(|e| db_error("delete_nodes_by_file", e))?;
        }
        for gid in self.find("ref_file_path", file_path, "delete_nodes_by_file")? {
            self.db
                .delete_node(gid)
                .map_err(|e| db_error("delete_nodes_by_file", e))?;
        }
        Ok(())
    }

    /// Symbols whose indexed `property` equals `value`, by start line.
    fn symbols_where(&self, property: &str, value: &str, operation: &str) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        for gid in self.find(property, value, operation)? {
            if let Some(n) = self.graph_node(gid, operation)? {
                out.push(node_from(&n));
            }
        }
        out.sort_by_key(|n| n.start_line);
        Ok(out)
    }
}
