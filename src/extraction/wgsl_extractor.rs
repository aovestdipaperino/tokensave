/// Tree-sitter based WGSL (WebGPU Shading Language) source code extractor.
///
/// Parses WGSL source files and emits nodes and edges for the code graph.
/// Handles `.wgsl` files.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, C_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from WGSL source files using tree-sitter.
pub struct WgslExtractor;

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
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

    fn qualified_prefix(&self) -> String {
        let mut parts = vec![self.file_path.clone()];
        for (name, _) in &self.node_stack {
            parts.push(name.clone());
        }
        parts.join("::")
    }

    fn parent_node_id(&self) -> Option<&str> {
        self.node_stack.last().map(|(_, id)| id.as_str())
    }

    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }
}

impl WgslExtractor {
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
            parent_id: None,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state.node_stack.push((file_path.to_string(), file_node_id));

        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();
        Self::build_result(state, start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("wgsl");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load WGSL grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit_node(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "function_decl" => Self::visit_function_decl(state, node),
            "struct_decl" => Self::visit_struct_decl(state, node),
            "global_variable_decl" => Self::visit_global_variable(state, node),
            "global_constant_decl" => Self::visit_global_constant(state, node),
            "type_alias_decl" => Self::visit_type_alias(state, node),
            _ => {}
        }
    }

    // -------------------------------------------------------
    // function_decl
    // -------------------------------------------------------

    fn visit_function_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(header) = Self::find_child_by_kind(node, "function_header") else {
            return;
        };
        let name = Self::find_child_by_kind(header, "ident")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        // Collect stage attributes (@vertex, @fragment, @compute).
        let attrs = Self::collect_attributes(state, node);
        let docstring = if attrs.is_empty() {
            None
        } else {
            Some(attrs.join(" "))
        };

        let signature = Some(Self::extract_function_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let body = Self::find_child_by_kind(node, "compound_statement");
        let metrics = body
            .map(|b| count_complexity(b, &C_COMPLEXITY, &state.source))
            .unwrap_or_default();

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
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
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
            unsafe_blocks: metrics.unsafe_blocks,
            unchecked_calls: metrics.unchecked_calls,
            assertions: metrics.assertions,
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
            });
        }

        if let Some(body) = body {
            Self::extract_call_sites(state, body, &id);
        }
    }

    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            text[..brace_pos].trim().to_string()
        } else {
            text.trim().to_string()
        }
    }

    fn collect_attributes(state: &ExtractionState, node: TsNode<'_>) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute" {
                    // attribute children: "@", ident, optional "(…)"
                    if let Some(ident) = Self::find_child_by_kind(child, "ident") {
                        attrs.push(format!("@{}", state.node_text(ident)));
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        attrs
    }

    // -------------------------------------------------------
    // struct_decl
    // -------------------------------------------------------

    fn visit_struct_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "ident")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

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
            parent_id: None,
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

        // Visit struct members.
        if let Some(body) = Self::find_child_by_kind(node, "struct_body_decl") {
            state.node_stack.push((name, id));
            Self::visit_struct_members(state, body);
            state.node_stack.pop();
        }
    }

    fn visit_struct_members(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "struct_member" {
                    Self::visit_struct_member(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_struct_member(state: &mut ExtractionState, node: TsNode<'_>) {
        // struct_member: attribute* ident ":" type_decl
        let Some(ident) = Self::find_child_by_kind(node, "ident") else {
            return;
        };
        let name = state.node_text(ident);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let sig = state.node_text(node);
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
            signature: Some(sig.trim().trim_end_matches(',').trim().to_string()),
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
            parent_id: None,
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

    // -------------------------------------------------------
    // Global variables and constants
    // -------------------------------------------------------

    fn visit_global_variable(state: &mut ExtractionState, node: TsNode<'_>) {
        // global_variable_decl: attribute* variable_decl ("=" expression)?
        // variable_decl: "var" variable_qualifier? variable_ident_decl
        // variable_ident_decl: ident (":" type_decl)?
        let name = Self::find_descendant_by_kind(node, "variable_ident_decl")
            .and_then(|vid| Self::find_child_by_kind(vid, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        Self::emit_variable_node(state, node, name, NodeKind::Static);
    }

    fn visit_global_constant(state: &mut ExtractionState, node: TsNode<'_>) {
        // global_constant_decl: attribute* ("const"|"override") (ident | variable_ident_decl) "=" expression
        let name = Self::find_descendant_by_kind(node, "variable_ident_decl")
            .and_then(|vid| Self::find_child_by_kind(vid, "ident"))
            .or_else(|| Self::find_child_by_kind(node, "ident"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        Self::emit_variable_node(state, node, name, NodeKind::Const);
    }

    fn emit_variable_node(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        name: String,
        kind: NodeKind,
    ) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
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
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
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
            parent_id: None,
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

    // -------------------------------------------------------
    // Type aliases
    // -------------------------------------------------------

    fn visit_type_alias(state: &mut ExtractionState, node: TsNode<'_>) {
        // type_alias_decl: "type" ident "=" type_decl
        let name = Self::find_child_by_kind(node, "ident")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
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
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
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
            parent_id: None,
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

    // -------------------------------------------------------
    // Call site extraction
    // -------------------------------------------------------

    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                // WGSL: func_call_statement is a statement-level call.
                // The callee name is the first named child (the callable ident).
                if child.kind() == "func_call_statement" {
                    if let Some(callee) = child.named_child(0) {
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
                }
                Self::extract_call_sites(state, child, fn_node_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // -------------------------------------------------------
    // Utility helpers
    // -------------------------------------------------------

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

    fn find_descendant_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == kind {
                    return Some(child);
                }
                if let Some(found) = Self::find_descendant_by_kind(child, kind) {
                    return Some(found);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

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

impl crate::extraction::LanguageExtractor for WgslExtractor {
    fn extensions(&self) -> &[&str] {
        &["wgsl"]
    }

    fn language_name(&self) -> &'static str {
        "WGSL"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        WgslExtractor::extract_source(file_path, source)
    }
}
