/// Tree-sitter based Bash source code extractor.
///
/// Parses Bash/shell source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, BASH_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Bash source files using tree-sitter.
pub struct BashExtractor;

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
    /// Stack of (name, `node_id`) for building qualified names and parent edges.
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

impl BashExtractor {
    /// Extract code graph nodes and edges from a Bash source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Bash source code to parse.
    pub fn extract_bash(file_path: &str, source: &str) -> ExtractionResult {
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
            attrs_start_line: 0,
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
        state.node_stack.push((file_path.to_string(), file_node_id));

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("bash");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Bash grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit all children of a node.
    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::visit_node(state, child);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit a single AST node, dispatching on its type.
    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "function_definition" => Self::visit_function(state, node),
            "declaration_command" => Self::visit_declaration(state, node),
            "command" => Self::visit_command(state, node),
            _ => {}
        }
    }

    /// Extract a function definition.
    ///
    /// Bash functions are always top-level (no classes), so they get `NodeKind::Function`.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let kind = NodeKind::Function;
        let visibility = Visibility::Pub;
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &BASH_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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

        // Extract call sites from the function body.
        Self::extract_call_sites(state, node, &id);
    }

    /// Extract a `readonly VAR=value` or `local VAR=value` declaration at the top level.
    ///
    /// Only top-level `readonly` declarations are treated as constants.
    fn visit_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Only treat top-level readonly as constants.
        // A declaration_command starts with a word like "readonly", "local", "declare", "export".
        let text = state.node_text(node);
        if !text.starts_with("readonly") {
            return;
        }

        // Find the variable_assignment child to get the name.
        if let Some(assignment) = Self::find_child_by_kind(node, "variable_assignment") {
            if let Some(name_node) = assignment.child_by_field_name("name") {
                let name = state.node_text(name_node);
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
                    attrs_start_line: start_line,
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
        }
    }

    /// Extract a top-level command node.
    ///
    /// Detects `source` and `.` commands as Use (import) nodes.
    fn visit_command(state: &mut ExtractionState, node: TsNode<'_>) {
        let cmd_name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .unwrap_or_default();

        if cmd_name == "source" || cmd_name == "." {
            // Extract the source/dot import path from the first argument.
            if let Some(arg) = Self::find_first_argument(state, node) {
                let start_line = node.start_position().row as u32;
                let end_line = node.end_position().row as u32;
                let start_column = node.start_position().column as u32;
                let end_column = node.end_position().column as u32;
                let qualified_name = format!("{}::{}", state.qualified_prefix(), arg);
                let id = generate_node_id(&state.file_path, &NodeKind::Use, &arg, start_line);
                let text = state.node_text(node);

                let graph_node = Node {
                    id: id.clone(),
                    kind: NodeKind::Use,
                    name: arg,
                    qualified_name,
                    file_path: state.file_path.clone(),
                    start_line,
                    attrs_start_line: start_line,
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
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract the function signature (first line of the definition).
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract docstrings from `# comment` lines preceding definitions.
    ///
    /// Bash uses comment lines (# ...) as documentation. We look for `comment`
    /// sibling nodes that immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = text.trim_start_matches('#').trim().to_string();
                comments.push(stripped);
                prev = prev_node.prev_named_sibling();
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

    /// Recursively find command nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "command" => {
                        // Extract the command name.
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let callee_name = state.node_text(name_node);
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: callee_name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        // Recurse into command for nested command substitutions.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested function definitions.
                    "function_definition" => {}
                    _ => {
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Find the first argument of a command node.
    ///
    /// In tree-sitter-bash, command arguments have the field name "argument".
    fn find_first_argument(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if cursor.field_name() == Some("argument") {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Find the first child of a node with a given kind.
    fn find_child_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
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

    /// Build the final `ExtractionResult` from the accumulated state.
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

impl crate::extraction::LanguageExtractor for BashExtractor {
    fn extensions(&self) -> &[&str] {
        &["sh", "bash"]
    }

    fn language_name(&self) -> &'static str {
        "Bash"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_bash(file_path, source)
    }
}
