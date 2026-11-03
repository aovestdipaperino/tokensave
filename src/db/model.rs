//! Backend-neutral types of the storage API (#904).
//!
//! Every storage backend takes and returns these types, so they live
//! outside any one backend's query code.

use crate::types::{Edge, Node, NodeKind};

/// A trait-dispatch caller kept in the per-database cache: the caller node,
/// the call edge, the trait method id and whether the call is direct.
pub(crate) type CachedTraitDispatchCaller = (Node, Edge, String, bool);

/// A predicate for [`Database::get_nodes_filtered`](super::Database::get_nodes_filtered), built by the caller
/// instead of applied to a materialised `Vec<Node>` in Rust.
///
/// Six MCP handlers used to call `get_all_nodes()` and filter the result,
/// so a single tool call materialised the whole node table to keep a
/// fraction of it (#410). Each field here is one of the predicates they
/// applied; an empty filter is exactly `get_all_nodes()`.
#[derive(Debug, Clone, Default)]
pub struct NodeFilter {
    pub(crate) path_prefix: Option<String>,
    pub(crate) kinds: Option<Vec<&'static str>>,
    pub(crate) public_only: bool,
    pub(crate) min_lines: Option<u32>,
    pub(crate) name_contains: Option<String>,
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
}

/// A stored fingerprint row, paired with its node id.
#[derive(Debug, Clone)]
pub struct StoredFingerprint {
    pub node_id: String,
    pub ast_hash: String,
    pub cfg_hash: String,
    pub call_seq_hash: String,
    pub shingles: Vec<u32>,
    pub body_tokens: u32,
    pub source_hash: String,
}

/// One resolved, cross-file `uses` edge from an import statement, as read by
/// [`Database::get_cross_file_import_rows`].
#[derive(Debug, Clone)]
pub struct ImportEdgeRow {
    /// File containing the import statement.
    pub file: String,
    /// Zero-based line of the `Use` node.
    pub line: u32,
    /// Name of the `Use` node (the imported path).
    pub imported: String,
    /// Source text of the statement, when stored.
    pub statement: Option<String>,
    /// File of the node the import resolved to.
    pub resolved_file: String,
    /// Kind of the `Use` node's parent; `"file"` when it has none.
    pub parent_kind: String,
}

/// Direction of the edges read by [`Database::get_edge_endpoint_ids`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeEndpoint {
    /// Targets of edges whose source is in the given set.
    Targets,
    /// Sources of edges whose target is in the given set.
    Sources,
}

/// One edge, flattened with the node facts the audit predicate needs.
#[derive(Debug, Clone)]
pub struct AuditEdgeRow {
    /// File of the edge's source node.
    pub src_file: String,
    /// Id of the edge's target node.
    pub dst_id: String,
    /// Name of the target node.
    pub dst_name: String,
    /// File of the target node.
    pub dst_file: String,
    /// Kind of the target node.
    pub dst_kind: String,
    /// Zero-based start line of the target node.
    pub dst_line: i64,
    /// Parent id of the target node.
    pub dst_parent: Option<String>,
}
