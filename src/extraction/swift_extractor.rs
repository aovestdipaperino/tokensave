/// Tree-sitter based Swift source code extractor.
///
/// Parses Swift source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, SWIFT_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Swift source files using tree-sitter.
pub struct SwiftExtractor;

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
    /// Depth of class/struct/enum/protocol/extension nesting. > 0 means inside a type body.
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

impl SwiftExtractor {
    /// Extract code graph nodes and edges from a Swift source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Swift source code to parse.
    pub fn extract_swift(file_path: &str, source: &str) -> ExtractionResult {
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
        state
            .node_stack
            .push((file_path.to_string(), file_node_id));

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_swift::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Swift grammar: {e}"))?;
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
            "import_declaration" => Self::visit_import(state, node),
            "class_declaration" => Self::visit_class_declaration(state, node),
            "protocol_declaration" => Self::visit_protocol(state, node),
            "function_declaration" => Self::visit_function(state, node),
            "init_declaration" => Self::visit_init(state, node),
            "property_declaration" => Self::visit_property(state, node),
            "typealias_declaration" => Self::visit_typealias(state, node),
            _ => {}
        }
    }

    // ----------------------------------
    // Import
    // ----------------------------------

    /// Extract an import declaration (e.g. `import Foundation`).
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| {
                // Fallback: get everything after "import "
                let text = state.node_text(node);
                text.strip_prefix("import ")
                    .unwrap_or(&text)
                    .trim()
                    .to_string()
            });

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

    // ----------------------------------
    // class_declaration (class, struct, enum, extension)
    // ----------------------------------

    /// Dispatch class_declaration based on the `declaration_kind` field.
    ///
    /// tree-sitter-swift uses `class_declaration` for class, struct, enum, and extension.
    /// The first anonymous child carries the keyword: "class", "struct", "enum", or "extension".
    fn visit_class_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let decl_kind = Self::declaration_kind_keyword(state, node);
        match decl_kind.as_str() {
            "class" => Self::visit_class(state, node),
            "struct" => Self::visit_struct(state, node),
            "enum" => Self::visit_enum(state, node),
            "extension" => Self::visit_extension(state, node),
            _ => Self::visit_class(state, node), // fallback
        }
    }

    /// Returns the declaration keyword ("class", "struct", "enum", "extension") for a
    /// class_declaration node by reading the first child with field "declaration_kind".
    fn declaration_kind_keyword(state: &ExtractionState, node: TsNode<'_>) -> String {
        // The keyword is stored as the first anonymous child with field name "declaration_kind".
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.field_name() == Some("declaration_kind") {
                    return state.node_text(cursor.node());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        // Fallback: check first child text.
        if let Some(first) = node.child(0) {
            return state.node_text(first);
        }
        "class".to_string()
    }

    /// Extract a class definition.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_type_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_first_line_signature(state, node);
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

        // Extract inheritance (class Foo: Bar).
        Self::extract_inheritance(state, node, &id);

        // Visit class body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a struct definition.
    fn visit_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_type_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_first_line_signature(state, node);
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

        // Visit struct body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an enum definition.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_type_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_first_line_signature(state, node);
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

        // Visit enum body (enum_class_body with enum_entry children).
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_enum_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit enum body children, extracting enum entries as EnumVariant nodes.
    fn visit_enum_body(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "enum_entry" => Self::visit_enum_entry(state, child),
                    "function_declaration" => Self::visit_function(state, child),
                    "init_declaration" => Self::visit_init(state, child),
                    "property_declaration" => Self::visit_property(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an enum entry as an EnumVariant node.
    fn visit_enum_entry(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| {
                Self::find_child_by_kind(node, "simple_identifier").map(|n| state.node_text(n))
            })
            .unwrap_or_else(|| {
                // Fallback: parse text after "case "
                let text = state.node_text(node);
                text.strip_prefix("case ")
                    .unwrap_or(&text)
                    .trim()
                    .to_string()
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

    /// Extract an extension declaration.
    fn visit_extension(state: &mut ExtractionState, node: TsNode<'_>) {
        // For extension, the name is in a user_type or type_identifier child with field "name".
        let name = node
            .child_by_field_name("name")
            .map(|n| {
                // Could be a user_type wrapping a type_identifier.
                Self::find_child_by_kind(n, "type_identifier")
                    .map(|ti| state.node_text(ti))
                    .unwrap_or_else(|| state.node_text(n))
            })
            .unwrap_or_else(|| "<anonymous>".to_string());

        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_first_line_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Extension, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Extension,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Visit extension body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    // ----------------------------------
    // Protocol
    // ----------------------------------

    /// Extract a protocol declaration (maps to Interface).
    fn visit_protocol(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_first_line_signature(state, node);
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

        // Visit protocol body. Protocol functions are protocol_function_declaration.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_protocol_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit protocol body children, extracting protocol function declarations as Method nodes.
    fn visit_protocol_body(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "protocol_function_declaration" => {
                        Self::visit_protocol_function(state, child);
                    }
                    "property_declaration" => Self::visit_property(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a protocol function declaration as a Method node.
    fn visit_protocol_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| {
                Self::find_child_by_kind(node, "simple_identifier").map(|n| state.node_text(n))
            })
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let signature = Self::extract_first_line_signature(state, node);

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

    // ----------------------------------
    // Function / Method
    // ----------------------------------

    /// Extract a function or method declaration.
    ///
    /// If class_depth > 0, it becomes a Method; otherwise a Function.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| {
                Self::find_child_by_kind(node, "simple_identifier").map(|n| state.node_text(n))
            })
            .unwrap_or_else(|| "<anonymous>".to_string());

        let in_class = state.class_depth > 0;
        let kind = if in_class {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let visibility = Self::extract_visibility(state, node);
        let is_async = Self::has_async_keyword(node, &state.source);
        let signature = Self::extract_first_line_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &SWIFT_COMPLEXITY, &state.source);

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

        // Extract call sites from the function body.
        Self::extract_call_sites(state, node, &id);
    }

    // ----------------------------------
    // Init (Constructor)
    // ----------------------------------

    /// Extract an init declaration as a Constructor node.
    fn visit_init(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = "init".to_string();
        let visibility = Self::extract_visibility(state, node);
        let signature = Self::extract_first_line_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(node, &SWIFT_COMPLEXITY, &state.source);

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

        // Extract call sites from the init body.
        Self::extract_call_sites(state, node, &id);
    }

    // ----------------------------------
    // Property
    // ----------------------------------

    /// Extract a property declaration (let/var).
    ///
    /// Inside a class/struct/protocol body, becomes Property. At top level, becomes Const.
    fn visit_property(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_property_name(state, node);
        let in_class = state.class_depth > 0;
        let kind = if in_class {
            NodeKind::Property
        } else {
            NodeKind::Const
        };
        let visibility = Self::extract_visibility(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let text = state.node_text(node);

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
            signature: Some(text.lines().next().unwrap_or("").trim().to_string()),
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

    // ----------------------------------
    // Typealias
    // ----------------------------------

    /// Extract a typealias declaration.
    fn visit_typealias(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| {
                Self::find_child_by_kind(node, "type_identifier").map(|n| state.node_text(n))
            })
            .unwrap_or_else(|| "<anonymous>".to_string());

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

    /// Extract the type name from a class_declaration node.
    ///
    /// The name is in a child with field "name", which is a `type_identifier` for
    /// class/struct/enum, or a `user_type` wrapping `type_identifier` for extensions.
    fn extract_type_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        node.child_by_field_name("name")
            .map(|n| {
                // For class/struct/enum, this is a type_identifier directly.
                // For extension, it may be a user_type wrapping type_identifier.
                if n.kind() == "user_type" {
                    Self::find_child_by_kind(n, "type_identifier")
                        .map(|ti| state.node_text(ti))
                        .unwrap_or_else(|| state.node_text(n))
                } else {
                    state.node_text(n)
                }
            })
            .unwrap_or_else(|| "<anonymous>".to_string())
    }

    /// Extract inheritance specifiers (e.g. `class Foo: Bar`).
    ///
    /// Creates Extends UnresolvedRef for each inherited type.
    fn extract_inheritance(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "inheritance_specifier" {
                    // The inheritance_specifier has a child with field "inherits_from"
                    // which is a user_type containing the parent type name.
                    if let Some(inherits_from) = child.child_by_field_name("inherits_from") {
                        let base_name = Self::find_child_by_kind(inherits_from, "type_identifier")
                            .map(|ti| state.node_text(ti))
                            .unwrap_or_else(|| state.node_text(inherits_from));
                        let line = inherits_from.start_position().row as u32;
                        let column = inherits_from.start_position().column as u32;
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

    /// Extract the property name from a property_declaration.
    ///
    /// The name is in the `pattern` child (field "name") -> `simple_identifier` (field "bound_identifier").
    fn extract_property_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(pattern) = node.child_by_field_name("name") {
            if let Some(ident) = pattern.child_by_field_name("bound_identifier") {
                return state.node_text(ident);
            }
            // Fallback: find simple_identifier in pattern.
            if let Some(ident) = Self::find_child_by_kind(pattern, "simple_identifier") {
                return state.node_text(ident);
            }
            return state.node_text(pattern);
        }
        // Fallback: find pattern child then simple_identifier.
        if let Some(pattern) = Self::find_child_by_kind(node, "pattern") {
            if let Some(ident) = Self::find_child_by_kind(pattern, "simple_identifier") {
                return state.node_text(ident);
            }
        }
        "<anonymous>".to_string()
    }

    /// Extract the visibility modifier from a node's modifiers children.
    ///
    /// Looks for a `visibility_modifier` inside `modifiers` to determine visibility.
    fn extract_visibility(state: &ExtractionState, node: TsNode<'_>) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    if let Some(vis_mod) = Self::find_child_by_kind(child, "visibility_modifier") {
                        let text = state.node_text(vis_mod);
                        return match text.as_str() {
                            "private" => Visibility::Private,
                            "fileprivate" => Visibility::Private,
                            "internal" => Visibility::PubCrate,
                            "public" => Visibility::Pub,
                            "open" => Visibility::Pub,
                            _ => Visibility::Pub,
                        };
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        // Default: Swift internal (we map to Pub for simplicity, like other extractors).
        Visibility::Pub
    }

    /// Check if a function declaration has the `async` keyword.
    ///
    /// In tree-sitter-swift, `async` is an anonymous child node with kind "async".
    fn has_async_keyword(node: TsNode<'_>, _source: &[u8]) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "async" {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Extract the first line of a node's text as its signature.
    fn extract_first_line_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract docstrings from `/// comment` lines preceding definitions.
    ///
    /// Swift uses `///` doc comments. We look for `comment` sibling nodes that
    /// immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = text
                    .trim_start_matches("///")
                    .trim_start_matches("//")
                    .trim()
                    .to_string();
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

    /// Recursively find call nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        // Extract the callee name. In Swift tree-sitter, a call_expression
                        // has the function being called as its first child.
                        let callee_name = Self::extract_callee_name(state, child);
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
                    // Skip nested definitions to avoid polluting call sites.
                    "function_declaration" | "init_declaration" | "class_declaration"
                    | "protocol_declaration" => {}
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
    ///
    /// In Swift, call_expression children are: callee (simple_identifier or navigation_expression)
    /// followed by call_suffix with value_arguments.
    fn extract_callee_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let first_child = node.named_child(0)?;
        match first_child.kind() {
            "simple_identifier" => Some(state.node_text(first_child)),
            "navigation_expression" => {
                // For chained calls like `super.init(...)` or `foo.bar(...)`,
                // get the suffix (last navigation_suffix child's simple_identifier).
                let mut last_name = None;
                let mut cursor = first_child.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "navigation_suffix" {
                            if let Some(ident) =
                                Self::find_child_by_kind(child, "simple_identifier")
                            {
                                last_name = Some(state.node_text(ident));
                            }
                        } else if child.kind() == "simple_identifier" && last_name.is_none() {
                            last_name = Some(state.node_text(child));
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
                last_name
            }
            _ => Some(state.node_text(first_child)),
        }
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

impl crate::extraction::LanguageExtractor for SwiftExtractor {
    fn extensions(&self) -> &[&str] {
        &["swift"]
    }

    fn language_name(&self) -> &str {
        "Swift"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_swift(file_path, source)
    }
}
