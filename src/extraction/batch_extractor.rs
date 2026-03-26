/// Tree-sitter based Batch/CMD source code extractor.
///
/// Parses Windows Batch (.bat/.cmd) source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::ComplexityMetrics;
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Batch/CMD source files using tree-sitter.
pub struct BatchExtractor;

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
    /// Stack of (name, node_id) for building qualified names and parent edges.
    node_stack: Vec<(String, String)>,
    file_path: String,
    source: Vec<u8>,
    timestamp: u64,
}

impl ExtractionState {
    fn new(file_path: &str, source: &str) -> Self {
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
        }
    }

    /// Returns the current qualified name prefix from the node stack.
    fn qualified_prefix(&self) -> String {
        let mut parts = vec![self.file_path.clone()];
        for (name, _) in &self.node_stack {
            parts.push(name.clone());
        }
        parts.join("::")
    }

    /// Returns the current parent node ID, or None if at file root level.
    fn parent_node_id(&self) -> Option<&str> {
        self.node_stack.last().map(|(_, id)| id.as_str())
    }

    /// Gets the text of a tree-sitter node from the source.
    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }
}

impl BatchExtractor {
    /// Extract code graph nodes and edges from a Batch source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Batch source code to parse.
    pub fn extract_batch(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                return Self::build_result(state, start);
            }
        };

        // Create the File root node.
        let file_node = Node {
            id: generate_node_id(file_path, &NodeKind::File, file_path, 0),
            kind: NodeKind::File,
            name: file_path.to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            start_line: 0,
            end_line: source.lines().count().saturating_sub(1) as u32,
            start_column: 0,
            end_column: 0,
            signature: None,
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            updated_at: state.timestamp,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state
            .node_stack
            .push((file_path.to_string(), file_node_id));

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_top_level(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_batch::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Batch grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit all top-level children of the root program node.
    ///
    /// Batch files use labels as function-like constructs. Labels are top-level
    /// siblings in the AST (not containers). We group code between consecutive
    /// labels as the body of each label's "function".
    fn visit_top_level(state: &mut ExtractionState, root: TsNode<'_>) {
        let child_count = root.child_count();
        let mut i: usize = 0;

        while i < child_count {
            let child = match root.child(i as u32) {
                Some(c) => c,
                None => { i += 1; continue; }
            };

            match child.kind() {
                "label" => {
                    Self::visit_label(state, root, i);
                }
                "variable_assignment" => {
                    Self::visit_variable_assignment(state, child);
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// Extract a label as a Function node.
    ///
    /// In Batch, labels (:Name) serve as subroutine entry points.
    /// The body extends from the label to the next label or end of file.
    fn visit_label(state: &mut ExtractionState, root: TsNode<'_>, label_index: usize) {
        let label_node = match root.child(label_index as u32) {
            Some(n) => n,
            None => return,
        };

        let label_text = state.node_text(label_node);
        // Strip leading ':'
        let name = label_text.trim_start_matches(':').trim().to_string();
        if name.is_empty() || name.eq_ignore_ascii_case("EOF") {
            return;
        }

        let kind = NodeKind::Function;
        let visibility = Visibility::Pub;
        let start_line = label_node.start_position().row as u32;
        let start_column = label_node.start_position().column as u32;

        // Find the end line: scan forward to the next label or end of file.
        let child_count = root.child_count();
        let mut end_line = label_node.end_position().row as u32;
        let mut end_column = label_node.end_position().column as u32;
        let mut j = label_index + 1;
        while j < child_count {
            if let Some(sibling) = root.child(j as u32) {
                if sibling.kind() == "label" {
                    // End just before the next label.
                    break;
                }
                end_line = sibling.end_position().row as u32;
                end_column = sibling.end_position().column as u32;
            }
            j += 1;
        }

        let signature = Some(label_text.trim().to_string());
        let docstring = Self::extract_docstring(state, root, label_index);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = ComplexityMetrics::default();

        let graph_node = Node {
            id: id.clone(),
            kind,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring,
            visibility,
            is_async: false,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
            unsafe_blocks: metrics.unsafe_blocks,
            unchecked_calls: metrics.unchecked_calls,
            assertions: metrics.assertions,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from siblings belonging to this label's body.
        Self::extract_label_call_sites(state, root, label_index, &id);
    }

    /// Extract a `set VAR=value` variable assignment as a Const node.
    ///
    /// Only top-level variable assignments are treated as constants.
    fn visit_variable_assignment(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Text looks like "set MAX_RETRIES=3" or "set /a X=1"
        // Parse the variable name: strip "set " prefix (case-insensitive), then take up to "="
        let after_set = text
            .strip_prefix("set ")
            .or_else(|| text.strip_prefix("SET "))
            .or_else(|| text.strip_prefix("Set "))
            .unwrap_or(&text);

        // Handle /a, /p options
        let after_opts = if after_set.starts_with("/a ") || after_set.starts_with("/A ") ||
                            after_set.starts_with("/p ") || after_set.starts_with("/P ") {
            &after_set[3..]
        } else {
            after_set
        };

        // Name is everything before '='
        let name = match after_opts.split('=').next() {
            Some(n) if !n.is_empty() => n.trim().to_string(),
            _ => return,
        };

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Const,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract docstrings from `REM` or `::` comment lines preceding a label.
    ///
    /// Looks backward from the label's position in the root children list
    /// for consecutive comment nodes.
    fn extract_docstring(
        state: &ExtractionState,
        root: TsNode<'_>,
        label_index: usize,
    ) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut idx = label_index;

        while idx > 0 {
            idx -= 1;
            let prev = root.child(idx as u32)?;
            if prev.kind() == "comment" {
                let text = state.node_text(prev);
                let stripped = text
                    .trim()
                    .strip_prefix("REM ")
                    .or_else(|| text.trim().strip_prefix("rem "))
                    .or_else(|| text.trim().strip_prefix(":: "))
                    .or_else(|| text.trim().strip_prefix("::"))
                    .unwrap_or(text.trim())
                    .trim()
                    .to_string();
                comments.push(stripped);
            } else {
                break;
            }
        }

        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        Some(comments.join("\n"))
    }

    /// Extract call sites from the body of a label (sibling nodes after the label).
    ///
    /// Scans forward from the label until the next label or end of file.
    /// Looks for `call_stmt` nodes and extracts the callee label name.
    fn extract_label_call_sites(
        state: &mut ExtractionState,
        root: TsNode<'_>,
        label_index: usize,
        fn_node_id: &str,
    ) {
        let child_count = root.child_count();
        let mut j = label_index + 1;

        while j < child_count {
            if let Some(sibling) = root.child(j as u32) {
                if sibling.kind() == "label" {
                    break;
                }
                Self::extract_call_sites_recursive(state, sibling, fn_node_id);
            }
            j += 1;
        }
    }

    /// Recursively find call_stmt nodes and create unresolved Calls references.
    fn extract_call_sites_recursive(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        fn_node_id: &str,
    ) {
        if node.kind() == "call_stmt" {
            let text = state.node_text(node);
            // Parse the callee: "call :LabelName ..." → "LabelName"
            if let Some(callee) = Self::parse_call_target(&text) {
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: fn_node_id.to_string(),
                    reference_name: callee,
                    reference_kind: EdgeKind::Calls,
                    line: node.start_position().row as u32,
                    column: node.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
            }
        }

        // Recurse into children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::extract_call_sites_recursive(state, child, fn_node_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Parse the call target from a `call :Label` statement.
    ///
    /// Returns the label name (without the leading ':'), or None if the call
    /// is not to a label (e.g., `call external.bat`).
    fn parse_call_target(text: &str) -> Option<String> {
        let trimmed = text.trim();
        // Expected: "call :LabelName [args...]"
        let after_call = trimmed
            .strip_prefix("call ")
            .or_else(|| trimmed.strip_prefix("CALL "))?;
        let target = after_call.split_whitespace().next()?;
        if target.starts_with(':') {
            let name = target.trim_start_matches(':');
            if !name.is_empty() && !name.eq_ignore_ascii_case("EOF") {
                return Some(name.to_string());
            }
        }
        None
    }

    /// Build the final ExtractionResult from the accumulated state.
    fn build_result(state: ExtractionState, start: Instant) -> ExtractionResult {
        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: state.unresolved_refs,
            errors: state.errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

impl crate::extraction::LanguageExtractor for BatchExtractor {
    fn extensions(&self) -> &[&str] {
        &["bat", "cmd"]
    }

    fn language_name(&self) -> &str {
        "Batch"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_batch(file_path, source)
    }
}
