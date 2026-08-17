//! Resolution must not pay for columns it never reads — #306.
//!
//! `get_all_nodes()` selects every column, including `docstring` and
//! `signature`. Both are unbounded TEXT: #362 found a single `const` whose
//! signature was roughly 43 KB, because an extractor stores a const's whole
//! initializer there. The resolver reads neither — only `id`, `kind`, `name`,
//! `qualified_name`, `file_path`, `start_line`, `visibility` and `parent_id` —
//! yet the full `Vec<Node>` stays resident for the entire resolution pass,
//! which is the graph-sized peak #306 is about.
//!
//! The resolver borrows from that slice for its whole lifetime and needs a
//! global name index to resolve cross-file references, so it cannot be
//! chunked or streamed without a redesign. Dropping the two columns it never
//! looks at is the tractable win, and this test is the guarantee that comes
//! with it: the slim load must resolve *identically*, so nobody has to trust
//! a comment about which fields matter.

use tempfile::tempdir;
use tokensave::resolution::ReferenceResolver;
use tokensave::tokensave::TokenSave;

/// A project with enough cross-file structure that resolution has real work
/// to do: a trait, an impl, a caller in another file, and a re-export.
async fn indexed_project() -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("src/lib.rs"),
        "pub mod shapes;\npub mod render;\npub use shapes::Circle;\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/shapes.rs"),
        r#"
pub trait Shape {
    fn area(&self) -> f64;
}

/// A circle, documented at length so the docstring column is non-trivial.
pub struct Circle {
    pub radius: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

pub const LOOKUP: [(&str, f64); 3] = [("unit", 1.0), ("double", 2.0), ("triple", 3.0)];
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/render.rs"),
        r#"
use crate::shapes::{Circle, Shape};

pub fn describe(c: &Circle) -> String {
    let a = c.area();
    format!("area {a}")
}

pub fn total(shapes: &[Circle]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}
"#,
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

/// The behavioural contract: resolving from the slim load produces exactly
/// the same edges as resolving from the full load. If a future change makes
/// the resolver read `signature` or `docstring`, this fails rather than
/// silently resolving against `None`.
#[tokio::test]
async fn slim_node_load_resolves_identically_to_the_full_load() {
    let (_tmp, cg) = indexed_project().await;
    let db = cg.db();

    let unresolved = db.get_unresolved_refs().await.unwrap();

    let full = db.get_all_nodes().await.unwrap();
    let slim = db.get_all_nodes_for_resolution().await.unwrap();
    assert_eq!(
        full.len(),
        slim.len(),
        "the slim load must cover every node, not a subset"
    );
    assert!(
        !full.is_empty(),
        "precondition: the project must have nodes"
    );

    let mut edges_full = {
        let r = ReferenceResolver::from_nodes(db, &full);
        r.create_edges(&r.resolve_all(&unresolved).resolved)
    };
    let mut edges_slim = {
        let r = ReferenceResolver::from_nodes(db, &slim);
        r.create_edges(&r.resolve_all(&unresolved).resolved)
    };

    let key = |e: &tokensave::types::Edge| {
        (
            e.source.clone(),
            e.target.clone(),
            format!("{:?}", e.kind),
            e.line,
        )
    };
    edges_full.sort_by_key(key);
    edges_slim.sort_by_key(key);

    assert_eq!(
        edges_full.iter().map(key).collect::<Vec<_>>(),
        edges_slim.iter().map(key).collect::<Vec<_>>(),
        "slim and full loads must resolve to identical edges"
    );
}

/// The point of the change: the two unbounded columns are not loaded. Asserted
/// directly, because "identical edges" would also hold if the slim load
/// quietly went back to selecting everything.
#[tokio::test]
async fn slim_node_load_omits_the_unbounded_text_columns() {
    let (_tmp, cg) = indexed_project().await;
    let db = cg.db();

    let full = db.get_all_nodes().await.unwrap();
    assert!(
        full.iter().any(|n| n.signature.is_some()),
        "precondition: the fixture must have at least one signature to omit"
    );

    for node in db.get_all_nodes_for_resolution().await.unwrap() {
        assert!(
            node.signature.is_none(),
            "{}: signature must not be loaded for resolution",
            node.qualified_name
        );
        assert!(
            node.docstring.is_none(),
            "{}: docstring must not be loaded for resolution",
            node.qualified_name
        );
    }
}

/// The fields resolution *does* read must survive the slim load intact —
/// otherwise the identical-edges test above could pass for a project whose
/// references all happen to be unresolvable.
#[tokio::test]
async fn slim_node_load_preserves_every_field_resolution_reads() {
    let (_tmp, cg) = indexed_project().await;
    let db = cg.db();

    let full = db.get_all_nodes().await.unwrap();
    let slim = db.get_all_nodes_for_resolution().await.unwrap();

    let mut full_keyed: Vec<_> = full.iter().collect();
    let mut slim_keyed: Vec<_> = slim.iter().collect();
    full_keyed.sort_by(|a, b| a.id.cmp(&b.id));
    slim_keyed.sort_by(|a, b| a.id.cmp(&b.id));

    for (f, s) in full_keyed.iter().zip(slim_keyed.iter()) {
        assert_eq!(f.id, s.id);
        assert_eq!(f.kind, s.kind, "{}: kind", f.id);
        assert_eq!(f.name, s.name, "{}: name", f.id);
        assert_eq!(
            f.qualified_name, s.qualified_name,
            "{}: qualified_name",
            f.id
        );
        assert_eq!(f.file_path, s.file_path, "{}: file_path", f.id);
        assert_eq!(f.start_line, s.start_line, "{}: start_line", f.id);
        assert_eq!(f.visibility, s.visibility, "{}: visibility", f.id);
        assert_eq!(f.parent_id, s.parent_id, "{}: parent_id", f.id);
    }
}
