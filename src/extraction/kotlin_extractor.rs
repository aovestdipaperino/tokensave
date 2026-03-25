/// Tree-sitter based Kotlin source code extractor.
///
/// Parses Kotlin source files and emits nodes and edges for the code graph.
/// Supports Kotlin constructs including classes, data classes, sealed classes,
/// objects, companion objects, interfaces, enums, extension functions, and annotations.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, KOTLIN_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Kotlin source files using tree-sitter.
pub struct KotlinExtractor;

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
    /// Track nesting depth to distinguish methods from top-level functions.
    class_depth: usize,
    /// Track whether we are inside an interface (for abstract method detection).
    inside_trait: bool,
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
            inside_trait: false,
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

impl KotlinExtractor {
    /// Extract code graph nodes and edges from a Kotlin source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Kotlin source code to parse.
    pub fn extract_kotlin(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = tree_sitter_kotlin_sg::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Kotlin grammar: {e}"))?;
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
            "package_header" => Self::visit_package(state, node),
            "import_list" => Self::visit_import_list(state, node),
            "import_header" => Self::visit_import(state, node),
            "function_declaration" => Self::visit_function(state, node),
            "class_declaration" => Self::visit_class_declaration(state, node),
            "object_declaration" => Self::visit_object(state, node),
            "companion_object" => Self::visit_companion_object(state, node),
            "property_declaration" => Self::visit_property(state, node),
            "secondary_constructor" => Self::visit_secondary_constructor(state, node),
            _ => {
                // Recurse into children for any unhandled node types.
                Self::visit_children(state, node);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Package
    // -----------------------------------------------------------------------

    /// Extract a package header.
    fn visit_package(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<unknown>".to_string());

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::KotlinPackage, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::KotlinPackage,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node).lines().next().unwrap_or("").to_string()),
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

    // -----------------------------------------------------------------------
    // Imports
    // -----------------------------------------------------------------------

    /// Extract imports from an import_list node.
    fn visit_import_list(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "import_header" {
                    Self::visit_import(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single import header as a Use node.
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let path = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| {
                let text = state.node_text(node);
                text.trim()
                    .strip_prefix("import ")
                    .unwrap_or(&text)
                    .trim()
                    .to_string()
            });

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
            signature: Some(state.node_text(node).trim().to_string()),
            docstring: None,
            visibility: Visibility::Private,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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
            reference_name: path,
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    // -----------------------------------------------------------------------
    // Class declarations (class, data class, sealed class, interface, enum)
    // -----------------------------------------------------------------------

    /// Dispatch a class_declaration based on its modifiers and leading keywords.
    fn visit_class_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Determine if this is an interface, enum, data class, sealed class, or regular class
        // by checking the unnamed children and modifiers.
        let is_interface = Self::has_keyword_child(node, "interface");
        let is_enum = Self::has_keyword_child(node, "enum");
        let has_data = Self::has_modifier_keyword(node, state, "data");
        let has_sealed = Self::has_modifier_keyword(node, state, "sealed");

        if is_interface {
            Self::visit_interface(state, node);
        } else if is_enum {
            Self::visit_enum(state, node);
        } else if has_data {
            Self::visit_data_class(state, node);
        } else if has_sealed {
            Self::visit_sealed_class(state, node);
        } else {
            Self::visit_class(state, node);
        }
    }

    /// Extract a regular class.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_class_name(state, node);
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_kdoc(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

        Self::extract_annotations_from_modifiers(state, node, &id);
        Self::extract_delegation_specifiers(state, node, &id);

        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a data class.
    fn visit_data_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_class_name(state, node);
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_kdoc(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::DataClass, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::DataClass,
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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

        Self::extract_annotations_from_modifiers(state, node, &id);
        Self::extract_delegation_specifiers(state, node, &id);

        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a sealed class.
    fn visit_sealed_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_class_name(state, node);
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_kdoc(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::SealedClass, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::SealedClass,
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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

        Self::extract_annotations_from_modifiers(state, node, &id);

        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an interface (treated as Trait kind).
    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_class_name(state, node);
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_kdoc(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Trait, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Trait,
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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

        Self::extract_annotations_from_modifiers(state, node, &id);

        let prev_inside_trait = state.inside_trait;
        state.inside_trait = true;
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
        state.inside_trait = prev_inside_trait;
    }

    /// Extract an enum class.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_class_name(state, node);
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_kdoc(state, node);
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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

        Self::extract_annotations_from_modifiers(state, node, &id);

        // Visit enum body to extract enum entries.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "enum_class_body") {
            Self::visit_enum_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit an enum body to extract individual entries.
    fn visit_enum_body(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enum_entry" {
                    Self::visit_enum_entry(state, child);
                } else {
                    // Other members (functions, etc.) inside the enum body
                    Self::visit_node(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single enum entry.
    fn visit_enum_entry(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "simple_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| state.node_text(node).trim().to_string());

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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

    // -----------------------------------------------------------------------
    // Object
    // -----------------------------------------------------------------------

    /// Extract an object declaration (Kotlin singleton).
    fn visit_object(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_kdoc(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id =
            generate_node_id(&state.file_path, &NodeKind::KotlinObject, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::KotlinObject,
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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

        Self::extract_delegation_specifiers(state, node, &id);

        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    // -----------------------------------------------------------------------
    // Companion Object
    // -----------------------------------------------------------------------

    /// Extract a companion object.
    fn visit_companion_object(state: &mut ExtractionState, node: TsNode<'_>) {
        // Companion objects may have a name or be anonymous.
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "Companion".to_string());
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::CompanionObject,
            &name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::CompanionObject,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node).lines().next().unwrap_or("").trim().to_string()),
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    // -----------------------------------------------------------------------
    // Functions / Methods
    // -----------------------------------------------------------------------

    /// Extract a function or method declaration.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "simple_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        // Check if this is an extension function (has a receiver type before the dot and name).
        let is_extension = Self::is_extension_function(node);

        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_kdoc(state, node);
        let signature = Self::extract_declaration_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        // Check for suspend modifier.
        let is_async = Self::has_modifier_keyword(node, state, "suspend");

        // Determine if this is a function without a body (abstract) inside an interface.
        let has_body = Self::find_child_by_kind(node, "function_body").is_some();

        let kind = if state.inside_trait && !has_body {
            NodeKind::AbstractMethod
        } else if state.class_depth > 0 {
            NodeKind::Method
        } else {
            NodeKind::Function
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &KOTLIN_COMPLEXITY);

        // For extension functions, build a richer signature including the receiver type.
        let final_signature = if is_extension {
            Some(Self::extract_extension_signature(state, node))
        } else {
            signature
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
            signature: final_signature,
            docstring,
            visibility,
            is_async,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
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

        Self::extract_annotations_from_modifiers(state, node, &id);

        // Extract call sites from the body.
        if let Some(body) = Self::find_child_by_kind(node, "function_body") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    // -----------------------------------------------------------------------
    // Property (val/var)
    // -----------------------------------------------------------------------

    /// Extract a property declaration (val or var).
    fn visit_property(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_property_name(state, node);

        // Determine val vs var.
        let is_var = Self::find_child_by_kind(node, "binding_pattern_kind")
            .map(|bpk| {
                let text = state.node_text(bpk);
                text.trim() == "var"
            })
            .unwrap_or(false);

        let visibility = Self::extract_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = NodeKind::Property;
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

        let sig_text = state.node_text(node);
        let sig = if is_var {
            format!("var {}", sig_text.trim().strip_prefix("var ").unwrap_or(sig_text.trim()))
        } else {
            format!("val {}", sig_text.trim().strip_prefix("val ").unwrap_or(sig_text.trim()))
        };
        // Truncate at '=' for the signature
        let sig = sig.split('=').next().unwrap_or(&sig).trim().to_string();

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
            signature: Some(sig),
            docstring: Self::extract_kdoc(state, node),
            visibility,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
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

    // -----------------------------------------------------------------------
    // Secondary Constructor
    // -----------------------------------------------------------------------

    /// Extract a secondary constructor.
    fn visit_secondary_constructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let name = "constructor".to_string();
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(node, &KOTLIN_COMPLEXITY);

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
            signature: Some(
                state
                    .node_text(node)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            ),
            docstring: Self::extract_kdoc(state, node),
            visibility: Self::extract_visibility(node, state),
            is_async: false,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
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

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Extract the class name from a class_declaration node.
    fn extract_class_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string())
    }

    /// Extract the property name from a property_declaration node.
    fn extract_property_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // property_declaration has variable_declaration child which has simple_identifier
        if let Some(var_decl) = Self::find_child_by_kind(node, "variable_declaration") {
            if let Some(ident) = Self::find_child_by_kind(var_decl, "simple_identifier") {
                return state.node_text(ident);
            }
        }
        // Fallback: look for multi_variable_declaration
        if let Some(multi) = Self::find_child_by_kind(node, "multi_variable_declaration") {
            return state.node_text(multi);
        }
        "<anonymous>".to_string()
    }

    /// Check if a function_declaration is an extension function.
    /// Extension functions have a user_type child followed by a "." token before the name.
    fn is_extension_function(node: TsNode<'_>) -> bool {
        let mut found_user_type = false;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "user_type" && !found_user_type {
                    found_user_type = true;
                } else if !child.is_named() && child.kind() == "." && found_user_type {
                    return true;
                } else if child.kind() == "simple_identifier" {
                    // If we hit the name without finding a dot after user_type,
                    // this is not an extension function.
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Build the extension function signature from the node text.
    fn extract_extension_signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        // Cut at the function body.
        if let Some(brace_pos) = text.find('{') {
            return text[..brace_pos].trim().to_string();
        }
        if let Some(eq_pos) = text.find('=') {
            return text[..eq_pos].trim().to_string();
        }
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract Kotlin visibility from modifier list.
    fn extract_visibility(node: TsNode<'_>, state: &ExtractionState) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let inner_child = inner.node();
                            if inner_child.kind() == "visibility_modifier" {
                                let text = state.node_text(inner_child);
                                match text.trim() {
                                    "private" => return Visibility::Private,
                                    "internal" => return Visibility::PubCrate,
                                    "protected" => return Visibility::PubSuper,
                                    "public" => return Visibility::Pub,
                                    _ => {}
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
        // Kotlin default visibility is public.
        Visibility::Pub
    }

    /// Check if a node has a specific modifier keyword (e.g. "data", "sealed", "suspend").
    fn has_modifier_keyword(node: TsNode<'_>, state: &ExtractionState, keyword: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let inner_child = inner.node();
                            let text = state.node_text(inner_child);
                            if text.trim() == keyword {
                                return true;
                            }
                            // Check inside modifier children (e.g., class_modifier > data)
                            let mut deep = inner_child.walk();
                            if deep.goto_first_child() {
                                loop {
                                    let deep_child = deep.node();
                                    if !deep_child.is_named() && deep_child.kind() == keyword {
                                        return true;
                                    }
                                    if !deep.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                            if !inner.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                // Also check direct keyword children (e.g. "enum" appears as unnamed child).
                if !child.is_named() && child.kind() == keyword {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Check if a node has a specific unnamed keyword child.
    fn has_keyword_child(node: TsNode<'_>, keyword: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if !child.is_named() && child.kind() == keyword {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Extract the declaration signature (everything before the body).
    fn extract_declaration_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        // Cut at first '{' for brace-delimited bodies.
        if let Some(brace_pos) = text.find('{') {
            return Some(text[..brace_pos].trim().to_string());
        }
        // Cut at first '=' for expression-bodied definitions.
        if Self::find_child_by_kind(node, "function_body").is_some() {
            if let Some(eq_pos) = text.find('=') {
                return Some(text[..eq_pos].trim().to_string());
            }
        }
        Some(text.lines().next().unwrap_or("").trim().to_string())
    }

    /// Extract KDoc comments (/** ... */) preceding a declaration.
    fn extract_kdoc(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "multiline_comment" => {
                    let text = state.node_text(sibling);
                    if text.starts_with("/**") {
                        return Some(Self::clean_kdoc(&text));
                    }
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

    /// Clean a KDoc comment block, stripping markers.
    fn clean_kdoc(comment: &str) -> String {
        let trimmed = comment.trim();
        let inner = if trimmed.starts_with("/**") && trimmed.ends_with("*/") {
            &trimmed[3..trimmed.len() - 2]
        } else {
            trimmed
        };
        inner
            .lines()
            .map(|line| {
                let stripped = line.trim();
                stripped
                    .strip_prefix("* ")
                    .unwrap_or(stripped.strip_prefix('*').unwrap_or(stripped))
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }

    /// Extract delegation specifiers (superclass/interface) and create Extends unresolved refs.
    fn extract_delegation_specifiers(state: &mut ExtractionState, node: TsNode<'_>, owner_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "delegation_specifier" {
                    Self::extract_single_delegation(state, child, owner_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single delegation specifier (e.g. `: Foo()` or `: Bar`).
    fn extract_single_delegation(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        owner_id: &str,
    ) {
        // delegation_specifier can contain constructor_invocation or user_type.
        let type_name = Self::find_child_by_kind(node, "constructor_invocation")
            .and_then(|ci| Self::find_child_by_kind(ci, "user_type"))
            .or_else(|| Self::find_child_by_kind(node, "user_type"))
            .map(|ut| state.node_text(ut))
            .unwrap_or_else(|| state.node_text(node));

        let base_name = type_name
            .split('<')
            .next()
            .unwrap_or(&type_name)
            .trim()
            .to_string();

        if !base_name.is_empty() {
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: owner_id.to_string(),
                reference_name: base_name,
                reference_kind: EdgeKind::Extends,
                line: node.start_position().row as u32,
                column: node.start_position().column as u32,
                file_path: state.file_path.clone(),
            });
        }
    }

    /// Extract annotations from the modifiers of a declaration and create
    /// AnnotationUsage nodes and Annotates edges.
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

    /// Search inside a modifiers node for annotation nodes.
    fn extract_annotations_from_node(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "annotation" {
                    let annot_name = Self::extract_annotation_name(state, child);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name =
                        format!("{}::@{}", state.qualified_prefix(), annot_name);
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
                        branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            updated_at: state.timestamp,
                    };
                    state.nodes.push(graph_node);

                    // Annotates unresolved ref.
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: id.clone(),
                        reference_name: annot_name,
                        reference_kind: EdgeKind::Annotates,
                        line: start_line,
                        column: start_column,
                        file_path: state.file_path.clone(),
                    });

                    // Direct Annotates edge from the annotation to the target.
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

    /// Extract the name from an annotation node.
    fn extract_annotation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // annotation has constructor_invocation or user_type children.
        if let Some(ci) = Self::find_child_by_kind(node, "constructor_invocation") {
            if let Some(ut) = Self::find_child_by_kind(ci, "user_type") {
                if let Some(ti) = Self::find_child_by_kind(ut, "type_identifier") {
                    return state.node_text(ti);
                }
                return state.node_text(ut);
            }
        }
        if let Some(ut) = Self::find_child_by_kind(node, "user_type") {
            if let Some(ti) = Self::find_child_by_kind(ut, "type_identifier") {
                return state.node_text(ti);
            }
            return state.node_text(ut);
        }
        // Fallback: text after '@'
        let text = state.node_text(node);
        text.trim()
            .strip_prefix('@')
            .unwrap_or(&text)
            .split('(')
            .next()
            .unwrap_or(&text)
            .trim()
            .to_string()
    }

    /// Recursively find call_expression nodes and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        let callee_name = Self::extract_call_name(state, child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: callee_name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested definitions.
                    "function_declaration"
                    | "class_declaration"
                    | "object_declaration"
                    | "companion_object" => {}
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

    /// Extract the callee name from a call_expression.
    fn extract_call_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // call_expression's first named child is the function reference.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let child = cursor.node();
            match child.kind() {
                "navigation_expression" | "simple_identifier" => {
                    return state.node_text(child);
                }
                _ => {
                    return state.node_text(child);
                }
            }
        }
        let text = state.node_text(node);
        text.split('(').next().unwrap_or(&text).trim().to_string()
    }

    /// Find the first child node of a specific kind.
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

impl crate::extraction::LanguageExtractor for KotlinExtractor {
    fn extensions(&self) -> &[&str] {
        &["kt", "kts"]
    }

    fn language_name(&self) -> &str {
        "Kotlin"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        KotlinExtractor::extract_kotlin(file_path, source)
    }
}
