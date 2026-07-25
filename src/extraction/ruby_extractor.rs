/// Tree-sitter based Ruby source code extractor.
///
/// Parses Ruby source files and emits nodes and edges for the code graph.
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, RUBY_COMPLEXITY};
use crate::extraction::ts_state::{find_child_by_kind, ExtractionState};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Ruby source files using tree-sitter.
pub struct RubyExtractor;

impl RubyExtractor {
    /// Extract code graph nodes and edges from a Ruby source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Ruby source code to parse.
    pub fn extract_ruby(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                return state.build_result(start);
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state.node_stack.push((file_path.to_string(), file_node_id));

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        state.build_result(start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("ruby");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Ruby grammar: {e}"))?;
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
            "method" => Self::visit_method(state, node, false),
            "singleton_method" => Self::visit_singleton_method(state, node),
            "class" => Self::visit_class(state, node),
            "module" => Self::visit_module(state, node),
            "assignment" => Self::visit_assignment_for_const(state, node),
            // Bare `private`/`protected`/`public` mode switches parse as a plain
            // identifier statement; defensively also handle a no-arg call.
            "identifier" | "call" | "method_call" => {
                Self::visit_visibility_directive(state, node);
            }
            // Traverse blocks (do...end) for nested definitions
            "do_block" | "block" => Self::visit_children(state, node),
            _ => {}
        }
    }

