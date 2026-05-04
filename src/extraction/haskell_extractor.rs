use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct HaskellExtractor;

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

impl HaskellExtractor {
    pub fn extract_haskell(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("haskell");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Haskell grammar: {e}"))?;
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
            "function" => Self::visit_function(state, node),
            "bind" => Self::visit_bind(state, node),
            "data_type" | "newtype" => Self::visit_data_type(state, node),
            "class" => Self::visit_class(state, node),
            "instance" => Self::visit_instance(state, node),
            "import" => Self::visit_import(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        // tree-sitter-haskell: function has a `name` field or first child is the variable name.
        let name = Self::extract_function_name(state, node);
        let Some(name) = name else { return };

        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        Self::emit(
            state,
            id,
            NodeKind::Function,
            name,
            qualified_name,
            node,
            sig,
            None,
        );
    }

    fn visit_bind(state: &mut ExtractionState, node: TsNode<'_>) {
        // `bind` is a pattern binding — skip signatures, only emit function-shaped binds.
        let name = Self::extract_function_name(state, node);
        let Some(name) = name else { return };

        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        Self::emit(
            state,
            id,
            NodeKind::Function,
            name,
            qualified_name,
            node,
            sig,
            None,
        );
    }

    fn visit_data_type(state: &mut ExtractionState, node: TsNode<'_>) {
        // data Foo = ...  or  newtype Bar = ...
        // The type name is typically the second named child after the `data`/`newtype` keyword.
        let name = Self::find_first_constructor_name(state, node);
        let Some(name) = name else { return };

        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        Self::emit(
            state,
            id,
            NodeKind::Class,
            name,
            qualified_name,
            node,
            sig,
            None,
        );
    }

    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        // class (Context =>) ClassName tvar where ...
        let name = Self::find_first_constructor_name(state, node);
        let Some(name) = name else { return };

        let sig = Self::first_line(state, node);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        Self::emit(
            state,
            id,
            NodeKind::Class,
            name,
            qualified_name,
            node,
            sig,
            None,
        );
    }

    fn visit_instance(state: &mut ExtractionState, node: TsNode<'_>) {
        // instance ClassName Type where ...
        let sig_text = Self::first_line(state, node).unwrap_or_default();
        let name = sig_text
            .trim_start_matches("instance")
            .trim()
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        if name.is_empty() {
            return;
        }

        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::instance {}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        Self::emit(
            state,
            id,
            NodeKind::Class,
            name,
            qualified_name,
            node,
            Some(sig_text),
            None,
        );
    }

    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // "import Module.Name" or "import qualified Module.Name"
        let parts: Vec<&str> = text.split_whitespace().collect();
        let name = parts
            .iter()
            .skip(1)
            .find(|p| p.chars().next().map_or(false, |c| c.is_uppercase()))
            .copied()
            .unwrap_or("?")
            .to_string();

        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name,
            qualified_name: format!("{}::import", state.file_path),
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
        state.edges.push(Edge {
            source: state.file_node_id.clone(),
            target: id,
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });
    }

    /// Finds the function/binding name. Tries named field "name", then first `variable` child.
    fn extract_function_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Try the `name` field first.
        if let Some(n) = node.child_by_field_name("name") {
            return Some(state.node_text(n));
        }
        // Fall back to first child with kind "variable" or "operator".
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if matches!(child.kind(), "variable" | "operator" | "prefix_id") {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Finds the first uppercase identifier (constructor/type name) in a node.
    fn find_first_constructor_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if matches!(child.kind(), "constructor" | "type" | "name") {
                    let text = state.node_text(child);
                    if text.chars().next().map_or(false, |c| c.is_uppercase()) {
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

    fn emit(
        state: &mut ExtractionState,
        id: String,
        kind: NodeKind,
        name: String,
        qualified_name: String,
        node: TsNode<'_>,
        signature: Option<String>,
        docstring: Option<String>,
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
            line: Some(node.start_position().row as u32),
        });
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

impl crate::extraction::LanguageExtractor for HaskellExtractor {
    fn extensions(&self) -> &[&str] {
        &["hs", "lhs"]
    }

    fn language_name(&self) -> &'static str {
        "Haskell"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_haskell(file_path, source)
    }
}
