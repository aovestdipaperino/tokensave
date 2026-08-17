//! Pushing handler predicates into SQL — the easy half of #410.
//!
//! Eight MCP handlers called `get_all_nodes()` and filtered in Rust, so a
//! single tool call materialised the whole node table to keep a fraction of
//! it. Six of them filter by predicates the caller supplies — a path prefix,
//! a set of kinds, a visibility, a minimum span, a substring — all of which
//! belong in the query.
//!
//! Every test here asserts the SQL result is *identical* to the in-Rust
//! filter it replaces. That equivalence is the whole safety argument: these
//! handlers are user-visible, and a subtly different predicate would silently
//! change what `redundancy`, `module_api`, `unused_imports`, `gini`, `health`
//! and `literal_search` report.

use tempfile::tempdir;
use tokensave::db::NodeFilter;
use tokensave::tokensave::TokenSave;
use tokensave::types::{Node, NodeKind, Visibility};

/// A project with several files, kinds, visibilities and span lengths, so the
/// predicates below have something to discriminate between.
async fn indexed() -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src/inner")).unwrap();
    std::fs::create_dir_all(root.join("other")).unwrap();

    std::fs::write(
        root.join("src/lib.rs"),
        "pub mod inner;\npub fn exported() -> i32 { 1 }\nfn private_one() -> i32 { 2 }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/inner/deep.rs"),
        "use std::collections::HashMap;\npub fn deep_fn() -> i32 {\n    let mut m = HashMap::new();\n    m.insert(1, 2);\n    m.len() as i32\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("other/elsewhere.rs"),
        "pub fn elsewhere() -> i32 { 3 }\npub struct Marker;\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

fn ids(mut nodes: Vec<Node>) -> Vec<String> {
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    nodes.into_iter().map(|n| n.id).collect()
}

fn ids_ref(nodes: &[Node]) -> Vec<String> {
    let mut v: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    v.sort();
    v
}

/// Matches the prefix rule every handler wrote by hand: an exact path match,
/// or a directory prefix with a separator so `src` never matches `srcfoo`.
fn path_matches(file_path: &str, prefix: &str) -> bool {
    let with_slash = if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{prefix}/")
    };
    file_path == prefix || file_path.starts_with(&with_slash)
}

/// `handle_module_api`: public nodes under a path.
#[tokio::test]
async fn filter_matches_module_api_predicate() {
    let (_tmp, cg) = indexed().await;
    let all = cg.get_all_nodes().await.unwrap();

    let expected: Vec<Node> = all
        .iter()
        .filter(|n| n.visibility == Visibility::Pub && path_matches(&n.file_path, "src"))
        .cloned()
        .collect();

    let actual = cg
        .db()
        .get_nodes_filtered(&NodeFilter::new().path_prefix("src").public_only())
        .await
        .unwrap();

    assert!(
        !expected.is_empty(),
        "fixture must have public nodes in src"
    );
    assert_eq!(ids_ref(&expected), ids(actual));
}

/// `collect_candidates` (redundancy): executable nodes of at least N lines,
/// optionally under a path.
#[tokio::test]
async fn filter_matches_redundancy_candidate_predicate() {
    let (_tmp, cg) = indexed().await;
    let all = cg.get_all_nodes().await.unwrap();
    let kinds = [
        NodeKind::Function,
        NodeKind::Method,
        NodeKind::SingletonMethod,
    ];

    let expected: Vec<Node> = all
        .iter()
        .filter(|n| kinds.contains(&n.kind))
        .filter(|n| n.end_line.saturating_sub(n.start_line) + 1 >= 3)
        .cloned()
        .collect();

    let actual = cg
        .db()
        .get_nodes_filtered(&NodeFilter::new().kinds(&kinds).min_lines(3))
        .await
        .unwrap();

    assert!(
        !expected.is_empty(),
        "fixture must have a function of at least 3 lines"
    );
    assert_eq!(ids_ref(&expected), ids(actual));
}

/// `handle_unused_imports`: `Use` nodes under a path.
#[tokio::test]
async fn filter_matches_unused_imports_predicate() {
    let (_tmp, cg) = indexed().await;
    let all = cg.get_all_nodes().await.unwrap();

    let expected: Vec<Node> = all
        .iter()
        .filter(|n| n.kind == NodeKind::Use && path_matches(&n.file_path, "src"))
        .cloned()
        .collect();

    let actual = cg
        .db()
        .get_nodes_filtered(&NodeFilter::new().path_prefix("src").kinds(&[NodeKind::Use]))
        .await
        .unwrap();

    assert!(!expected.is_empty(), "fixture must have a `use` in src");
    assert_eq!(ids_ref(&expected), ids(actual));
}

