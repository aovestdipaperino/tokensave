/// Tree-sitter based TOML extractor.
///
/// The grammar's root is `document`, whose direct children are `pair`,
/// `table`, and `table_array_element` nodes. Tables and table-arrays are
/// emitted as `Module` nodes; key-value pairs become `Const` nodes
/// parented to their enclosing table (or to the file if at top level).
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

pub struct TomlExtractor;

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
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

impl TomlExtractor {
    pub fn extract_toml(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

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

        if let Ok(tree) = Self::parse(source) {
            Self::visit_document(&mut state, tree.root_node());
        }

        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn parse(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("toml");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load TOML grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    fn visit_document(state: &mut ExtractionState, root: TsNode<'_>) {
        let mut cursor = root.walk();
        if !cursor.goto_first_child() {
            return;
        }
        let file_id = state.file_node_id.clone();
        let file_qn = state.file_path.clone();
        loop {
            let child = cursor.node();
            match child.kind() {
                "pair" => Self::emit_pair(state, child, &file_id, &file_qn),
                "table" | "table_array_element" => Self::emit_table(state, child),
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn emit_table(state: &mut ExtractionState, table_node: TsNode<'_>) {
        let Some(name) = Self::find_table_name(state, table_node) else {
            return;
        };
        let start_line = table_node.start_position().row as u32;
        let end_line = table_node.end_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        let module = Node {
            id: id.clone(),
            kind: NodeKind::Module,
            name: name.clone(),
            qualified_name: qualified_name.clone(),
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column: table_node.start_position().column as u32,
            end_column: table_node.end_position().column as u32,
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
        state.nodes.push(module);
        state.edges.push(Edge {
            source: state.file_node_id.clone(),
            target: id.clone(),
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });

        // Pairs are direct children of the table node, parented to it
        // with a qualified name nested under the table's qualified name.
        let mut cursor = table_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "pair" {
                    Self::emit_pair(state, child, &id, &qualified_name);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// The first key-shaped child of a `table` / `table_array_element`
    /// node holds its name (`bare_key`, `dotted_key`, or `quoted_key`).
    fn find_table_name(state: &ExtractionState, table_node: TsNode<'_>) -> Option<String> {
        let mut cursor = table_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if matches!(child.kind(), "bare_key" | "dotted_key" | "quoted_key") {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    fn emit_pair(
        state: &mut ExtractionState,
        pair_node: TsNode<'_>,
        parent_id: &str,
        parent_qn: &str,
    ) {
        let Some(key_node) = Self::first_key_child(pair_node) else {
            return;
        };
        let name = state.node_text(key_node);
        let start_line = pair_node.start_position().row as u32;
        let end_line = pair_node.end_position().row as u32;
        let qualified_name = format!("{parent_qn}::{name}");
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);

        let pair = Node {
            id: id.clone(),
            kind: NodeKind::Const,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column: pair_node.start_position().column as u32,
            end_column: pair_node.end_position().column as u32,
            signature: Some(
                state
                    .node_text(pair_node)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
            ),
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
        state.nodes.push(pair);
        state.edges.push(Edge {
            source: parent_id.to_string(),
            target: id,
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });
    }

    fn first_key_child(pair_node: TsNode<'_>) -> Option<TsNode<'_>> {
        let mut cursor = pair_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if matches!(child.kind(), "bare_key" | "dotted_key" | "quoted_key") {
                    return Some(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }
}

impl crate::extraction::LanguageExtractor for TomlExtractor {
    fn extensions(&self) -> &[&str] {
        &["toml"]
    }

    fn language_name(&self) -> &'static str {
        "TOML"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_toml(file_path, source)
    }
}
