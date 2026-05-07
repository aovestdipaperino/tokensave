/// Tree-sitter based Protobuf source code extractor.
///
/// Parses `.proto` files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

/// Extracts code graph nodes and edges from Protobuf source files using tree-sitter.
pub struct ProtoExtractor;

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

impl ProtoExtractor {
    /// Extract code graph nodes and edges from a Protobuf source file.
    pub fn extract_proto(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("protobuf");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Protobuf grammar: {e}"))?;
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
            "package" => Self::visit_package(state, node),
            "import" => Self::visit_import(state, node),
            "message" => Self::visit_message(state, node),
            "enum" => Self::visit_enum(state, node),
            "service" => Self::visit_service(state, node),
            _ => {}
        }
    }

    /// Extract a `package` declaration.
    fn visit_package(state: &mut ExtractionState, node: TsNode<'_>) {
        // package -> fullIdent -> ident
        let name = Self::find_child_by_kind(node, "fullIdent")
            .and_then(|fi| Self::find_child_by_kind(fi, "ident"))
            .map_or_else(|| "<unknown>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Package, &name, start_line);
        let signature = Some(
            state
                .node_text(node)
                .trim_end_matches(';')
                .trim()
                .to_string(),
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Package,
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

    /// Extract an `import` statement.
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        // import -> strLit (the quoted path)
        let name = Self::find_child_by_kind(node, "strLit").map_or_else(
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
        let signature = Some(
            state
                .node_text(node)
                .trim_end_matches(';')
                .trim()
                .to_string(),
        );

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

    /// Extract a `message` definition.
    fn visit_message(state: &mut ExtractionState, node: TsNode<'_>) {
        // message -> messageName -> ident, messageBody -> (field | message | oneof | enum | ...)
        let name = Self::find_child_by_kind(node, "messageName")
            .and_then(|mn| Self::find_child_by_kind(mn, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::ProtoMessage, &name, start_line);
        let signature = Some(format!("message {name}"));

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::ProtoMessage,
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

        // Visit message body for fields, nested messages, enums, oneofs.
        state.node_stack.push((name, id));
        if let Some(body) = Self::find_child_by_kind(node, "messageBody") {
            Self::visit_message_body(state, body);
        }
        state.node_stack.pop();
    }

    /// Visit the body of a message, extracting fields, nested messages, enums, and oneofs.
    fn visit_message_body(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "field" => Self::visit_field(state, child),
                    "message" => Self::visit_message(state, child),
                    "enum" => Self::visit_enum(state, child),
                    "oneof" => Self::visit_oneof(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a field declaration within a message.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        // field -> type, fieldName -> ident, `=`, fieldNumber -> intLit
        let name = Self::find_child_by_kind(node, "fieldName")
            .and_then(|fn_node| Self::find_child_by_kind(fn_node, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let type_text = Self::find_child_by_kind(node, "type")
            .map_or_else(|| "unknown".to_string(), |n| state.node_text(n));

        let field_number = Self::find_child_by_kind(node, "fieldNumber")
            .and_then(|fn_node| Self::find_child_by_kind(fn_node, "intLit"))
            .map_or_else(|| "?".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);
        let signature = Some(format!("{type_text} {name} = {field_number}"));

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

    /// Extract an `enum` definition.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        // enum -> enumName -> ident, enumBody -> enumField*
        let name = Self::find_child_by_kind(node, "enumName")
            .and_then(|en| Self::find_child_by_kind(en, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Enum, &name, start_line);
        let signature = Some(format!("enum {name}"));

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

        // Visit enum body for variants.
        state.node_stack.push((name, id));
        if let Some(body) = Self::find_child_by_kind(node, "enumBody") {
            Self::visit_enum_body(state, body);
        }
        state.node_stack.pop();
    }

    /// Visit the body of an enum, extracting variants.
    fn visit_enum_body(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enumField" {
                    Self::visit_enum_field(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an enum variant (enumField).
    fn visit_enum_field(state: &mut ExtractionState, node: TsNode<'_>) {
        // enumField -> ident, intLit
        let name = Self::find_child_by_kind(node, "ident")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let value = Self::find_child_by_kind(node, "intLit")
            .map_or_else(|| "?".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::EnumVariant, &name, start_line);
        let signature = Some(format!("{name} = {value}"));

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

    /// Extract a `service` definition.
    fn visit_service(state: &mut ExtractionState, node: TsNode<'_>) {
        // service -> serviceName -> ident, rpc*
        let name = Self::find_child_by_kind(node, "serviceName")
            .and_then(|sn| Self::find_child_by_kind(sn, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::ProtoService, &name, start_line);
        let signature = Some(format!("service {name}"));

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::ProtoService,
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

        // Visit service body for rpc methods.
        state.node_stack.push((name, id));
        Self::visit_service_body(state, node);
        state.node_stack.pop();
    }

    /// Visit the children of a service node, extracting rpc methods.
    fn visit_service_body(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "rpc" {
                    Self::visit_rpc(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an `rpc` method definition within a service.
    fn visit_rpc(state: &mut ExtractionState, node: TsNode<'_>) {
        // rpc -> rpcName -> ident, enumMessageType (request), enumMessageType (response)
        let name = Self::find_child_by_kind(node, "rpcName")
            .and_then(|rn| Self::find_child_by_kind(rn, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::ProtoRpc, &name, start_line);

        // Build signature from the full rpc text (first line)
        let text = state.node_text(node);
        let signature = text
            .lines()
            .next()
            .map(|l| l.trim().trim_end_matches(';').trim().to_string())
            .filter(|l| !l.is_empty());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::ProtoRpc,
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

    /// Extract a `oneof` block, visiting its fields.
    fn visit_oneof(state: &mut ExtractionState, node: TsNode<'_>) {
        // oneof -> oneofName -> ident, oneofField*
        // We extract the oneof fields as regular fields.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "oneofField" {
                    Self::visit_oneof_field(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a field within a oneof block.
    fn visit_oneof_field(state: &mut ExtractionState, node: TsNode<'_>) {
        // oneof_field -> type, fieldName -> ident, `=`, fieldNumber -> intLit.
        let name = Self::find_child_by_kind(node, "fieldName")
            .and_then(|fn_node| Self::find_child_by_kind(fn_node, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let type_text = Self::find_child_by_kind(node, "type")
            .map_or_else(|| "unknown".to_string(), |n| state.node_text(n));

        let field_number = Self::find_child_by_kind(node, "fieldNumber")
            .and_then(|fn_node| Self::find_child_by_kind(fn_node, "intLit"))
            .map_or_else(|| "?".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);
        let signature = Some(format!("{type_text} {name} = {field_number}"));

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

    // ----------------------------
    // Helper methods
    // ----------------------------

    /// Extract docstrings from `// comment` lines preceding definitions.
    ///
    /// Protobuf uses line comments (`//`) as documentation. We look for `comment`
    /// sibling nodes that immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = text.trim_start_matches("//").trim().to_string();
                comments.push(stripped);
                prev = prev_node.prev_named_sibling();
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        Some(comments.join("\n"))
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
            unresolved_refs: Vec::new(),
            errors: state.errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

impl crate::extraction::LanguageExtractor for ProtoExtractor {
    fn extensions(&self) -> &[&str] {
        &["proto"]
    }

    fn language_name(&self) -> &'static str {
        "Protobuf"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_proto(file_path, source)
    }
}