/// `handle_literal_search`'s substring supplement, which is case-insensitive
/// over both `name` and `qualified_name`.
#[tokio::test]
async fn filter_matches_literal_search_substring_predicate() {
    let (_tmp, cg) = indexed().await;
    let all = cg.get_all_nodes().await.unwrap();
    let needle = "deep";

    let expected: Vec<Node> = all
        .iter()
        .filter(|n| {
            n.name.to_ascii_lowercase().contains(needle)
                || n.qualified_name.to_ascii_lowercase().contains(needle)
        })
        .cloned()
        .collect();

    let actual = cg
        .db()
        .get_nodes_filtered(&NodeFilter::new().name_contains(needle))
        .await
        .unwrap();

    assert!(!expected.is_empty(), "fixture must contain `deep`");
    assert_eq!(ids_ref(&expected), ids(actual));
}

/// The path filter used by `compute_health_snapshot` and `handle_gini`, and
/// the boundary case a hand-written `starts_with` gets wrong: `src` must not
/// match a sibling directory whose name merely starts with it.
#[tokio::test]
async fn path_prefix_does_not_match_a_sibling_with_the_same_stem() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("srcfoo")).unwrap();
    std::fs::write(root.join("src/a.rs"), "pub fn a() -> i32 { 1 }\n").unwrap();
    std::fs::write(root.join("srcfoo/b.rs"), "pub fn b() -> i32 { 2 }\n").unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    let scoped = cg
        .db()
        .get_nodes_filtered(&NodeFilter::new().path_prefix("src"))
        .await
        .unwrap();

    assert!(
        scoped.iter().any(|n| n.file_path.starts_with("src/")),
        "src/ must be included"
    );
    assert!(
        !scoped.iter().any(|n| n.file_path.starts_with("srcfoo")),
        "srcfoo must not be swept in by a prefix match: {:?}",
        scoped.iter().map(|n| &n.file_path).collect::<Vec<_>>()
    );
}

/// A path containing SQL `LIKE` wildcards must be matched literally, not
/// treated as a pattern. Without escaping, a directory named `a_b` would also
/// match `axb` — a silent wrong-results bug rather than an error.
#[tokio::test]
async fn path_prefix_treats_like_wildcards_literally() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("a_b")).unwrap();
    std::fs::create_dir_all(root.join("axb")).unwrap();
    std::fs::write(root.join("a_b/one.rs"), "pub fn one() -> i32 { 1 }\n").unwrap();
    std::fs::write(root.join("axb/two.rs"), "pub fn two() -> i32 { 2 }\n").unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    let scoped = cg
        .db()
        .get_nodes_filtered(&NodeFilter::new().path_prefix("a_b"))
        .await
        .unwrap();

    assert!(
        scoped.iter().any(|n| n.file_path.starts_with("a_b/")),
        "the literal directory must match"
    );
    assert!(
        !scoped.iter().any(|n| n.file_path.starts_with("axb")),
        "`_` must not act as a single-character wildcard: {:?}",
        scoped.iter().map(|n| &n.file_path).collect::<Vec<_>>()
    );
}

/// An empty filter is `get_all_nodes()`, so a handler that has nothing to
/// scope by keeps working rather than silently returning nothing.
#[tokio::test]
async fn an_empty_filter_returns_every_node() {
    let (_tmp, cg) = indexed().await;
    let all = cg.get_all_nodes().await.unwrap();
    let filtered = cg
        .db()
        .get_nodes_filtered(&NodeFilter::new())
        .await
        .unwrap();
    assert_eq!(ids_ref(&all), ids(filtered));
}

/// `handle_test_risk` needs a graph-wide `node_id → file_path` map, because
/// it walks every edge and an edge can point anywhere — a test in `tests/`
/// calling a function in `src/` is the point of the tool, so the map cannot
/// be scoped (#411).
///
/// What it does not need is the other twenty-six columns. `get_node_paths`
/// returns the mapping straight from SQL instead of materialising a `Node`
/// per row, each 248 bytes plus its unbounded `signature` and `docstring`,
/// to keep two short strings.
#[tokio::test]
async fn node_paths_matches_the_map_built_from_full_nodes() {
    let (_tmp, cg) = indexed().await;

    let expected: std::collections::HashMap<String, String> = cg
        .get_all_nodes()
        .await
        .unwrap()
        .into_iter()
        .map(|n| (n.id, n.file_path))
        .collect();

    let actual = cg.db().get_node_paths().await.unwrap();

    assert!(!expected.is_empty(), "fixture must have nodes");
    assert_eq!(
        expected, actual,
        "the projection must agree with the map built from full nodes"
    );
}
