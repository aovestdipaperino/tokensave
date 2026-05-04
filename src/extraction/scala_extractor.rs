// Rust guideline compliant 2025-10-17
/// Tree-sitter based Scala source code extractor.
///
/// Parses Scala source files and emits nodes and edges for the code graph.
/// Supports Scala 2 and Scala 3 constructs including classes, case classes,
/// traits, objects, enums, and extension methods.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, SCALA_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Scala source files using tree-sitter.
pub struct ScalaExtractor;

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
    /// Track nesting depth to distinguish inner classes from top-level classes.
    class_depth: usize,
    /// Track whether we are inside a trait (for abstract method detection).
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

impl ScalaExtractor {
    /// Extract code graph nodes and edges from a Scala source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Scala source code to parse.
    pub fn extract_scala(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("scala");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Scala grammar: {e}"))?;
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
            "import_declaration" => Self::visit_import(state, node),
            "class_definition" => Self::visit_class(state, node),
            "trait_definition" => Self::visit_trait(state, node),
            "object_definition" => Self::visit_object(state, node),
            "enum_definition" => Self::visit_enum(state, node),
            "function_definition" => Self::visit_function_def(state, node),
            "function_declaration" => Self::visit_function_decl(state, node),
            "val_definition" | "val_declaration" => Self::visit_val(state, node),
            "var_definition" | "var_declaration" => Self::visit_var(state, node),
            "type_definition" => Self::visit_type_def(state, node),
            _ => {
                // Recurse into children for any unhandled node types.
                Self::visit_children(state, node);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Package
    // -----------------------------------------------------------------------

    /// Extract a package clause.
    fn visit_package(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<unknown>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::ScalaPackage, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::ScalaPackage,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(
                state
                    .node_text(node)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
            ),
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

        // If the package clause has a body, visit it.
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
    }

    // -----------------------------------------------------------------------
    // Imports
    // -----------------------------------------------------------------------

    /// Extract an import declaration as a Use node.
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let path = text
            .trim()
            .strip_prefix("import ")
            .unwrap_or(&text)
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
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring: None,
            visibility: Visibility::Private,
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
            reference_name: path,
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    // -----------------------------------------------------------------------
    // Class / Case Class
    // -----------------------------------------------------------------------

    /// Extract a class definition. Detects case classes via modifiers.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let is_case = Self::has_modifier_keyword(node, state, "case");
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_scaladoc(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = if is_case {
            NodeKind::CaseClass
        } else if state.class_depth > 0 {
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);
        Self::extract_extends(state, node, &id);
        Self::extract_type_parameters(state, node, &id);
        Self::extract_class_params_as_fields(state, node, &id);

        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    // -----------------------------------------------------------------------
    // Trait
    // -----------------------------------------------------------------------

    /// Extract a trait definition.
    fn visit_trait(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_scaladoc(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);
        Self::extract_extends(state, node, &id);
        Self::extract_type_parameters(state, node, &id);

        let prev_inside_trait = state.inside_trait;
        state.inside_trait = true;
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
        state.inside_trait = prev_inside_trait;
    }

    // -----------------------------------------------------------------------
    // Object
    // -----------------------------------------------------------------------

    /// Extract an object definition (Scala singleton).
    fn visit_object(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_scaladoc(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::ScalaObject, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::ScalaObject,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);
        Self::extract_extends(state, node, &id);

        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    // -----------------------------------------------------------------------
    // Enum (Scala 3)
    // -----------------------------------------------------------------------

    /// Extract an enum definition.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_scaladoc(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);
        Self::extract_type_parameters(state, node, &id);

        // Visit enum body to extract enum cases.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_enum_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit enum body to extract individual enum cases.
    fn visit_enum_body(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "simple_enum_case" | "full_enum_case" => {
                        Self::visit_enum_case(state, child);
                    }
                    "enum_case_definitions" => {
                        Self::visit_enum_body(state, child);
                    }
                    _ => {
                        Self::visit_node(state, child);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single enum case.
    fn visit_enum_case(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .or_else(|| Self::find_child_by_kind(node, "identifier"))
            .map_or_else(
                || state.node_text(node).trim().to_string(),
                |n| state.node_text(n),
            );

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
            signature: Some(state.node_text(node).trim().to_string()),
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

    // -----------------------------------------------------------------------
    // Functions / Methods
    // -----------------------------------------------------------------------

    /// Extract a function/method definition (has a body).
    fn visit_function_def(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_scaladoc(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = if state.class_depth > 0 {
            NodeKind::Method
        } else {
            NodeKind::Function
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &SCALA_COMPLEXITY, &state.source);

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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);

        // Extract call sites from the body.
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a function/method declaration (abstract, no body).
    fn visit_function_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_scaladoc(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = if state.inside_trait {
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);
    }

    // -----------------------------------------------------------------------
    // Val / Var
    // -----------------------------------------------------------------------

    /// Extract a val definition or declaration as a `ValField` node.
    fn visit_val(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_val_var_name(state, node);
        let visibility = Self::extract_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::ValField, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::ValField,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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
            docstring: Self::extract_scaladoc(state, node),
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
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);

        // Extract call sites from the value expression.
        if let Some(value) = node.child_by_field_name("value") {
            Self::extract_call_sites(state, value, &id);
        }
    }

    /// Extract a var definition or declaration as a `VarField` node.
    fn visit_var(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_val_var_name(state, node);
        let visibility = Self::extract_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::VarField, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::VarField,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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
            docstring: Self::extract_scaladoc(state, node),
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
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);

        if let Some(value) = node.child_by_field_name("value") {
            Self::extract_call_sites(state, value, &id);
        }
    }

    // -----------------------------------------------------------------------
    // Type definition
    // -----------------------------------------------------------------------

    /// Extract a type alias definition.
    fn visit_type_def(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));
        let visibility = Self::extract_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::TypeAlias, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::TypeAlias,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node).trim().to_string()),
            docstring: Self::extract_scaladoc(state, node),
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

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Extract the name from a node's "name" field.
    fn extract_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        node.child_by_field_name("name").map(|n| state.node_text(n))
    }

    /// Extract the name from a val/var definition.
    ///
    /// `val_definition` uses a "pattern" field; `val_declaration` uses "name".
    fn extract_val_var_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(name_node) = node.child_by_field_name("name") {
            return state.node_text(name_node);
        }
        if let Some(pattern_node) = node.child_by_field_name("pattern") {
            let text = state.node_text(pattern_node);
            // For simple patterns like `x`, return the text directly.
            // For tuple patterns like `(a, b)`, return the whole thing.
            return text;
        }
        "<anonymous>".to_string()
    }

    /// Extract Scala visibility from `access_modifier` or modifiers children.
    fn extract_visibility(node: TsNode<'_>, state: &ExtractionState) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "access_modifier" => {
                        let text = state.node_text(child);
                        if text.contains("private") {
                            return Visibility::Private;
                        } else if text.contains("protected") {
                            return Visibility::PubCrate;
                        }
                    }
                    "modifiers" => {
                        // Check inside modifiers for access_modifier.
                        let mut inner = child.walk();
                        if inner.goto_first_child() {
                            loop {
                                let inner_child = inner.node();
                                if inner_child.kind() == "access_modifier" {
                                    let text = state.node_text(inner_child);
                                    if text.contains("private") {
                                        return Visibility::Private;
                                    } else if text.contains("protected") {
                                        return Visibility::PubCrate;
                                    }
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
        // Scala default visibility is public.
        Visibility::Pub
    }

    /// Check if a node has a specific modifier keyword (e.g. "case", "abstract", "sealed").
    fn has_modifier_keyword(node: TsNode<'_>, state: &ExtractionState, keyword: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    let text = state.node_text(child);
                    if text.split_whitespace().any(|w| w == keyword) {
                        return true;
                    }
                }
                // "case" can also appear as a direct keyword child.
                if child.kind() == keyword {
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
    fn extract_declaration_signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        // Cut at first '{' for brace-delimited bodies.
        if let Some(brace_pos) = text.find('{') {
            return text[..brace_pos].trim().to_string();
        }
        // Cut at first '=' for expression-bodied definitions (but not inside type bounds).
        // Only if there's a body field (function_definition, val_definition).
        if node.child_by_field_name("body").is_some() || node.child_by_field_name("value").is_some()
        {
            if let Some(eq_pos) = text.find('=') {
                return text[..eq_pos].trim().to_string();
            }
        }
        text.lines().next().unwrap_or("").trim().to_string()
    }

    /// Extract Scaladoc comments (/** ... */) preceding a declaration.
    fn extract_scaladoc(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "block_comment" => {
                    let text = state.node_text(sibling);
                    if text.starts_with("/**") {
                        return Some(Self::clean_scaladoc(&text));
                    }
                    current = sibling.prev_named_sibling();
                }
                "comment" => {
                    current = sibling.prev_named_sibling();
                }
                _ => break,
            }
        }
        None
    }

    /// Clean a Scaladoc comment block, stripping markers.
    fn clean_scaladoc(comment: &str) -> String {
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

    /// Extract extends/with clauses and create Extends edges.
    fn extract_extends(state: &mut ExtractionState, node: TsNode<'_>, owner_id: &str) {
        if let Some(extends) = node.child_by_field_name("extend") {
            let mut cursor = extends.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "extends" && child.kind() != "with" {
                        let type_name = state.node_text(child);
                        // Strip generic params for the ref name.
                        let base_name = type_name
                            .split('[')
                            .next()
                            .unwrap_or(&type_name)
                            .trim()
                            .to_string();
                        if !base_name.is_empty() {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: owner_id.to_string(),
                                reference_name: base_name,
                                reference_kind: EdgeKind::Extends,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
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

    /// Extract type parameters and create `GenericParam` nodes.
    fn extract_type_parameters(state: &mut ExtractionState, node: TsNode<'_>, owner_id: &str) {
        let tp_node = node
            .child_by_field_name("type_parameters")
            .or_else(|| Self::find_child_by_kind(node, "type_parameters"));
        if let Some(tp) = tp_node {
            let mut cursor = tp.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind().contains("type_parameter") {
                        let param_name = Self::find_child_by_kind(child, "identifier")
                            .or_else(|| Self::find_child_by_kind(child, "type_identifier"))
                            .map_or_else(|| state.node_text(child), |n| state.node_text(n));
                        let start_line = child.start_position().row as u32;
                        let id = generate_node_id(
                            &state.file_path,
                            &NodeKind::GenericParam,
                            &param_name,
                            start_line,
                        );
                        state.nodes.push(Node {
                            id: id.clone(),
                            kind: NodeKind::GenericParam,
                            name: param_name,
                            qualified_name: format!("{}::<generic>", state.qualified_prefix()),
                            file_path: state.file_path.clone(),
                            start_line,
                            attrs_start_line: start_line,
                            end_line: child.end_position().row as u32,
                            start_column: child.start_position().column as u32,
                            end_column: child.end_position().column as u32,
                            signature: Some(state.node_text(child)),
                            docstring: None,
                            visibility: Visibility::Private,
                            is_async: false,
                            branches: 0,
                            loops: 0,
                            returns: 0,
                            max_nesting: 0,
                            unsafe_blocks: 0,
                            unchecked_calls: 0,
                            assertions: 0,
                            updated_at: state.timestamp,
                        });
                        state.edges.push(Edge {
                            source: owner_id.to_string(),
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
    }

    /// Extract class parameters (constructor params) as field nodes.
    ///
    /// In Scala, `class Foo(val x: Int, var y: String)` creates fields.
    /// Parameters with `val` or `var` are public fields; others are private.
    fn extract_class_params_as_fields(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        owner_id: &str,
    ) {
        let cp_node = node.child_by_field_name("class_parameters");
        if let Some(cp) = cp_node {
            let mut cursor = cp.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "class_parameter" {
                        let param_name = child
                            .child_by_field_name("name")
                            .map_or_else(|| "<param>".to_string(), |n| state.node_text(n));

                        let text = state.node_text(child);
                        let is_val = text.contains("val ");
                        let is_var = text.contains("var ");
                        let kind = if is_var {
                            NodeKind::VarField
                        } else {
                            NodeKind::ValField
                        };
                        let visibility = if is_val || is_var {
                            Visibility::Pub
                        } else {
                            Visibility::Private
                        };

                        let start_line = child.start_position().row as u32;
                        let id = generate_node_id(&state.file_path, &kind, &param_name, start_line);

                        state.nodes.push(Node {
                            id: id.clone(),
                            kind,
                            name: param_name,
                            qualified_name: format!("{}::<param>", state.qualified_prefix()),
                            file_path: state.file_path.clone(),
                            start_line,
                            attrs_start_line: start_line,
                            end_line: child.end_position().row as u32,
                            start_column: child.start_position().column as u32,
                            end_column: child.end_position().column as u32,
                            signature: Some(text.trim().to_string()),
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
                        });
                        state.edges.push(Edge {
                            source: owner_id.to_string(),
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
    }

    /// Recursively find `call_expression` nodes and create unresolved Calls references.
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
                    "instance_expression" => {
                        let type_name = Self::extract_instance_type(state, child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: format!("new {type_name}"),
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested definitions.
                    "function_definition"
                    | "function_declaration"
                    | "class_definition"
                    | "object_definition"
                    | "trait_definition" => {}
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

    /// Extract the callee name from a `call_expression`.
    fn extract_call_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // call_expression's first named child is usually the function reference.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let child = cursor.node();
            if child.kind() == "field_expression" {
                // e.g. obj.method(...)
                return state.node_text(child);
            }
            if child.kind() == "identifier" {
                return state.node_text(child);
            }
            // generic_function wraps the callee
            if child.kind() == "generic_function" {
                if let Some(inner) = child.child(0) {
                    return state.node_text(inner);
                }
            }
            return state.node_text(child);
        }
        let text = state.node_text(node);
        text.split('(').next().unwrap_or(&text).trim().to_string()
    }

    /// Extract the type name from an `instance_expression` (new Foo(...)).
    fn extract_instance_type(state: &ExtractionState, node: TsNode<'_>) -> String {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "type_identifier"
                        || child.kind() == "generic_type"
                        || child.kind() == "stable_type_identifier")
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

    // -----------------------------------------------------------------------
    // Annotations
    // -----------------------------------------------------------------------

    /// Extract annotations from a declaration node and create `AnnotationUsage`
    /// nodes and Annotates edges.
    ///
    /// Scala annotations appear as direct `"annotation"` children of the
    /// declaration node (`class_definition`, `function_definition`, etc.).
    fn extract_annotations(state: &mut ExtractionState, node: TsNode<'_>, target_id: &str) {
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
                        attrs_start_line: start_line,
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
                        unsafe_blocks: 0,
                        unchecked_calls: 0,
                        assertions: 0,
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
    ///
    /// Looks for a `type_identifier` child first, then falls back to
    /// stripping `@` prefix and `(` suffix from the text.
    fn extract_annotation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(ti) = Self::find_child_by_kind(node, "type_identifier") {
            return state.node_text(ti);
        }
        // Fallback: text after '@', before '('
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

impl crate::extraction::LanguageExtractor for ScalaExtractor {
    fn extensions(&self) -> &[&str] {
        &["scala", "sc"]
    }

    fn language_name(&self) -> &'static str {
        "Scala"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        ScalaExtractor::extract_scala(file_path, source)
    }
}