    /// Extract a regular method definition (`def method_name`).
    ///
    /// `is_singleton` controls whether this becomes a Method regardless of class depth
    /// (singleton methods are always `NodeKind::Method`).
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>, is_singleton: bool) {
        // tree-sitter-ruby's `method` node exposes a `name` field typed `_method_name`,
        // which covers plain identifiers as well as operator defs (`def []=`, `def <=>`)
        // and setter defs (`def name=`) — neither of which is an `identifier` node kind.
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let in_class = state.class_depth > 0 || is_singleton;
        let kind = if in_class {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let visibility = if is_singleton {
            Visibility::Pub
        } else {
            state.visibility_mode.clone()
        };
        let signature = Self::extract_method_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &RUBY_COMPLEXITY, &state.source);

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
            cognitive_complexity: metrics.cognitive_complexity,
            distinct_operators: metrics.distinct_operators,
            distinct_operands: metrics.distinct_operands,
            total_operators: metrics.total_operators,
            total_operands: metrics.total_operands,
            updated_at: state.timestamp,
            parent_id: None,
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

        // Extract call sites from the method body.
        Self::extract_call_sites(state, node, &id);
    }

    /// Extract a singleton method definition (`def self.method_name` or `def obj.method_name`).
    fn visit_singleton_method(state: &mut ExtractionState, node: TsNode<'_>) {
        // singleton_method has: "def", object (self or identifier), ".", identifier, parameters?, body
        // We want the method name (the identifier after ".")
        let name = Self::find_last_identifier_before_params(state, node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let kind = NodeKind::Method;
        let visibility = Visibility::Pub;
        let signature = Self::extract_singleton_method_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &RUBY_COMPLEXITY, &state.source);

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
            cognitive_complexity: metrics.cognitive_complexity,
            distinct_operators: metrics.distinct_operators,
            distinct_operands: metrics.distinct_operands,
            total_operators: metrics.total_operators,
            total_operands: metrics.total_operands,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);
        state.singleton_method_ids.push(id.clone());

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the method body.
        Self::extract_call_sites(state, node, &id);
    }

    /// Extract a class definition.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        // In tree-sitter-ruby, class node children include: "class", constant (name), superclass?, body
        let name = find_child_by_kind(node, "constant")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_class_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
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

        // Extract superclass (inheritance): `class Foo < Bar`
        Self::extract_superclass(state, node, &id);

        // Visit class body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        let saved_visibility_mode = state.visibility_mode.clone();
        state.visibility_mode = Visibility::Pub;
        if let Some(body) = find_child_by_kind(node, "body_statement") {
            Self::visit_children(state, body);
        }
        state.visibility_mode = saved_visibility_mode;
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a module definition.
    fn visit_module(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = find_child_by_kind(node, "constant")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        // Build "module ModuleName" signature
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
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

        // Visit module body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        let saved_visibility_mode = state.visibility_mode.clone();
        state.visibility_mode = Visibility::Pub;
        if let Some(body) = find_child_by_kind(node, "body_statement") {
            Self::visit_children(state, body);
        }
        state.visibility_mode = saved_visibility_mode;
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Check if an assignment is a Ruby constant (starts with uppercase) and extract it.
    ///
    /// Ruby constants are identifiers that start with an uppercase letter.
    fn visit_assignment_for_const(state: &mut ExtractionState, node: TsNode<'_>) {
        // In tree-sitter-ruby, assignment has left and right children.
        // Constants are represented as "constant" kind nodes on the left side.
        let left = node.child_by_field_name("left");
        if let Some(left_node) = left {
            if left_node.kind() == "constant" {
                let name = state.node_text(left_node);
                let start_line = node.start_position().row as u32;
                let end_line = node.end_position().row as u32;
                let start_column = node.start_position().column as u32;
                let end_column = node.end_position().column as u32;
                let text = state.node_text(node);
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
                    cognitive_complexity: 0,
                    distinct_operators: 0,
                    distinct_operands: 0,
                    total_operators: 0,
                    total_operands: 0,
                    updated_at: state.timestamp,
                    parent_id: None,
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

    /// Resolve a Ruby visibility modifier name to a `Visibility`, if the name
    /// is one of `public`/`private`/`protected`. Ruby has no `protected`
    /// variant in our enum; both `private` and `protected` map to
    /// `Visibility::Private` since the distortion this fixes only requires
    /// distinguishing public API from non-public.
    fn resolve_visibility_keyword(name: &str) -> Option<Visibility> {
        match name {
            "public" => Some(Visibility::Pub),
            "private" | "protected" => Some(Visibility::Private),
            _ => None,
        }
    }

    /// Resolve a Ruby *singleton* visibility directive name to a `Visibility`.
    /// These target `def self.foo` methods by symbol rather than switching the
    /// default mode. Ruby core has no `protected_class_method`, so only these two.
    fn resolve_class_method_keyword(name: &str) -> Option<Visibility> {
        match name {
            "public_class_method" => Some(Visibility::Pub),
            "private_class_method" => Some(Visibility::Private),
            _ => None,
        }
    }

    /// Handle `private`/`protected`/`public`/`private_class_method`/
    /// `public_class_method` directives: bare mode switches (`private`),
    /// symbol-list retroactive marking (`private :foo, :bar`), and inline
    /// `def` (`private def foo; end`).
    fn visit_visibility_directive(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "identifier" => {
                let name = state.node_text(node);
                if let Some(visibility) = Self::resolve_visibility_keyword(&name) {
                    state.visibility_mode = visibility;
                }
            }
            "call" | "method_call" => {
                // Real visibility directives are receiverless. A call with an explicit
                // receiver (e.g. `policy.private`, `config.public(:run)`) is an ordinary
                // method call that merely shares a name — it must not change visibility.
                if node.child_by_field_name("receiver").is_some() {
                    return;
                }
                let Some(method_node) = node.child_by_field_name("method") else {
                    return;
                };
                let name = state.node_text(method_node);
                let class_method_visibility = Self::resolve_class_method_keyword(&name);
                let is_class_method = class_method_visibility.is_some();
                let Some(visibility) =
                    class_method_visibility.or_else(|| Self::resolve_visibility_keyword(&name))
                else {
                    return;
                };

                let Some(args) = node.child_by_field_name("arguments") else {
                    // Bare call with no argument list (e.g. `private()`): only a
                    // mode switch makes sense here.
                    if !is_class_method {
                        state.visibility_mode = visibility;
                    }
                    return;
                };

                let mut saw_arg = false;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let arg = cursor.node();
                        match arg.kind() {
                            "simple_symbol" => {
                                saw_arg = true;
                                let symbol_name =
                                    state.node_text(arg).trim_start_matches(':').to_string();
                                Self::mark_method_visibility(
                                    state,
                                    &symbol_name,
                                    is_class_method,
                                    visibility.clone(),
                                );
                            }
                            "delimited_symbol" => {
                                // Any symbol argument counts, so the mode isn't switched
                                // below — even an interpolated one we can't resolve.
                                saw_arg = true;
                                if let Some(symbol_name) =
                                    Self::static_delimited_symbol_name(state, arg)
                                {
                                    Self::mark_method_visibility(
                                        state,
                                        &symbol_name,
                                        is_class_method,
                                        visibility.clone(),
                                    );
                                }
                            }
                            "method" => {
                                saw_arg = true;
                                let saved_visibility_mode = state.visibility_mode.clone();
                                state.visibility_mode = visibility.clone();
                                Self::visit_method(state, arg, false);
                                state.visibility_mode = saved_visibility_mode;
                            }
                            "singleton_method" => {
                                saw_arg = true;
                                Self::visit_singleton_method(state, arg);
                                // `visit_singleton_method` always hardcodes `Pub` (singletons
                                // sit outside the `visibility_mode` path), so a plain `private
                                // def self.foo; end` is correctly left public (a no-op in
                                // Ruby). Only `private_class_method` actually privatizes it.
                                if is_class_method {
                                    if let Some(node) = state.nodes.last_mut() {
                                        node.visibility = visibility.clone();
                                    }
                                }
                            }
                            _ => {
                                // Any other named argument (e.g. `private attr_reader :foo`,
                                // whose arg is a nested `call`) is still an argument: Ruby
                                // applies the directive to it and returns without switching
                                // the default visibility. Unnamed nodes (punctuation like
                                // `(`, `)`, `,`) don't count, so `private()` still switches
                                // the mode.
                                if arg.is_named() {
                                    saw_arg = true;
                                }
                            }
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }

                if !saw_arg && !is_class_method {
                    state.visibility_mode = visibility;
                }
            }
            _ => {}
        }
    }

    /// Retroactively mark the method named `name` defined in the *current*
    /// class/module body (the owner on top of `state.node_stack` at directive
    /// time) as having `visibility`. Matches on `qualified_name` rather than
    /// bare `name` + `file_path`, so a same-named method in an unrelated or
    /// ancestor class elsewhere in the file is left untouched — only the
    /// method actually defined in the enclosing body of this directive is
    /// affected.
    ///
    /// `want_singleton` selects which same-named node to target: instance methods
    /// and singleton methods (`def self.foo`) share both `NodeKind::Method` and
    /// `qualified_name`, so `state.singleton_method_ids` disambiguates them.
    /// `want_singleton == true` (`private_class_method`/`public_class_method`)
    /// matches only the singleton; `false` (`private`/`protected`/`public`)
    /// matches the instance method, or a top-level `Function` (file-scope
    /// `private :foo`).
    fn mark_method_visibility(
        state: &mut ExtractionState,
        name: &str,
        want_singleton: bool,
        visibility: Visibility,
    ) {
        let target_qn = format!("{}::{}", state.qualified_prefix(), name);
        let singleton_ids = &state.singleton_method_ids;
        if let Some(node) = state.nodes.iter_mut().rev().find(|n| {
            n.qualified_name == target_qn && {
                let is_singleton = singleton_ids.contains(&n.id);
                if want_singleton {
                    n.kind == NodeKind::Method && is_singleton
                } else {
                    !is_singleton && (n.kind == NodeKind::Method || n.kind == NodeKind::Function)
                }
            }
        }) {
            node.visibility = visibility;
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract the superclass from a class definition (`class Foo < Bar`).
    ///
    /// Creates an Extends `UnresolvedRef` from the class to its superclass.
    fn extract_superclass(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        // In tree-sitter-ruby, the superclass is a child node with field name "superclass"
        // or a "superclass" kind node. The superclass node contains the constant name.
        if let Some(superclass_node) = node.child_by_field_name("superclass") {
            let base_name = state.node_text(superclass_node);
            // Strip any leading whitespace/symbols from the superclass name
            let base_name = base_name.trim().trim_start_matches('<').trim().to_string();
            if !base_name.is_empty() {
                let line = superclass_node.start_position().row as u32;
                let column = superclass_node.start_position().column as u32;
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: class_id.to_string(),
                    reference_name: base_name,
                    reference_kind: EdgeKind::Extends,
                    line,
                    column,
                    file_path: state.file_path.clone(),
                });
            }
        } else {
            // Try finding a superclass child node by kind
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "superclass" {
                        // The superclass node contains "< ConstantName"
                        // Find the constant child inside superclass
                        if let Some(const_node) = find_child_by_kind(child, "constant")
                            .or_else(|| find_child_by_kind(child, "scope_resolution"))
                        {
                            let base_name = state.node_text(const_node);
                            let line = const_node.start_position().row as u32;
                            let column = const_node.start_position().column as u32;
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: class_id.to_string(),
                                reference_name: base_name,
                                reference_kind: EdgeKind::Extends,
                                line,
                                column,
                                file_path: state.file_path.clone(),
                            });
                        }
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    /// Extract the method signature (def name(params) ... end).
    ///
    /// Returns the first line of the method, which contains the signature.
    fn extract_method_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        // The signature is everything on the first line.
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract the singleton method signature (def self.name(params)).
    fn extract_singleton_method_signature(
        state: &ExtractionState,
        node: TsNode<'_>,
    ) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract the class signature (class Name or class Name < Base).
    fn extract_class_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
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
    /// Ruby uses comment lines (# ...) as documentation. We look for `comment`
    /// sibling nodes that immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Look at the previous sibling nodes for consecutive comment lines.
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
        // Comments were collected in reverse order; reverse them back.
        comments.reverse();
        Some(comments.join("\n"))
    }

    /// Decode a static `delimited_symbol` (`:"foo"`, `:"[]="`) to its method
    /// name. Returns `None` for symbols we can't resolve at extraction time —
    /// anything containing an `interpolation` (`:"#{x}"`) or `escape_sequence`
    /// (method names never need escapes) — so those are skipped rather than
    /// marked on the wrong method.
    fn static_delimited_symbol_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut name = String::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "string_content" {
                name.push_str(&state.node_text(child));
            } else {
                // interpolation / escape_sequence — not statically decodable.
                return None;
            }
        }
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    /// Find the method name identifier in a singleton method.
    ///
    /// In `def self.foo(args)`, we want "foo" (the identifier after the dot).
    /// tree-sitter-ruby's `singleton_method` has: "def", object, ".", name (identifier), parameters?, body
    fn find_last_identifier_before_params(
        state: &ExtractionState,
        node: TsNode<'_>,
    ) -> Option<String> {
        // Walk children and find the last identifier before "method_parameters" or body
        let mut cursor = node.walk();
        let mut last_ident: Option<String> = None;
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "identifier" => {
                        last_ident = Some(state.node_text(child));
                    }
                    "method_parameters" | "body_statement" => {
                        break;
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        last_ident
    }

    /// Recursively find call nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call" | "method_call" => {
                        // In tree-sitter-ruby, a call node has a "method" field for the method name.
                        // For simple calls like `foo(args)`, the first named child is the method name.
                        let callee_name =
                            if let Some(method_node) = child.child_by_field_name("method") {
                                Some(state.node_text(method_node))
                            } else {
                                // Fall back to first named child
                                child.named_child(0).map(|n| state.node_text(n))
                            };

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
                        // Recurse into the call for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested method/singleton_method/class/module definitions to avoid
                    // polluting call sites with their internal calls.
                    "method" | "singleton_method" | "class" | "module" => {}
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
}

impl crate::extraction::LanguageExtractor for RubyExtractor {
    fn extensions(&self) -> &[&str] {
        &["rb"]
    }

    fn language_name(&self) -> &'static str {
        "Ruby"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_ruby(file_path, source)
    }
}
