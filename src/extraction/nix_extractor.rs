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
        let language = crate::extraction::ts_provider::language("nix");
        parser
            .set_language(&language)
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
            // Recurse through apply_expression to find nested definitions
            // (e.g., flake outputs wrapped in eachDefaultSystem calls).
            "apply_expression" => {
                Self::visit_children(state, node);
            }
            // Recurse through parenthesized expressions.
            "parenthesized_expression" => {
                Self::visit_children(state, node);
            }
            // Recurse into top-level attrset expressions (e.g., flake.nix root).
            "attrset_expression" => {
                Self::visit_attrset_bindings(state, node);
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

        // Enhancement 3: Flake output schema awareness.
        // In flake.nix files, known output attrs are forced to Module classification.
        let force_module = Self::is_flake_file(state) && Self::is_flake_output_attr(&name);

        // Enhancement 1: Check if the expression is a builder call (derivation).
        // We store the builder info before classifying to avoid borrow issues.
        let builder_info: Option<(String, usize)> = expr.and_then(|e| {
            Self::is_builder_call(state, e).map(|(_callee, attrset)| {
                // Store the attrset node id so we can find it again
                (String::new(), attrset.id())
            })
        });
        let is_builder = builder_info.is_some();

        // Classify based on value expression type
        let classification = if force_module {
            Some(BindingKind::Module)
        } else {
            expr.map(|e| Self::classify_expression(e))
        };

        match classification {
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

                // Also recurse into the function body for nested definitions.
                // This is needed for flake.nix outputs which wrap definitions
                // inside function bodies, apply expressions, and let expressions.
                if let Some(expr_node) = expr {
                    state.node_stack.push((name.clone(), id));
                    Self::visit_function_body_for_defs(state, expr_node);
                    state.node_stack.pop();
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
                // Also handle builder calls whose attrset arguments contain fields.
                if let Some(expr_node) = expr {
                    state.node_stack.push((name.clone(), id.clone()));
                    if expr_node.kind() == "attrset_expression" {
                        Self::visit_attrset_bindings(state, expr_node);
                    } else if let Some((_callee, attrset_node)) =
                        Self::is_builder_call(state, expr_node)
                    {
                        // Forced-Module binding with a builder call: extract derivation fields.
                        Self::extract_derivation_fields(state, attrset_node, &id);
                    }
                    state.node_stack.pop();
                }
            }
            _ => {
                // Const binding.
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
                        target: id.clone(),
                        kind: EdgeKind::Contains,
                        line: Some(start_line),
                    });
                }

                // Enhancement 1: If this is a builder call, extract derivation fields.
                if is_builder {
                    if let Some(expr_node) = expr {
                        if let Some((_callee, attrset_node)) =
                            Self::is_builder_call(state, expr_node)
                        {
                            let parent_id_for_fields = id;
                            Self::extract_derivation_fields(
                                state,
                                attrset_node,
                                &parent_id_for_fields,
                            );
                        }
                    }
                } else {
                    // Enhancement 2: Check for import paths in non-function bindings.
                    if let Some(expr_node) = expr {
                        Self::check_import_in_expr(state, expr_node);
                    }
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
                if child.kind() == "binding_set" {
                    // Visit bindings and inherits inside the binding_set
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let item = inner.node();
                            match item.kind() {
                                "binding" => Self::visit_binding(state, item),
                                "inherit" | "inherit_from" => Self::visit_inherit(state, item),
                                _ => {}
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

    /// Recurse into a function body to find nested definitions.
    /// This walks through function_expression, apply_expression, let_expression, etc.
    /// to find attrset_expression and let_expression nodes that contain bindings.
    fn visit_function_body_for_defs(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "function_expression" => {
                // Recurse into the function body
                Self::visit_children_for_defs(state, node);
            }
            "apply_expression" => {
                // Recurse into both function and argument
                Self::visit_children_for_defs(state, node);
            }
            "let_expression" => {
                // Process bindings and body
                Self::visit_let_expression(state, node);
            }
            "attrset_expression" => {
                Self::visit_attrset_bindings(state, node);
            }
            "parenthesized_expression" => {
                Self::visit_children_for_defs(state, node);
            }
            _ => {}
        }
    }

    /// Helper to recurse into children looking for definitions.
    fn visit_children_for_defs(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::visit_function_body_for_defs(state, child);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Check if the file path ends with `flake.nix`.
    fn is_flake_file(state: &ExtractionState) -> bool {
        state.file_path.ends_with("flake.nix")
    }

    /// Check if a name is a known flake output attribute.
    fn is_flake_output_attr(name: &str) -> bool {
        FLAKE_OUTPUT_ATTRS.contains(&name)
    }

    /// Check if an expression node is a builder call (e.g., `pkgs.stdenv.mkDerivation { ... }`).
    /// Returns the last segment of the callee name if it matches a builder function.
    fn is_builder_call<'a>(
        state: &ExtractionState,
        node: TsNode<'a>,
    ) -> Option<(String, TsNode<'a>)> {
        if node.kind() != "apply_expression" {
            return None;
        }
        let func_node = node.child_by_field_name("function")?;
        let arg_node = node.child_by_field_name("argument")?;
        if arg_node.kind() != "attrset_expression" {
            return None;
        }

        // Extract the callee name and check its last segment
        let callee_name = Self::extract_callee_name(state, func_node)?;
        let last_segment = callee_name.rsplit('.').next().unwrap_or(&callee_name);
        if BUILDER_FUNCTIONS.contains(&last_segment) {
            Some((callee_name, arg_node))
        } else {
            None
        }
    }

    /// Extract Field nodes from the bindings inside a derivation attrset.
    fn extract_derivation_fields(
        state: &mut ExtractionState,
        attrset_node: TsNode<'_>,
        parent_id: &str,
    ) {
        let mut cursor = attrset_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "binding_set" {
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let item = inner.node();
                            if item.kind() == "binding" {
                                if let Some(field_name) = Self::extract_binding_name(state, item) {
                                    let start_line = item.start_position().row as u32;
                                    let end_line = item.end_position().row as u32;
                                    let start_column = item.start_position().column as u32;
                                    let end_column = item.end_position().column as u32;
                                    let kind = NodeKind::Field;
                                    let qualified_name =
                                        format!("{}::{}", state.qualified_prefix(), field_name);
                                    let id = generate_node_id(
                                        &state.file_path,
                                        &kind,
                                        &field_name,
                                        start_line,
                                    );

                                    let text = state.node_text(item);
                                    let signature = text
                                        .lines()
                                        .next()
                                        .map(|l| l.trim().to_string())
                                        .filter(|l| !l.is_empty());

                                    let graph_node = Node {
                                        id: id.clone(),
                                        kind,
                                        name: field_name,
                                        qualified_name,
                                        file_path: state.file_path.clone(),
                                        start_line,
                                        end_line,
                                        start_column,
                                        end_column,
                                        signature,
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

                                    state.edges.push(Edge {
                                        source: parent_id.to_string(),
                                        target: id,
                                        kind: EdgeKind::Contains,
                                        line: Some(start_line),
                                    });
                                }
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
    /// Also handles Enhancement 2: import path resolution.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "apply_expression" => {
                        // In Nix, apply_expression has `function` and `argument` fields.
                        // The function field can be a variable_expression or a select_expression.
                        let callee_name = child
                            .child_by_field_name("function")
                            .and_then(|func_node| Self::extract_callee_name(state, func_node));

                        if let Some(ref name) = callee_name {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: name.clone(),
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }

                        // Enhancement 2: Import path resolution.
                        // When the callee is `import` and the argument is a path_expression,
                        // emit a Use node and an unresolved Uses ref for cross-file tracking.
                        if callee_name.as_deref() == Some("import") {
                            if let Some(arg) = child.child_by_field_name("argument") {
                                if arg.kind() == "path_expression" {
                                    let path_text = state.node_text(arg);
                                    Self::emit_import_use_node(state, &path_text, child);
                                }
                            }
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

    /// Check for `import ./path` expressions in an arbitrary expression node (for non-function bindings).
    fn check_import_in_expr(state: &mut ExtractionState, node: TsNode<'_>) {
        if node.kind() == "apply_expression" {
            let callee_name = node
                .child_by_field_name("function")
                .and_then(|func_node| Self::extract_callee_name(state, func_node));
            if callee_name.as_deref() == Some("import") {
                if let Some(arg) = node.child_by_field_name("argument") {
                    if arg.kind() == "path_expression" {
                        let path_text = state.node_text(arg);
                        Self::emit_import_use_node(state, &path_text, node);
                    }
                }
            }
        }
        // Recurse into children
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::check_import_in_expr(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Emit a Use node and Uses unresolved ref for an `import ./path.nix` expression.
    fn emit_import_use_node(state: &mut ExtractionState, path: &str, apply_node: TsNode<'_>) {
        let start_line = apply_node.start_position().row as u32;
        let end_line = apply_node.end_position().row as u32;
        let start_column = apply_node.start_position().column as u32;
        let end_column = apply_node.end_position().column as u32;
        let kind = NodeKind::Use;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), path);
        let id = generate_node_id(&state.file_path, &kind, path, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name: path.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
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
                line: Some(start_line),
            });
        }

        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: id,
            reference_name: path.to_string(),
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Extract the callee name from a function position in an apply_expression.
    fn extract_callee_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        match node.kind() {
            "variable_expression" => {
                // variable_expression has a `name` field (identifier)
                node.child_by_field_name("name").map(|n| state.node_text(n))
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

/// Known Nix builder function names whose attrset arguments should produce Field nodes.
const BUILDER_FUNCTIONS: &[&str] = &[
    "mkDerivation",
    "mkShell",
    "buildPythonPackage",
    "buildGoModule",
    "buildRustPackage",
    "buildNpmPackage",
];

/// Known flake output attribute names that should be classified as Module nodes.
const FLAKE_OUTPUT_ATTRS: &[&str] = &[
    "packages",
    "devShells",
    "apps",
    "nixosModules",
    "nixosConfigurations",
    "overlays",
    "lib",
    "checks",
    "formatter",
];

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
