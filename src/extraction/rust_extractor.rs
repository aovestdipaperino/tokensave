/// Tree-sitter based Rust source code extractor.
///
/// Parses Rust source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Rust source files using tree-sitter.
pub struct RustExtractor;

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

impl RustExtractor {
    /// Extract code graph nodes and edges from a Rust source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Rust source code to parse.
    pub fn extract(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Rust grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit all named children of a node.
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
            "function_item" | "function_signature_item" => Self::visit_function(state, node),
            "struct_item" => Self::visit_struct(state, node),
            "enum_item" => Self::visit_enum(state, node),
            "trait_item" => Self::visit_trait(state, node),
            "impl_item" => Self::visit_impl(state, node),
            "use_declaration" => Self::visit_use(state, node),
            "const_item" => Self::visit_const(state, node),
            "static_item" => Self::visit_static(state, node),
            "type_item" => Self::visit_type_alias(state, node),
            "mod_item" => Self::visit_module(state, node),
            "macro_invocation" => Self::visit_macro_invocation(state, node),
            _ => {
                // For other node types, recurse into children to find nested items.
                Self::visit_children(state, node);
            }
        }
    }

    /// Extract a function or free function node.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let is_inside_impl = state
            .node_stack
            .iter()
            .any(|(_, id)| id.starts_with("impl:"));
        let is_inside_trait = state
            .node_stack
            .iter()
            .any(|(_, id)| id.starts_with("trait:"));
        let kind = if is_inside_impl || is_inside_trait {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let visibility = Self::extract_visibility(node, state);
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let is_async = Self::detect_async(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
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
            is_async,
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

    /// Extract a struct node and its fields.
    fn visit_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let signature = Self::extract_struct_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
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

        // Check for derive macros on preceding attribute items.
        Self::extract_derive_macros(state, node, &id);

        // Extract fields.
        state.node_stack.push((name, id.clone()));
        Self::extract_fields(state, node);
        state.node_stack.pop();
    }

    /// Extract an enum node and its variants.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_docstring(state, node);
        let text = state.node_text(node);
        let signature = Some(text.lines().next().unwrap_or("").to_string());
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

        // Check for derive macros on preceding attribute items.
        Self::extract_derive_macros(state, node, &id);

        // Extract enum variants.
        state.node_stack.push((name, id.clone()));
        Self::extract_enum_variants(state, node);
        state.node_stack.pop();
    }

    /// Extract a trait node and its methods.
    fn visit_trait(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_docstring(state, node);
        let signature = Some(format!("trait {name}"));
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

        // Visit trait body: methods inside become Method nodes.
        state.node_stack.push((name, id));
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.node_stack.pop();
    }

    /// Extract an impl block node and its methods.
    fn visit_impl(state: &mut ExtractionState, node: TsNode<'_>) {
        let type_name =
            Self::extract_impl_type_name(state, node).unwrap_or_else(|| "<unknown>".to_string());
        let trait_name = Self::extract_impl_trait_name(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), type_name);
        let id = generate_node_id(&state.file_path, &NodeKind::Impl, &type_name, start_line);

        let signature = if let Some(ref trait_n) = trait_name {
            Some(format!("impl {trait_n} for {type_name}"))
        } else {
            Some(format!("impl {type_name}"))
        };

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Impl,
            name: type_name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
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

        // If this is a trait impl, create an Implements edge/ref.
        if let Some(ref trait_n) = trait_name {
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: id.clone(),
                reference_name: trait_n.clone(),
                reference_kind: EdgeKind::Implements,
                line: start_line,
                column: start_column,
                file_path: state.file_path.clone(),
            });
        }

        // Visit impl body: functions become Method nodes.
        state.node_stack.push((type_name, id));
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.node_stack.pop();
    }

    /// Extract a use declaration node.
    fn visit_use(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Strip the `use ` prefix and trailing `;`.
        let path = text
            .trim()
            .strip_prefix("use ")
            .unwrap_or(&text)
            .trim_end_matches(';')
            .trim()
            .to_string();
        let visibility = Self::extract_visibility(node, state);
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

    /// Extract a const item node.
    fn visit_const(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_docstring(state, node);
        let text = state.node_text(node);
        let signature = Some(text.lines().next().unwrap_or("").to_string());
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

    /// Extract a static item node.
    fn visit_static(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_docstring(state, node);
        let text = state.node_text(node);
        let signature = Some(text.lines().next().unwrap_or("").to_string());
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

    /// Extract a type alias node.
    fn visit_type_alias(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_docstring(state, node);
        let text = state.node_text(node);
        let signature = Some(text.trim().to_string());
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

    /// Extract a module item node.
    fn visit_module(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("mod {name}")),
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

        // Visit the module body.
        state.node_stack.push((name, id));
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.node_stack.pop();
    }

    /// Record a macro invocation as an unresolved call reference.
    fn visit_macro_invocation(state: &mut ExtractionState, node: TsNode<'_>) {
        let macro_name = node
            .child_by_field_name("macro")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| {
                // Fallback: first named child is typically the macro name.
                let text = state.node_text(node);
                text.split('!').next().unwrap_or("").trim().to_string()
            });
        let start_line = node.start_position().row as u32;
        let start_column = node.start_position().column as u32;

        if let Some(parent_id) = state.parent_node_id() {
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: parent_id.to_string(),
                reference_name: macro_name,
                reference_kind: EdgeKind::Calls,
                line: start_line,
                column: start_column,
                file_path: state.file_path.clone(),
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

    /// Extract the type name from an impl_item (the "type" field).
    fn extract_impl_type_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        node.child_by_field_name("type").map(|n| state.node_text(n))
    }

    /// Extract the trait name from an impl_item, if it is a trait impl.
    ///
    /// For `impl Trait for Type`, tree-sitter gives us a "trait" field.
    fn extract_impl_trait_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        node.child_by_field_name("trait")
            .map(|n| state.node_text(n))
    }

    /// Extract visibility from a node.
    fn extract_visibility(node: TsNode<'_>, state: &ExtractionState) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "visibility_modifier" {
                    let text = state.node_text(child);
                    return match text.as_str() {
                        "pub" => Visibility::Pub,
                        s if s.contains("crate") => Visibility::PubCrate,
                        s if s.contains("super") => Visibility::PubSuper,
                        _ => Visibility::Pub,
                    };
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        Visibility::Private
    }

    /// Extract the function signature (everything from `fn` up to the body `{`).
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        // Find the opening brace and take everything before it.
        if let Some(brace_pos) = text.find('{') {
            Some(text[..brace_pos].trim().to_string())
        } else {
            // For trait method declarations without a body (ending with `;`).
            Some(text.trim_end_matches(';').trim().to_string())
        }
    }

    /// Extract the struct signature (the header line).
    fn extract_struct_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        // Take the first line, or up to the opening brace.
        if let Some(brace_pos) = text.find('{') {
            Some(text[..brace_pos].trim().to_string())
        } else {
            Some(text.lines().next().unwrap_or("").trim().to_string())
        }
    }

    /// Extract docstrings from preceding comment nodes.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "line_comment" | "block_comment" => {
                    let text = state.node_text(sibling);
                    comments.push(text);
                    current = sibling.prev_named_sibling();
                }
                "attribute_item" => {
                    // Skip attributes (like #[derive(...)]) that sit between doc
                    // comments and the item.
                    current = sibling.prev_named_sibling();
                }
                _ => break,
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

    /// Strip comment markers from a single comment text.
    fn clean_comment(comment: &str) -> String {
        let trimmed = comment.trim();
        if let Some(stripped) = trimmed.strip_prefix("///") {
            stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
        } else if let Some(stripped) = trimmed.strip_prefix("//!") {
            stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
        } else if let Some(stripped) = trimmed.strip_prefix("//") {
            stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
        } else if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
            // Block comment: strip /* and */ and clean each line.
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

    /// Detect if a function is async.
    fn detect_async(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let text = state.node_text(node);
        let trimmed = text.trim_start();
        trimmed.starts_with("async ")
            || trimmed.starts_with("pub async ")
            || trimmed.starts_with("pub(crate) async ")
            || trimmed.starts_with("pub(super) async ")
    }

    /// Extract fields from a struct's field_declaration_list.
    fn extract_fields(state: &mut ExtractionState, struct_node: TsNode<'_>) {
        if let Some(body) = struct_node.child_by_field_name("body") {
            let mut cursor = body.walk();
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

    /// Extract a single field_declaration node.
    fn extract_single_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_visibility(node, state);
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
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(',').trim().to_string()),
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract enum variants from the enum body.
    fn extract_enum_variants(state: &mut ExtractionState, enum_node: TsNode<'_>) {
        if let Some(body) = enum_node.child_by_field_name("body") {
            let mut cursor = body.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "enum_variant" {
                        Self::extract_single_variant(state, child);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    /// Extract a single enum variant.
    fn extract_single_variant(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let text = state.node_text(node);
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
            signature: Some(text.trim().trim_end_matches(',').to_string()),
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

    /// Recursively find call_expression nodes inside a given node and create
    /// unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        if let Some(callee) = child.child_by_field_name("function") {
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
                    "macro_invocation" => {
                        let macro_name = child
                            .child_by_field_name("macro")
                            .map(|n| state.node_text(n))
                            .unwrap_or_else(|| {
                                let text = state.node_text(child);
                                text.split('!').next().unwrap_or("").trim().to_string()
                            });
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: macro_name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                    // Skip nested function definitions — they are handled separately.
                    "function_item" => {}
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

    /// Extract derive macros from attribute items preceding a struct/enum.
    fn extract_derive_macros(state: &mut ExtractionState, node: TsNode<'_>, item_id: &str) {
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            if sibling.kind() == "attribute_item" {
                let text = state.node_text(sibling);
                if text.contains("derive") {
                    Self::parse_derive_list(state, &text, item_id, sibling);
                }
                current = sibling.prev_named_sibling();
            } else if sibling.kind() == "line_comment" || sibling.kind() == "block_comment" {
                // Skip comments between attributes and the item.
                current = sibling.prev_named_sibling();
            } else {
                break;
            }
        }
    }

    /// Parse a derive attribute list and emit DerivesMacro edges.
    fn parse_derive_list(
        state: &mut ExtractionState,
        attr_text: &str,
        item_id: &str,
        attr_node: TsNode<'_>,
    ) {
        // attr_text is like: `#[derive(Debug, Clone, Serialize)]`
        // Find the content inside derive(...).
        if let Some(start) = attr_text.find("derive(") {
            let after = &attr_text[start + 7..];
            if let Some(end) = after.find(')') {
                let inner = &after[..end];
                let line = attr_node.start_position().row as u32;
                for trait_name in inner.split(',') {
                    let trait_name = trait_name.trim();
                    if !trait_name.is_empty() {
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: item_id.to_string(),
                            reference_name: trait_name.to_string(),
                            reference_kind: EdgeKind::DerivesMacro,
                            line,
                            column: attr_node.start_position().column as u32,
                            file_path: state.file_path.to_string(),
                        });
                    }
                }
            }
        }
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

impl crate::extraction::LanguageExtractor for RustExtractor {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn language_name(&self) -> &str {
        "Rust"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        RustExtractor::extract(file_path, source)
    }
}
