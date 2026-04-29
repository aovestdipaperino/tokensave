/// Tree-sitter based Markdown source code extractor.
///
/// Parses Markdown source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

pub struct MarkdownExtractor;

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    file_path: String,
    source: Vec<u8>,
    timestamp: u64,
    node_stack: Vec<(String, String, usize)>,
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
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            timestamp,
            node_stack: Vec::new(),
        }
    }

    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }
}

impl MarkdownExtractor {
    pub fn extract_markdown(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let file_node = Node {
            id: generate_node_id(file_path, &NodeKind::File, file_path, 0),
            kind: NodeKind::File,
            name: file_path.to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            start_line: 0,
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
        state
            .node_stack
            .push((file_path.to_string(), file_node_id, 0));

        match Self::parse_source(source) {
            Ok(tree) => {
                let root = tree.root_node();
                Self::visit_children(&mut state, root);
            }
            Err(_msg) => {
                state.edges.push(Edge {
                    source: state.node_stack[0].1.clone(),
                    target: state.node_stack[0].1.clone(),
                    kind: EdgeKind::Contains,
                    line: Some(0),
                });
            }
        }

        state.node_stack.pop();

        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_markdown_fork::language())
            .map_err(|e| format!("failed to load markdown grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::visit_node(state, child);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        let kind = node.kind();
        match kind {
            "atx_heading" => {
                Self::visit_heading(state, node);
            }
            "link" => {
                Self::visit_link(state, node);
            }
            _ => {
                Self::visit_children(state, node);
            }
        }
    }

    fn visit_heading(state: &mut ExtractionState, node: TsNode<'_>) {
        let marker = node
            .children(&mut node.walk())
            .find(|n| n.kind().starts_with("atx_h") && n.kind().contains("_marker"));
        let level = marker
            .as_ref()
            .map_or(1, |m| {
                let kind = m.kind();
                let parts: Vec<&str> = kind.split('_').collect();
                if parts.len() >= 2 {
                    parts[1]
                        .trim_start_matches('h')
                        .parse::<usize>()
                        .unwrap_or(1)
                } else {
                    1
                }
            })
            .min(6);

        let title_node = node
            .children(&mut node.walk())
            .find(|n| n.kind() == "heading_content")
            .map(|n| state.node_text(n).trim().to_string())
            .unwrap_or_default();

        if title_node.is_empty() {
            return;
        }

        while state.node_stack.len() > 1 {
            let last_level = state.node_stack[state.node_stack.len() - 1].2;
            if last_level >= level {
                state.node_stack.pop();
            } else {
                break;
            }
        }

        let kind = NodeKind::Module;
        let parent_name = &state.node_stack[state.node_stack.len() - 1].0;
        let qualified_name = format!("{parent_name}::{title_node}");
        let id = generate_node_id(
            &state.file_path,
            &kind,
            &title_node,
            node.start_position().row as u32,
        );

        let node_obj = Node {
            id: id.clone(),
            kind,
            name: title_node.clone(),
            qualified_name: qualified_name.clone(),
            file_path: state.file_path.clone(),
            start_line: node.start_position().row as u32,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            signature: Some(format!("{} {}", "#".repeat(level), title_node)),
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

        if let Some((_, parent_id, _)) = state.node_stack.last() {
            state.edges.push(Edge {
                source: parent_id.clone(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(node.start_position().row as u32),
            });
        }

        state.nodes.push(node_obj);
        state.node_stack.push((title_node, id, level));
    }

    fn visit_link(state: &mut ExtractionState, node: TsNode<'_>) {
        let url_node = node
            .children(&mut node.walk())
            .find(|n| n.kind() == "link_destination");

        let Some(url_node) = url_node else {
            return;
        };

        let url = state.node_text(url_node);

        if url.starts_with("http://") || url.starts_with("https://") {
            return;
        }

        let target_path = url.trim_start_matches("file:");
        let target_ext = target_path.rsplit('.').next().unwrap_or("");

        if !is_code_extension(target_ext) {
            return;
        }

        let text_node = node
            .children(&mut node.walk())
            .find(|n| n.kind() == "link_text");
        let link_text = text_node.map_or_else(|| target_path.to_string(), |n| state.node_text(n));

        let target_id = generate_node_id(target_path, &NodeKind::Use, &link_text, 0);

        if let Some((_, parent_id, _)) = state.node_stack.last() {
            state.edges.push(Edge {
                source: parent_id.clone(),
                target: target_id,
                kind: EdgeKind::Uses,
                line: Some(node.start_position().row as u32),
            });
        }
    }
}

fn is_code_extension(ext: &str) -> bool {
    matches!(
        ext,
        "rs" | "py"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "kt"
            | "scala"
            | "R"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "ps1"
            | "ex"
            | "exs"
            | "erl"
            | "hrl"
            | "fs"
            | "fsx"
            | "ml"
            | "mli"
            | "hs"
            | "lhs"
            | "lua"
            | "pl"
            | "pm"
            | "t"
            | "nix"
            | "yaml"
            | "yml"
            | "toml"
            | "json"
            | "xml"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "md"
            | "markdown"
            | "sql"
            | "db"
            | "proto"
            | "v"
            | "vhd"
            | "vhdl"
            | "sage"
            | "sagews"
            | "ipynb"
    )
}

impl crate::extraction::LanguageExtractor for MarkdownExtractor {
    fn extensions(&self) -> &[&str] {
        &["md", "markdown"]
    }

    fn language_name(&self) -> &'static str {
        "Markdown"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_markdown(file_path, source)
    }
}
