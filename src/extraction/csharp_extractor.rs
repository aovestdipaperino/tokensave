/// Tree-sitter based C# source code extractor.
///
/// Parses C# source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, CSHARP_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from C# source files using tree-sitter.
pub struct CSharpExtractor;

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

impl CSharpExtractor {
    /// Extract code graph nodes and edges from a C# source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the C# source code to parse.
    pub fn extract_csharp(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("c_sharp");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load C# grammar: {e}"))?;
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
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                Self::visit_namespace(state, node);
            }
            "using_directive" => Self::visit_using(state, node),
            "class_declaration" => Self::visit_class(state, node),
            "struct_declaration" => Self::visit_struct(state, node),
            "interface_declaration" => Self::visit_interface(state, node),
            "enum_declaration" => Self::visit_enum(state, node),
            "method_declaration" => Self::visit_method(state, node),
            "constructor_declaration" => Self::visit_constructor(state, node),
            "property_declaration" => Self::visit_property(state, node),
            "field_declaration" => Self::visit_field(state, node),
            "record_declaration" | "record_struct_declaration" => Self::visit_record(state, node),
            "delegate_declaration" => Self::visit_delegate(state, node),
            "event_declaration" | "event_field_declaration" => Self::visit_event(state, node),
            "attribute_list" => Self::visit_attribute_list(state, node),
            _ => {
                // Recurse into children for any unhandled node types.
                Self::visit_children(state, node);
            }
        }
    }

    /// Extract a namespace declaration.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| {
            // For file-scoped namespaces or qualified names
            Self::extract_qualified_name_child(state, node)
                .unwrap_or_else(|| "<anonymous>".to_string())
        });
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Namespace, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Namespace,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("namespace {name}")),
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
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Visit namespace body.
        state.node_stack.push((name, id));
        // For braced namespaces, visit the body (declaration_list)
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        } else {
            // For file-scoped namespace, visit remaining children
            Self::visit_children(state, node);
        }
        state.node_stack.pop();
    }

    /// Extract a using directive as a Use node.
    fn visit_using(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Strip "using " prefix and trailing ";"
        let path = text
            .trim()
            .strip_prefix("using ")
            .unwrap_or(&text)
            .trim()
            .strip_prefix("static ")
            .unwrap_or(text.trim().strip_prefix("using ").unwrap_or(&text).trim())
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
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
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

        // Extract attributes on this class.
        Self::extract_attributes_from_declaration(state, node, &id);

        // Extract base list (extends/implements).
        Self::extract_base_list(state, node, &id, true);

        // Visit class body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a struct declaration.
    fn visit_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Struct,
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

        // Extract base list (struct can implement interfaces).
        Self::extract_base_list(state, node, &id, false);

        // Visit struct body.
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
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
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

        // Extract base list (interfaces can extend other interfaces).
        Self::extract_base_list(state, node, &id, false);

        // Visit interface body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an enum declaration with its members.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract enum members from the body.
        state.node_stack.push((name, id));
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_enum_members(state, body);
        }
        state.node_stack.pop();
    }

    /// Extract enum members from an `enum_member_declaration_list`.
    fn extract_enum_members(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enum_member_declaration" {
                    Self::extract_single_enum_member(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single enum member as an `EnumVariant` node.
    fn extract_single_enum_member(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| {
            // Fallback: try to get the identifier child directly
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "identifier" {
                        return state.node_text(child);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            "<anonymous>".to_string()
        });
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

    /// Extract a method declaration.
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let is_async = Self::has_modifier(node, state, "async");

        // If inside a class/struct, it's a Method; otherwise Function
        let kind = if state.class_depth > 0 {
            NodeKind::Method
        } else {
            NodeKind::Function
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &CSHARP_COMPLEXITY, &state.source);

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
            is_async,
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

        // Extract attributes on this method.
        Self::extract_attributes_from_declaration(state, node, &id);

        // Extract call sites from the method body.
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a constructor declaration.
    fn visit_constructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(node, &CSHARP_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Constructor,
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

        // Extract attributes on this constructor.
        Self::extract_attributes_from_declaration(state, node, &id);

        // Extract call sites from the constructor body.
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a property declaration.
    fn visit_property(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::CSharpProperty,
            &name,
            start_line,
        );

        // Extract the type from the type field
        let type_str = node
            .child_by_field_name("type")
            .map(|n| state.node_text(n))
            .unwrap_or_default();
        let sig = format!("{type_str} {name}");

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::CSharpProperty,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(sig),
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

    /// Extract field declarations.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let visibility = Self::extract_csharp_visibility(node, state);

        // In C# tree-sitter, field_declaration contains a variable_declaration
        // which has variable_declarator children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declaration" {
                    Self::extract_variable_declarators(state, child, &visibility, node);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract variable declarators from a `variable_declaration` node.
    fn extract_variable_declarators(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        visibility: &Visibility,
        field_decl: TsNode<'_>,
    ) {
        let start_line = field_decl.start_position().row as u32;
        let end_line = field_decl.end_position().row as u32;
        let start_column = field_decl.start_position().column as u32;
        let end_column = field_decl.end_position().column as u32;
        let signature_text = state.node_text(field_decl).trim().to_string();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declarator" {
                    let field_name = child
                        .child_by_field_name("name")
                        .or_else(|| {
                            // Try direct identifier child
                            let mut inner = child.walk();
                            if inner.goto_first_child() {
                                loop {
                                    let ic = inner.node();
                                    if ic.kind() == "identifier" {
                                        return Some(ic);
                                    }
                                    if !inner.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                            None
                        })
                        .map_or_else(|| state.node_text(child), |n| state.node_text(n));

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
                        attrs_start_line: start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(signature_text.clone()),
                        docstring: None,
                        visibility: visibility.clone(),
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
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a record declaration.
    fn visit_record(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Record, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Record,
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

        // Visit record body if present.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a delegate declaration.
    fn visit_delegate(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Delegate, &name, start_line);
        let signature_text = state
            .node_text(node)
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_string();

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Delegate,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(signature_text),
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

    /// Extract an event declaration.
    fn visit_event(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let name = Self::extract_event_name(state, node).unwrap_or_else(|| {
            // Fallback: parse from text
            text.split_whitespace()
                .last()
                .unwrap_or("<anonymous>")
                .trim_end_matches(';')
                .to_string()
        });

        let visibility = Self::extract_csharp_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Event, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Event,
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

    /// Extract attribute lists as `AnnotationUsage` nodes with Annotates edges.
    fn visit_attribute_list(state: &mut ExtractionState, node: TsNode<'_>) {
        // Find the next declaration sibling - that's the declaration this attribute annotates.
        let target_id = Self::find_next_declaration_id(state, node);

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute" {
                    let attr_name = Self::extract_attribute_name(state, child);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::@{}", state.qualified_prefix(), attr_name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::AnnotationUsage,
                        &attr_name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::AnnotationUsage,
                        name: attr_name.clone(),
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        attrs_start_line: start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(format!("[{}]", state.node_text(child).trim())),
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
                        reference_name: attr_name,
                        reference_kind: EdgeKind::Annotates,
                        line: start_line,
                        column: start_column,
                        file_path: state.file_path.clone(),
                    });

                    // If we found the target, create a direct Annotates edge.
                    if let Some(ref tid) = target_id {
                        state.edges.push(Edge {
                            source: id,
                            target: tid.clone(),
                            kind: EdgeKind::Annotates,
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

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract the name of a node by looking for a "name" field child.
    fn extract_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        node.child_by_field_name("name").map(|n| state.node_text(n))
    }

    /// Try to extract a `qualified_name` child for namespace declarations.
    fn extract_qualified_name_child(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "qualified_name" || child.kind() == "identifier" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Extract the event name from an event declaration.
    fn extract_event_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Try the "name" field first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(state.node_text(name_node));
        }
        // For event_field_declaration, look for variable_declaration > variable_declarator
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declaration" {
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let ic = inner.node();
                            if ic.kind() == "variable_declarator" {
                                if let Some(name_node) = ic.child_by_field_name("name") {
                                    return Some(state.node_text(name_node));
                                }
                                // Try identifier child
                                let mut deep = ic.walk();
                                if deep.goto_first_child() {
                                    loop {
                                        let dc = deep.node();
                                        if dc.kind() == "identifier" {
                                            return Some(state.node_text(dc));
                                        }
                                        if !deep.goto_next_sibling() {
                                            break;
                                        }
                                    }
                                }
                                return Some(state.node_text(ic));
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
        None
    }

    /// Extract C# visibility from modifier keywords.
    fn extract_csharp_visibility(node: TsNode<'_>, state: &ExtractionState) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifier" {
                    let text = state.node_text(child);
                    match text.as_str() {
                        "public" => return Visibility::Pub,
                        "private" => return Visibility::Private,
                        "internal" => return Visibility::PubCrate,
                        "protected" => return Visibility::PubSuper,
                        _ => {}
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        // No modifier -> Private for class members
        Visibility::Private
    }

    /// Check if a node has a specific modifier keyword.
    fn has_modifier(node: TsNode<'_>, state: &ExtractionState, modifier: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifier" {
                    let text = state.node_text(child);
                    if text == modifier {
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

    /// Extract the declaration signature (text from start up to the opening `{`).
    fn extract_declaration_signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            text[..brace_pos].trim().to_string()
        } else {
            text.trim_end_matches(';').trim().to_string()
        }
    }

    /// Extract XML doc comments (/// ...) preceding a declaration.
    fn extract_xml_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_sibling();
        while let Some(sibling) = current {
            let text = state.node_text(sibling);
            let trimmed = text.trim();
            if trimmed.starts_with("///") {
                comments.push(trimmed.to_string());
                current = sibling.prev_sibling();
            } else if sibling.kind() == "attribute_list" {
                // Skip attribute lists between comments and the declaration
                current = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if comments.is_empty() {
            return None;
        }

        // Comments are collected in reverse order (bottom-up), so reverse them.
        comments.reverse();

        // Clean the comments: strip ///, strip XML tags for clean text.
        let cleaned: Vec<String> = comments
            .iter()
            .map(|line| {
                let stripped = line.strip_prefix("///").unwrap_or(line).trim();
                // Strip XML tags like <summary>, </summary>, <param>, etc.
                Self::strip_xml_tags(stripped)
            })
            .filter(|s| !s.is_empty())
            .collect();

        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned.join("\n").trim().to_string())
        }
    }

    /// Strip XML tags from a string.
    fn strip_xml_tags(s: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        for c in s.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                result.push(c);
            }
        }
        result.trim().to_string()
    }

    /// Extract base list (extends/implements) from a type declaration.
    /// For classes, the first base type is Extends, the rest are Implements.
    /// For structs/interfaces, all are Implements.
    fn extract_base_list(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        type_id: &str,
        is_class: bool,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "base_list" {
                    Self::extract_base_types(state, child, type_id, is_class);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract types from a `base_list` node.
    fn extract_base_types(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        type_id: &str,
        is_class: bool,
    ) {
        let mut is_first = true;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "identifier"
                        || child.kind() == "generic_name"
                        || child.kind() == "qualified_name")
                {
                    let type_name = state.node_text(child);
                    let edge_kind = if is_class && is_first {
                        is_first = false;
                        EdgeKind::Extends
                    } else {
                        EdgeKind::Implements
                    };

                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: type_id.to_string(),
                        reference_name: type_name,
                        reference_kind: edge_kind,
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

    /// Extract attributes from a declaration node's `attribute_list` children.
    /// Creates `AnnotationUsage` nodes and Annotates edges pointing to the target declaration.
    fn extract_attributes_from_declaration(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute_list" {
                    Self::visit_attribute_list_for_target(state, child, target_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit an `attribute_list` node and create `AnnotationUsage` nodes targeting a known declaration.
    fn visit_attribute_list_for_target(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute" {
                    let attr_name = Self::extract_attribute_name(state, child);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::@{}", state.qualified_prefix(), attr_name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::AnnotationUsage,
                        &attr_name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::AnnotationUsage,
                        name: attr_name,
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        attrs_start_line: start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(state.node_text(child).trim().to_string()),
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

                    // Annotates edge from annotation to target declaration.
                    state.edges.push(Edge {
                        source: id.clone(),
                        target: target_id.to_string(),
                        kind: EdgeKind::Annotates,
                        line: Some(start_line),
                    });

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

    /// Extract the attribute name from an attribute node.
    fn extract_attribute_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(name_node) = node.child_by_field_name("name") {
            return state.node_text(name_node);
        }
        // Fallback: find the first named child that is an identifier
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" || child.kind() == "qualified_name" {
                    return state.node_text(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_text(node).trim().to_string()
    }

    /// Find the next declaration sibling after an `attribute_list` and compute its ID.
    fn find_next_declaration_id(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut current = node.next_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "attribute_list" => {
                    current = sibling.next_named_sibling();
                }
                "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "method_declaration"
                | "constructor_declaration"
                | "property_declaration"
                | "field_declaration"
                | "record_declaration"
                | "record_struct_declaration"
                | "delegate_declaration"
                | "event_declaration"
                | "event_field_declaration" => {
                    let name = Self::extract_name(state, sibling)
                        .unwrap_or_else(|| "<anonymous>".to_string());
                    let kind = match sibling.kind() {
                        "class_declaration" => {
                            if state.class_depth > 0 {
                                NodeKind::InnerClass
                            } else {
                                NodeKind::Class
                            }
                        }
                        "struct_declaration" => NodeKind::Struct,
                        "interface_declaration" => NodeKind::Interface,
                        "enum_declaration" => NodeKind::Enum,
                        "method_declaration" => {
                            if state.class_depth > 0 {
                                NodeKind::Method
                            } else {
                                NodeKind::Function
                            }
                        }
                        "constructor_declaration" => NodeKind::Constructor,
                        "property_declaration" => NodeKind::CSharpProperty,
                        "record_declaration" | "record_struct_declaration" => NodeKind::Record,
                        "delegate_declaration" => NodeKind::Delegate,
                        "event_declaration" | "event_field_declaration" => NodeKind::Event,
                        _ => return None,
                    };
                    let start_line = sibling.start_position().row as u32;
                    return Some(generate_node_id(&state.file_path, &kind, &name, start_line));
                }
                _ => return None,
            }
        }
        None
    }

    /// Recursively find `invocation_expression` nodes and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "invocation_expression" => {
                        let callee_name = Self::extract_invocation_name(state, child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: callee_name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                        // Recurse for nested calls inside arguments.
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
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested declarations.
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

    /// Extract the name from an `invocation_expression` node.
    fn extract_invocation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // invocation_expression: function + argument_list
        if let Some(func_node) = node.child_by_field_name("function") {
            return state.node_text(func_node);
        }
        // Fallback: first child
        if let Some(first) = node.child(0) {
            if first.kind() != "argument_list" {
                return state.node_text(first);
            }
        }
        state.node_text(node)
    }

    /// Extract the type name from an `object_creation_expression`.
    fn extract_object_creation_type(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(type_node) = node.child_by_field_name("type") {
            return state.node_text(type_node);
        }
        // Fallback: look for type identifier children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "identifier"
                        || child.kind() == "generic_name"
                        || child.kind() == "qualified_name")
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

impl crate::extraction::LanguageExtractor for CSharpExtractor {
    fn extensions(&self) -> &[&str] {
        &["cs"]
    }

    fn language_name(&self) -> &'static str {
        "C#"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        CSharpExtractor::extract_csharp(file_path, source)
    }
}
