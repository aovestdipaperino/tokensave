//! Tree-sitter based HTML extractor (#507).
//!
//! Markup defines no callable symbols, so the graph worth building is the set
//! of names a page introduces and the files it pulls in. Three kinds of node:
//!
//! * an element carrying an `id` — emitted as `Field`, the kind the XAML
//!   extractor already gives an `x:Name`d element, and the same kind the CSS
//!   extractor gives an id selector, so a page and its stylesheet name the
//!   same thing the same way
//! * a custom element (`<my-widget>`, any tag containing a `-`) — emitted as
//!   `Class`, since it names a component rather than a builtin tag
//! * `<link rel="stylesheet" href>` and `<script src>` — emitted as `Use`,
//!   the page's imports
//!
//! What is deliberately *not* emitted is a reference from `class="..."` to a
//! CSS selector of that name. Class names are ordinary words — `container`,
//! `header`, `active` — and resolving them by bare name against every symbol
//! in the project is how a stylesheet ends up owning an edge into unrelated
//! Python, which is the failure #503 was about. Linking markup to stylesheets
//! needs a scoped resolution path, not the general one.
//!
//! Builtin tags are not emitted either. A node per `<div>` would bury a
//! project's real symbols under thousands of rows that carry no name.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

pub struct HtmlExtractor;

struct State {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    file_path: String,
    source: Vec<u8>,
    file_node_id: String,
    timestamp: u64,
}

impl State {
    fn new(file_path: &str, source: &str) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
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

    fn emit(&mut self, kind: NodeKind, name: &str, signature: Option<String>, ts_node: TsNode<'_>) {
        if name.is_empty() {
            return;
        }
        let start_line = ts_node.start_position().row as u32;
        let id = generate_node_id(&self.file_path, &kind, name, start_line);
        if self.nodes.iter().any(|n| n.id == id) {
            return;
        }
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

impl HtmlExtractor {
    pub fn extract_html(file_path: &str, source: &str) -> ExtractionResult {
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
            unresolved_refs: Vec::new(),
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn parse(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&crate::extraction::ts_provider::language("html"))
            .map_err(|e| format!("failed to load HTML grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    fn walk(state: &mut State, node: TsNode<'_>) {
        if matches!(node.kind(), "start_tag" | "self_closing_tag") {
            Self::visit_tag(state, node);
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

    /// Reads one opening tag: its name, and the attributes worth a node.
    fn visit_tag(state: &mut State, tag: TsNode<'_>) {
        let mut tag_name = String::new();
        let mut attributes: Vec<(String, String)> = Vec::new();

        let mut cursor = tag.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "tag_name" => tag_name = state.text(child),
                    "attribute" => {
                        if let Some(pair) = Self::read_attribute(state, child) {
                            attributes.push(pair);
                        }
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        let lookup = |wanted: &str| {
            attributes
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
                .map(|(_, value)| value.clone())
        };

        // An element's `id` names it, whatever the tag is.
        if let Some(id) = lookup("id") {
            let signature = Some(format!("<{tag_name} id=\"{id}\">"));
            state.emit(NodeKind::Field, &id, signature, tag);
        }

        // A custom element is a component: the spec requires a hyphen in the
        // name, which is what separates `<my-widget>` from every builtin tag.
        if tag_name.contains('-') {
            let signature = Some(format!("<{tag_name}>"));
            state.emit(NodeKind::Class, &tag_name.clone(), signature, tag);
        }

        // The page's imports.
        let imported = match tag_name.to_ascii_lowercase().as_str() {
            "script" => lookup("src"),
            "link" => lookup("rel")
                .filter(|rel| rel.eq_ignore_ascii_case("stylesheet"))
                .and_then(|_| lookup("href")),
            _ => None,
        };
        if let Some(path) = imported {
            let signature = Some(format!("<{tag_name}> {path}"));
            state.emit(NodeKind::Use, &path, signature, tag);
        }
    }

    /// `name="value"` as a pair, with the quotes stripped.
    fn read_attribute(state: &State, attribute: TsNode<'_>) -> Option<(String, String)> {
        let mut name = None;
        let mut value = None;
        let mut cursor = attribute.walk();
        if !cursor.goto_first_child() {
            return None;
        }
        loop {
            let child = cursor.node();
            match child.kind() {
                "attribute_name" => name = Some(state.text(child)),
                "attribute_value" => value = Some(state.text(child)),
                "quoted_attribute_value" => {
                    // The quoted form wraps the text in an `attribute_value`.
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            if inner.node().kind() == "attribute_value" {
                                value = Some(state.text(inner.node()));
                                break;
                            }
                            if !inner.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                    // An empty quoted value has no inner node at all.
                    value.get_or_insert_default();
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        Some((name?, value.unwrap_or_default()))
    }
}

impl crate::extraction::LanguageExtractor for HtmlExtractor {
    fn extensions(&self) -> &[&str] {
        &["html", "htm"]
    }

    fn language_name(&self) -> &'static str {
        "html"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_html(file_path, source)
    }
}
