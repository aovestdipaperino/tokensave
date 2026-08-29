// Rust guideline compliant 2026-08-29
//! Obsidian Canvas (`.canvas`) extractor (#459).
//!
//! A `.canvas` file is the [JSON Canvas](https://jsoncanvas.org) format
//! Obsidian uses for its node-graph boards: a `nodes[]` array of cards and a
//! `edges[]` array of labelled connections between them. Because it is plain
//! JSON with a documented schema, this is a hand-rolled `serde_json` reader
//! rather than a tree-sitter grammar.
//!
//! Before this extractor existed the whole file was skipped, so a term written
//! only on a canvas was unfindable and a note reachable only through canvas
//! edges looked orphaned — even though the vault itself links it.
//!
//! Emitted per file:
//!   * a `File` root node;
//!   * one `Module` node per `type: "text"` card, named from its first
//!     heading or first non-empty line, carrying the card's full Markdown as
//!     its docstring — the same kind the Markdown extractor gives a heading,
//!     so a card and a note section behave alike in search;
//!   * one `Module` node per `type: "group"` that carries a label, since a
//!     group label is authored text like any other;
//!   * a `Uses` edge to the target `File` for every `type: "file"` card, and
//!     a `Uses` edge between cards for every entry in `edges[]`, so canvas
//!     structure reaches the graph rather than being dropped.
//!
//! `type: "link"` cards are skipped: they point at external URLs, which the
//! graph has nothing to resolve them against.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

/// Longest card title kept as a node name; the full text lives in the
/// docstring, so this only bounds what shows up in a symbol listing.
const MAX_TITLE_LEN: usize = 80;

/// Extracts graph nodes and edges from Obsidian `.canvas` boards.
pub struct CanvasExtractor;

impl CanvasExtractor {
    /// Extract nodes and edges from a `.canvas` file.
    ///
    /// A file that is not valid JSON, or whose top level is not an object,
    /// yields the `File` node alone plus one error. Reporting the file but
    /// none of its contents is deliberate: a malformed canvas should still be
    /// visible to `tokensave_files` rather than vanishing the way an
    /// unsupported extension used to.
    pub fn extract_canvas(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut errors = Vec::new();
        let end_line = source.lines().count().saturating_sub(1) as u32;

        let file_node_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        nodes.push(Self::make_node(
            file_node_id.clone(),
            NodeKind::File,
            file_path.to_string(),
            file_path,
            0,
            end_line,
            None,
            None,
            timestamp,
        ));

        let parsed: Value = match serde_json::from_str(source) {
            Ok(value) => value,
            Err(e) => {
                errors.push(format!("{file_path}: canvas is not valid JSON: {e}"));
                return Self::finish(nodes, edges, errors, start);
            }
        };
        let Some(canvas_nodes) = parsed.get("nodes").and_then(Value::as_array) else {
            // A canvas with no cards is legal and empty, not an error.
            return Self::finish(nodes, edges, errors, start);
        };

        // Canvas node id -> the graph node id it became, so `edges[]` can be
        // rewritten into graph edges. Cards that produce no node (a `file` or
        // `link` card) are absent, and an edge touching one is skipped.
        let mut card_ids: Vec<(String, String)> = Vec::new();

        for card in canvas_nodes {
            let Some(card_id) = card.get("id").and_then(Value::as_str) else {
                continue;
            };
            let line = Self::line_of_card(source, card_id);
            match card.get("type").and_then(Value::as_str) {
                Some("text") => {
                    let text = card.get("text").and_then(Value::as_str).unwrap_or("");
                    let Some(title) = Self::card_title(text) else {
                        continue;
                    };
                    let id = generate_node_id(file_path, &NodeKind::Module, &title, line);
                    nodes.push(Self::make_node(
                        id.clone(),
                        NodeKind::Module,
                        title,
                        file_path,
                        line,
                        line,
                        Some("canvas text card".to_string()),
                        Some(text.to_string()),
                        timestamp,
                    ));
                    edges.push(Edge {
                        source: file_node_id.clone(),
                        target: id.clone(),
                        kind: EdgeKind::Contains,
                        line: Some(line),
                    });
                    card_ids.push((card_id.to_string(), id));
                }
                Some("group") => {
                    let Some(title) = card
                        .get("label")
                        .and_then(Value::as_str)
                        .and_then(Self::card_title)
                    else {
                        continue;
                    };
                    let id = generate_node_id(file_path, &NodeKind::Module, &title, line);
                    nodes.push(Self::make_node(
                        id.clone(),
                        NodeKind::Module,
                        title,
                        file_path,
                        line,
                        line,
                        Some("canvas group".to_string()),
                        None,
                        timestamp,
                    ));
                    edges.push(Edge {
                        source: file_node_id.clone(),
                        target: id.clone(),
                        kind: EdgeKind::Contains,
                        line: Some(line),
                    });
                    card_ids.push((card_id.to_string(), id));
                }
                Some("file") => {
                    // The reference this issue exists for. Unlike the Markdown
                    // extractor's link handling, the target is NOT filtered to
                    // code extensions: a canvas card almost always points at
                    // another `.md` note, and those are exactly the edges that
                    // made a linked note look orphaned.
                    let Some(target) = card
                        .get("file")
                        .and_then(Value::as_str)
                        .filter(|t| !t.is_empty())
                    else {
                        continue;
                    };
                    let target_id = generate_node_id(target, &NodeKind::File, target, 0);
                    edges.push(Edge {
                        source: file_node_id.clone(),
                        target: target_id,
                        kind: EdgeKind::Uses,
                        line: Some(line),
                    });
                }
                // `link` cards point outside the vault; anything else is a
                // schema version this build does not know.
                _ => {}
            }
        }

        Self::append_card_edges(&parsed, source, &card_ids, &mut edges);
        Self::finish(nodes, edges, errors, start)
    }

