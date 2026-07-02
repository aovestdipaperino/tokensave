//! Shared traversal state and tree-sitter helpers for language extractors.
//!
//! Most tree-sitter based extractors need the same bookkeeping while walking
//! an AST: accumulators for nodes/edges, a stack of enclosing scopes for
//! qualified names, and small node-search utilities. Extractors with extra
//! per-language state (e.g. C++ access specifiers) keep their own state
//! structs; everything else shares this one.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::Node as TsNode;

use crate::types::{Edge, ExtractionResult, Node, UnresolvedRef};

/// Internal state used during AST traversal.
pub(crate) struct ExtractionState {
    pub(crate) nodes: Vec<Node>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) unresolved_refs: Vec<UnresolvedRef>,
    pub(crate) errors: Vec<String>,
    /// Stack of (name, `node_id`) for building qualified names and parent edges.
    pub(crate) node_stack: Vec<(String, String)>,
    pub(crate) file_path: String,
    pub(crate) source: Vec<u8>,
    pub(crate) timestamp: u64,
    /// Nesting depth of enclosing class-like scopes (used by extractors that
    /// treat top-level and member functions differently; others leave it 0).
    pub(crate) class_depth: usize,
}

impl ExtractionState {
    pub(crate) fn new(file_path: &str, source: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
            node_stack: Vec::new(),
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            timestamp,
            class_depth: 0,
        }
    }

    /// Returns the current qualified name prefix from the node stack.
    pub(crate) fn qualified_prefix(&self) -> String {
        let mut parts = vec![self.file_path.clone()];
        for (name, _) in &self.node_stack {
            parts.push(name.clone());
        }
        parts.join("::")
    }

    /// Returns the current parent node ID, or None if at file root level.
    pub(crate) fn parent_node_id(&self) -> Option<&str> {
        self.node_stack.last().map(|(_, id)| id.as_str())
    }

    /// Gets the text of a tree-sitter node from the source.
    pub(crate) fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }

    /// Consumes the state into an `ExtractionResult`, stamping the duration.
    pub(crate) fn build_result(self, start: Instant) -> ExtractionResult {
        ExtractionResult {
            nodes: self.nodes,
            edges: self.edges,
            unresolved_refs: self.unresolved_refs,
            errors: self.errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

/// Find the first direct child of a node with a given kind.
pub(crate) fn find_child_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == kind {
                return Some(child);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Find the first descendant of a node with a given kind (recursive DFS).
pub(crate) fn find_descendant_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if current.kind() == kind {
            return Some(current);
        }
        // Push children via cursor (O(N) per node) and reverse so the
        // first child pops first. Previous revision used `current.child(i)`
        // in a `for i in (0..N).rev()` loop, which is O(N²) per node
        // because `child(i)` walks sibling links from index 0.
        let start = stack.len();
        let mut cursor = current.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        stack[start..].reverse();
    }
    None
}

/// Returns true if the node has a direct child of the given kind.
pub(crate) fn has_child_kind(node: TsNode<'_>, kind: &str) -> bool {
    find_child_by_kind(node, kind).is_some()
}
