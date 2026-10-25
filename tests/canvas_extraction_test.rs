//! Obsidian `.canvas` boards must reach the graph — #459.
//!
//! `.canvas` had no extractor, so every sync reported it as an unsupported
//! extension and its contents were invisible: a term written only on a canvas
//! was unfindable, and a note reachable only through canvas edges looked
//! orphaned even though the vault links it.
//!
//! These tests pin the three things the reporter asked for — text cards
//! searchable, `file` cards becoming references, and edge structure preserved
//! — plus the malformed-input behaviour, since a canvas is user-authored JSON
//! and a broken one must not take the file back out of the index.

#![cfg(feature = "lang-canvas")]

use tokensave::extraction::CanvasExtractor;
use tokensave::types::{EdgeKind, NodeKind};

/// A board shaped like a real architecture map: two text cards, a group, a
/// card pointing at a vault note, an external link, and a labelled edge.
const BOARD: &str = r##"{
  "nodes": [
    {"id": "aaa1", "type": "text", "text": "# Ingest pipeline\nReads from the queue.", "x": 0, "y": 0, "width": 400, "height": 200},
    {"id": "bbb2", "type": "text", "text": "no heading here, just prose about backpressure", "x": 0, "y": 300, "width": 400, "height": 200},
    {"id": "ccc3", "type": "file", "file": "Notes/Runbook.md", "x": 500, "y": 0, "width": 400, "height": 200},
    {"id": "ddd4", "type": "link", "url": "https://example.com/spec", "x": 500, "y": 300, "width": 400, "height": 200},
    {"id": "eee5", "type": "group", "label": "Phase two", "x": -50, "y": -50, "width": 900, "height": 700}
  ],
  "edges": [
    {"id": "edge1", "fromNode": "aaa1", "toNode": "bbb2", "label": "feeds"},
    {"id": "edge2", "fromNode": "aaa1", "toNode": "ccc3", "label": "documented in"}
  ]
}"##;

/// The reported symptom: a term written only on a canvas is unfindable.
/// Both text cards must become named, searchable nodes, and the full card
/// text must survive so a term from the body is reachable too.
#[test]
fn text_cards_become_searchable_nodes() {
    let result = CanvasExtractor::extract_canvas("Boards/Arch.canvas", BOARD);

    let titles: Vec<&str> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .map(|n| n.name.as_str())
        .collect();

    assert!(
        titles.contains(&"Ingest pipeline"),
        "a card opening with a heading is named by it, got {titles:?}"
    );
    assert!(
        titles.contains(&"no heading here, just prose about backpressure"),
        "a card with no heading falls back to its first line, got {titles:?}"
    );
    assert!(
        titles.contains(&"Phase two"),
        "a group label is authored text and must be indexed, got {titles:?}"
    );

    // The body, not just the title — this is what makes an arbitrary term on
    // a card findable.
    let ingest = result
        .nodes
        .iter()
        .find(|n| n.name == "Ingest pipeline")
        .expect("ingest card");
    assert!(
        ingest
            .docstring
            .as_deref()
            .is_some_and(|d| d.contains("Reads from the queue")),
        "the card's full markdown must be carried, got {:?}",
        ingest.docstring
    );
}

/// The piece that would otherwise be lost: a `file` card is a reference to
/// another vault note. Deliberately not filtered to code extensions the way
/// Markdown links are — a canvas card almost always points at a `.md` note,
/// and those are exactly the edges that made a note look orphaned.
#[test]
fn file_cards_become_references_to_the_target_note() {
    let result = CanvasExtractor::extract_canvas("Boards/Arch.canvas", BOARD);

    let expected = tokensave::types::generate_node_id(
        "Notes/Runbook.md",
        &NodeKind::File,
        "Notes/Runbook.md",
        0,
    );
    assert!(
        result
            .edges
            .iter()
            .any(|e| e.target == expected && e.kind == EdgeKind::Uses),
        "the .md target must get a Uses edge"
    );
}

/// `edges[]` carries the board's structure. Only edges whose both endpoints
/// produced a node are kept: an edge into a `file` card is already covered by
/// that card's own reference, and emitting it twice would double-count.
#[test]
fn card_to_card_edges_are_kept_and_file_edges_are_not_double_counted() {
    let result = CanvasExtractor::extract_canvas("Boards/Arch.canvas", BOARD);

    let ingest = result
        .nodes
        .iter()
        .find(|n| n.name == "Ingest pipeline")
        .expect("ingest card");
    let prose = result
        .nodes
        .iter()
        .find(|n| n.name.starts_with("no heading"))
        .expect("prose card");

    assert!(
        result
            .edges
            .iter()
            .any(|e| e.source == ingest.id && e.target == prose.id && e.kind == EdgeKind::Uses),
        "a card-to-card edge must reach the graph"
    );

    // `edge2` points at the `file` card, which has no node of its own.
    let from_ingest = result
        .edges
        .iter()
        .filter(|e| e.source == ingest.id && e.kind == EdgeKind::Uses)
        .count();
    assert_eq!(
        from_ingest, 1,
        "the edge into the file card must not be emitted a second time"
    );
}

/// An external link has nothing in the graph to resolve against.
#[test]
fn link_cards_produce_nothing() {
    let result = CanvasExtractor::extract_canvas("Boards/Arch.canvas", BOARD);
    assert!(
        !result.nodes.iter().any(|n| n.name.contains("example.com")),
        "external link cards are not indexed"
    );
}

/// A canvas is user-authored JSON. A broken one must still leave the file in
/// the index — reporting it with an error beats having it vanish the way an
/// unsupported extension used to.
#[test]
fn malformed_json_still_yields_the_file_node() {
    let result = CanvasExtractor::extract_canvas("Boards/Broken.canvas", "{ not json");

    assert_eq!(
        result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::File)
            .count(),
        1,
        "the file node survives a parse failure"
    );
    assert_eq!(result.errors.len(), 1, "and the failure is reported");
    assert!(result.errors[0].contains("Boards/Broken.canvas"));
}

/// An empty board is legal, not an error.
#[test]
fn an_empty_canvas_is_not_an_error() {
    let result =
        CanvasExtractor::extract_canvas("Boards/Empty.canvas", r#"{"nodes":[],"edges":[]}"#);
    assert!(result.errors.is_empty());
    assert_eq!(result.nodes.len(), 1, "just the file node");
}

/// A card whose text is only whitespace would produce an empty node name,
/// which `generate_node_id` asserts against in debug builds.
#[test]
fn a_blank_card_is_skipped_rather_than_named_empty() {
    let board = r#"{"nodes":[{"id":"x1","type":"text","text":"   \n\n  "}],"edges":[]}"#;
    let result = CanvasExtractor::extract_canvas("Boards/Blank.canvas", board);
    assert_eq!(
        result.nodes.len(),
        1,
        "only the file node, got {:?}",
        result.nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
}
