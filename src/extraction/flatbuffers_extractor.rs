/// Tree-sitter based `FlatBuffers` schema extractor.
///
/// Parses `.fbs` files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::extraction::ts_state::find_child_by_kind;
use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

/// Extracts code graph nodes and edges from `FlatBuffers` schema files using tree-sitter.
pub struct FlatBuffersExtractor;

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
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

impl FlatBuffersExtractor {
    /// Extract code graph nodes and edges from a `FlatBuffers` schema file.
    pub fn extract_flatbuffers(file_path: &str, source: &str) -> ExtractionResult {
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state.node_stack.push((file_path.to_string(), file_node_id));

        // Malformed input still yields a partial result: record the problem
        // and walk whatever parsed instead of bailing out.
        if tree.root_node().has_error() {
            state
                .errors
                .push("parse produced error nodes; results are partial".to_string());
        }

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("flatbuffers");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load FlatBuffers grammar: {e}"))?;
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
            "namespace_decl" => Self::visit_namespace(state, node),
            "include" => Self::visit_include(state, node),
            "type_decl" => Self::visit_type_decl(state, node),
            "enum_decl" => Self::visit_enum(state, node),
            "union_decl" => Self::visit_union(state, node),
            "rpc_decl" => Self::visit_rpc(state, node),
            _ => {}
        }
    }

    /// Extract a `namespace` declaration.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        // namespace_decl -> full_ident
        let name = node
            .child_by_field_name("namespace_ident")
            .or_else(|| find_child_by_kind(node, "full_ident"))
            .map_or_else(|| "<unknown>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);
        let signature = Some(format!("namespace {name}"));

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    /// Extract an `include` statement.
    fn visit_include(state: &mut ExtractionState, node: TsNode<'_>) {
        // include -> string_constant (the quoted path)
        let name = node
            .child_by_field_name("include_name")
            .or_else(|| find_child_by_kind(node, "string_constant"))
            .map_or_else(
                || "<unknown>".to_string(),
                |n| {
                    let text = state.node_text(n);
                    // Strip surrounding quotes
                    text.trim_matches('"').trim_matches('\'').to_string()
                },
            );

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);
        let signature = Some(format!("include \"{name}\""));

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    /// Extract a `table` or `struct` definition.
    fn visit_type_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        // type_decl -> identifier (name), field_decl* (fields)
        let name = node
            .child_by_field_name("table_or_struct_name")
            .or_else(|| find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        // The table/struct keyword is an anonymous token; its kind names it.
        let keyword = node
            .child_by_field_name("table_or_struct_declaration")
            .map_or("table", |n| n.kind());

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, &name, start_line);
        let signature = Some(format!("{keyword} {name}"));

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
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Visit the fields of the table or struct.
        state.node_stack.push((name, id));
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "field_decl" {
                    Self::visit_field(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_stack.pop();
    }

    /// Extract a field declaration within a table or struct.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        // field_decl -> identifier (key), type, optional value (default)
        let name = node
            .child_by_field_name("field_key")
            .or_else(|| find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let type_text = node
            .child_by_field_name("field_type")
            .or_else(|| find_child_by_kind(node, "type"))
            .map_or_else(|| "unknown".to_string(), |n| state.node_text(n));

        let default_value = node
            .child_by_field_name("field_value")
            .or_else(|| find_child_by_kind(node, "value"))
            .map(|n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);
        let signature = Some(match default_value {
            Some(value) => format!("{name}: {type_text} = {value}"),
            None => format!("{name}: {type_text}"),
        });

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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    /// Extract an `enum` definition.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        // enum_decl -> identifier (name), type (base type), enum_val_decl*
        let name = node
            .child_by_field_name("enum_name")
            .or_else(|| find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let base_type = node
            .child_by_field_name("enum_type")
            .or_else(|| find_child_by_kind(node, "type"))
            .map_or_else(|| "unknown".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Enum, &name, start_line);
        let signature = Some(format!("enum {name} : {base_type}"));

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
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Visit enum body for variants.
        state.node_stack.push((name, id));
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enum_val_decl" {
                    Self::visit_enum_variant(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_stack.pop();
    }

    /// Extract an enum variant (`enum_val_decl`).
    fn visit_enum_variant(state: &mut ExtractionState, node: TsNode<'_>) {
        // enum_val_decl -> identifier (key), optional int_constant (value)
        let name = node
            .child_by_field_name("enum_key")
            .or_else(|| find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let value = node
            .child_by_field_name("enum_int_constant")
            .or_else(|| find_child_by_kind(node, "int_constant"))
            .map(|n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::EnumVariant, &name, start_line);
        let signature = Some(match value {
            Some(value) => format!("{name} = {value}"),
            None => name.clone(),
        });

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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    /// Extract a `union` definition.
    fn visit_union(state: &mut ExtractionState, node: TsNode<'_>) {
        // union_decl -> identifier (name), union_field_decl* or bare full_ident*
        let name = node
            .child_by_field_name("union_name")
            .or_else(|| find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Union, &name, start_line);
        let signature = Some(format!("union {name}"));

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Union,
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
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Visit union members: keyed members and bare type references.
        state.node_stack.push((name, id));
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "union_field_decl" => Self::visit_union_field(state, child),
                    // Bare `union U { A, B }` members carry no key, so the
                    // member is named by its type.
                    "full_ident" => Self::visit_bare_union_member(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_stack.pop();
    }

    /// Extract a keyed union member (`key: Type`).
    fn visit_union_field(state: &mut ExtractionState, node: TsNode<'_>) {
        // union_field_decl -> identifier (key), type (value)
        let name = node
            .child_by_field_name("union_field_key")
            .or_else(|| find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let type_text = node
            .child_by_field_name("union_field_value")
            .or_else(|| find_child_by_kind(node, "type"))
            .map_or_else(|| "unknown".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);
        let signature = Some(format!("{name}: {type_text}"));

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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    /// Extract a bare union member (a lone type with no key).
    fn visit_bare_union_member(state: &mut ExtractionState, node: TsNode<'_>) {
        let type_text = state.node_text(node);

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), type_text);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &type_text, start_line);
        let signature = Some(type_text.clone());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
            name: type_text,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    /// Extract an `rpc_service` definition.
    fn visit_rpc(state: &mut ExtractionState, node: TsNode<'_>) {
        // rpc_decl -> identifier (name), rpc_method*
        let name = node
            .child_by_field_name("rpc_name")
            .or_else(|| find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Interface, &name, start_line);
        let signature = Some(format!("rpc_service {name}"));

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
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Visit the service body for methods.
        state.node_stack.push((name, id));
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "rpc_method" {
                    Self::visit_rpc_method(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_stack.pop();
    }

    /// Extract an RPC method definition within a service.
    fn visit_rpc_method(state: &mut ExtractionState, node: TsNode<'_>) {
        // rpc_method -> identifier (name, parameter, return type)
        let mut idents = Vec::new();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" {
                    idents.push(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        let mut iter = idents.iter();
        let name = iter.next().cloned().unwrap_or("<anonymous>".to_string());
        let param = iter.next().cloned().unwrap_or("unknown".to_string());
        let return_type = iter.next().cloned().unwrap_or("unknown".to_string());

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let signature = Some(format!("{name}({param}): {return_type}"));

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    // ----------------------------
    // Helper methods
    // ----------------------------

    /// Collect `///` documentation lines attached to a definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut docs: Vec<String> = Vec::new();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "documentation" {
                    let text = state.node_text(child);
                    docs.push(text.trim_start_matches('/').trim().to_string());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        }
    }

    /// Build the final `ExtractionResult` from the accumulated state.
    fn build_result(state: ExtractionState, start: Instant) -> ExtractionResult {
        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: Vec::new(),
            errors: state.errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

impl crate::extraction::LanguageExtractor for FlatBuffersExtractor {
    fn extensions(&self) -> &[&str] {
        &["fbs"]
    }

    fn language_name(&self) -> &'static str {
        "FlatBuffers"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_flatbuffers(file_path, source)
    }
}
