/// Tree-sitter based PowerShell source code extractor.
///
/// Parses PowerShell source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, POWERSHELL_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from PowerShell source files using tree-sitter.
pub struct PowerShellExtractor;

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

impl PowerShellExtractor {
    /// Extract code graph nodes and edges from a PowerShell source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the PowerShell source code to parse.
    pub fn extract_powershell(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("powershell");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load PowerShell grammar: {e}"))?;
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
            "function_statement" => Self::visit_function(state, node),
            "pipeline" => Self::visit_pipeline(state, node),
            "statement_list" => Self::visit_children(state, node),
            _ => {}
        }
    }

    /// Visit a pipeline node, which can contain assignments (consts) or commands (imports).
    fn visit_pipeline(state: &mut ExtractionState, node: TsNode<'_>) {
        // A pipeline wraps either an assignment_expression or a pipeline_chain > command.
        if let Some(assignment) = Self::find_child_by_kind(node, "assignment_expression") {
            Self::visit_assignment(state, assignment, node);
        } else if let Some(chain) = Self::find_child_by_kind(node, "pipeline_chain") {
            if let Some(command) = Self::find_child_by_kind(chain, "command") {
                Self::visit_command(state, command);
            }
        }
    }

    /// Extract a function definition.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "function_name")
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
        let metrics = count_complexity(node, &POWERSHELL_COMPLEXITY, &state.source);

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

    /// Extract a typed variable assignment at the top level as a Const.
    ///
    /// Matches patterns like `[int]$MaxRetries = 3`.
    /// The AST is: pipeline > `assignment_expression` > `left_assignment_expression` > ... > `cast_expression` > variable.
    fn visit_assignment(state: &mut ExtractionState, node: TsNode<'_>, pipeline_node: TsNode<'_>) {
        // Only treat top-level typed assignments as constants.
        // We detect a cast_expression inside the left_assignment_expression.
        let Some(left) = Self::find_child_by_kind(node, "left_assignment_expression") else {
            return;
        };

        // Look for a cast_expression recursively inside the left side.
        let Some(cast) = Self::find_descendant_by_kind(left, "cast_expression") else {
            return;
        };

        // The variable is a child of the cast_expression.
        let Some(var_node) = Self::find_descendant_by_kind(cast, "variable") else {
            return;
        };

        let var_text = state.node_text(var_node);
        // Strip leading $ from variable name.
        let name = var_text.trim_start_matches('$').to_string();

        let start_line = pipeline_node.start_position().row as u32;
        let end_line = pipeline_node.end_position().row as u32;
        let start_column = pipeline_node.start_position().column as u32;
        let end_column = pipeline_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
        let text = state.node_text(pipeline_node);

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

    /// Extract a top-level command node.
    ///
    /// Detects `Import-Module` and dot-source (`. .\script.ps1`) as Use (import) nodes.
    fn visit_command(state: &mut ExtractionState, node: TsNode<'_>) {
        let cmd_name = node
            .child_by_field_name("command_name")
            .map(|n| state.node_text(n))
            .unwrap_or_default();

        if cmd_name == "Import-Module" {
            // Extract the module name from command_elements.
            if let Some(elements) = node.child_by_field_name("command_elements") {
                if let Some(token) = Self::find_child_by_kind(elements, "generic_token") {
                    let module_name = state.node_text(token);
                    Self::emit_use_node(state, node, &module_name);
                }
            }
        } else if Self::find_child_by_kind(node, "command_invokation_operator").is_some() {
            // Dot-source command: `. .\Utils.ps1`
            // The path is in command_name_expr > command_name.
            if let Some(name_expr) = node.child_by_field_name("command_name") {
                let path = state.node_text(name_expr);
                Self::emit_use_node(state, node, &path);
            }
        }
    }

    /// Emit a Use (import) node for an import command.
    fn emit_use_node(state: &mut ExtractionState, node: TsNode<'_>, name: &str) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, name, start_line);
        let text = state.node_text(node);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name: name.to_string(),
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

    /// Extract docstrings from preceding comment nodes.
    ///
    /// PowerShell uses `# comment` lines or `<# ... #>` block comments as documentation.
    /// We look for `comment` sibling nodes that immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = if text.starts_with("<#") {
                    // Block comment: strip <# and #> delimiters.
                    text.trim_start_matches("<#")
                        .trim_end_matches("#>")
                        .trim()
                        .to_string()
                } else {
                    // Line comment: strip leading #.
                    text.trim_start_matches('#').trim().to_string()
                };
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
                        if let Some(name_node) = child.child_by_field_name("command_name") {
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
                    "function_statement" => {}
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

    /// Find the first descendant of a node with a given kind (recursive DFS).
    fn find_descendant_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            if current.kind() == kind {
                return Some(current);
            }
            let count = current.child_count();
            // Push children in reverse so first child is processed first.
            let mut i = count;
            while i > 0 {
                i -= 1;
                if let Some(child) = current.child(i as u32) {
                    stack.push(child);
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

impl crate::extraction::LanguageExtractor for PowerShellExtractor {
    fn extensions(&self) -> &[&str] {
        &["ps1", "psm1"]
    }

    fn language_name(&self) -> &'static str {
        "PowerShell"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_powershell(file_path, source)
    }
}