    /// Turn `edges[]` into graph edges between the cards they connect.
    ///
    /// Only edges whose *both* endpoints produced a node are kept — an edge
    /// into a `file` card is already represented by that card's own `Uses`
    /// edge to the target file, and duplicating it here would double-count.
    fn append_card_edges(
        parsed: &Value,
        source: &str,
        card_ids: &[(String, String)],
        edges: &mut Vec<Edge>,
    ) {
        let Some(canvas_edges) = parsed.get("edges").and_then(Value::as_array) else {
            return;
        };
        let lookup = |canvas_id: &str| -> Option<&str> {
            card_ids
                .iter()
                .find(|(cid, _)| cid == canvas_id)
                .map(|(_, node_id)| node_id.as_str())
        };
        for edge in canvas_edges {
            let (Some(from), Some(to)) = (
                edge.get("fromNode").and_then(Value::as_str),
                edge.get("toNode").and_then(Value::as_str),
            ) else {
                continue;
            };
            let (Some(source_id), Some(target_id)) = (lookup(from), lookup(to)) else {
                continue;
            };
            let line = edge
                .get("id")
                .and_then(Value::as_str)
                .map_or(0, |id| Self::line_of_card(source, id));
            edges.push(Edge {
                source: source_id.to_string(),
                target: target_id.to_string(),
                kind: EdgeKind::Uses,
                line: Some(line),
            });
        }
    }

    /// A display title for a card: its first Markdown heading if it opens with
    /// one, otherwise its first non-empty line, trimmed of heading marks and
    /// bounded by [`MAX_TITLE_LEN`].
    ///
    /// `None` when the card holds nothing but whitespace — a node with an
    /// empty name would be stripped later anyway, and `generate_node_id`
    /// asserts against it in debug builds.
    fn card_title(text: &str) -> Option<String> {
        let first = text
            .lines()
            .map(|l| l.trim().trim_start_matches('#').trim())
            .find(|l| !l.is_empty())?;
        let mut title: String = first.chars().take(MAX_TITLE_LEN).collect();
        if first.chars().count() > MAX_TITLE_LEN {
            title.push('…');
        }
        Some(title)
    }

    /// Line on which a canvas card's `id` appears in the source.
    ///
    /// `serde_json` does not carry spans, and a canvas is as likely to be
    /// minified onto one line as pretty-printed, so this locates the quoted id
    /// textually. Ids are random and unique within a file, making a substring
    /// hit reliable; a miss falls back to line 0 rather than guessing.
    fn line_of_card(source: &str, card_id: &str) -> u32 {
        let needle = format!("\"{card_id}\"");
        source.find(&needle).map_or(0, |byte| {
            source[..byte].bytes().filter(|c| *c == b'\n').count() as u32
        })
    }

    fn finish(
        nodes: Vec<Node>,
        edges: Vec<Edge>,
        errors: Vec<String>,
        start: Instant,
    ) -> ExtractionResult {
        ExtractionResult {
            nodes,
            edges,
            unresolved_refs: Vec::new(),
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_node(
        id: String,
        kind: NodeKind,
        name: String,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        signature: Option<String>,
        docstring: Option<String>,
        timestamp: u64,
    ) -> Node {
        Node {
            id,
            kind,
            name: name.clone(),
            qualified_name: name,
            file_path: file_path.to_string(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column: 0,
            end_column: 0,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: timestamp,
            parent_id: None,
        }
    }
}

impl crate::extraction::LanguageExtractor for CanvasExtractor {
    fn extensions(&self) -> &[&str] {
        &["canvas"]
    }

    fn language_name(&self) -> &'static str {
        "Canvas"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_canvas(file_path, source)
    }
}
