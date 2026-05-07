use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, ComplexityMetrics, JULIA_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct JuliaExtractor;

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

impl JuliaExtractor {
    pub fn extract_julia(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(t) => t,
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
        let language = crate::extraction::ts_provider::language("julia");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Julia grammar: {e}"))?;
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
            "function_definition" => Self::visit_function(state, node),
            "macro_definition" => Self::visit_macro(state, node),
            "struct_definition" => Self::visit_struct(state, node),
            "abstract_definition" => Self::visit_abstract_type(state, node),
            "module_definition" => Self::visit_module(state, node),
            "import_statement" | "using_statement" => Self::visit_import(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        let docstring = Self::extract_docstring(state, node);
        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let metrics = if node.child_count() > 0 {
            count_complexity(node, &JULIA_COMPLEXITY, &state.source)
        } else {
            ComplexityMetrics::default()
        };

        Self::push_node(
            state,
            id.clone(),
            NodeKind::Function,
            name.clone(),
            qualified_name,
            node,
            sig,
            docstring,
            metrics.branches,
            metrics.loops,
            metrics.returns,
            metrics.max_nesting,
            metrics.assertions,
        );

        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_calls(state, body, &id);
        }
    }

    fn visit_macro(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = format!("@{}", state.node_text(name_node));
        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        Self::push_node(
            state,
            id,
            NodeKind::Function,
            name,
            qualified_name,
            node,
            sig,
            None,
            0,
            0,
            0,
            0,
            0,
        );
    }

    fn visit_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        let docstring = Self::extract_docstring(state, node);
        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        state.node_stack.push((name.clone(), id.clone()));
        Self::push_node(
            state,
            id,
            NodeKind::Class,
            name,
            qualified_name,
            node,
            sig,
            docstring,
            0,
            0,
            0,
            0,
            0,
        );
        state.node_stack.pop();
    }

    fn visit_abstract_type(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        Self::push_node(
            state,
            id,
            NodeKind::Class,
            name,
            qualified_name,
            node,
            sig,
            None,
            0,
            0,
            0,
            0,
            0,
        );
    }

    fn visit_module(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        Self::push_node(
            state,
            id.clone(),
            NodeKind::Module,
            name.clone(),
            qualified_name,
            node,
            None,
            None,
            0,
            0,
            0,
            0,
            0,
        );

        state.node_stack.push((name, id));
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.node_stack.pop();
    }

    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let name = text.split_whitespace().nth(1).unwrap_or("?").to_string();
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_node(
        state: &mut ExtractionState,
        id: String,
        kind: NodeKind,
        name: String,
        qualified_name: String,
        node: TsNode<'_>,
        signature: Option<String>,
        docstring: Option<String>,
        branches: u32,
        loops: u32,
        returns: u32,
        max_nesting: u32,
        assertions: u32,
    ) {
        let graph_node = Node {
            id: id.clone(),
            kind,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line: node.start_position().row as u32,
            attrs_start_line: node.start_position().row as u32,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            signature,
            docstring,
            visibility: Visibility::Pub,
            is_async: false,
            branches,
            loops,
            returns,
            max_nesting,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(node.start_position().row as u32),
            });
        }
    }

    fn extract_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "call_expression" {
                    if let Some(callee) = child.child_by_field_name("function") {
                        let name = state.node_text(callee);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_id.to_string(),
                            reference_name: name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                    Self::extract_calls(state, child, fn_id);
                } else if !matches!(
                    child.kind(),
                    "function_definition" | "macro_definition" | "struct_definition"
                ) {
                    Self::extract_calls(state, child, fn_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn first_line(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        text.lines().next().map(|l| l.trim().to_string())
    }

    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Julia docstrings appear as string literals immediately before the definition.
        let prev = node.prev_named_sibling()?;
        if matches!(prev.kind(), "string_literal" | "string") {
            return Some(state.node_text(prev).trim_matches('"').trim().to_string());
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

impl crate::extraction::LanguageExtractor for JuliaExtractor {
    fn extensions(&self) -> &[&str] {
        &["jl"]
    }

    fn language_name(&self) -> &'static str {
        "Julia"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_julia(file_path, source)
    }
}
