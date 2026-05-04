/// Tree-sitter based Markdown source code extractor.
///
/// Markdown is parsed with two cooperating tree-sitter grammars:
/// - the **block** grammar (`tree_sitter_md::LANGUAGE`) for document
///   structure (headings, paragraphs, code blocks);
/// - the **inline** grammar (`tree_sitter_md::INLINE_LANGUAGE`) for the
///   contents of paragraphs and headings (links, emphasis, code spans).
///
/// We walk the block tree, recurse into `inline` content nodes by re-parsing
/// their text with the inline grammar, then look for `inline_link` nodes
/// to emit `Uses` edges to referenced source files.
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
    /// (heading title, node id, level) — heading levels strictly increase
    /// going *down* the stack. Headings of equal or shallower level pop
    /// the stack so we always parent to the nearest ancestor heading.
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
        state
            .node_stack
            .push((file_path.to_string(), file_node_id, 0));

        let mut inline_parser = match Self::make_inline_parser() {
            Ok(p) => p,
            Err(_) => {
                state.node_stack.pop();
                return ExtractionResult {
                    nodes: state.nodes,
                    edges: state.edges,
                    unresolved_refs: Vec::new(),
                    errors: Vec::new(),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        match Self::parse_block(source) {
            Ok(tree) => {
                let root = tree.root_node();
                Self::visit_block(&mut state, root, &mut inline_parser);
            }
            Err(_msg) => {
                // Parse failed; skip extraction rather than creating a self-loop.
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

    fn parse_block(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&tokensave_large_treesitters::markdown::LANGUAGE.into())
            .map_err(|e| format!("failed to load markdown block grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter block parse returned None".to_string())
    }

    fn make_inline_parser() -> Result<Parser, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&tokensave_large_treesitters::markdown::INLINE_LANGUAGE.into())
            .map_err(|e| format!("failed to load markdown inline grammar: {e}"))?;
        Ok(parser)
    }

    /// Walks the block tree. Headings produce `Module` nodes; `inline`
    /// content nodes are re-parsed with the inline grammar to find links.
    fn visit_block(state: &mut ExtractionState, node: TsNode<'_>, inline_parser: &mut Parser) {
        let kind = node.kind();
        match kind {
            "atx_heading" => Self::visit_heading(state, node),
            // TODO: setext_heading (H1\n===, H2\n---).
            "inline" => Self::visit_inline_content(state, node, inline_parser),
            _ => Self::visit_block_children(state, node, inline_parser),
        }
    }

    fn visit_block_children(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        inline_parser: &mut Parser,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit_block(state, cursor.node(), inline_parser);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Re-parses an `inline` content node with the inline grammar and walks
    /// the result for `inline_link` nodes. Line numbers from the inline tree
    /// are local to the inline source — we add the block-tree row offset so
    /// emitted edges point back to the correct line in the original file.
    fn visit_inline_content(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        inline_parser: &mut Parser,
    ) {
        let text = state.node_text(node);
        let Some(tree) = inline_parser.parse(&text, None) else {
            return;
        };
        let row_offset = node.start_position().row as u32;
        Self::walk_inline(state, tree.root_node(), row_offset);
    }

    fn walk_inline(state: &mut ExtractionState, node: TsNode<'_>, row_offset: u32) {
        if node.kind() == "inline_link" {
            Self::visit_inline_link(state, node, row_offset);
            return;
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::walk_inline(state, cursor.node(), row_offset);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_heading(state: &mut ExtractionState, node: TsNode<'_>) {
        // Count `#` characters in the leading marker — robust against
        // grammar versions that name the marker `atx_h{1..6}_marker`.
        let level = node
            .children(&mut node.walk())
            .find(|n| n.kind().starts_with("atx_h") && n.kind().ends_with("_marker"))
            .map_or(1, |m| {
                state.node_text(m).chars().filter(|c| *c == '#').count()
            })
            .clamp(1, 6);

        // The heading title is in the `inline` child (tree-sitter-md 0.5).
        let title_node = node
            .children(&mut node.walk())
            .find(|n| n.kind() == "inline")
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
            attrs_start_line: node.start_position().row as u32,
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

    /// Emits a `Uses` edge for an inline link whose destination references a
    /// source file. External (`http(s)://`) and non-code-extension links are
    /// skipped to avoid low-signal edges.
    fn visit_inline_link(state: &mut ExtractionState, node: TsNode<'_>, row_offset: u32) {
        let Some(url_node) = node
            .children(&mut node.walk())
            .find(|n| n.kind() == "link_destination")
        else {
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

        let target_id = generate_node_id(target_path, &NodeKind::File, target_path, 0);

        if let Some((_, parent_id, _)) = state.node_stack.last() {
            state.edges.push(Edge {
                source: parent_id.clone(),
                target: target_id,
                kind: EdgeKind::Uses,
                // The inline tree uses local rows; add the offset of the
                // host `inline` node to map back to the original file.
                line: Some(node.start_position().row as u32 + row_offset),
            });
        }
    }
}

fn is_code_extension(ext: &str) -> bool {
    // Only include actual programming-language source files.
    // Config (yaml, toml, json), markup (html, css, markdown), and
    // notebook (ipynb) files are excluded to avoid low-signal edges.
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
            | "sql"
            | "proto"
            | "v"
            | "vhd"
            | "vhdl"
            | "sage"
            | "sagews"
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
