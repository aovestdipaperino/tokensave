//! Tree-sitter based CSS extractor (#507).
//!
//! A stylesheet has no functions to call, so the useful graph is the set of
//! *names* it defines and the ones it consumes. Four things are worth a node:
//!
//! * class selectors (`.btn`) — the names markup refers to, emitted as `Class`
//! * id selectors (`#main`) — emitted as `Field`, the same kind the HTML
//!   extractor gives an element's `id`, so the two sides of a page share a
//!   vocabulary
//! * custom properties (`--brand-color`) — emitted as `Const`, which is what
//!   they are
//! * `@keyframes` names — emitted as `Module`, a named block other rules
//!   reference by name
//!
//! Two edge sources, both deliberately exact rather than name-guessed. An
//! `@import` becomes a `Use` node, and `var(--x)` becomes an unresolved
//! reference to the custom property. Selector *usage* from markup is not
//! emitted as a reference: a class named `container` or `header` would be
//! matched by bare name against every symbol in the project, and inventing
//! cross-language edges out of a stylesheet is exactly the failure #503 was
//! about. A custom property is safe because `--` cannot begin an identifier in
//! any language here, so the name cannot collide.
//!
//! A rule set with several selectors emits one node per selector, since a
//! reader searching for `.btn` should find it whether it was written alone or
//! beside `.btn-primary`.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct CssExtractor;

struct State {
    /// Names already emitted, so a selector written in several places is one
    /// node rather than one per occurrence.
    seen: std::collections::HashSet<(NodeKind, String)>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    file_path: String,
    source: Vec<u8>,
    file_node_id: String,
    timestamp: u64,
}

impl State {
    fn new(file_path: &str, source: &str) -> Self {
        Self {
            seen: std::collections::HashSet::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            file_node_id: generate_node_id(file_path, &NodeKind::File, file_path, 0),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    fn text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source).unwrap_or_default().to_string()
    }

    /// Emits a node of `kind` named `name`, contained by the file.
    fn emit(&mut self, kind: NodeKind, name: &str, ts_node: TsNode<'_>) {
        if name.is_empty() {
            return;
        }
        // A stylesheet repeats selectors freely — `.btn` may be written once
        // plainly, again inside `@media`, and again beside a sibling selector.
        // Those are one name, so the first occurrence is the definition and
        // the rest are dropped. Keying on the node id would not do it: the id
        // carries the line, so every repeat would be a distinct node with the
        // same name, which makes a search for `.btn` return the same answer
        // three times.
        if !self.seen.insert((kind.clone(), name.to_string())) {
            return;
        }
        let start_line = ts_node.start_position().row as u32;
        let id = generate_node_id(&self.file_path, &kind, name, start_line);
        self.nodes.push(Node {
            id: id.clone(),
            kind,
            name: name.to_string(),
            qualified_name: format!("{}::{}", self.file_path, name),
            file_path: self.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: ts_node.end_position().row as u32,
            start_column: ts_node.start_position().column as u32,
            end_column: ts_node.end_position().column as u32,
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
            updated_at: self.timestamp,
            parent_id: Some(self.file_node_id.clone()),
        });
        let file_id = self.file_node_id.clone();
        self.edges.push(Edge {
            source: file_id,
            target: id,
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });
    }
}

impl CssExtractor {
    pub fn extract_css(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = State::new(file_path, source);
        let mut errors = Vec::new();

        state.nodes.push(Node {
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        });

        match Self::parse(source) {
            Ok(tree) => Self::walk(&mut state, tree.root_node()),
            Err(message) => errors.push(message),
        }

        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: state.unresolved_refs,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn parse(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&crate::extraction::ts_provider::language("css"))
            .map_err(|e| format!("failed to load CSS grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// One pass over the whole tree.
    ///
    /// Selectors, custom properties and `var()` calls are found at any depth —
    /// a rule nested in `@media`, `@supports` or `@layer` is still a rule —
    /// so this recurses rather than walking only the stylesheet's children.
    fn walk(state: &mut State, node: TsNode<'_>) {
        match node.kind() {
            "class_selector" => {
                if let Some(name) = Self::last_child_of_kind(node, "class_name") {
                    let text = state.text(name);
                    state.emit(NodeKind::Class, &text, node);
                }
            }
            "id_selector" => {
                if let Some(name) = Self::last_child_of_kind(node, "id_name") {
                    let text = state.text(name);
                    state.emit(NodeKind::Field, &text, node);
                }
            }
            "keyframes_statement" => {
                if let Some(name) = Self::last_child_of_kind(node, "keyframes_name") {
                    let text = state.text(name);
                    state.emit(NodeKind::Module, &text, node);
                }
            }
            "declaration" => {
                // `--brand: #fff` defines a custom property; every other
                // declaration sets a known CSS property and defines nothing.
                if let Some(property) = Self::last_child_of_kind(node, "property_name") {
                    let text = state.text(property);
                    if text.starts_with("--") {
                        state.emit(NodeKind::Const, &text, node);
                    }
                }
            }
            "import_statement" => Self::emit_import(state, node),
            "call_expression" => Self::emit_var_reference(state, node),
            _ => {}
        }

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::walk(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// `@import "theme.css"` — a `Use` node named by the imported path.
    fn emit_import(state: &mut State, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return;
        }
        loop {
            let child = cursor.node();
            if matches!(child.kind(), "string_value" | "call_expression") {
                let raw = state.text(child);
                let path = raw.trim_matches(['"', '\''].as_slice());
                let path = path
                    .strip_prefix("url(")
                    .and_then(|rest| rest.strip_suffix(')'))
                    .map_or(path, |inner| inner.trim_matches(['"', '\''].as_slice()));
                if !path.is_empty() {
                    state.emit(NodeKind::Use, path, node);
                }
                return;
            }
            if !cursor.goto_next_sibling() {
                return;
            }
        }
    }

    /// `var(--brand)` — a reference to the custom property of that name.
    ///
    /// Only `var` is followed. Every other CSS function (`rgb`, `calc`,
    /// `translate`) names a builtin, and a reference to a builtin resolves to
    /// nothing but costs a lookup on every stylesheet in the project.
    fn emit_var_reference(state: &mut State, node: TsNode<'_>) {
        let Some(function) = Self::last_child_of_kind(node, "function_name") else {
            return;
        };
        if state.text(function) != "var" {
            return;
        }
        let Some(arguments) = Self::last_child_of_kind(node, "arguments") else {
            return;
        };
        let mut cursor = arguments.walk();
        if !cursor.goto_first_child() {
            return;
        }
        loop {
            let child = cursor.node();
            if child.kind() == "plain_value" {
                let name = state.text(child);
                if name.starts_with("--") {
                    let from = state.file_node_id.clone();
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: from,
                        reference_name: name,
                        reference_kind: EdgeKind::Uses,
                        line: node.start_position().row as u32,
                        column: node.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                }
                return;
            }
            if !cursor.goto_next_sibling() {
                return;
            }
        }
    }

    fn last_child_of_kind<'t>(node: TsNode<'t>, kind: &str) -> Option<TsNode<'t>> {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return None;
        }
        let mut found = None;
        loop {
            let child = cursor.node();
            if child.kind() == kind {
                found = Some(child);
            }
            if !cursor.goto_next_sibling() {
                return found;
            }
        }
    }
}

impl crate::extraction::LanguageExtractor for CssExtractor {
    fn extensions(&self) -> &[&str] {
        &["css"]
    }

    fn language_name(&self) -> &'static str {
        "css"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_css(file_path, source)
    }
}
