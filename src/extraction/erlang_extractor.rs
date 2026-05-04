use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct ErlangExtractor;

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
    file_path: String,
    source: Vec<u8>,
    file_node_id: String,
    timestamp: u64,
}

impl ExtractionState {
    fn new(file_path: &str, source: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let file_node_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            file_node_id,
            timestamp,
        }
    }

    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }
}

impl ErlangExtractor {
    pub fn extract_erlang(file_path: &str, source: &str) -> ExtractionResult {
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
            id: state.file_node_id.clone(),
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
        state.nodes.push(file_node);

        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        Self::build_result(state, start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("erlang");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Erlang grammar: {e}"))?;
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
            "fun_decl" => Self::visit_fun_decl(state, node),
            "module_attribute" => Self::visit_module_attr(state, node),
            "type_alias" | "opaque" => Self::visit_type(state, node),
            "spec" => Self::visit_spec(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    fn visit_fun_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        // fun_decl contains one or more function_clause nodes.
        // The function name is in the first function_clause's `name` child.
        let Some(first_clause) = Self::find_child(node, "function_clause") else {
            return;
        };

        let name = Self::extract_atom_name(state, first_clause)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let arity = Self::count_arity(first_clause);
        let full_name = format!("{}/{}", name, arity);
        let qualified_name = format!("{}::{}", state.file_path, full_name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::Function,
            &full_name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: full_name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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
        state.edges.push(Edge {
            source: state.file_node_id.clone(),
            target: id.clone(),
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });

        // Collect call sites from all clauses.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "function_clause" {
                    if let Some(body) = child.child_by_field_name("body") {
                        Self::extract_calls(state, body, &id);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_module_attr(state: &mut ExtractionState, node: TsNode<'_>) {
        // -module(name). represented as module_attribute with atom child.
        let text = state.node_text(node);
        if !text.starts_with("-module") {
            return;
        }
        let name = Self::extract_attr_value(state, node).unwrap_or_else(|| "?".to_string());
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
            name,
            qualified_name: format!("{}::module", state.file_path),
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            signature: Some(text.trim().to_string()),
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
        state.edges.push(Edge {
            source: state.file_node_id.clone(),
            target: id,
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });
    }

    fn visit_type(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_attr_value(state, node).unwrap_or_else(|| "?".to_string());
        let start_line = node.start_position().row as u32;
        let sig = Self::first_line(state, node);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
            name,
            qualified_name: format!("{}::type", state.file_path),
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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
        state.edges.push(Edge {
            source: state.file_node_id.clone(),
            target: id,
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });
    }

    fn visit_spec(state: &mut ExtractionState, node: TsNode<'_>) {
        // -spec name(Type) -> Type.  Track as an unresolved ref to the function.
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        // Extract just the function name from spec.
        if let Some(name) = Self::extract_attr_value(state, node) {
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: state.file_node_id.clone(),
                reference_name: name,
                reference_kind: EdgeKind::Calls,
                line: start_line,
                column: 0,
                file_path: state.file_path.clone(),
            });
        }
        let _ = text;
    }

    /// Finds the first child of a node with a given kind.
    fn find_child<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
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

    /// Extracts the atom (function name) from the first child of a function_clause.
    fn extract_atom_name(state: &ExtractionState, clause: TsNode<'_>) -> Option<String> {
        if let Some(n) = clause.child_by_field_name("name") {
            return Some(state.node_text(n));
        }
        // Fall back to first atom child.
        let mut cursor = clause.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "atom" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Counts the arity of a function_clause by counting its argument patterns.
    fn count_arity(clause: TsNode<'_>) -> usize {
        if let Some(args) = clause.child_by_field_name("args") {
            return args.named_child_count();
        }
        // Find a `clause_args` or `argument_list` child.
        let mut cursor = clause.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if matches!(
                    child.kind(),
                    "clause_args" | "argument_list" | "pat_argument_list"
                ) {
                    return child.named_child_count();
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        0
    }

    /// Extracts the value from an attribute like `-module(foo)` → "foo".
    fn extract_attr_value(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "atom" {
                    let text = state.node_text(child);
                    // Skip keywords like "module", "type", "spec".
                    if !matches!(
                        text.as_str(),
                        "module" | "type" | "opaque" | "spec" | "callback"
                    ) {
                        return Some(text);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    fn extract_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "call" {
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
                } else {
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

impl crate::extraction::LanguageExtractor for ErlangExtractor {
    fn extensions(&self) -> &[&str] {
        &["erl", "hrl"]
    }

    fn language_name(&self) -> &'static str {
        "Erlang"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_erlang(file_path, source)
    }
}
