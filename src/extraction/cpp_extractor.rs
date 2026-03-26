/// Tree-sitter based C++ source code extractor.
///
/// Parses C++ source files and emits nodes and edges for the code graph.
/// Handles `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`, `.hh` files.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, CPP_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from C++ source files using tree-sitter.
pub struct CppExtractor;

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
    /// Current access specifier visibility inside a class/struct body.
    access_specifier: Visibility,
    /// Tracks class nesting depth (for inner classes).
    class_depth: usize,
    /// True if currently inside a `class` (default private), false if inside a `struct` (default public).
    #[allow(dead_code)]
    in_class_default_private: bool,
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
            access_specifier: Visibility::Private,
            class_depth: 0,
            in_class_default_private: true,
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

impl CppExtractor {
    /// Extract code graph nodes and edges from a C++ source file.
    pub fn extract_source(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = tree_sitter_cpp::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load C++ grammar: {e}"))?;
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
            "function_definition" => Self::visit_function_definition(state, node),
            "declaration" => Self::visit_declaration(state, node),
            "type_definition" => Self::visit_type_definition(state, node),
            "class_specifier" => Self::visit_class_specifier(state, node),
            "struct_specifier" => Self::visit_struct_specifier(state, node),
            "union_specifier" => Self::visit_standalone_union(state, node),
            "enum_specifier" => Self::visit_standalone_enum(state, node),
            "namespace_definition" => Self::visit_namespace(state, node),
            "template_declaration" => Self::visit_template(state, node),
            "using_declaration" => Self::visit_using_declaration(state, node),
            "preproc_def" => Self::visit_preproc_def(state, node),
            "preproc_include" => Self::visit_preproc_include(state, node),
            "access_specifier" => Self::visit_access_specifier(state, node),
            _ => {
                // For other node types, skip. Comments are picked up as docstrings.
            }
        }
    }

    // -------------------------------------------------------
    // function_definition (top-level or inside class body)
    // -------------------------------------------------------

    /// Extract a function definition (has a body).
    fn visit_function_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        let in_class = state.class_depth > 0;

        // Check for constructor / destructor
        if in_class {
            if Self::is_constructor(state, node) {
                Self::visit_constructor(state, node);
                return;
            }
            if Self::is_destructor(state, node) {
                Self::visit_destructor(state, node);
                return;
            }
        }

        let is_static = Self::has_storage_class(state, node, "static");
        let is_pure_virtual = Self::is_pure_virtual(state, node);

        let visibility = if in_class {
            state.access_specifier.clone()
        } else if is_static {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        let name = Self::extract_function_name(state, node)
            .unwrap_or_else(|| "<anonymous>".to_string());
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = if in_class {
            if is_pure_virtual {
                NodeKind::AbstractMethod
            } else {
                NodeKind::Method
            }
        } else {
            NodeKind::Function
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &CPP_COMPLEXITY, &state.source);

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

        // Extract call sites from the function body.
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Check if a function_definition is a constructor.
    fn is_constructor(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let name = Self::extract_function_name(state, node);
        if let Some(name) = &name {
            if let Some((class_name, _)) = state.node_stack.last() {
                if name == class_name {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a function_definition is a destructor.
    fn is_destructor(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let name = Self::extract_function_name(state, node);
        if let Some(name) = &name {
            if name.starts_with('~') {
                return true;
            }
        }
        Self::find_descendant_by_kind(node, "destructor_name").is_some()
    }

    /// Visit a constructor definition.
    fn visit_constructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_function_name(state, node)
            .unwrap_or_else(|| "<anonymous>".to_string());
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(node, &CPP_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Constructor,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring,
            visibility: state.access_specifier.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Visit a destructor definition.
    fn visit_destructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_function_name(state, node)
            .unwrap_or_else(|| Self::extract_destructor_name(state, node));
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let metrics = count_complexity(node, &CPP_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring,
            visibility: state.access_specifier.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract the destructor name from a node.
    fn extract_destructor_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(dtor) = Self::find_descendant_by_kind(node, "destructor_name") {
            return state.node_text(dtor);
        }
        if let Some((class_name, _)) = state.node_stack.last() {
            return format!("~{}", class_name);
        }
        "~<unknown>".to_string()
    }

    /// Extract the function name from a function_definition or declaration node.
    fn extract_function_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(declarator) = Self::find_descendant_by_kind(node, "function_declarator") {
            // Check for destructor_name first
            if let Some(dtor) = Self::find_child_by_kind(declarator, "destructor_name") {
                return Some(state.node_text(dtor));
            }
            // The function name is the identifier child of the function_declarator
            if let Some(ident) = Self::find_child_by_kind(declarator, "identifier") {
                return Some(state.node_text(ident));
            }
            // Could be a field_identifier (for methods)
            if let Some(ident) = Self::find_child_by_kind(declarator, "field_identifier") {
                return Some(state.node_text(ident));
            }
            // Could be a qualified_identifier
            if let Some(qi) = Self::find_child_by_kind(declarator, "qualified_identifier") {
                if let Some(ident) = Self::find_child_by_kind(qi, "identifier") {
                    return Some(state.node_text(ident));
                }
            }
            // Could be inside a pointer_declarator -> function_declarator
            if let Some(ident) = Self::find_child_by_kind(declarator, "parenthesized_declarator") {
                if let Some(inner_ident) = Self::find_descendant_by_kind(ident, "identifier") {
                    return Some(state.node_text(inner_ident));
                }
            }
            // type_identifier (for constructors, the name matches the class)
            if let Some(ident) = Self::find_child_by_kind(declarator, "type_identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    /// Extract the function signature (everything except the body).
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            Some(text[..brace_pos].trim().to_string())
        } else {
            let trimmed = text.trim().trim_end_matches(';').trim().to_string();
            Some(trimmed)
        }
    }

    // -------------------------------------------------------
    // declaration
    // -------------------------------------------------------

    /// Visit a declaration node.
    fn visit_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let in_class = state.class_depth > 0;

        // Check for class/struct/union/enum specifiers inside the declaration
        if Self::has_child_kind(node, "class_specifier")
            || Self::has_child_kind(node, "struct_specifier")
            || Self::has_child_kind(node, "union_specifier")
            || Self::has_child_kind(node, "enum_specifier")
        {
            Self::visit_children(state, node);
            return;
        }

        // Check if this is a function prototype
        if Self::find_descendant_by_kind(node, "function_declarator").is_some() {
            if in_class {
                Self::visit_class_method_declaration(state, node);
            } else {
                Self::visit_function_prototype(state, node);
            }
            return;
        }

        // If inside a class, treat as a field declaration
        if in_class {
            Self::visit_field_declaration_from_declaration(state, node);
            return;
        }

        // Otherwise, treat as a global variable
        Self::visit_global_variable(state, node);
    }

    /// Visit a method declaration (prototype) inside a class body.
    fn visit_class_method_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let is_pure_virtual = Self::is_pure_virtual(state, node);

        let name = Self::extract_function_name(state, node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        // Check if constructor
        if let Some((class_name, _)) = state.node_stack.last() {
            if name == *class_name {
                let text = state.node_text(node);
                let signature = Some(text.trim().trim_end_matches(';').trim().to_string());
                let docstring = Self::extract_docstring(state, node);
                let start_line = node.start_position().row as u32;
                let end_line = node.end_position().row as u32;
                let start_column = node.start_position().column as u32;
                let end_column = node.end_position().column as u32;
                let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                let id = generate_node_id(
                    &state.file_path,
                    &NodeKind::Constructor,
                    &name,
                    start_line,
                );

                let metrics = count_complexity(node, &CPP_COMPLEXITY, &state.source);
                let graph_node = Node {
                    id: id.clone(),
                    kind: NodeKind::Constructor,
                    name,
                    qualified_name,
                    file_path: state.file_path.clone(),
                    start_line,
                    end_line,
                    start_column,
                    end_column,
                    signature,
                    docstring,
                    visibility: state.access_specifier.clone(),
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

                if let Some(parent_id) = state.parent_node_id() {
                    state.edges.push(Edge {
                        source: parent_id.to_string(),
                        target: id,
                        kind: EdgeKind::Contains,
                        line: Some(start_line),
                    });
                }
                return;
            }
        }

        let text = state.node_text(node);
        let signature = Some(text.trim().trim_end_matches(';').trim().to_string());
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = if is_pure_virtual {
            NodeKind::AbstractMethod
        } else {
            NodeKind::Method
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &CPP_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring,
            visibility: state.access_specifier.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Visit a field-like declaration inside a class body (not a function).
    fn visit_field_declaration_from_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_variable_name(state, node);
        let Some(name) = name else { return };

        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
            visibility: state.access_specifier.clone(),
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a function prototype (declaration without body).
    fn visit_function_prototype(state: &mut ExtractionState, node: TsNode<'_>) {
        let is_static = Self::has_storage_class(state, node, "static");
        let visibility = if is_static {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        let name = Self::extract_function_name(state, node)
            .unwrap_or_else(|| "<anonymous>".to_string());
        let text = state.node_text(node);
        let signature = Some(text.trim().trim_end_matches(';').trim().to_string());
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name,
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a global variable declaration.
    fn visit_global_variable(state: &mut ExtractionState, node: TsNode<'_>) {
        let is_static = Self::has_storage_class(state, node, "static");
        let visibility = if is_static {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        let Some(name) = Self::extract_variable_name(state, node) else {
            return;
        };

        let text = state.node_text(node);
        let signature = Some(text.trim().trim_end_matches(';').trim().to_string());
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Static, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Static,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring: None,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a variable name from a declaration node.
    fn extract_variable_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(init_decl) = Self::find_child_by_kind(node, "init_declarator") {
            if let Some(ident) = Self::find_child_by_kind(init_decl, "identifier") {
                return Some(state.node_text(ident));
            }
            if let Some(ptr_decl) = Self::find_child_by_kind(init_decl, "pointer_declarator") {
                if let Some(ident) = Self::find_child_by_kind(ptr_decl, "identifier") {
                    return Some(state.node_text(ident));
                }
            }
        }
        if let Some(ident) = Self::find_child_by_kind(node, "identifier") {
            return Some(state.node_text(ident));
        }
        if let Some(ptr_decl) = Self::find_child_by_kind(node, "pointer_declarator") {
            if let Some(ident) = Self::find_child_by_kind(ptr_decl, "identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    // -------------------------------------------------------
    // class_specifier
    // -------------------------------------------------------

    /// Visit a class specifier (default visibility: Private).
    fn visit_class_specifier(state: &mut ExtractionState, node: TsNode<'_>) {
        if Self::find_child_by_kind(node, "field_declaration_list").is_none() {
            return;
        }
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        if name == "<anonymous>" {
            return;
        }

        let docstring = Self::extract_docstring(state, node);
        Self::create_class_node(state, &name, node, docstring, true);
    }

    /// Visit a struct specifier (default visibility: Pub).
    fn visit_struct_specifier(state: &mut ExtractionState, node: TsNode<'_>) {
        if Self::find_child_by_kind(node, "field_declaration_list").is_none() {
            return;
        }
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        if name == "<anonymous>" {
            return;
        }

        let docstring = Self::extract_docstring(state, node);
        Self::create_struct_node(state, &name, node, docstring);
    }

    /// Create a Class node and walk its body.
    fn create_class_node(
        state: &mut ExtractionState,
        name: &str,
        node: TsNode<'_>,
        docstring: Option<String>,
        default_private: bool,
    ) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, name, start_line);
        let text = state.node_text(node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
            name: name.to_string(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract base classes (inheritance).
        Self::extract_base_classes(state, node, &id);

        // Save and set access specifier state
        let old_access = state.access_specifier.clone();
        let old_in_class_default = state.in_class_default_private;
        let old_depth = state.class_depth;

        state.access_specifier = if default_private {
            Visibility::Private
        } else {
            Visibility::Pub
        };
        state.in_class_default_private = default_private;
        state.class_depth += 1;

        // Walk the class body
        state.node_stack.push((name.to_string(), id.clone()));
        if let Some(body) = Self::find_child_by_kind(node, "field_declaration_list") {
            Self::visit_class_body(state, body);
        }
        state.node_stack.pop();

        // Restore state
        state.access_specifier = old_access;
        state.in_class_default_private = old_in_class_default;
        state.class_depth = old_depth;
    }

    /// Create a Struct node (C++ struct with default public).
    fn create_struct_node(
        state: &mut ExtractionState,
        name: &str,
        node: TsNode<'_>,
        docstring: Option<String>,
    ) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, name, start_line);
        let text = state.node_text(node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Struct,
            name: name.to_string(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract base classes (inheritance).
        Self::extract_base_classes(state, node, &id);

        // Save and set access specifier state
        let old_access = state.access_specifier.clone();
        let old_in_class_default = state.in_class_default_private;
        let old_depth = state.class_depth;

        state.access_specifier = Visibility::Pub;
        state.in_class_default_private = false;
        state.class_depth += 1;

        // Walk the struct body
        state.node_stack.push((name.to_string(), id.clone()));
        if let Some(body) = Self::find_child_by_kind(node, "field_declaration_list") {
            Self::visit_class_body(state, body);
        }
        state.node_stack.pop();

        // Restore state
        state.access_specifier = old_access;
        state.in_class_default_private = old_in_class_default;
        state.class_depth = old_depth;
    }

    /// Walk the body of a class/struct, handling access specifiers and members.
    fn visit_class_body(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "access_specifier" => Self::visit_access_specifier(state, child),
                    "field_declaration" => Self::visit_field_declaration(state, child),
                    "function_definition" => Self::visit_function_definition(state, child),
                    "declaration" => Self::visit_declaration(state, child),
                    "class_specifier" => Self::visit_class_specifier(state, child),
                    "struct_specifier" => Self::visit_struct_specifier(state, child),
                    "template_declaration" => Self::visit_template(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit a field_declaration inside a class/struct body.
    fn visit_field_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Check if this is actually a method declaration (has a function_declarator)
        if Self::find_descendant_by_kind(node, "function_declarator").is_some() {
            Self::visit_class_method_declaration(state, node);
            return;
        }

        // It's a field
        let name = Self::find_descendant_by_kind(node, "field_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        if name == "<anonymous>" {
            return;
        }

        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
            visibility: state.access_specifier.clone(),
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // visit_field_method_declaration was identical to visit_class_method_declaration
    // and has been removed. Both call sites now use visit_class_method_declaration.

    // -------------------------------------------------------
    // access_specifier
    // -------------------------------------------------------

    /// Update the current access specifier based on an access_specifier node.
    fn visit_access_specifier(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state
            .node_text(node)
            .trim()
            .trim_end_matches(':')
            .trim()
            .to_string();
        state.access_specifier = match text.as_str() {
            "public" => Visibility::Pub,
            "private" => Visibility::Private,
            "protected" => Visibility::PubSuper,
            _ => state.access_specifier.clone(),
        };
    }

    // -------------------------------------------------------
    // namespace
    // -------------------------------------------------------

    /// Visit a namespace definition.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .or_else(|| Self::find_child_by_kind(node, "namespace_identifier"))
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Namespace, &name, start_line);
        let text = state.node_text(node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Namespace,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Walk namespace body
        state.node_stack.push((name, id));
        if let Some(body) = Self::find_child_by_kind(node, "declaration_list") {
            Self::visit_children(state, body);
        }
        state.node_stack.pop();
    }

    // -------------------------------------------------------
    // template
    // -------------------------------------------------------

    /// Visit a template declaration.
    fn visit_template(state: &mut ExtractionState, node: TsNode<'_>) {
        let inner_name = Self::extract_template_inner_name(state, node);
        let name = inner_name.unwrap_or_else(|| "<anonymous>".to_string());

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Template, &name, start_line);
        let text = state.node_text(node);
        let signature = text
            .find('{')
            .map(|pos| text[..pos].trim().to_string())
            .or_else(|| Some(text.trim().trim_end_matches(';').trim().to_string()));

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Template,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // If the template wraps a function, extract call sites
        if let Some(func_def) = Self::find_child_by_kind(node, "function_definition") {
            if let Some(body) = Self::find_child_by_kind(func_def, "compound_statement") {
                Self::extract_call_sites(state, body, &id);
            }
        }
    }

    /// Extract the name of the inner declaration in a template.
    fn extract_template_inner_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(func_def) = Self::find_child_by_kind(node, "function_definition") {
            return Self::extract_function_name(state, func_def);
        }
        if let Some(class_spec) = Self::find_child_by_kind(node, "class_specifier") {
            return Self::find_child_by_kind(class_spec, "type_identifier")
                .map(|n| state.node_text(n));
        }
        if let Some(struct_spec) = Self::find_child_by_kind(node, "struct_specifier") {
            return Self::find_child_by_kind(struct_spec, "type_identifier")
                .map(|n| state.node_text(n));
        }
        if let Some(decl) = Self::find_child_by_kind(node, "declaration") {
            return Self::extract_function_name(state, decl);
        }
        None
    }

    // -------------------------------------------------------
    // type_definition (typedef)
    // -------------------------------------------------------

    /// Visit a type_definition node (typedef).
    fn visit_type_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        if let Some(struct_spec) = Self::find_child_by_kind(node, "struct_specifier") {
            Self::visit_typedef_struct(state, node, struct_spec);
            return;
        }
        if let Some(union_spec) = Self::find_child_by_kind(node, "union_specifier") {
            Self::visit_typedef_union(state, node, union_spec);
            return;
        }
        if let Some(enum_spec) = Self::find_child_by_kind(node, "enum_specifier") {
            Self::visit_typedef_enum(state, node, enum_spec);
            return;
        }
        if Self::find_descendant_by_kind(node, "function_declarator").is_some() {
            Self::visit_typedef_function_pointer(state, node);
            return;
        }
        Self::visit_simple_typedef(state, node);
    }

    /// Extract a typedef for a struct.
    fn visit_typedef_struct(
        state: &mut ExtractionState,
        typedef_node: TsNode<'_>,
        struct_spec: TsNode<'_>,
    ) {
        let typedef_name = Self::find_typedef_name(state, typedef_node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = typedef_node.start_position().row as u32;
        let end_line = typedef_node.end_position().row as u32;
        let start_column = typedef_node.start_position().column as u32;
        let end_column = typedef_node.end_position().column as u32;
        let text = state.node_text(typedef_node);
        let docstring = Self::extract_docstring(state, typedef_node);

        let typedef_qualified = format!("{}::{}", state.qualified_prefix(), typedef_name);
        let typedef_id = generate_node_id(
            &state.file_path,
            &NodeKind::Typedef,
            &typedef_name,
            start_line,
        );
        let typedef_graph_node = Node {
            id: typedef_id.clone(),
            kind: NodeKind::Typedef,
            name: typedef_name.clone(),
            qualified_name: typedef_qualified,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: docstring.clone(),
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
        state.nodes.push(typedef_graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: typedef_id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        if Self::find_child_by_kind(struct_spec, "field_declaration_list").is_some() {
            let struct_name = Self::find_child_by_kind(struct_spec, "type_identifier")
                .map(|n| state.node_text(n))
                .unwrap_or_else(|| typedef_name.clone());
            Self::create_struct_node(state, &struct_name, struct_spec, docstring);
        }
    }

    /// Extract a typedef for a union.
    fn visit_typedef_union(
        state: &mut ExtractionState,
        typedef_node: TsNode<'_>,
        union_spec: TsNode<'_>,
    ) {
        let typedef_name = Self::find_typedef_name(state, typedef_node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = typedef_node.start_position().row as u32;
        let end_line = typedef_node.end_position().row as u32;
        let start_column = typedef_node.start_position().column as u32;
        let end_column = typedef_node.end_position().column as u32;
        let text = state.node_text(typedef_node);
        let docstring = Self::extract_docstring(state, typedef_node);

        let typedef_qualified = format!("{}::{}", state.qualified_prefix(), typedef_name);
        let typedef_id = generate_node_id(
            &state.file_path,
            &NodeKind::Typedef,
            &typedef_name,
            start_line,
        );
        let typedef_graph_node = Node {
            id: typedef_id.clone(),
            kind: NodeKind::Typedef,
            name: typedef_name.clone(),
            qualified_name: typedef_qualified,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: docstring.clone(),
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
        state.nodes.push(typedef_graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: typedef_id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        if Self::find_child_by_kind(union_spec, "field_declaration_list").is_some() {
            let union_name = Self::find_child_by_kind(union_spec, "type_identifier")
                .map(|n| state.node_text(n))
                .unwrap_or_else(|| typedef_name.clone());
            Self::create_union_node(state, &union_name, union_spec, docstring);
        }
    }

    /// Extract a typedef for an enum.
    fn visit_typedef_enum(
        state: &mut ExtractionState,
        typedef_node: TsNode<'_>,
        enum_spec: TsNode<'_>,
    ) {
        let typedef_name = Self::find_typedef_name(state, typedef_node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = typedef_node.start_position().row as u32;
        let end_line = typedef_node.end_position().row as u32;
        let start_column = typedef_node.start_position().column as u32;
        let end_column = typedef_node.end_position().column as u32;
        let text = state.node_text(typedef_node);
        let docstring = Self::extract_docstring(state, typedef_node);

        let typedef_qualified = format!("{}::{}", state.qualified_prefix(), typedef_name);
        let typedef_id = generate_node_id(
            &state.file_path,
            &NodeKind::Typedef,
            &typedef_name,
            start_line,
        );
        let typedef_graph_node = Node {
            id: typedef_id.clone(),
            kind: NodeKind::Typedef,
            name: typedef_name.clone(),
            qualified_name: typedef_qualified,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: docstring.clone(),
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
        state.nodes.push(typedef_graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: typedef_id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        if Self::find_child_by_kind(enum_spec, "enumerator_list").is_some() {
            let enum_name = Self::find_child_by_kind(enum_spec, "type_identifier")
                .map(|n| state.node_text(n))
                .unwrap_or_else(|| typedef_name.clone());
            Self::create_enum_node(state, &enum_name, enum_spec, docstring);
        }
    }

    /// Extract a function pointer typedef.
    fn visit_typedef_function_pointer(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_function_pointer_typedef_name(state, node).unwrap_or_else(|| {
            Self::find_typedef_name(state, node).unwrap_or_else(|| "<anonymous>".to_string())
        });

        let text = state.node_text(node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Typedef, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Typedef,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract the name from a function pointer typedef.
    fn extract_function_pointer_typedef_name(
        state: &ExtractionState,
        node: TsNode<'_>,
    ) -> Option<String> {
        if let Some(func_decl) = Self::find_descendant_by_kind(node, "function_declarator") {
            if let Some(paren_decl) =
                Self::find_child_by_kind(func_decl, "parenthesized_declarator")
            {
                if let Some(ident) = Self::find_descendant_by_kind(paren_decl, "identifier") {
                    return Some(state.node_text(ident));
                }
                if let Some(ident) =
                    Self::find_descendant_by_kind(paren_decl, "type_identifier")
                {
                    return Some(state.node_text(ident));
                }
            }
        }
        None
    }

    /// Simple typedef.
    fn visit_simple_typedef(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_typedef_name(state, node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let text = state.node_text(node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Typedef, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Typedef,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Find the typedef name (last type_identifier direct child).
    fn find_typedef_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut last_type_id = None;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_identifier" {
                    last_type_id = Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        last_type_id
    }

    // -------------------------------------------------------
    // Standalone union / enum
    // -------------------------------------------------------

    /// Visit a standalone union specifier.
    fn visit_standalone_union(state: &mut ExtractionState, node: TsNode<'_>) {
        if Self::find_child_by_kind(node, "field_declaration_list").is_none() {
            return;
        }
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        if name == "<anonymous>" {
            return;
        }

        let docstring = Self::extract_docstring(state, node);
        Self::create_union_node(state, &name, node, docstring);
    }

    /// Visit a standalone enum specifier.
    fn visit_standalone_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        if Self::find_child_by_kind(node, "enumerator_list").is_none() {
            return;
        }
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        if name == "<anonymous>" {
            return;
        }

        let docstring = Self::extract_docstring(state, node);
        Self::create_enum_node(state, &name, node, docstring);
    }

    // -------------------------------------------------------
    // using declaration
    // -------------------------------------------------------

    /// Visit a using declaration.
    fn visit_using_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let name = text
            .trim()
            .trim_start_matches("using")
            .trim()
            .trim_start_matches("namespace")
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_string();

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // -------------------------------------------------------
    // Node creation helpers
    // -------------------------------------------------------

    /// Create a Union node.
    fn create_union_node(
        state: &mut ExtractionState,
        name: &str,
        spec_node: TsNode<'_>,
        docstring: Option<String>,
    ) {
        let start_line = spec_node.start_position().row as u32;
        let end_line = spec_node.end_position().row as u32;
        let start_column = spec_node.start_position().column as u32;
        let end_column = spec_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Union, name, start_line);
        let text = state.node_text(spec_node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Union,
            name: name.to_string(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Create an Enum node with EnumVariant children.
    fn create_enum_node(
        state: &mut ExtractionState,
        name: &str,
        spec_node: TsNode<'_>,
        docstring: Option<String>,
    ) {
        let start_line = spec_node.start_position().row as u32;
        let end_line = spec_node.end_position().row as u32;
        let start_column = spec_node.start_position().column as u32;
        let end_column = spec_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Enum, name, start_line);
        let text = state.node_text(spec_node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Enum,
            name: name.to_string(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        state.node_stack.push((name.to_string(), id.clone()));
        Self::extract_enum_variants(state, spec_node);
        state.node_stack.pop();
    }

    // -------------------------------------------------------
    // Preprocessor
    // -------------------------------------------------------

    /// Extract a preprocessor #define.
    fn visit_preproc_def(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::PreprocessorDef,
            &name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::PreprocessorDef,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a preprocessor #include.
    fn visit_preproc_include(state: &mut ExtractionState, node: TsNode<'_>) {
        let path = Self::find_child_by_kind(node, "string_literal")
            .or_else(|| Self::find_child_by_kind(node, "system_lib_string"))
            .map(|n| {
                let text = state.node_text(n);
                text.trim_matches(|c| c == '"' || c == '<' || c == '>')
                    .to_string()
            })
            .unwrap_or_else(|| "<unknown>".to_string());

        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), path);
        let id = generate_node_id(&state.file_path, &NodeKind::Include, &path, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Include,
            name: path,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // -------------------------------------------------------
    // Enum variant extraction
    // -------------------------------------------------------

    /// Extract enum variants from an enum_specifier node.
    fn extract_enum_variants(state: &mut ExtractionState, enum_spec: TsNode<'_>) {
        if let Some(enumerator_list) = Self::find_child_by_kind(enum_spec, "enumerator_list") {
            let mut cursor = enumerator_list.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "enumerator" {
                        Self::extract_single_enumerator(state, child);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    /// Extract a single enumerator as an EnumVariant node.
    fn extract_single_enumerator(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::EnumVariant,
            &name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::EnumVariant,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // -------------------------------------------------------
    // Inheritance
    // -------------------------------------------------------

    /// Extract base classes from a class/struct specifier.
    fn extract_base_classes(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        if let Some(base_clause) = Self::find_child_by_kind(node, "base_class_clause") {
            let mut cursor = base_clause.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "type_identifier" {
                        let base_name = state.node_text(child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: class_id.to_string(),
                            reference_name: base_name,
                            reference_kind: EdgeKind::Extends,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                    if child.kind() == "qualified_identifier" {
                        let base_name = state.node_text(child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: class_id.to_string(),
                            reference_name: base_name,
                            reference_kind: EdgeKind::Extends,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    // -------------------------------------------------------
    // Call site extraction
    // -------------------------------------------------------

    /// Recursively find call_expression nodes and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "call_expression" {
                    if let Some(callee) = child.named_child(0) {
                        let callee_name = state.node_text(callee);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: callee_name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                    Self::extract_call_sites(state, child, fn_node_id);
                } else {
                    Self::extract_call_sites(state, child, fn_node_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // -------------------------------------------------------
    // Docstring extraction
    // -------------------------------------------------------

    /// Extract docstrings from preceding comment nodes.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            if sibling.kind() == "comment" {
                let text = state.node_text(sibling);
                comments.push(text);
                current = sibling.prev_named_sibling();
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        let cleaned: Vec<String> = comments.iter().map(|c| Self::clean_comment(c)).collect();
        let result = cleaned.join("\n").trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Strip comment markers from a single C/C++ comment text.
    fn clean_comment(comment: &str) -> String {
        let trimmed = comment.trim();
        if let Some(stripped) = trimmed.strip_prefix("///") {
            stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
        } else if let Some(stripped) = trimmed.strip_prefix("//") {
            stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
        } else if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
            let inner = &trimmed[2..trimmed.len() - 2];
            inner
                .lines()
                .map(|line| {
                    let l = line.trim();
                    l.strip_prefix("* ")
                        .or_else(|| l.strip_prefix('*'))
                        .unwrap_or(l)
                })
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        } else {
            trimmed.to_string()
        }
    }

    // -------------------------------------------------------
    // Utility helpers
    // -------------------------------------------------------

    /// Check if a declaration has a specific storage class specifier.
    fn has_storage_class(state: &ExtractionState, node: TsNode<'_>, class: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "storage_class_specifier" {
                    let text = state.node_text(child);
                    if text == class {
                        return true;
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Check if a function/method declaration is pure virtual (= 0).
    fn is_pure_virtual(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let text = state.node_text(node);
        text.contains("= 0")
    }

    /// Check if a node has a direct child of the given kind.
    fn has_child_kind(node: TsNode<'_>, kind: &str) -> bool {
        Self::find_child_by_kind(node, kind).is_some()
    }

    /// Find the first direct child of a node with a given kind.
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

    /// Find the first descendant of a node with a given kind (recursive search).
    fn find_descendant_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == kind {
                    return Some(child);
                }
                if let Some(found) = Self::find_descendant_by_kind(child, kind) {
                    return Some(found);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
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

impl crate::extraction::LanguageExtractor for CppExtractor {
    fn extensions(&self) -> &[&str] {
        &["cpp", "cc", "cxx", "hpp", "hxx", "hh"]
    }

    fn language_name(&self) -> &str {
        "C++"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        CppExtractor::extract_source(file_path, source)
    }
}
