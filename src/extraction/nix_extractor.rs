/// Tree-sitter based Nix source code extractor.
///
/// Parses Nix source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, NIX_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Nix source files using tree-sitter.
pub struct NixExtractor;

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

impl NixExtractor {
    /// Extract code graph nodes and edges from a Nix source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Nix source code to parse.
    pub fn extract_nix(file_path: &str, source: &str) -> ExtractionResult {
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
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_nix::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Nix grammar: {e}"))?;
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
            "let_expression" => Self::visit_let_expression(state, node),
            "binding" => Self::visit_binding(state, node),
            "inherit" | "inherit_from" => Self::visit_inherit(state, node),
            // Recurse through structural nodes that wrap definitions.
            "function_expression" | "binding_set" => {
                Self::visit_children(state, node);
            }
            _ => {}
        }
    }

    /// Visit a let expression. Process bindings inside binding_set and the body.
    fn visit_let_expression(state: &mut ExtractionState, node: TsNode<'_>) {
        // Process binding_set for definitions
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "binding_set" {
                    Self::visit_children(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Process the body field (the expression after `in`).
        // If it's an attrset, extract its bindings and inherits.
        if let Some(body) = node.child_by_field_name("body") {
            if body.kind() == "attrset_expression" {
                Self::visit_attrset_bindings(state, body);
            }
        }
    }

    /// Visit a binding node. Classify as Function, Module, or Const based on the value.
    fn visit_binding(state: &mut ExtractionState, node: TsNode<'_>) {
        // Extract name from attrpath child
        let name = Self::extract_binding_name(state, node);
        let name = match name {
            Some(n) => n,
            None => return,
        };

        // Get the expression (value) child
        let expr = node.child_by_field_name("expression");

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;

        // Classify based on value expression type
        match expr.map(|e| Self::classify_expression(e)) {
            Some(BindingKind::Function) => {
                let kind = NodeKind::Function;
                let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                let id = generate_node_id(&state.file_path, &kind, &name, start_line);
                let signature = Self::extract_function_signature(state, node);
                let metrics = if let Some(expr_node) = expr {
                    if expr_node.child_count() > 0 {
                        count_complexity(expr_node, &NIX_COMPLEXITY, &state.source)
                    } else {
                        Default::default()
                    }
                } else {
                    Default::default()
                };

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
                    visibility: Visibility::Pub,
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
                if let Some(expr_node) = expr {
                    Self::extract_call_sites(state, expr_node, &id);
                }
            }
            Some(BindingKind::Module) => {
                let kind = NodeKind::Module;
                let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                let id = generate_node_id(&state.file_path, &kind, &name, start_line);

                let text = state.node_text(node);
                let signature = text
                    .lines()
                    .next()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty());

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
                        target: id.clone(),
                        kind: EdgeKind::Contains,
                        line: Some(start_line),
                    });
                }

                // Visit bindings inside the attrset (module body).
                if let Some(expr_node) = expr {
                    state.node_stack.push((name.clone(), id));
                    Self::visit_attrset_bindings(state, expr_node);
                    state.node_stack.pop();
                }
            }
            _ => {
                // Const
                let kind = NodeKind::Const;
                let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                let id = generate_node_id(&state.file_path, &kind, &name, start_line);

                let text = state.node_text(node);
                let signature = Some(text.lines().next().unwrap_or("").trim().to_string())
                    .filter(|s| !s.is_empty());

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

    /// Visit bindings inside an attrset_expression (used for module bodies).
    fn visit_attrset_bindings(state: &mut ExtractionState, node: TsNode<'_>) {
        // attrset_expression contains binding_set
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "binding_set" => {
                        // Visit bindings and inherits inside the binding_set
                        let mut inner = child.walk();
                        if inner.goto_first_child() {
                            loop {
                                let item = inner.node();
                                match item.kind() {
                                    "binding" => Self::visit_binding(state, item),
                                    "inherit" | "inherit_from" => {
                                        Self::visit_inherit(state, item)
                                    }
                                    _ => {}
                                }
                                if !inner.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit an inherit or inherit_from node. Creates Use nodes for imported attributes.
    fn visit_inherit(state: &mut ExtractionState, node: TsNode<'_>) {
        // inherit has `inherited_attrs` with attr children
        // inherit_from has `expression` and `inherited_attrs`
        let start_line = node.start_position().row as u32;
        let start_column = node.start_position().column as u32;

        // Find inherited_attrs
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "inherited_attrs" {
                    // Each attr child is an identifier being imported
                    let mut attr_cursor = child.walk();
                    if attr_cursor.goto_first_child() {
                        loop {
                            let attr = attr_cursor.node();
                            if attr.kind() == "identifier" {
                                let attr_name = state.node_text(attr);
                                let kind = NodeKind::Use;
                                let qualified_name =
                                    format!("{}::{}", state.qualified_prefix(), attr_name);
                                let id = generate_node_id(
                                    &state.file_path,
                                    &kind,
                                    &attr_name,
                                    start_line,
                                );
                                let attr_line = attr.start_position().row as u32;

                                let graph_node = Node {
                                    id: id.clone(),
                                    kind,
                                    name: attr_name.clone(),
                                    qualified_name,
                                    file_path: state.file_path.clone(),
                                    start_line: attr_line,
                                    end_line: attr.end_position().row as u32,
                                    start_column: attr.start_position().column as u32,
                                    end_column: attr.end_position().column as u32,
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
                                state.nodes.push(graph_node);

                                if let Some(parent_id) = state.parent_node_id() {
                                    state.edges.push(Edge {
                                        source: parent_id.to_string(),
                                        target: id.clone(),
                                        kind: EdgeKind::Contains,
                                        line: Some(attr_line),
                                    });
                                }

                                // Also create an unresolved Uses ref
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: id,
                                    reference_name: attr_name,
                                    reference_kind: EdgeKind::Uses,
                                    line: start_line,
                                    column: start_column,
                                    file_path: state.file_path.clone(),
                                });
                            }
                            if !attr_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // ----------------------------
    // Classification helpers
    // ----------------------------

    /// Classify a binding's value expression.
    fn classify_expression(node: TsNode<'_>) -> BindingKind {
        match node.kind() {
            "function_expression" => BindingKind::Function,
            "attrset_expression" => {
                // Check if the attrset has named bindings (not just simple values).
                // If it has binding children with function values, treat as Module.
                if Self::attrset_has_named_bindings(node) {
                    BindingKind::Module
                } else {
                    BindingKind::Const
                }
            }
            _ => BindingKind::Const,
        }
    }

    /// Check if an attrset_expression has named bindings (making it a module-like structure).
    fn attrset_has_named_bindings(node: TsNode<'_>) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "binding_set" {
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let item = inner.node();
                            if item.kind() == "binding" {
                                return true;
                            }
                            if !inner.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    // ----------------------------
    // Name/signature extraction
    // ----------------------------

    /// Extract the name from a binding's attrpath.
    fn extract_binding_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // binding has an attrpath child with attr children (identifiers)
        if let Some(attrpath) = node.child_by_field_name("attrpath") {
            // Take the first attr (identifier) from the attrpath
            let mut cursor = attrpath.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "identifier" {
                        return Some(state.node_text(child));
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        None
    }

    /// Extract the function signature from a binding.
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        // Take the first line as the signature
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract docstrings from `# comment` lines preceding a node.
    ///
    /// In Nix, comments may be siblings of the node at the same level, or they
    /// may be at the parent level (e.g., a comment before `binding_set` in a
    /// `let_expression` or `attrset_expression`).
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();

        // If no previous sibling at this level, check the parent's previous sibling.
        // This handles cases where the comment is a child of `let_expression` or
        // `attrset_expression` but the binding is inside `binding_set`.
        if prev.is_none() {
            if let Some(parent) = node.parent() {
                if parent.kind() == "binding_set" {
                    prev = parent.prev_named_sibling();
                }
            }
        }

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

    /// Recursively find call nodes (apply_expression) and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "apply_expression" => {
                        // In Nix, apply_expression has `function` and `argument` fields.
                        // The function field can be a variable_expression or a select_expression.
                        let callee_name =
                            child.child_by_field_name("function").and_then(|func_node| {
                                Self::extract_callee_name(state, func_node)
                            });

                        if let Some(name) = callee_name {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        // Recurse into the apply_expression for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // In Nix, lambdas are pervasive (e.g. callbacks to genList).
                    // Recurse into nested function_expressions to capture calls.
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

    /// Extract the callee name from a function position in an apply_expression.
    fn extract_callee_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        match node.kind() {
            "variable_expression" => {
                // variable_expression has a `name` field (identifier)
                node.child_by_field_name("name")
                    .map(|n| state.node_text(n))
            }
            "select_expression" => {
                // select_expression: expression.attrpath
                // Extract the full dotted path for the callee name.
                Some(state.node_text(node))
            }
            "apply_expression" => {
                // Curried call: (f x) y — extract the innermost function name
                node.child_by_field_name("function")
                    .and_then(|f| Self::extract_callee_name(state, f))
            }
            _ => Some(state.node_text(node)),
        }
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

/// Internal classification of a binding's value.
enum BindingKind {
    Function,
    Module,
    Const,
}

impl crate::extraction::LanguageExtractor for NixExtractor {
    fn extensions(&self) -> &[&str] {
        &["nix"]
    }

    fn language_name(&self) -> &str {
        "Nix"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_nix(file_path, source)
    }
}
