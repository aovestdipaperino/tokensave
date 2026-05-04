/// Tree-sitter based C source code extractor.
///
/// Parses C source files and emits nodes and edges for the code graph.
/// Handles `.c` and `.h` files.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, C_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from C source files using tree-sitter.
pub struct CExtractor;

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

impl CExtractor {
    /// Extract code graph nodes and edges from a C source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the C source code to parse.
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
        let language = crate::extraction::ts_provider::language("c");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load C grammar: {e}"))?;
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
            "struct_specifier" => Self::visit_standalone_struct(state, node),
            "union_specifier" => Self::visit_standalone_union(state, node),
            "enum_specifier" => Self::visit_standalone_enum(state, node),
            "preproc_def" => Self::visit_preproc_def(state, node),
            "preproc_include" => Self::visit_preproc_include(state, node),
            _ => {
                // For other node types, skip. Comments are picked up as docstrings.
            }
        }
    }

    // -------------------------------------------------------
    // function_definition
    // -------------------------------------------------------

    /// Extract a function definition (has a body).
    fn visit_function_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        let is_static = Self::has_storage_class(state, node, "static");
        let visibility = if is_static {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        let name =
            Self::extract_function_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let signature = Some(Self::extract_function_signature(state, node));
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
        let metrics = count_complexity(node, &C_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
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
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract the function name from a `function_definition` or declaration node.
    /// The name is typically inside a `function_declarator` -> `identifier`.
    fn extract_function_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Look for function_declarator which contains the name
        if let Some(declarator) = Self::find_descendant_by_kind(node, "function_declarator") {
            // The function name is the identifier child of the function_declarator
            if let Some(ident) = Self::find_child_by_kind(declarator, "identifier") {
                return Some(state.node_text(ident));
            }
            // Could also be inside a pointer_declarator -> function_declarator
            if let Some(ident) = Self::find_child_by_kind(declarator, "parenthesized_declarator") {
                // For function pointer patterns, try finding identifier deeper
                if let Some(inner_ident) = Self::find_descendant_by_kind(ident, "identifier") {
                    return Some(state.node_text(inner_ident));
                }
            }
        }
        None
    }

    /// Extract the function signature (everything except the body).
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            text[..brace_pos].trim().to_string()
        } else {
            // For declarations without a body (prototypes), use the full text without semicolon
            text.trim().trim_end_matches(';').trim().to_string()
        }
    }

    // -------------------------------------------------------
    // declaration (prototypes, variables, etc.)
    // -------------------------------------------------------

    /// Visit a declaration node. This can be a function prototype, global variable,
    /// or other declaration.
    fn visit_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Check if this declaration contains a function_declarator (prototype)
        if Self::find_descendant_by_kind(node, "function_declarator").is_some() {
            Self::visit_function_prototype(state, node);
            return;
        }

        // Check if this is a standalone struct/union/enum declaration
        // (e.g., `struct Foo { ... };`)
        if Self::has_child_kind(node, "struct_specifier")
            || Self::has_child_kind(node, "union_specifier")
            || Self::has_child_kind(node, "enum_specifier")
        {
            // These are handled by their own visitor when they appear as standalone declarations
            // with a body. Visit children to catch them.
            Self::visit_children(state, node);
            return;
        }

        // Otherwise, treat as a global variable declaration
        Self::visit_global_variable(state, node);
    }

    /// Extract a function prototype (declaration without body).
    fn visit_function_prototype(state: &mut ExtractionState, node: TsNode<'_>) {
        let is_static = Self::has_storage_class(state, node, "static");
        let visibility = if is_static {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        let name =
            Self::extract_function_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
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

        // Get the variable name from init_declarator or direct declarator
        let Some(name) = Self::extract_variable_name(state, node) else {
            return;
        };

        let text = state.node_text(node);
        let signature = Some(text.trim().trim_end_matches(';').trim().to_string());
        let docstring = Self::extract_docstring(state, node);
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a variable name from a declaration node.
    fn extract_variable_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Look for init_declarator first (e.g., `int x = 0;`)
        if let Some(init_decl) = Self::find_child_by_kind(node, "init_declarator") {
            // The identifier is the first child of init_declarator
            if let Some(ident) = Self::find_child_by_kind(init_decl, "identifier") {
                return Some(state.node_text(ident));
            }
            // Could be a pointer declarator: `int *x = NULL;`
            if let Some(ptr_decl) = Self::find_child_by_kind(init_decl, "pointer_declarator") {
                if let Some(ident) = Self::find_child_by_kind(ptr_decl, "identifier") {
                    return Some(state.node_text(ident));
                }
            }
        }
        // Direct identifier child (e.g., `int x;`)
        if let Some(ident) = Self::find_child_by_kind(node, "identifier") {
            return Some(state.node_text(ident));
        }
        // Pointer declarator without init (e.g., `char *name;`)
        if let Some(ptr_decl) = Self::find_child_by_kind(node, "pointer_declarator") {
            if let Some(ident) = Self::find_child_by_kind(ptr_decl, "identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    // -------------------------------------------------------
    // type_definition (typedef)
    // -------------------------------------------------------

    /// Visit a `type_definition` node (typedef).
    fn visit_type_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        // Check for typedef struct { ... } Name;
        if let Some(struct_spec) = Self::find_child_by_kind(node, "struct_specifier") {
            Self::visit_typedef_struct(state, node, struct_spec);
            return;
        }

        // Check for typedef union { ... } Name;
        if let Some(union_spec) = Self::find_child_by_kind(node, "union_specifier") {
            Self::visit_typedef_union(state, node, union_spec);
            return;
        }

        // Check for typedef enum { ... } Name;
        if let Some(enum_spec) = Self::find_child_by_kind(node, "enum_specifier") {
            Self::visit_typedef_enum(state, node, enum_spec);
            return;
        }

        // Check for function pointer typedef: typedef int (*name)(args);
        if Self::find_descendant_by_kind(node, "function_declarator").is_some() {
            Self::visit_typedef_function_pointer(state, node);
            return;
        }

        // Simple typedef: typedef old_type new_name;
        Self::visit_simple_typedef(state, node);
    }

    /// Extract a typedef for a struct.
    fn visit_typedef_struct(
        state: &mut ExtractionState,
        typedef_node: TsNode<'_>,
        struct_spec: TsNode<'_>,
    ) {
        // Get the typedef name (the type_identifier at the end)
        let typedef_name = Self::find_typedef_name(state, typedef_node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = typedef_node.start_position().row as u32;
        let end_line = typedef_node.end_position().row as u32;
        let start_column = typedef_node.start_position().column as u32;
        let end_column = typedef_node.end_position().column as u32;
        let text = state.node_text(typedef_node);
        let docstring = Self::extract_docstring(state, typedef_node);

        // Create Typedef node
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
            attrs_start_line: start_line,
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

        // Also create a Struct node if it has a body
        if Self::find_child_by_kind(struct_spec, "field_declaration_list").is_some() {
            let struct_name = Self::find_child_by_kind(struct_spec, "type_identifier")
                .map_or_else(|| typedef_name.clone(), |n| state.node_text(n));

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

        // Create Typedef node
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
            attrs_start_line: start_line,
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

        // Also create a Union node if it has a body
        if Self::find_child_by_kind(union_spec, "field_declaration_list").is_some() {
            let union_name = Self::find_child_by_kind(union_spec, "type_identifier")
                .map_or_else(|| typedef_name.clone(), |n| state.node_text(n));

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

        // Create Typedef node
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
            attrs_start_line: start_line,
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

        // Also create an Enum node if it has a body
        if Self::find_child_by_kind(enum_spec, "enumerator_list").is_some() {
            let enum_name = Self::find_child_by_kind(enum_spec, "type_identifier")
                .map_or_else(|| typedef_name.clone(), |n| state.node_text(n));

            Self::create_enum_node(state, &enum_name, enum_spec, docstring);
        }
    }

    /// Extract a function pointer typedef.
    fn visit_typedef_function_pointer(state: &mut ExtractionState, node: TsNode<'_>) {
        // For `typedef int (*compare_fn)(const void *, const void *);`
        // The name is inside the parenthesized_declarator within the function_declarator
        let name = Self::extract_function_pointer_typedef_name(state, node).unwrap_or_else(|| {
            // Fallback: try the standard typedef name extraction
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
            attrs_start_line: start_line,
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
        // In `typedef int (*name)(args)`, the name is inside
        // function_declarator -> parenthesized_declarator -> pointer_declarator -> identifier
        // or function_declarator -> parenthesized_declarator -> identifier
        if let Some(func_decl) = Self::find_descendant_by_kind(node, "function_declarator") {
            if let Some(paren_decl) =
                Self::find_child_by_kind(func_decl, "parenthesized_declarator")
            {
                if let Some(ident) = Self::find_descendant_by_kind(paren_decl, "identifier") {
                    return Some(state.node_text(ident));
                }
                if let Some(ident) = Self::find_descendant_by_kind(paren_decl, "type_identifier") {
                    return Some(state.node_text(ident));
                }
            }
        }
        None
    }

    /// Extract a simple typedef (e.g., `typedef unsigned long ulong;`).
    fn visit_simple_typedef(state: &mut ExtractionState, node: TsNode<'_>) {
        let name =
            Self::find_typedef_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());

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
            attrs_start_line: start_line,
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

    /// Find the typedef name, which is usually the last `type_identifier` child of the
    /// `type_definition` node.
    fn find_typedef_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // The typedef name is typically the last type_identifier direct child
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
    // Standalone struct/union/enum (inside a declaration)
    // -------------------------------------------------------

    /// Visit a standalone struct specifier (e.g., `struct Point { int x; int y; };`).
    fn visit_standalone_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        // Only handle if it has a body (field_declaration_list)
        if Self::find_child_by_kind(node, "field_declaration_list").is_none() {
            return;
        }
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        // Skip anonymous structs that are not inside a typedef
        if name == "<anonymous>" {
            return;
        }

        let docstring = Self::extract_docstring(state, node);
        Self::create_struct_node(state, &name, node, docstring);
    }

    /// Visit a standalone union specifier.
    fn visit_standalone_union(state: &mut ExtractionState, node: TsNode<'_>) {
        if Self::find_child_by_kind(node, "field_declaration_list").is_none() {
            return;
        }
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

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
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        if name == "<anonymous>" {
            return;
        }

        let docstring = Self::extract_docstring(state, node);
        Self::create_enum_node(state, &name, node, docstring);
    }

    // -------------------------------------------------------
    // Node creation helpers
    // -------------------------------------------------------

    /// Create a Struct node and its field children.
    fn create_struct_node(
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
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, name, start_line);
        let text = state.node_text(spec_node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Struct,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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

        // Extract fields.
        state.node_stack.push((name.to_string(), id.clone()));
        Self::extract_struct_fields(state, spec_node);
        state.node_stack.pop();
    }

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
            attrs_start_line: start_line,
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

        // Extract fields (same as struct).
        state.node_stack.push((name.to_string(), id.clone()));
        Self::extract_struct_fields(state, spec_node);
        state.node_stack.pop();
    }

    /// Create an Enum node with `EnumVariant` children.
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
            attrs_start_line: start_line,
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

        // Extract enum variants.
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
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

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

    /// Extract a preprocessor #include.
    fn visit_preproc_include(state: &mut ExtractionState, node: TsNode<'_>) {
        // The include path is in a string_literal or system_lib_string child
        let path = Self::find_child_by_kind(node, "string_literal")
            .or_else(|| Self::find_child_by_kind(node, "system_lib_string"))
            .map_or_else(
                || "<unknown>".to_string(),
                |n| {
                    let text = state.node_text(n);
                    // Strip quotes/angle brackets
                    text.trim_matches(|c| c == '"' || c == '<' || c == '>')
                        .to_string()
                },
            );

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

    // -------------------------------------------------------
    // Field and enum variant extraction
    // -------------------------------------------------------

    /// Extract fields from a struct or union specifier.
    fn extract_struct_fields(state: &mut ExtractionState, spec_node: TsNode<'_>) {
        if let Some(field_list) = Self::find_child_by_kind(spec_node, "field_declaration_list") {
            let mut cursor = field_list.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "field_declaration" {
                        Self::extract_single_field(state, child);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    /// Extract a single field from a `field_declaration` node.
    fn extract_single_field(state: &mut ExtractionState, node: TsNode<'_>) {
        // In C, the field name is a field_identifier child of the field_declaration
        // or inside a field_declarator child.
        let name = Self::find_descendant_by_kind(node, "field_identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

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
            attrs_start_line: start_line,
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

        // Contains edge from parent (the struct/union).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract enum variants from an `enum_specifier` node.
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

    /// Extract a single enumerator as an `EnumVariant` node.
    fn extract_single_enumerator(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::EnumVariant, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::EnumVariant,
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

        // Contains edge from parent (the enum).
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
    // Call site extraction
    // -------------------------------------------------------

    /// Recursively find `call_expression` nodes and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "call_expression" {
                    // Get the callee: the first named child (usually an identifier)
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
                    // Also recurse into the call expression for nested calls.
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
        // Comments are collected in reverse order (closest first).
        comments.reverse();
        let cleaned: Vec<String> = comments.iter().map(|c| Self::clean_comment(c)).collect();
        let result = cleaned.join("\n").trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Strip comment markers from a single C comment text.
    fn clean_comment(comment: &str) -> String {
        let trimmed = comment.trim();
        if let Some(stripped) = trimmed.strip_prefix("//") {
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

    /// Check if a declaration has a specific storage class specifier (e.g., "static").
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
                // Recurse into children
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

impl crate::extraction::LanguageExtractor for CExtractor {
    fn extensions(&self) -> &[&str] {
        &["c", "h"]
    }

    fn language_name(&self) -> &'static str {
        "C"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        CExtractor::extract_source(file_path, source)
    }
}
