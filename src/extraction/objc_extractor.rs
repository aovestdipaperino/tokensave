/// Tree-sitter based Objective-C source code extractor.
///
/// Parses Objective-C source files and emits nodes and edges for the code graph.
/// Handles `.m` and `.mm` files.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, OBJC_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Objective-C source files using tree-sitter.
pub struct ObjcExtractor;

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
    /// Depth of @interface/@implementation nesting. > 0 means inside a type body.
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

impl ObjcExtractor {
    /// Extract code graph nodes and edges from an Objective-C source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Objective-C source code to parse.
    pub fn extract_objc(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("objc");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Objective-C grammar: {e}"))?;
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
            "preproc_include" => Self::visit_preproc_include(state, node),
            "preproc_def" => Self::visit_preproc_def(state, node),
            "type_definition" => Self::visit_type_definition(state, node),
            "protocol_declaration" => Self::visit_protocol(state, node),
            "class_interface" => Self::visit_class_interface(state, node),
            "class_implementation" => Self::visit_class_implementation(state, node),
            "function_definition" => Self::visit_function_definition(state, node),
            "declaration" => Self::visit_declaration(state, node),
            _ => {
                // For other node types, skip. Comments are picked up as docstrings.
            }
        }
    }

    // -------------------------------------------------------
    // preproc_include (#import / #include)
    // -------------------------------------------------------

    /// Extract a preprocessor #import or #include.
    fn visit_preproc_include(state: &mut ExtractionState, node: TsNode<'_>) {
        let path = Self::find_child_by_kind(node, "string_literal")
            .or_else(|| Self::find_child_by_kind(node, "system_lib_string"))
            .map_or_else(
                || "<unknown>".to_string(),
                |n| {
                    let text = state.node_text(n);
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
    // preproc_def (#define)
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
    // type_definition (typedef, including NS_ENUM)
    // -------------------------------------------------------

    /// Visit a `type_definition` node (typedef).
    ///
    /// For `typedef NS_ENUM(NSInteger, LogLevel) { ... };` the grammar produces
    /// a `type_definition` whose first `macro_type_specifier` child has name "`NS_ENUM`".
    /// The enum variant names appear as `type_identifier` children with field "declarator".
    fn visit_type_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        // Check if this is an NS_ENUM pattern
        if let Some(macro_spec) = Self::find_child_by_kind(node, "macro_type_specifier") {
            let macro_name = Self::find_child_by_kind(macro_spec, "identifier")
                .map(|n| state.node_text(n))
                .unwrap_or_default();
            if macro_name == "NS_ENUM" || macro_name == "NS_OPTIONS" {
                Self::visit_ns_enum(state, node, &macro_spec);
                return;
            }
        }

        // Otherwise, fall through to C-style typedef handling (simple typedef)
        Self::visit_simple_typedef(state, node);
    }

    /// Handle `NS_ENUM` / `NS_OPTIONS` typedef.
    ///
    /// The grammar parses this as:
    /// `type_definition`
    ///   `macro_type_specifier` (`NS_ENUM`)
    ///     identifier (`NS_ENUM`)
    ///     `type_descriptor` (`NSInteger`)
    ///     ERROR (, `LogLevel`)
    ///   ERROR ({)
    ///   `type_identifier` (`LogLevelDebug`)   [field=declarator]
    ///   `type_identifier` (`LogLevelInfo`)    [field=declarator]
    ///   ...
    ///   ERROR (})
    fn visit_ns_enum(state: &mut ExtractionState, node: TsNode<'_>, macro_spec: &TsNode<'_>) {
        // Extract the enum name. It's in the ERROR child of the macro_type_specifier
        // as an identifier after the comma. Alternatively, look at all direct children.
        let enum_name = Self::extract_ns_enum_name(state, *macro_spec)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());
        let qualified_name = format!("{}::{}", state.qualified_prefix(), enum_name);
        let id = generate_node_id(&state.file_path, &NodeKind::Enum, &enum_name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Enum,
            name: enum_name.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract enum variants: type_identifier children with field "declarator"
        state.node_stack.push((enum_name.clone(), id.clone()));
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_identifier" && cursor.field_name() == Some("declarator") {
                    let variant_name = state.node_text(child);
                    Self::create_enum_variant(state, &variant_name, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_stack.pop();
    }

    /// Extract the `NS_ENUM` name from the `macro_type_specifier`.
    /// The name is inside an ERROR child as an identifier.
    fn extract_ns_enum_name(state: &ExtractionState, macro_spec: TsNode<'_>) -> Option<String> {
        // Look for ERROR child containing an identifier
        let mut cursor = macro_spec.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "ERROR" {
                    // Inside ERROR, find the identifier
                    if let Some(ident) = Self::find_child_by_kind(child, "identifier") {
                        return Some(state.node_text(ident));
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Create an enum variant node.
    fn create_enum_variant(state: &mut ExtractionState, name: &str, node: TsNode<'_>) {
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::EnumVariant, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::EnumVariant,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Visit a simple typedef (not `NS_ENUM`).
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

    /// Find the typedef name (last `type_identifier` child).
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
    // @protocol
    // -------------------------------------------------------

    /// Extract a protocol declaration.
    ///
    /// Maps to Interface node kind with method declarations inside.
    fn visit_protocol(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let text = state.node_text(node);
        let signature = Self::extract_first_line(&text);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Interface, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Interface,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(signature),
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

        // Extract protocol inheritance from protocol_reference_list
        if let Some(ref_list) = Self::find_child_by_kind(node, "protocol_reference_list") {
            Self::extract_protocol_refs(state, ref_list, &id, start_line);
        }

        // Extract method declarations inside the protocol
        state.class_depth += 1;
        state.node_stack.push((name, id));
        Self::visit_protocol_children(state, node);
        state.node_stack.pop();
        state.class_depth -= 1;
    }

    /// Visit children of a protocol to extract method declarations.
    fn visit_protocol_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "method_declaration" {
                    Self::visit_method_declaration(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract protocol references (inheritance/conformance) from a `protocol_reference_list`.
    fn extract_protocol_refs(
        state: &mut ExtractionState,
        ref_list: TsNode<'_>,
        from_node_id: &str,
        line: u32,
    ) {
        let mut cursor = ref_list.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" {
                    let name = state.node_text(child);
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: from_node_id.to_string(),
                        reference_name: name,
                        reference_kind: EdgeKind::Implements,
                        line,
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

    // -------------------------------------------------------
    // @interface (class declaration)
    // -------------------------------------------------------

    /// Extract a class interface declaration (@interface ... @end).
    ///
    /// This includes properties, method declarations, superclass, and protocol conformance.
    fn visit_class_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let text = state.node_text(node);
        let signature = Self::extract_first_line(&text);
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
            signature: Some(signature),
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

        // Extract superclass (: BaseClass)
        if let Some(superclass) = node.child_by_field_name("superclass") {
            let super_name = state.node_text(superclass);
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: id.clone(),
                reference_name: super_name,
                reference_kind: EdgeKind::Extends,
                line: start_line,
                column: superclass.start_position().column as u32,
                file_path: state.file_path.clone(),
            });
        }

        // Extract protocol conformance (<Protocol1, Protocol2>)
        if let Some(params) = Self::find_child_by_kind(node, "parameterized_arguments") {
            Self::extract_protocol_conformance(state, params, &id, start_line);
        }

        // Extract properties and method declarations
        state.class_depth += 1;
        state.node_stack.push((name, id));
        Self::visit_interface_children(state, node);
        state.node_stack.pop();
        state.class_depth -= 1;
    }

    /// Extract protocol conformance from `parameterized_arguments` (<Serializable>).
    fn extract_protocol_conformance(
        state: &mut ExtractionState,
        params: TsNode<'_>,
        from_node_id: &str,
        line: u32,
    ) {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_name" {
                    if let Some(type_id) = Self::find_child_by_kind(child, "type_identifier") {
                        let name = state.node_text(type_id);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: from_node_id.to_string(),
                            reference_name: name,
                            reference_kind: EdgeKind::Implements,
                            line,
                            column: type_id.start_position().column as u32,
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

    /// Visit children of a class interface to extract properties and method declarations.
    fn visit_interface_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "property_declaration" => Self::visit_property_declaration(state, child),
                    "method_declaration" => Self::visit_method_declaration(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // -------------------------------------------------------
    // @property
    // -------------------------------------------------------

    /// Extract a property declaration.
    fn visit_property_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Property name is inside struct_declaration > struct_declarator > identifier
        // or struct_declaration > struct_declarator > pointer_declarator > identifier
        let name =
            Self::extract_property_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());

        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Property, &name, start_line);

        let visibility = Visibility::Pub;

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Property,
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

    /// Extract the property name from a `property_declaration` node.
    fn extract_property_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(struct_decl) = Self::find_child_by_kind(node, "struct_declaration") {
            if let Some(struct_declarator) =
                Self::find_child_by_kind(struct_decl, "struct_declarator")
            {
                // Direct identifier
                if let Some(ident) = Self::find_child_by_kind(struct_declarator, "identifier") {
                    return Some(state.node_text(ident));
                }
                // Pointer declarator (NSString *name)
                if let Some(ptr_decl) =
                    Self::find_child_by_kind(struct_declarator, "pointer_declarator")
                {
                    if let Some(ident) = Self::find_child_by_kind(ptr_decl, "identifier") {
                        return Some(state.node_text(ident));
                    }
                }
            }
        }
        None
    }

    // -------------------------------------------------------
    // method_declaration (in @interface or @protocol)
    // -------------------------------------------------------

    /// Extract a method declaration (no body).
    fn visit_method_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let name =
            Self::extract_method_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let _is_class_method = Self::is_class_method(state, node);

        let text = state.node_text(node);
        let signature = Some(text.trim().trim_end_matches(';').trim().to_string());
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = NodeKind::Method;

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind,
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
    // @implementation
    // -------------------------------------------------------

    /// Extract a class implementation block (@implementation ... @end).
    ///
    /// Maps to Impl node kind with method definitions inside.
    fn visit_class_implementation(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let text = state.node_text(node);
        let signature = Self::extract_first_line(&text);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Impl, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Impl,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(signature),
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

        // Visit method definitions inside the implementation
        state.class_depth += 1;
        state.node_stack.push((name, id));
        Self::visit_implementation_children(state, node);
        state.node_stack.pop();
        state.class_depth -= 1;
    }

    /// Visit children of a class implementation to extract method definitions.
    fn visit_implementation_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "implementation_definition" {
                    Self::visit_implementation_definition(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit an `implementation_definition` which wraps a `method_definition`.
    fn visit_implementation_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        // The implementation_definition contains a method_definition child
        if let Some(method_def) = Self::find_child_by_kind(node, "method_definition") {
            // Check for docstring on the implementation_definition itself
            // (comments appear as siblings of implementation_definition inside class_implementation)
            let docstring = Self::extract_impl_method_docstring(state, node);
            Self::visit_method_definition(state, method_def, docstring);
        }
    }

    /// Extract docstring for an `implementation_definition` by looking at preceding
    /// sibling comments within the `class_implementation`.
    fn extract_impl_method_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
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

    // -------------------------------------------------------
    // method_definition (with body, inside @implementation)
    // -------------------------------------------------------

    /// Extract a method definition (has a body).
    fn visit_method_definition(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        docstring: Option<String>,
    ) {
        let name =
            Self::extract_method_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());

        let text = state.node_text(node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let metrics = count_complexity(node, &OBJC_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the method body
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    // -------------------------------------------------------
    // function_definition (C functions)
    // -------------------------------------------------------

    /// Extract a top-level C function definition.
    fn visit_function_definition(state: &mut ExtractionState, node: TsNode<'_>) {
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
        let metrics = count_complexity(node, &OBJC_COMPLEXITY, &state.source);

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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the function body
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a C declaration (function prototype or variable).
    fn visit_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Check if this declaration contains a function_declarator (prototype)
        if Self::find_descendant_by_kind(node, "function_declarator").is_some() {
            Self::visit_function_prototype(state, node);
        }
    }

    /// Extract a function prototype declaration.
    fn visit_function_prototype(state: &mut ExtractionState, node: TsNode<'_>) {
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
    // Call site extraction
    // -------------------------------------------------------

    /// Recursively find `call_expression` and `message_expression` nodes and create
    /// unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        // C function call
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
                    }
                    "message_expression" => {
                        // Objective-C message send: [receiver method]
                        Self::extract_message_call(state, child, fn_node_id);
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
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

    /// Extract a message expression call site.
    ///
    /// For `[NSString stringWithFormat:...]`, the receiver is "`NSString`" and
    /// the method is "stringWithFormat".
    fn extract_message_call(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let method_name = node
            .child_by_field_name("method")
            .map(|n| state.node_text(n));
        let receiver_name = node
            .child_by_field_name("receiver")
            .map(|n| state.node_text(n));

        if let Some(method) = method_name {
            let reference_name = if let Some(receiver) = receiver_name {
                format!("{receiver}.{method}")
            } else {
                method
            };
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: fn_node_id.to_string(),
                reference_name,
                reference_kind: EdgeKind::Calls,
                line: node.start_position().row as u32,
                column: node.start_position().column as u32,
                file_path: state.file_path.clone(),
            });
        }
    }

    // -------------------------------------------------------
    // Method name and type extraction helpers
    // -------------------------------------------------------

    /// Extract the method name from a `method_definition` or `method_declaration` node.
    ///
    /// The method name is the first identifier child (not inside `method_type` or `method_parameter`).
    fn extract_method_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
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
        None
    }

    /// Check if a method is a class method (+) vs instance method (-).
    fn is_class_method(state: &ExtractionState, node: TsNode<'_>) -> bool {
        if let Some(first_child) = node.child(0) {
            let text = state.node_text(first_child);
            return text == "+";
        }
        false
    }

    /// Extract the function name from a `function_definition` or declaration node.
    fn extract_function_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(declarator) = Self::find_descendant_by_kind(node, "function_declarator") {
            if let Some(ident) = Self::find_child_by_kind(declarator, "identifier") {
                return Some(state.node_text(ident));
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
            text.trim().trim_end_matches(';').trim().to_string()
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

    /// Strip comment markers from a single C/ObjC comment text.
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

    /// Extract first line of text as a signature.
    fn extract_first_line(text: &str) -> String {
        text.lines().next().unwrap_or(text).trim().to_string()
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

impl crate::extraction::LanguageExtractor for ObjcExtractor {
    fn extensions(&self) -> &[&str] {
        &["m", "mm"]
    }

    fn language_name(&self) -> &'static str {
        "Objective-C"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        ObjcExtractor::extract_objc(file_path, source)
    }
}
