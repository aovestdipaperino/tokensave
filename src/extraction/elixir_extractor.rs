use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct ElixirExtractor;

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

impl ElixirExtractor {
    pub fn extract_elixir(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("elixir");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Elixir grammar: {e}"))?;
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
        if node.kind() != "call" {
            Self::visit_children(state, node);
            return;
        }

        // In Elixir's tree-sitter grammar, def/defmodule/etc. are `call` nodes.
        // The function being called is the first child (target/function).
        let head = Self::call_head(state, node);
        match head.as_deref() {
            Some("defmodule") => Self::visit_defmodule(state, node),
            Some("def") | Some("defp") => {
                Self::visit_def(state, node, head.as_deref() == Some("defp"))
            }
            Some("defmacro") | Some("defmacrop") => Self::visit_defmacro(state, node),
            Some("defstruct") => Self::visit_defstruct(state, node),
            Some("import") | Some("require") | Some("use") | Some("alias") => {
                Self::visit_use(state, node)
            }
            _ => Self::visit_children(state, node),
        }
    }

    fn visit_defmodule(state: &mut ExtractionState, node: TsNode<'_>) {
        // defmodule MyModule do ... end
        let name = Self::call_arg_name(state, node).unwrap_or_else(|| "?".to_string());
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
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
        // Recurse into the do_block body.
        if let Some(body) = Self::find_do_block(node) {
            Self::visit_children(state, body);
        }
        state.node_stack.pop();
    }

    fn visit_def(state: &mut ExtractionState, node: TsNode<'_>, is_private: bool) {
        // def name(args) do ... end
        let name = Self::call_arg_name(state, node).unwrap_or_else(|| "?".to_string());
        let start_line = node.start_position().row as u32;
        let sig = Self::first_line(state, node);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
        let visibility = if is_private {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        // Extract @doc attribute from preceding attribute call.
        let docstring = Self::extract_doc(state, node);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
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
            visibility,
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

        if let Some(body) = Self::find_do_block(node) {
            Self::extract_calls(state, body, &id);
        }
    }

    fn visit_defmacro(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::call_arg_name(state, node).unwrap_or_else(|| "?".to_string());
        let start_line = node.start_position().row as u32;
        let sig = Self::first_line(state, node);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
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

    fn visit_defstruct(state: &mut ExtractionState, node: TsNode<'_>) {
        // defstruct is a macro that defines a struct in the current module.
        // Emit as a Class node using the enclosing module name.
        let name = state
            .node_stack
            .last()
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| "?".to_string());
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

    fn visit_use(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let name = Self::call_arg_name(state, node).unwrap_or_else(|| "?".to_string());
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name,
            qualified_name: format!("{}::use", state.file_path),
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

    /// Returns the identifier of the function being called (the `call` head).
    fn call_head(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // In tree-sitter-elixir, call has a `target` field or first named child is the callee.
        if let Some(target) = node.child_by_field_name("target") {
            return Some(state.node_text(target));
        }
        // Fall back: first identifier child.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Returns the name from the first argument of a call (e.g. module name in defmodule).
    fn call_arg_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Look for the `arguments` child, then find the first alias/identifier/call.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "arguments" {
                    // First named child of arguments.
                    if let Some(arg) = child.named_child(0) {
                        return Some(state.node_text(arg));
                    }
                }
                // For `def name(args)` the function name might be directly a `call`
                // child (a call of name/args).
                if child.kind() == "call" {
                    if let Some(inner_head) = Self::call_head(state, child) {
                        return Some(inner_head);
                    }
                }
                if child.kind() == "alias" || child.kind() == "identifier" {
                    let text = state.node_text(child);
                    // Skip the defmodule/def keyword itself.
                    if !matches!(
                        text.as_str(),
                        "defmodule"
                            | "def"
                            | "defp"
                            | "defmacro"
                            | "defmacrop"
                            | "defstruct"
                            | "import"
                            | "require"
                            | "use"
                            | "alias"
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

    /// Finds a `do_block` child for recursing into body.
    fn find_do_block(node: TsNode<'_>) -> Option<TsNode<'_>> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "do_block" || child.kind() == "body" {
                    return Some(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    fn extract_doc(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // @doc "..." precedes def as a sibling call node.
        let prev = node.prev_named_sibling()?;
        if prev.kind() == "call" {
            let head = Self::call_head(state, prev)?;
            if head == "@doc" {
                let text = state.node_text(prev);
                return Some(text);
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
                    let head = Self::call_head(state, child);
                    if let Some(name) = head {
                        if !matches!(
                            name.as_str(),
                            "def" | "defp" | "defmacro" | "defmacrop" | "defmodule"
                        ) {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_id.to_string(),
                                reference_name: name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
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

impl crate::extraction::LanguageExtractor for ElixirExtractor {
    fn extensions(&self) -> &[&str] {
        &["ex", "exs"]
    }

    fn language_name(&self) -> &'static str {
        "Elixir"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_elixir(file_path, source)
    }
}
