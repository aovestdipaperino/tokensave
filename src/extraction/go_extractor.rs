/// Tree-sitter based Go source code extractor.
///
/// Parses Go source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Go source files using tree-sitter.
pub struct GoExtractor;

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

impl GoExtractor {
    /// Extract code graph nodes and edges from a Go source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Go source code to parse.
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
        let language = tree_sitter_go::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Go grammar: {e}"))?;
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
            "package_clause" => Self::visit_package(state, node),
            "import_declaration" => Self::visit_imports(state, node),
            "function_declaration" => Self::visit_function(state, node),
            "method_declaration" => Self::visit_method(state, node),
            "type_declaration" => Self::visit_type_declaration(state, node),
            "const_declaration" => Self::visit_const_declaration(state, node),
            "var_declaration" => Self::visit_var_declaration(state, node),
            _ => {
                // For other node types, recurse into children to find nested items.
                // But skip comment nodes at top level (they are picked up as docstrings).
            }
        }
    }

    /// Extract a package clause node.
    fn visit_package(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "package_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<unknown>".to_string());
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::GoPackage, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::GoPackage,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node)),
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (File).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract import declarations. Each import spec becomes a Use node.
    fn visit_imports(state: &mut ExtractionState, node: TsNode<'_>) {
        // Imports can be: import "foo" or import ( "foo"; "bar" )
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "import_spec" => {
                        Self::visit_single_import(state, child);
                    }
                    "import_spec_list" => {
                        // Walk into the spec list to find individual import_spec nodes.
                        let mut inner = child.walk();
                        if inner.goto_first_child() {
                            loop {
                                let spec = inner.node();
                                if spec.kind() == "import_spec" {
                                    Self::visit_single_import(state, spec);
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

    /// Extract a single import spec as a Use node.
    fn visit_single_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Strip quotes from the import path.
        let path = text.trim().trim_matches('"').to_string();
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), path);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &path, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name: path.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring: None,
            visibility: Visibility::Private,
            is_async: false,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (File).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Unresolved Uses reference.
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: id,
            reference_name: path,
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Extract a function declaration node.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        // In Go, function name is an `identifier` child.
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::go_visibility(&name);
        let signature = Self::extract_signature(state, node);
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

        // Extract generic type parameters.
        Self::extract_type_params(state, node, &id);

        // Extract call sites from the function body.
        if let Some(body) = Self::find_child_by_kind(node, "block") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a method declaration node (function with receiver).
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        // In Go, method name is a `field_identifier` child.
        let name = Self::find_child_by_kind(node, "field_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::go_visibility(&name);
        let signature = Self::extract_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::StructMethod, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::StructMethod,
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
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (File).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract receiver type and create a Receives edge.
        Self::extract_receiver(state, node, &id);

        // Extract call sites from the method body.
        if let Some(body) = Self::find_child_by_kind(node, "block") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a type declaration (struct, interface, or type alias).
    fn visit_type_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // A type_declaration contains either a type_spec or a type_alias child.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "type_spec" => Self::visit_type_spec(state, child, node),
                    "type_alias" => Self::visit_type_alias(state, child, node),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a type_spec node, dispatching on whether it defines a struct or interface.
    fn visit_type_spec(state: &mut ExtractionState, spec_node: TsNode<'_>, decl_node: TsNode<'_>) {
        let name = Self::find_child_by_kind(spec_node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        // Check what type is being defined.
        if let Some(struct_type) = Self::find_child_by_kind(spec_node, "struct_type") {
            Self::visit_struct(state, &name, struct_type, decl_node);
        } else if let Some(iface_type) = Self::find_child_by_kind(spec_node, "interface_type") {
            Self::visit_interface(state, &name, iface_type, decl_node);
        } else {
            // A plain type definition (e.g., `type Foo int`) that is not a type alias.
            // Treat it like a type alias for graph purposes.
            Self::visit_named_type(state, &name, decl_node);
        }
    }

    /// Extract a struct type definition.
    fn visit_struct(
        state: &mut ExtractionState,
        name: &str,
        struct_type: TsNode<'_>,
        decl_node: TsNode<'_>,
    ) {
        let visibility = Self::go_visibility(name);
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, name, start_line);

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
            visibility,
            is_async: false,
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

        // Extract fields from the struct.
        state.node_stack.push((name.to_string(), id.clone()));
        Self::extract_struct_fields(state, struct_type);
        state.node_stack.pop();
    }

    /// Extract fields from a struct_type node.
    fn extract_struct_fields(state: &mut ExtractionState, struct_type: TsNode<'_>) {
        if let Some(field_list) = Self::find_child_by_kind(struct_type, "field_declaration_list") {
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

    /// Extract a single field from a field_declaration node.
    fn extract_single_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "field_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::go_visibility(&name);
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
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring: None,
            visibility,
            is_async: false,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (the struct).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract struct tags (raw_string_literal in field_declaration).
        if let Some(tag_node) = Self::find_child_by_kind(node, "raw_string_literal") {
            Self::extract_struct_tag(state, tag_node, &name, &id);
        }
    }

    /// Extract a struct tag from a raw_string_literal node.
    fn extract_struct_tag(
        state: &mut ExtractionState,
        tag_node: TsNode<'_>,
        field_name: &str,
        field_id: &str,
    ) {
        let tag_text = state.node_text(tag_node);
        let start_line = tag_node.start_position().row as u32;
        let end_line = tag_node.end_position().row as u32;
        let start_column = tag_node.start_position().column as u32;
        let end_column = tag_node.end_position().column as u32;
        let tag_name = format!("{}:tag", field_name);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), tag_name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::StructTag,
            &tag_name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::StructTag,
            name: tag_name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(tag_text),
            docstring: None,
            visibility: Visibility::Private,
            is_async: false,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from field.
        state.edges.push(Edge {
            source: field_id.to_string(),
            target: id,
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });
    }

    /// Extract an interface type definition.
    fn visit_interface(
        state: &mut ExtractionState,
        name: &str,
        iface_type: TsNode<'_>,
        decl_node: TsNode<'_>,
    ) {
        let visibility = Self::go_visibility(name);
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::InterfaceType, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::InterfaceType,
            name: name.to_string(),
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

        // Extract embedded interfaces (type_elem children).
        Self::extract_interface_embeddings(state, iface_type, &id);
    }

    /// Extract embedded interface types from an interface_type node.
    fn extract_interface_embeddings(
        state: &mut ExtractionState,
        iface_type: TsNode<'_>,
        iface_id: &str,
    ) {
        let mut cursor = iface_type.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_elem" {
                    // type_elem contains a type_identifier for the embedded interface.
                    if let Some(type_id) = Self::find_child_by_kind(child, "type_identifier") {
                        let embedded_name = state.node_text(type_id);
                        let line = child.start_position().row as u32;
                        let column = child.start_position().column as u32;
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: iface_id.to_string(),
                            reference_name: embedded_name,
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

    /// Extract a type alias (e.g., `type StringSlice = []string`).
    fn visit_type_alias(
        state: &mut ExtractionState,
        alias_node: TsNode<'_>,
        decl_node: TsNode<'_>,
    ) {
        let name = Self::find_child_by_kind(alias_node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::go_visibility(&name);
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::TypeAlias, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::TypeAlias,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring,
            visibility,
            is_async: false,
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

    /// Extract a named type definition that is neither struct nor interface.
    fn visit_named_type(state: &mut ExtractionState, name: &str, decl_node: TsNode<'_>) {
        let visibility = Self::go_visibility(name);
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::TypeAlias, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::TypeAlias,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring,
            visibility,
            is_async: false,
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

    /// Extract a const declaration. May contain multiple const_spec children.
    fn visit_const_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "const_spec" {
                    Self::visit_const_spec(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single const spec.
    fn visit_const_spec(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::go_visibility(&name);
        let text = state.node_text(node);
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
            visibility,
            is_async: false,
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

    /// Extract a var declaration. May contain multiple var_spec children.
    fn visit_var_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "var_spec" {
                    Self::visit_var_spec(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single var spec as a Static node (Go vars are package-level state).
    fn visit_var_spec(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::go_visibility(&name);
        let text = state.node_text(node);
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
            signature: Some(text.trim().to_string()),
            docstring: None,
            visibility,
            is_async: false,
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

    /// Extract the receiver type from a method_declaration and create a Receives edge.
    fn extract_receiver(state: &mut ExtractionState, node: TsNode<'_>, method_id: &str) {
        // The first parameter_list child is the receiver.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter_list" {
                    // This is the receiver parameter list.
                    // Extract the type name from the parameter_declaration inside.
                    if let Some(param) = Self::find_child_by_kind(child, "parameter_declaration") {
                        let receiver_type = Self::extract_receiver_type_name(state, param);
                        if let Some(type_name) = receiver_type {
                            let line = child.start_position().row as u32;
                            let column = child.start_position().column as u32;
                            // Create an unresolved Receives reference.
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: method_id.to_string(),
                                reference_name: type_name.clone(),
                                reference_kind: EdgeKind::Receives,
                                line,
                                column,
                                file_path: state.file_path.clone(),
                            });
                            // Also try to create a direct Receives edge if we can find
                            // the struct node. We look for it by matching name.
                            let struct_id = state
                                .nodes
                                .iter()
                                .find(|n| n.kind == NodeKind::Struct && n.name == type_name)
                                .map(|n| n.id.clone());
                            if let Some(struct_id) = struct_id {
                                state.edges.push(Edge {
                                    source: method_id.to_string(),
                                    target: struct_id,
                                    kind: EdgeKind::Receives,
                                    line: Some(line),
                                });
                            }
                        }
                    }
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the type name from a receiver parameter_declaration.
    /// Handles both `c Circle` and `c *Circle` forms.
    fn extract_receiver_type_name(state: &ExtractionState, param: TsNode<'_>) -> Option<String> {
        // Look for type_identifier directly or inside pointer_type.
        if let Some(type_id) = Self::find_child_by_kind(param, "type_identifier") {
            return Some(state.node_text(type_id));
        }
        if let Some(ptr_type) = Self::find_child_by_kind(param, "pointer_type") {
            if let Some(type_id) = Self::find_child_by_kind(ptr_type, "type_identifier") {
                return Some(state.node_text(type_id));
            }
        }
        None
    }

    /// Extract type parameters (generics) from a function or method declaration.
    fn extract_type_params(state: &mut ExtractionState, node: TsNode<'_>, parent_id: &str) {
        if let Some(type_params) = Self::find_child_by_kind(node, "type_parameter_list") {
            let mut cursor = type_params.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "type_parameter_declaration" {
                        // Each type_parameter_declaration has an identifier for the param name.
                        if let Some(ident) = Self::find_child_by_kind(child, "identifier") {
                            let name = state.node_text(ident);
                            let start_line = child.start_position().row as u32;
                            let end_line = child.end_position().row as u32;
                            let start_column = child.start_position().column as u32;
                            let end_column = child.end_position().column as u32;
                            let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                            let id = generate_node_id(
                                &state.file_path,
                                &NodeKind::GenericParam,
                                &name,
                                start_line,
                            );
                            let text = state.node_text(child);

                            let graph_node = Node {
                                id: id.clone(),
                                kind: NodeKind::GenericParam,
                                name,
                                qualified_name,
                                file_path: state.file_path.clone(),
                                start_line,
                                end_line,
                                start_column,
                                end_column,
                                signature: Some(text.trim().to_string()),
                                docstring: None,
                                visibility: Visibility::Private,
                                is_async: false,
                                updated_at: state.timestamp,
                            };
                            state.nodes.push(graph_node);

                            // Contains edge from the function/method.
                            state.edges.push(Edge {
                                source: parent_id.to_string(),
                                target: id,
                                kind: EdgeKind::Contains,
                                line: Some(start_line),
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

    /// Recursively find call_expression and selector_expression nodes inside a
    /// given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        // Get the callee: either an identifier or a selector_expression.
                        let callee = child.named_child(0);
                        if let Some(callee) = callee {
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
                    }
                    // Skip nested function literals to avoid polluting call sites.
                    "func_literal" => {}
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

    /// Extract the function/method signature (everything up to the body `{`).
    fn extract_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            Some(text[..brace_pos].trim().to_string())
        } else {
            Some(text.trim().to_string())
        }
    }

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

    /// Strip comment markers from a single Go comment text.
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

    /// Determine Go visibility: uppercase first character means exported (Pub),
    /// lowercase means unexported (Private).
    fn go_visibility(name: &str) -> Visibility {
        if name.starts_with(|c: char| c.is_uppercase()) {
            Visibility::Pub
        } else {
            Visibility::Private
        }
    }

    /// Find the first named child of a node with a given kind.
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

impl crate::extraction::LanguageExtractor for GoExtractor {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn language_name(&self) -> &str {
        "Go"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        GoExtractor::extract_source(file_path, source)
    }
}
