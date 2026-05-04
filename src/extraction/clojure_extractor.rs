use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct ClojureExtractor;

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

impl ClojureExtractor {
    pub fn extract_clojure(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("clojure");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Clojure grammar: {e}"))?;
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
        if node.kind() != "list_lit" {
            Self::visit_children(state, node);
            return;
        }

        // Check first element: should be a sym_lit naming the special form.
        let head_text = Self::first_sym(state, node);
        let head = head_text.as_deref().unwrap_or("");

        match head {
            "ns" => Self::visit_ns(state, node),
            "defn" | "defn-" => Self::visit_defn(state, node, false),
            "defmacro" => Self::visit_defn(state, node, true),
            "def" | "defonce" => Self::visit_def(state, node),
            "defprotocol" | "defrecord" | "deftype" | "definterface" => {
                Self::visit_deftype(state, node)
            }
            "require" | "use" | "import" => Self::visit_require(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    fn visit_ns(state: &mut ExtractionState, node: TsNode<'_>) {
        // (ns my.namespace ...)
        let Some(name) = Self::nth_sym(state, node, 1) else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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

        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
    }

    fn visit_defn(state: &mut ExtractionState, node: TsNode<'_>, is_macro: bool) {
        // (defn name [args] "docstring" body)
        let Some(name) = Self::nth_sym(state, node, 1) else {
            return;
        };
        let docstring = Self::extract_string_child(state, node);
        let kind = if is_macro {
            NodeKind::Function
        } else {
            NodeKind::Function
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let sig = Self::first_line(state, node);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name,
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

        // Collect call sites from the body.
        Self::extract_calls(state, node, &id, 2);
    }

    fn visit_def(state: &mut ExtractionState, node: TsNode<'_>) {
        // (def name value)
        let Some(name) = Self::nth_sym(state, node, 1) else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
        let sig = Self::first_line(state, node);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Const,
            name,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    fn visit_deftype(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name) = Self::nth_sym(state, node, 1) else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);
        let sig = Self::first_line(state, node);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
            name,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    fn visit_require(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Use, "require", start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name: "require".to_string(),
            qualified_name: format!("{}::require", state.file_path),
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

    /// Returns the text of the first `sym_lit` child.
    fn first_sym(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "sym_lit" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Returns the text of the Nth `sym_lit` child (0-indexed among named children).
    fn nth_sym(state: &ExtractionState, node: TsNode<'_>, n: usize) -> Option<String> {
        let mut count = 0;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "sym_lit" {
                    if count == n {
                        return Some(state.node_text(child));
                    }
                    count += 1;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Returns the first `str_lit` child, used for docstrings.
    fn extract_string_child(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "str_lit" {
                    let text = state.node_text(child);
                    return Some(text.trim_matches('"').to_string());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Collects call sites from the body of a defn, skipping the first `skip` children.
    fn extract_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str, skip: usize) {
        let child_count = node.child_count();
        let mut i = skip;
        while i < child_count {
            if let Some(child) = node.child(i as u32) {
                if child.kind() == "list_lit" {
                    if let Some(head) = Self::first_sym(state, child) {
                        if !matches!(
                            head.as_str(),
                            "defn" | "defn-" | "defmacro" | "def" | "defonce" | "ns"
                        ) {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_id.to_string(),
                                reference_name: head,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        Self::extract_calls(state, child, fn_id, 1);
                    }
                } else {
                    Self::extract_calls(state, child, fn_id, 0);
                }
            }
            i += 1;
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

impl crate::extraction::LanguageExtractor for ClojureExtractor {
    fn extensions(&self) -> &[&str] {
        &["clj", "cljs", "cljc"]
    }

    fn language_name(&self) -> &'static str {
        "Clojure"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_clojure(file_path, source)
    }
}
