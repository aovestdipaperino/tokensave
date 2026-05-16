//! Tree-sitter based Markdown source code extractor.
//!
//! Uses `tree-sitter-grammars/tree-sitter-markdown` (a split grammar):
//!
//!   * `block::LANGUAGE` parses block-level structure (sections, headings,
//!     paragraphs, lists). Inline content is left as opaque `(inline)` nodes.
//!   * `inline::LANGUAGE` is run over each `(inline)` node's byte range to
//!     produce links, emphasis, etc. We use `Parser::set_included_ranges`
//!     so the inline tree's byte/row positions stay in the original source.
//!
//! `atx_heading` / `setext_heading` nodes become `Module` nodes; `inline_link`
//! nodes whose destination is a project-local source file emit `Uses` edges.
//! Frontmatter (`(minus_metadata)`, `(plus_metadata)`) is skipped — the
//! grammar makes it opaque, so we don't recurse into it.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Range, Tree};

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
    /// One inline parser, lazily initialised, reused for every `(inline)`
    /// node we encounter to avoid re-creating it per heading/paragraph.
    inline_parser: Option<Parser>,
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
            inline_parser: None,
        }
    }

    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }

    /// Parse the byte range covered by `inline_node` with the inline grammar
    /// and return the resulting tree, or `None` if parsing fails. The tree's
    /// byte/row positions are anchored in the original source via
    /// `set_included_ranges`.
    fn parse_inline(&mut self, inline_node: TsNode<'_>) -> Option<Tree> {
        let parser = self.inline_parser.get_or_insert_with(|| {
            let mut p = Parser::new();
            let _ = p.set_language(&tokensave_large_treesitters::markdown::inline::LANGUAGE.into());
            p
        });
        let range = Range {
            start_byte: inline_node.start_byte(),
            end_byte: inline_node.end_byte(),
            start_point: inline_node.start_position(),
            end_point: inline_node.end_position(),
        };
        if parser.set_included_ranges(&[range]).is_err() {
            return None;
        }
        parser.parse(&self.source, None)
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
            parent_id: None,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state
            .node_stack
            .push((file_path.to_string(), file_node_id, 0));

        if let Ok(tree) = Self::parse(source) {
            let root = tree.root_node();
            Self::visit(&mut state, root);
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
        parser
            .set_language(&tokensave_large_treesitters::markdown::LANGUAGE.into())
            .map_err(|e| format!("failed to load markdown grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Walks the block tree. `atx_heading` / `setext_heading` produce `Module`
    /// nodes; `(inline)` nodes are re-parsed with the inline grammar to find
    /// links. Frontmatter (`(minus_metadata)`, `(plus_metadata)`) is opaque
    /// per the grammar — we never descend into it.
    fn visit(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "atx_heading" | "setext_heading" => Self::visit_heading(state, node),
            "minus_metadata" | "plus_metadata" => {
                // Opaque YAML/TOML frontmatter — don't descend.
            }
            "inline" => Self::visit_inline(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_heading(state: &mut ExtractionState, node: TsNode<'_>) {
        let level = Self::heading_level(node);

        // Block grammar exposes the heading text as the `heading_content`
        // field, which points to an `(inline)` node containing the text.
        let title = node
            .child_by_field_name("heading_content")
            .map(|n| state.node_text(n).trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            // Still recurse into the heading body so any `(inline)` children
            // (which would be unusual but not impossible) get their links.
            Self::visit_children(state, node);
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
        let qualified_name = format!("{parent_name}::{title}");
        let id = generate_node_id(
            &state.file_path,
            &kind,
            &title,
            node.start_position().row as u32,
        );

        let node_obj = Node {
            id: id.clone(),
            kind,
            name: title.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line: node.start_position().row as u32,
            attrs_start_line: node.start_position().row as u32,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            signature: Some(format!("{} {}", "#".repeat(level), title)),
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
            parent_id: None,
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
        state.node_stack.push((title, id, level));

        // Recurse so links inside the heading text (e.g.
        // `## See [main](src/main.rs)`) become `Uses` edges parented to
        // this heading.
        Self::visit_children(state, node);
    }

    /// Returns the ATX heading level (1-6) for `atx_heading` / `setext_heading`.
    /// ATX uses `atx_h{1..6}_marker` children; setext uses level-1 (`===`) or
    /// level-2 (`---`) underlines, identified by the marker child kind.
    fn heading_level(node: TsNode<'_>) -> usize {
        for child in node.children(&mut node.walk()) {
            let k = child.kind();
            if let Some(rest) = k.strip_prefix("atx_h") {
                if let Some(d) = rest.strip_suffix("_marker") {
                    if let Ok(n) = d.parse::<usize>() {
                        return n.clamp(1, 6);
                    }
                }
            }
            if k == "setext_h1_underline" {
                return 1;
            }
            if k == "setext_h2_underline" {
                return 2;
            }
        }
        1
    }

    /// Re-parse the `(inline)` node's byte range with the inline grammar and
    /// collect any `inline_link` nodes as `Uses` edges.
    fn visit_inline(state: &mut ExtractionState, inline_node: TsNode<'_>) {
        let Some(inline_tree) = state.parse_inline(inline_node) else {
            return;
        };
        let root = inline_tree.root_node();
        Self::collect_links(state, root);
    }

    fn collect_links(state: &mut ExtractionState, node: TsNode<'_>) {
        if node.kind() == "inline_link" || node.kind() == "image" {
            Self::visit_link(state, node);
            // Inline links can nest inside images and vice versa; keep walking.
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::collect_links(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_link(state: &mut ExtractionState, node: TsNode<'_>) {
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
                line: Some(node.start_position().row as u32),
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
