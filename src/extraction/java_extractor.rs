/// Tree-sitter based Java source code extractor.
///
/// Parses Java source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Java source files using tree-sitter.
pub struct JavaExtractor;

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
    /// Track nesting depth to distinguish inner classes from top-level classes.
    class_depth: usize,
    /// Track whether we are inside an interface (for abstract method detection).
    inside_interface: bool,
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
            inside_interface: false,
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

impl JavaExtractor {
    /// Extract code graph nodes and edges from a Java source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Java source code to parse.
    pub fn extract_java(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = tree_sitter_java::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Java grammar: {e}"))?;
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
            "package_declaration" => Self::visit_package(state, node),
            "import_declaration" => Self::visit_import(state, node),
            "class_declaration" => Self::visit_class(state, node),
            "interface_declaration" => Self::visit_interface(state, node),
            "enum_declaration" => Self::visit_enum(state, node),
            "annotation_type_declaration" => Self::visit_annotation_type(state, node),
            "method_declaration" => Self::visit_method(state, node),
            "constructor_declaration" => Self::visit_constructor(state, node),
            "field_declaration" => Self::visit_field(state, node),
            "static_initializer" => Self::visit_static_initializer(state, node),
            "marker_annotation" | "annotation" => {
                // Annotations at the top level (not inside modifiers) are visited here.
                // But most annotations are inside modifiers of declarations; those are
                // handled in the declaration visitors directly.
            }
            _ => {
                // Recurse into children for any unhandled node types.
                Self::visit_children(state, node);
            }
        }
    }

    /// Extract a package declaration.
    fn visit_package(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Package text is like "package com.example.app;"
        let pkg_name = text
            .trim()
            .strip_prefix("package ")
            .unwrap_or(&text)
            .trim_end_matches(';')
            .trim()
            .to_string();

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), pkg_name);
        let id = generate_node_id(&state.file_path, &NodeKind::Package, &pkg_name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Package,
            name: pkg_name,
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
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (the file).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract an import declaration as a Use node.
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Strip "import " prefix, optional "static ", and trailing ";"
        let path = text
            .trim()
            .strip_prefix("import ")
            .unwrap_or(&text)
            .trim()
            .strip_prefix("static ")
            .unwrap_or(text.trim().strip_prefix("import ").unwrap_or(&text).trim())
            .trim_end_matches(';')
            .trim()
            .to_string();

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

        // Contains edge from parent.
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

    /// Extract a class declaration.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_java_visibility(node, state);
        let docstring = Self::extract_java_docstring(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        // Determine if this is an inner class or a top-level class.
        let kind = if state.class_depth > 0 {
            NodeKind::InnerClass
        } else {
            NodeKind::Class
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

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

        // Extract extends/implements.
        Self::extract_superclass(state, node, &id);
        Self::extract_super_interfaces(state, node, &id);

        // Extract generic type parameters.
        Self::extract_type_parameters(state, node, &id);

        // Visit class body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an interface declaration.
    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_java_visibility(node, state);
        let docstring = Self::extract_java_docstring(state, node);
        let signature = Self::extract_declaration_signature(state, node);
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

        // Extract type parameters.
        Self::extract_type_parameters(state, node, &id);

        // Visit interface body. Methods inside an interface with no block are AbstractMethod.
        let prev_inside_interface = state.inside_interface;
        state.inside_interface = true;
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
        state.inside_interface = prev_inside_interface;
    }

    /// Extract an enum declaration with its constants.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_java_visibility(node, state);
        let docstring = Self::extract_java_docstring(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Enum, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Enum,
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

        // Extract enum constants from the enum_body.
        state.node_stack.push((name, id));
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_enum_constants(state, body);
        }
        state.node_stack.pop();
    }

    /// Extract enum constants from an enum_body node.
    fn extract_enum_constants(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enum_constant" {
                    Self::extract_single_enum_constant(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single enum constant as an EnumVariant node.
    fn extract_single_enum_constant(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
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
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node).trim().to_string()),
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
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

    /// Extract an annotation type declaration (@interface).
    fn visit_annotation_type(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_java_visibility(node, state);
        let docstring = Self::extract_java_docstring(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Annotation, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Annotation,
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a method declaration.
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_java_visibility(node, state);
        let docstring = Self::extract_java_docstring(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        // Determine if this is an abstract method.
        // A method is abstract if:
        //   1. It has the `abstract` modifier, or
        //   2. It is in an interface and has no body (no `block` child).
        let has_abstract_modifier = Self::has_modifier(node, state, "abstract");
        let has_body =
            node.child_by_field_name("body").is_some() || Self::has_child_of_kind(node, "block");
        let is_abstract = has_abstract_modifier || (state.inside_interface && !has_body);

        let kind = if is_abstract {
            NodeKind::AbstractMethod
        } else {
            NodeKind::Method
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

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

        // Extract annotations on this method from its modifiers.
        Self::extract_annotations_from_modifiers(state, node, &id);

        // Extract call sites from the method body.
        if has_body {
            Self::extract_call_sites(state, node, &id);
        }
    }

    /// Extract a constructor declaration.
    fn visit_constructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_java_visibility(node, state);
        let docstring = Self::extract_java_docstring(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);

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

        // Extract call sites from the constructor body.
        Self::extract_call_sites(state, node, &id);
    }

    /// Extract field declarations. Each variable_declarator in the field becomes a Field node.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let visibility = Self::extract_java_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let signature_text = state.node_text(node).trim().to_string();

        // Iterate over variable_declarator children to extract each field name.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declarator" {
                    // The variable_declarator contains an identifier as its "name" field.
                    let field_name = child
                        .child_by_field_name("name")
                        .map(|n| state.node_text(n))
                        .unwrap_or_else(|| {
                            Self::extract_name(state, child)
                                .unwrap_or_else(|| "<anonymous>".to_string())
                        });

                    let qualified_name = format!("{}::{}", state.qualified_prefix(), field_name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::Field,
                        &field_name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::Field,
                        name: field_name,
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(signature_text.clone()),
                        docstring: None,
                        visibility: visibility.clone(),
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
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a static initializer block.
    fn visit_static_initializer(state: &mut ExtractionState, node: TsNode<'_>) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let name = format!("<static_init>:{start_line}");
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::InitBlock, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::InitBlock,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some("static { ... }".to_string()),
            docstring: None,
            visibility: Visibility::Private,
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

    /// Extract the name of a node by looking for a "name" field child.
    fn extract_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        node.child_by_field_name("name").map(|n| state.node_text(n))
    }

    /// Extract Java visibility from modifiers child.
    fn extract_java_visibility(node: TsNode<'_>, state: &ExtractionState) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    let text = state.node_text(child);
                    if text.contains("public") {
                        return Visibility::Pub;
                    } else if text.contains("protected") {
                        return Visibility::PubCrate;
                    } else if text.contains("private") {
                        return Visibility::Private;
                    }
                    // No access modifier → package-private, map to Private.
                    return Visibility::Private;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        // No modifiers at all → package-private, map to Private.
        Visibility::Private
    }

    /// Check if a node has a specific modifier keyword.
    fn has_modifier(node: TsNode<'_>, state: &ExtractionState, modifier: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    let text = state.node_text(child);
                    return text.split_whitespace().any(|w| w == modifier);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Check if a node has a direct child of a given kind.
    fn has_child_of_kind(node: TsNode<'_>, kind: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.node().kind() == kind {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Extract the declaration signature (text from start up to the opening `{`).
    fn extract_declaration_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            Some(text[..brace_pos].trim().to_string())
        } else {
            // For declarations without a body (e.g., abstract methods ending with `;`).
            Some(text.trim_end_matches(';').trim().to_string())
        }
    }

    /// Extract Java-style doc comments (/** ... */) preceding a declaration.
    fn extract_java_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "block_comment" => {
                    let text = state.node_text(sibling);
                    if text.starts_with("/**") {
                        return Some(Self::clean_javadoc(&text));
                    }
                    // Skip non-javadoc block comments.
                    current = sibling.prev_named_sibling();
                }
                "line_comment" => {
                    current = sibling.prev_named_sibling();
                }
                _ => break,
            }
        }
        None
    }

    /// Clean a Javadoc comment block, stripping the /** and */ markers and leading * on each line.
    fn clean_javadoc(comment: &str) -> String {
        let trimmed = comment.trim();
        // Strip /** prefix and */ suffix.
        let inner = if trimmed.starts_with("/**") && trimmed.ends_with("*/") {
            &trimmed[3..trimmed.len() - 2]
        } else {
            trimmed
        };

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
    }

    /// Extract superclass (extends) from a class_declaration.
    fn extract_superclass(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        // In tree-sitter-java, `superclass` is a named child of class_declaration.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "superclass" {
                    // The superclass node contains the type identifier.
                    let mut inner_cursor = child.walk();
                    if inner_cursor.goto_first_child() {
                        loop {
                            let inner_child = inner_cursor.node();
                            if inner_child.is_named()
                                && inner_child.kind() != "extends"
                                && inner_child.kind() != "superclass"
                            {
                                let type_name = state.node_text(inner_child);
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: class_id.to_string(),
                                    reference_name: type_name,
                                    reference_kind: EdgeKind::Extends,
                                    line: inner_child.start_position().row as u32,
                                    column: inner_child.start_position().column as u32,
                                    file_path: state.file_path.clone(),
                                });
                                break;
                            }
                            if !inner_cursor.goto_next_sibling() {
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

    /// Extract super_interfaces (implements) from a class_declaration.
    fn extract_super_interfaces(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "super_interfaces" {
                    // Inside super_interfaces there is a type_list containing type_identifiers.
                    Self::extract_type_list_as_implements(state, child, class_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract types from a type_list as Implements unresolved refs.
    fn extract_type_list_as_implements(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        class_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "type_identifier" || child.kind() == "generic_type")
                {
                    let type_name = state.node_text(child);
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: class_id.to_string(),
                        reference_name: type_name,
                        reference_kind: EdgeKind::Implements,
                        line: child.start_position().row as u32,
                        column: child.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                } else if child.kind() == "type_list" {
                    // Recurse into nested type_list.
                    Self::extract_type_list_as_implements(state, child, class_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract type parameters (generics) from a declaration.
    fn extract_type_parameters(state: &mut ExtractionState, node: TsNode<'_>, parent_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_parameters" {
                    Self::extract_type_params_from_list(state, child, parent_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract individual type_parameter nodes from a type_parameters node.
    fn extract_type_params_from_list(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        parent_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_parameter" {
                    let param_name = state.node_text(child);
                    // Extract just the type name (first identifier).
                    let name = param_name.split_whitespace().next().unwrap_or(&param_name);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::GenericParam,
                        name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::GenericParam,
                        name: name.to_string(),
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(param_name.trim().to_string()),
                        docstring: None,
                        visibility: Visibility::Private,
                        is_async: false,
                        updated_at: state.timestamp,
                    };
                    state.nodes.push(graph_node);

                    // Contains edge from parent.
                    state.edges.push(Edge {
                        source: parent_id.to_string(),
                        target: id,
                        kind: EdgeKind::Contains,
                        line: Some(start_line),
                    });
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract annotations from the modifiers of a declaration and create
    /// AnnotationUsage nodes and Annotates edges/refs.
    fn extract_annotations_from_modifiers(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    Self::extract_annotations_from_node(state, child, target_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Search inside a modifiers node for marker_annotation and annotation nodes.
    fn extract_annotations_from_node(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "marker_annotation" || child.kind() == "annotation" {
                    let annot_name = Self::extract_annotation_name(state, child);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::@{}", state.qualified_prefix(), annot_name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::AnnotationUsage,
                        &annot_name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::AnnotationUsage,
                        name: annot_name.clone(),
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(state.node_text(child).trim().to_string()),
                        docstring: None,
                        visibility: Visibility::Private,
                        is_async: false,
                        updated_at: state.timestamp,
                    };
                    state.nodes.push(graph_node);

                    // Annotates unresolved ref (annotation → target method/class).
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: id.clone(),
                        reference_name: annot_name,
                        reference_kind: EdgeKind::Annotates,
                        line: start_line,
                        column: start_column,
                        file_path: state.file_path.clone(),
                    });

                    // Also create a direct Annotates edge from the annotation to the target.
                    state.edges.push(Edge {
                        source: id,
                        target: target_id.to_string(),
                        kind: EdgeKind::Annotates,
                        line: Some(start_line),
                    });
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the name from an annotation node (e.g., "Override" from "@Override").
    fn extract_annotation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // Annotation nodes have a child that is the name (identifier or scoped_identifier).
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "identifier" || child.kind() == "scoped_identifier")
                {
                    return state.node_text(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        // Fallback: extract from text.
        let text = state.node_text(node);
        text.trim_start_matches('@').to_string()
    }

    /// Recursively find method_invocation and object_creation_expression nodes inside a
    /// given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "method_invocation" => {
                        let callee_name = Self::extract_method_invocation_name(state, child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: callee_name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                        // Recurse for nested calls inside arguments, etc.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    "object_creation_expression" => {
                        let type_name = Self::extract_object_creation_type(state, child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: format!("new {type_name}"),
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                        // Recurse for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested method/constructor declarations.
                    "method_declaration" | "constructor_declaration" | "class_declaration" => {}
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

    /// Extract the method name from a method_invocation node.
    fn extract_method_invocation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // method_invocation can be like:
        //   identifier "helper" + argument_list
        //   field_access "System.out" + "." + identifier "println" + argument_list
        // The tree-sitter "name" field gives the method name.
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = state.node_text(name_node);
            // Optionally prefix with the object.
            if let Some(obj_node) = node.child_by_field_name("object") {
                let obj = state.node_text(obj_node);
                return format!("{obj}.{name}");
            }
            return name;
        }
        // Fallback: full text of invocation.
        let text = state.node_text(node);
        text.split('(').next().unwrap_or(&text).trim().to_string()
    }

    /// Extract the type name from an object_creation_expression.
    fn extract_object_creation_type(state: &ExtractionState, node: TsNode<'_>) -> String {
        // object_creation_expression: "new" type argument_list
        // The "type" field gives the type name.
        if let Some(type_node) = node.child_by_field_name("type") {
            return state.node_text(type_node);
        }
        // Fallback: try to extract the type from children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "type_identifier"
                        || child.kind() == "generic_type"
                        || child.kind() == "scoped_type_identifier")
                {
                    return state.node_text(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        "<unknown>".to_string()
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

impl crate::extraction::LanguageExtractor for JavaExtractor {
    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn language_name(&self) -> &str {
        "Java"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        JavaExtractor::extract_java(file_path, source)
    }
}
