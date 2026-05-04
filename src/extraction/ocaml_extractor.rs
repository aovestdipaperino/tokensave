use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, OCAML_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct OcamlExtractor;

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

impl OcamlExtractor {
    pub fn extract_ocaml(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("ocaml");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load OCaml grammar: {e}"))?;
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
            "value_definition" => Self::visit_value_definition(state, node),
            "type_definition" => Self::visit_type_definition(state, node),
            "module_definition" => Self::visit_module_definition(state, node),
            "class_definition" => Self::visit_class_definition(state, node),
            "open_module" => Self::visit_open(state, node),
            // structure/signature items — recurse to find definitions inside
            "structure_item" | "signature_item" | "structure" | "signature" => {
                Self::visit_children(state, node)
            }
            _ => {}
        }
    }

    fn visit_value_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        // value_definition contains one or more let_binding nodes.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "let_binding" {
                    Self::visit_let_binding(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_let_binding(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(pattern) = node.child_by_field_name("pattern") else {
            return;
        };

        let name = Self::extract_value_name(state, pattern);
        let Some(name) = name else { return };

        // Determine if it's a function (has parameters or a fun-shaped body).
        let is_fn = node.child_by_field_name("parameters").is_some()
            || node
                .child_by_field_name("body")
                .map(|b| matches!(b.kind(), "fun_expression" | "function_expression"))
                .unwrap_or(false);

        let kind = if is_fn {
            NodeKind::Function
        } else {
            NodeKind::Const
        };
        let docstring = Self::extract_docstring(state, node);
        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

        let metrics = if is_fn && node.child_count() > 0 {
            count_complexity(node, &OCAML_COMPLEXITY, &state.source)
        } else {
            Default::default()
        };

        let graph_node = Node {
            id: id.clone(),
            kind,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            signature: sig,
            docstring,
            visibility: Visibility::Pub,
            is_async: false,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
            unsafe_blocks: 0,
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

        if is_fn {
            if let Some(body) = node.child_by_field_name("body") {
                Self::extract_calls(state, body, &id);
            }
        }
    }

    fn visit_type_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        // type_definition contains type_binding nodes.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_binding" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = state.node_text(name_node);
                        let start_line = child.start_position().row as u32;
                        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                        let id =
                            generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);
                        let sig = Self::first_line(state, child);

                        let graph_node = Node {
                            id: id.clone(),
                            kind: NodeKind::Class,
                            name,
                            qualified_name,
                            file_path: state.file_path.clone(),
                            start_line,
                            attrs_start_line: start_line,
                            end_line: child.end_position().row as u32,
                            start_column: child.start_position().column as u32,
                            end_column: child.end_position().column as u32,
                            signature: sig,
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
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_module_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        // module_definition contains module_binding nodes.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "module_binding" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = state.node_text(name_node);
                        let start_line = child.start_position().row as u32;
                        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                        let id = generate_node_id(
                            &state.file_path,
                            &NodeKind::Module,
                            &name,
                            start_line,
                        );

                        let graph_node = Node {
                            id: id.clone(),
                            kind: NodeKind::Module,
                            name: name.clone(),
                            qualified_name,
                            file_path: state.file_path.clone(),
                            start_line,
                            attrs_start_line: start_line,
                            end_line: child.end_position().row as u32,
                            start_column: child.start_position().column as u32,
                            end_column: child.end_position().column as u32,
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

                        if let Some(parent_id) = state.parent_node_id() {
                            state.edges.push(Edge {
                                source: parent_id.to_string(),
                                target: id.clone(),
                                kind: EdgeKind::Contains,
                                line: Some(start_line),
                            });
                        }

                        // Recurse into module body.
                        state.node_stack.push((name, id));
                        if let Some(body) = child.child_by_field_name("body") {
                            Self::visit_children(state, body);
                        }
                        state.node_stack.pop();
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_class_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "class_binding" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = state.node_text(name_node);
                        let start_line = child.start_position().row as u32;
                        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                        let id =
                            generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);
                        let sig = Self::first_line(state, child);

                        let graph_node = Node {
                            id: id.clone(),
                            kind: NodeKind::Class,
                            name,
                            qualified_name,
                            file_path: state.file_path.clone(),
                            start_line,
                            attrs_start_line: start_line,
                            end_line: child.end_position().row as u32,
                            start_column: child.start_position().column as u32,
                            end_column: child.end_position().column as u32,
                            signature: sig,
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
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_open(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let name = text
            .split_whitespace()
            .nth(1)
            .unwrap_or("?")
            .trim_end_matches(';')
            .to_string();
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name,
            qualified_name: format!("{}::open", state.file_path),
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

    /// Extracts an OCaml value name from a pattern node.
    fn extract_value_name(state: &ExtractionState, pattern: TsNode<'_>) -> Option<String> {
        match pattern.kind() {
            "value_name" => {
                // value_name has a child identifier or operator.
                if let Some(inner) = pattern.child(0) {
                    return Some(state.node_text(inner));
                }
                Some(state.node_text(pattern))
            }
            "identifier" => Some(state.node_text(pattern)),
            _ => None,
        }
    }

    fn extract_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "application_expression" {
                    // First child is the callee.
                    if let Some(callee) = child.child(0) {
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
                } else if !matches!(child.kind(), "let_binding" | "fun_expression") {
                    Self::extract_calls(state, child, fn_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let prev = node.prev_named_sibling()?;
        if prev.kind() == "attribute" {
            let text = state.node_text(prev);
            if text.contains("@doc") || text.contains("@ocaml.doc") {
                return Some(text.trim_matches('[').trim_matches(']').trim().to_string());
            }
        }
        None
    }

    fn first_line(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        text.lines().next().map(|l| l.trim().to_string())
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

impl crate::extraction::LanguageExtractor for OcamlExtractor {
    fn extensions(&self) -> &[&str] {
        &["ml", "mli"]
    }

    fn language_name(&self) -> &'static str {
        "OCaml"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_ocaml(file_path, source)
    }
}
