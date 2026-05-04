/// Tree-sitter based Zig source code extractor.
///
/// Parses Zig source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, ZIG_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Zig source files using tree-sitter.
pub struct ZigExtractor;

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
    /// Depth of struct/enum/union nesting. > 0 means we are inside a type body.
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

impl ZigExtractor {
    /// Extract code graph nodes and edges from a Zig source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Zig source code to parse.
    pub fn extract_zig(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("zig");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Zig grammar: {e}"))?;
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
            "variable_declaration" => Self::visit_variable_declaration(state, node),
            "function_declaration" => Self::visit_function(state, node),
            "test_declaration" => Self::visit_test(state, node),
            _ => {}
        }
    }

    // ----------------------------------
    // variable_declaration
    // ----------------------------------

    /// Visit a `variable_declaration` node.
    ///
    /// In Zig, `const X = struct { ... }`, `const X = enum { ... }`, `const X = @import("...")`,
    /// and plain `const X: type = value` are all `variable_declaration` nodes.
    /// We dispatch based on the value child.
    fn visit_variable_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Get the name from the first identifier child.
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        // Check what the value is: struct, enum, union, @import, or plain const.
        // The value is typically the last named child that is not the type annotation.
        let value_child = Self::find_value_child(node);

        if let Some(val) = value_child {
            match val.kind() {
                "struct_declaration" => {
                    Self::visit_struct(state, node, val, &name);
                    return;
                }
                "enum_declaration" => {
                    Self::visit_enum(state, node, val, &name);
                    return;
                }
                "builtin_function" if Self::is_import_call(state, val) => {
                    Self::visit_import(state, node, val, &name);
                    return;
                }
                "field_expression" => {
                    // Handle `const mem = @import("std").mem` where the object
                    // of the field_expression is a builtin_function (@import).
                    if let Some(obj) = val.child_by_field_name("object") {
                        if obj.kind() == "builtin_function" && Self::is_import_call(state, obj) {
                            Self::visit_import(state, node, obj, &name);
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        // Plain const (not struct/enum/import).
        Self::visit_const(state, node, &name);
    }

    /// Find the "value" child of a `variable_declaration`.
    ///
    /// In tree-sitter-zig, the value part is the last significant named child
    /// (`struct_declaration`, `enum_declaration`, `builtin_function`, integer, identifier, etc.).
    fn find_value_child<'a>(node: TsNode<'a>) -> Option<TsNode<'a>> {
        let mut cursor = node.walk();
        let mut last_named: Option<TsNode<'a>> = None;
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    let kind = child.kind();
                    // Skip the name identifier and type annotations.
                    if kind != "identifier" && kind != "builtin_type" {
                        last_named = Some(child);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        last_named
    }

    /// Check if a `builtin_function` node is @import.
    fn is_import_call(state: &ExtractionState, node: TsNode<'_>) -> bool {
        Self::find_child_by_kind(node, "builtin_identifier")
            .is_some_and(|n| state.node_text(n) == "@import")
    }

    // ----------------------------------
    // Import (@import)
    // ----------------------------------

    /// Extract an import declaration: `const X = @import("module")`.
    fn visit_import(
        state: &mut ExtractionState,
        decl_node: TsNode<'_>,
        builtin_node: TsNode<'_>,
        name: &str,
    ) {
        // Extract the module path from the string argument.
        let module_name =
            Self::extract_import_module(state, builtin_node).unwrap_or_else(|| name.to_string());

        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), module_name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &module_name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name: module_name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(decl_node).trim().to_string()),
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract the module name from an @import `builtin_function` node.
    ///
    /// Looks for the string child inside the arguments: `@import("std")` -> `"std"`.
    fn extract_import_module(state: &ExtractionState, builtin_node: TsNode<'_>) -> Option<String> {
        // arguments -> string -> string_content
        let args = Self::find_child_by_kind(builtin_node, "arguments")?;
        let string_node = Self::find_child_by_kind(args, "string")?;
        let content = Self::find_child_by_kind(string_node, "string_content")?;
        let text = state.node_text(content);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    // ----------------------------------
    // Struct
    // ----------------------------------

    /// Extract a struct definition: `const Point = struct { ... }`.
    fn visit_struct(
        state: &mut ExtractionState,
        decl_node: TsNode<'_>,
        struct_node: TsNode<'_>,
        name: &str,
    ) {
        let docstring = Self::extract_docstring(state, decl_node);
        let signature = Self::extract_first_line_signature(state, decl_node);
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

        // Visit struct body for fields, methods, etc.
        state.node_stack.push((name.to_string(), id));
        state.class_depth += 1;
        Self::visit_struct_body(state, struct_node);
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit children of a `struct_declaration` to extract fields and methods.
    fn visit_struct_body(state: &mut ExtractionState, struct_node: TsNode<'_>) {
        let mut cursor = struct_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "container_field" => Self::visit_field(state, child),
                    "function_declaration" => Self::visit_function(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // ----------------------------------
    // Enum
    // ----------------------------------

    /// Extract an enum definition: `const LogLevel = enum { ... }`.
    fn visit_enum(
        state: &mut ExtractionState,
        decl_node: TsNode<'_>,
        enum_node: TsNode<'_>,
        name: &str,
    ) {
        let docstring = Self::extract_docstring(state, decl_node);
        let signature = Self::extract_first_line_signature(state, decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Enum, name, start_line);

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

        // Visit enum body for variants.
        state.node_stack.push((name.to_string(), id));
        Self::visit_enum_body(state, enum_node);
        state.node_stack.pop();
    }

    /// Visit children of an `enum_declaration` to extract variants.
    fn visit_enum_body(state: &mut ExtractionState, enum_node: TsNode<'_>) {
        let mut cursor = enum_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "container_field" {
                    Self::visit_enum_variant(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an enum variant from a `container_field` inside an enum.
    fn visit_enum_variant(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

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
        state.nodes.push(graph_node);

        // Contains edge from parent (enum).
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
    // Const (plain)
    // ----------------------------------

    /// Extract a plain constant: `const max_connections: u32 = 100`.
    fn visit_const(state: &mut ExtractionState, node: TsNode<'_>, name: &str) {
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Const,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring,
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // ----------------------------------
    // Field
    // ----------------------------------

    /// Extract a field from a `container_field` inside a struct.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
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
            signature: Some(state.node_text(node).trim().to_string()),
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

        // Contains edge from parent (struct).
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
    /// If `class_depth > 0`, the function is a Method; otherwise it is a Function.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let is_pub = Self::has_pub_keyword(state, node);
        let visibility = if is_pub {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let in_type = state.class_depth > 0;
        let kind = if in_type {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &ZIG_COMPLEXITY, &state.source);

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
    // Test declaration
    // ----------------------------------

    /// Extract a test declaration: `test "name" { ... }`.
    fn visit_test(state: &mut ExtractionState, node: TsNode<'_>) {
        // The test name is in a string child.
        let name = Self::find_child_by_kind(node, "string")
            .and_then(|s| Self::find_child_by_kind(s, "string_content"))
            .map_or_else(|| "<anonymous test>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::test::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
        let metrics = count_complexity(node, &ZIG_COMPLEXITY, &state.source);

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
            signature: Some(
                state
                    .node_text(node)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            ),
            docstring: None,
            visibility: Visibility::Private,
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

        // Extract call sites from the test body.
        Self::extract_call_sites(state, node, &id);
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Check if a `function_declaration` node has the `pub` keyword.
    ///
    /// In tree-sitter-zig, `pub fn` has an anonymous "pub" child before "fn".
    fn has_pub_keyword(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if !child.is_named() && state.node_text(child) == "pub" {
                    return true;
                }
                // Stop once we reach the "fn" keyword or a named child.
                if !child.is_named() && state.node_text(child) == "fn" {
                    break;
                }
                if child.is_named() {
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Extract the function signature (first line of the function declaration).
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim().to_string();
        // Strip the opening brace if it's on the same line.
        let sig = first_line.trim_end_matches('{').trim().to_string();
        if sig.is_empty() {
            None
        } else {
            Some(sig)
        }
    }

    /// Extract the first line as signature for type declarations.
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
    /// Zig uses `///` doc comment lines. We look for consecutive `comment` sibling
    /// nodes that immediately precede the given definition node and start with `///`.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                if text.starts_with("///") {
                    let stripped = text.trim_start_matches("///").trim().to_string();
                    comments.push(stripped);
                    prev = prev_node.prev_named_sibling();
                } else {
                    break;
                }
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

    /// Recursively find call expression nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        // Extract the callee name from the function field.
                        let callee_name =
                            if let Some(fn_node) = child.child_by_field_name("function") {
                                Some(Self::extract_callee_name(state, fn_node))
                            } else {
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
                    // Skip nested function/test definitions to avoid polluting call sites.
                    "function_declaration" | "test_declaration" => {}
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

    /// Extract the callee name from a call expression's function child.
    ///
    /// For simple calls like `foo()`, returns "foo".
    /// For member calls like `self.host`, returns the rightmost member name.
    /// For chained calls like `std.debug.print`, returns the full chain.
    fn extract_callee_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        match node.kind() {
            "builtin_function" => {
                // @sqrt, @as, etc.
                Self::find_child_by_kind(node, "builtin_identifier")
                    .map_or_else(|| state.node_text(node), |n| state.node_text(n))
            }
            _ => state.node_text(node),
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

impl crate::extraction::LanguageExtractor for ZigExtractor {
    fn extensions(&self) -> &[&str] {
        &["zig"]
    }

    fn language_name(&self) -> &'static str {
        "Zig"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_zig(file_path, source)
    }
}
