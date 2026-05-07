/// Tree-sitter based Perl source code extractor.
///
/// Parses Perl source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, PERL_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Perl source files using tree-sitter.
pub struct PerlExtractor;

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
    /// Depth of package nesting. > 0 means we are inside a package.
    class_depth: usize,
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
            class_depth: 0,
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

impl PerlExtractor {
    /// Extract code graph nodes and edges from a Perl source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Perl source code to parse.
    pub fn extract_perl(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("perl");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Perl grammar: {e}"))?;
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
            "package_statement" => Self::visit_package(state, node),
            "use_no_statement" => Self::visit_use(state, node),
            "binary_expression" => Self::visit_binary_expression_for_const(state, node),
            _ => {}
        }
    }

    /// Extract a function/method definition (`sub name { ... }`).
    ///
    /// If `class_depth` > 0, this is a method inside a package; otherwise it is a top-level function.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let kind = if state.class_depth > 0 {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let visibility = Visibility::Pub;
        let signature = Self::extract_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &PERL_COMPLEXITY, &state.source);

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
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a package declaration and track it as a Module.
    ///
    /// In Perl, `package Foo;` starts a new package scope. Subsequent subs
    /// belong to this package until another package statement or end of file.
    /// Since Perl packages don't have explicit end markers (no `end` keyword),
    /// we handle them by scanning ahead through siblings until the next
    /// `package_statement` or end of the `source_file` children.
    fn visit_package(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "package_name")
            .map_or_else(|| "<anonymous>".to_string(), |pn| state.node_text(pn));

        // Skip `package main;` — it just returns to the top-level scope.
        if name == "main" {
            // Pop any existing package scope.
            if state.class_depth > 0 {
                state.class_depth -= 1;
                state.node_stack.pop();
            }
            return;
        }

        // If we're already inside a package, pop it before starting a new one.
        if state.class_depth > 0 {
            state.class_depth -= 1;
            state.node_stack.pop();
        }

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let start_column = node.start_position().column as u32;

        // Determine the end line by looking at siblings until next package or EOF.
        let end_line = Self::find_package_end_line(node, state);
        let end_column = 0u32;

        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        let text = state.node_text(node);
        let signature = text
            .lines()
            .next()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
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

        // Push package onto the stack so subsequent subs become methods.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;

        // We don't recurse into the package_statement node itself —
        // the siblings (function_definition, etc.) will be visited by
        // the parent visit_children call. They will see class_depth > 0.
    }

    /// Find the end line of a package scope by looking at the next sibling
    /// that is a `package_statement`, or the last sibling in the `source_file`.
    fn find_package_end_line(node: TsNode<'_>, _state: &ExtractionState) -> u32 {
        let mut sibling = node.next_named_sibling();
        let mut last_end = node.end_position().row as u32;
        while let Some(sib) = sibling {
            if sib.kind() == "package_statement" {
                // The package ends just before the next package_statement.
                return sib.start_position().row.saturating_sub(1) as u32;
            }
            last_end = sib.end_position().row as u32;
            sibling = sib.next_named_sibling();
        }
        last_end
    }

    /// Extract a `use` or `require` statement as a Use node.
    fn visit_use(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("package_name")
            .map_or_else(|| "<unknown>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
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

    /// Check if a binary expression is an `our $CAPS_VAR = value` constant declaration.
    ///
    /// In tree-sitter-perl, `our $MAX_RETRIES = 3` is a `binary_expression` with:
    ///   - left child: `variable_declaration` { scope("our"), `scalar_variable("$MAX_RETRIES`") }
    ///   - right child: integer(3)
    fn visit_binary_expression_for_const(state: &mut ExtractionState, node: TsNode<'_>) {
        let left = node.child_by_field_name("variable");
        if let Some(left_node) = left {
            if left_node.kind() == "variable_declaration" {
                let scope_node = Self::find_child_by_kind(left_node, "scope");
                let is_our = scope_node.is_some_and(|s| state.node_text(s) == "our");

                if is_our {
                    // Get the variable name from scalar_variable child.
                    let var_name = left_node
                        .child_by_field_name("variable_name")
                        .map(|n| state.node_text(n))
                        .unwrap_or_default();

                    // Only treat ALL_CAPS variables as constants.
                    let bare_name = var_name.trim_start_matches('$');
                    if !bare_name.is_empty()
                        && bare_name
                            .chars()
                            .all(|c| c.is_ascii_uppercase() || c == '_')
                    {
                        let name = bare_name.to_string();
                        let start_line = node.start_position().row as u32;
                        let end_line = node.end_position().row as u32;
                        let start_column = node.start_position().column as u32;
                        let end_column = node.end_position().column as u32;
                        let text = state.node_text(node);
                        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                        let id =
                            generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
                        let docstring = Self::extract_docstring(state, node);

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
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract the function signature (first line of the sub definition).
    fn extract_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
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
    /// Perl uses comment lines (# ...) as documentation. We look for `comments`
    /// sibling nodes that immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comments" {
                let text = state.node_text(prev_node);
                // Split multi-line comment blocks into individual lines.
                for line in text.lines().rev() {
                    let stripped = line.trim_start_matches('#').trim().to_string();
                    if !stripped.is_empty() {
                        comments.push(stripped);
                    }
                }
                prev = prev_node.prev_named_sibling();
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        // Comments were collected in reverse order; reverse them back.
        comments.reverse();
        Some(comments.join("\n"))
    }

    /// Recursively find call nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression_with_spaced_args"
                    | "call_expression_with_args_with_brackets" => {
                        // These contain a call_expression_with_bareword child
                        // with a function_name field.
                        let callee_name =
                            Self::find_child_by_kind(child, "call_expression_with_bareword")
                                .and_then(|ceb| ceb.child_by_field_name("function_name"))
                                .map(|n| state.node_text(n));

                        if let Some(name) = callee_name {
                            // Skip Perl built-in keywords that aren't real calls.
                            if !Self::is_perl_builtin(&name) {
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: fn_node_id.to_string(),
                                    reference_name: name,
                                    reference_kind: EdgeKind::Calls,
                                    line: child.start_position().row as u32,
                                    column: child.start_position().column as u32,
                                    file_path: state.file_path.clone(),
                                });
                            }
                        }
                        // Also check for qualified calls (e.g., main::log_message)
                        if let Some(ceb) =
                            Self::find_child_by_kind(child, "call_expression_with_bareword")
                        {
                            if let Some(pkg) = ceb.child_by_field_name("package_name") {
                                let pkg_name = state.node_text(pkg);
                                if let Some(fn_name) = ceb.child_by_field_name("function_name") {
                                    let fn_text = state.node_text(fn_name);
                                    let qualified = format!("{pkg_name}::{fn_text}");
                                    state.unresolved_refs.push(UnresolvedRef {
                                        from_node_id: fn_node_id.to_string(),
                                        reference_name: qualified,
                                        reference_kind: EdgeKind::Calls,
                                        line: child.start_position().row as u32,
                                        column: child.start_position().column as u32,
                                        file_path: state.file_path.clone(),
                                    });
                                }
                            }
                        }
                        // Recurse into the call for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    "method_invocation" => {
                        // Method calls: Connection->new(...), $conn->connect()
                        if let Some(fn_name) = child.child_by_field_name("function_name") {
                            let name = state.node_text(fn_name);
                            // Try to get the object/package name for qualified reference.
                            let obj_name = child
                                .child_by_field_name("package_name")
                                .or_else(|| child.child_by_field_name("object_return_value"))
                                .map(|n| state.node_text(n));

                            if let Some(obj) = obj_name {
                                let qualified = format!("{obj}->{name}");
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: fn_node_id.to_string(),
                                    reference_name: qualified,
                                    reference_kind: EdgeKind::Calls,
                                    line: child.start_position().row as u32,
                                    column: child.start_position().column as u32,
                                    file_path: state.file_path.clone(),
                                });
                            } else {
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: fn_node_id.to_string(),
                                    reference_name: name,
                                    reference_kind: EdgeKind::Calls,
                                    line: child.start_position().row as u32,
                                    column: child.start_position().column as u32,
                                    file_path: state.file_path.clone(),
                                });
                            }
                        }
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

    /// Check if a name is a Perl built-in that we don't want to record as a call.
    fn is_perl_builtin(name: &str) -> bool {
        matches!(
            name,
            "my" | "our"
                | "local"
                | "return"
                | "print"
                | "say"
                | "die"
                | "warn"
                | "push"
                | "pop"
                | "shift"
                | "unshift"
                | "chomp"
                | "chop"
                | "defined"
                | "exists"
                | "delete"
                | "keys"
                | "values"
                | "each"
                | "map"
                | "grep"
                | "sort"
                | "reverse"
                | "join"
                | "split"
                | "length"
                | "substr"
                | "index"
                | "rindex"
                | "sprintf"
                | "printf"
                | "open"
                | "close"
                | "read"
                | "write"
                | "seek"
                | "tell"
                | "eof"
                | "binmode"
                | "stat"
                | "chmod"
                | "chown"
                | "mkdir"
                | "rmdir"
                | "unlink"
                | "rename"
                | "ref"
                | "bless"
        )
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

impl crate::extraction::LanguageExtractor for PerlExtractor {
    fn extensions(&self) -> &[&str] {
        &["pl", "pm"]
    }

    fn language_name(&self) -> &'static str {
        "Perl"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_perl(file_path, source)
    }
}
