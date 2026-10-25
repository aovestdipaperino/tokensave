//! `LIKE` escaping in every query that scopes by a caller's path: #524 and the
//! sweep after it.
//!
//! #524: `coupling`, `inheritance_depth` and `god_class` interpolated the
//! caller's path straight into the SQL string. Ten more queries bound it as a
//! parameter, `format!("{prefix}%")`, which closes the apostrophe but leaves
//! `_` and `%` live as `LIKE` wildcards. The wildcard consequences were silent
//! either way: a filter for `src/my_mod` also matched `src/myXmod` and `src/%`
//! matched everything. In the interpolated three, a path containing `'` also
//! closed the literal and failed the query outright.
//!
//! Binding a parameter fixes only the apostrophe, so each case below is
//! asserted separately against every one of these queries.

use serde_json::json;
use tempfile::tempdir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

/// Two sibling directories whose names differ only where `LIKE` would treat
/// `_` as a wildcard, plus one holding an apostrophe. Each carries a small
/// class hierarchy with a cross-file reference, a recursive function, and
/// annotated Rust and Java code, so every query under test has rows to return.
async fn indexed() -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    for dir in ["src/my_mod", "src/myXmod", "src/it's"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
        std::fs::write(
            root.join(dir).join("base.py"),
            "class Base:\n    def run(self):\n        return 1\n",
        )
        .unwrap();
        std::fs::write(
            root.join(dir).join("leaf.py"),
            "from .base import Base\n\n\nclass Middle(Base):\n    def run(self):\n        return Base.run(self)\n\n\nclass Leaf(Middle):\n    def go(self):\n        return self.run()\n",
        )
        .unwrap();
        std::fs::write(
            root.join(dir).join("walk.py"),
            "def walk(n):\n    if n <= 0:\n        return 0\n    return walk(n - 1)\n",
        )
        .unwrap();
        std::fs::write(
            root.join(dir).join("point.rs"),
            "#[inline]\npub fn fact(n: u64) -> u64 {\n    if n == 0 { 1 } else { n * fact(n - 1) }\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join(dir).join("Svc.java"),
            "package demo;\n\npublic class Svc {\n    @Deprecated\n    public void old() {\n    }\n}\n",
        )
        .unwrap();
    }

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

/// The file each call edge starts in. The two call-edge queries return node
/// ids rather than paths, and they scope on the source node.
async fn source_paths(cg: &TokenSave, ids: Vec<String>) -> Vec<String> {
    cg.db()
        .get_nodes_by_ids(&ids)
        .await
        .unwrap()
        .into_iter()
        .map(|node| node.file_path)
        .collect()
}

/// Every path each scoped query returns for `path_prefix`, as plain strings.
async fn scoped_paths(cg: &TokenSave, prefix: &str) -> Vec<(&'static str, Vec<String>)> {
    let coupling = cg
        .db()
        .get_file_coupling(false, Some(prefix), 100)
        .await
        .unwrap()
        .into_iter()
        .map(|(path, _)| path)
        .collect();

    let inheritance = cg
        .db()
        .get_inheritance_depth(Some(prefix), 100)
        .await
        .unwrap()
        .into_iter()
        .map(|(node, _)| node.file_path)
        .collect();

    let god = cg
        .db()
        .get_god_classes(Some(prefix), 100)
        .await
        .unwrap()
        .into_iter()
        .map(|(node, _, _, _)| node.file_path)
        .collect();

    let rank = cg
        .db()
        .get_ranked_nodes_by_edge_kind(&EdgeKind::Calls, None, true, Some(prefix), 100)
        .await
        .unwrap()
        .into_iter()
        .map(|(node, _)| node.file_path)
        .collect();

    let largest = cg
        .db()
        .get_largest_nodes(None, Some(prefix), 100)
        .await
        .unwrap()
        .into_iter()
        .map(|(node, _)| node.file_path)
        .collect();

    let distribution = cg
        .db()
        .get_node_distribution(Some(prefix))
        .await
        .unwrap()
        .into_iter()
        .map(|(path, _, _)| path)
        .collect();

    let call_edge_sources = cg
        .db()
        .get_call_edges(Some(prefix))
        .await
        .unwrap()
        .into_iter()
        .map(|(source, _)| source)
        .collect();
    let call_edges = source_paths(cg, call_edge_sources).await;

    let lined_call_edge_sources = cg
        .db()
        .get_call_edges_with_lines(Some(prefix))
        .await
        .unwrap()
        .into_iter()
        .map(|(source, _, _)| source)
        .collect();
    let call_edges_with_lines = source_paths(cg, lined_call_edge_sources).await;

    let complexity = cg
        .db()
        .get_complexity_ranked(None, Some(prefix), 100)
        .await
        .unwrap()
        .into_iter()
        .map(|(node, _, _, _, _)| node.file_path)
        .collect();

    let undocumented = cg
        .db()
        .get_undocumented_public_symbols(Some(prefix), 100)
        .await
        .unwrap()
        .into_iter()
        .map(|node| node.file_path)
        .collect();

    let annotation_sites = cg
        .db()
        .get_annotation_sites(None, Some(prefix), None, 100)
        .await
        .unwrap()
        .into_iter()
        .map(|site| {
            site["target"]["file"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();

    let nodes_by_dir = cg
        .db()
        .get_nodes_by_dir(
            prefix,
            &[NodeKind::Function, NodeKind::Method, NodeKind::Class],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|node| node.file_path)
        .collect();

    vec![
        ("coupling", coupling),
        ("inheritance_depth", inheritance),
        ("god_class", god),
        ("rank", rank),
        ("largest", largest),
        ("distribution", distribution),
        ("call_edges", call_edges),
        ("call_edges_with_lines", call_edges_with_lines),
        ("complexity", complexity),
        ("doc_coverage", undocumented),
        ("annotation_sites", annotation_sites),
        ("nodes_by_dir", nodes_by_dir),
    ]
}

/// The control: an ordinary path with no `LIKE` metacharacter scopes to its
/// own directory. If this fails, the fixture is wrong rather than the filter.
#[tokio::test]
async fn an_ordinary_path_scopes_to_its_own_directory() {
    let (_tmp, cg) = indexed().await;

    for (query, paths) in scoped_paths(&cg, "src/myXmod").await {
        assert!(
            !paths.is_empty(),
            "{query} returned nothing for the control path — fixture produces no rows"
        );
        assert!(
            paths.iter().all(|p| p.starts_with("src/myXmod/")),
            "{query} leaked outside the requested directory: {paths:?}"
        );
    }
}

/// `_` must match a literal underscore, not any single character. Before the
/// fix, `src/my_mod` also returned `src/myXmod`.
#[tokio::test]
async fn underscore_in_a_path_is_not_a_wildcard() {
    let (_tmp, cg) = indexed().await;

    for (query, paths) in scoped_paths(&cg, "src/my_mod").await {
        assert!(
            !paths.is_empty(),
            "{query} returned nothing for the literal directory"
        );
        assert!(
            !paths.iter().any(|p| p.starts_with("src/myXmod")),
            "{query} treated `_` as a single-character wildcard: {paths:?}"
        );
    }
}

/// `%` must match a literal percent sign. Before the fix, `src/%` returned
/// every directory in the project.
#[tokio::test]
async fn percent_in_a_path_is_not_a_wildcard() {
    let (_tmp, cg) = indexed().await;

    for (query, paths) in scoped_paths(&cg, "src/%").await {
        assert!(
            paths.is_empty(),
            "{query} treated `%` as a wildcard and returned every directory: {paths:?}"
        );
    }
}

/// A path names a file or a directory, never a leading fragment of one. The
/// filter used to be a plain string prefix, so `src/my` also matched
/// `src/my_mod` and `src/myXmod`.
#[tokio::test]
async fn a_partial_directory_name_matches_nothing() {
    let (_tmp, cg) = indexed().await;

    for (query, paths) in scoped_paths(&cg, "src/my").await {
        assert!(
            paths.is_empty(),
            "{query} matched `src/my` as a string prefix: {paths:?}"
        );
    }
}

/// A path containing `'` must be quoted, not close the SQL literal. Before
/// the fix the three interpolated queries failed with a SQLite syntax error.
#[tokio::test]
async fn an_apostrophe_in_a_path_does_not_break_the_query() {
    let (_tmp, cg) = indexed().await;

    for (query, paths) in scoped_paths(&cg, "src/it's").await {
        assert!(
            !paths.is_empty(),
            "{query} returned nothing for a path containing an apostrophe"
        );
        assert!(
            paths.iter().all(|p| p.starts_with("src/it's/")),
            "{query} leaked outside the apostrophe directory: {paths:?}"
        );
    }
}

/// The annotation histogram returns counts, not paths, so its scoping is
/// checked by count. Every directory holds the same files, so a correctly
/// scoped path counts exactly what the control directory counts.
#[tokio::test]
async fn annotation_histogram_counts_only_the_requested_directory() {
    let (_tmp, cg) = indexed().await;
    let total = |hist: Vec<(String, u64)>| -> u64 { hist.into_iter().map(|(_, n)| n).sum() };

    let control = total(
        cg.db()
            .get_annotation_histogram(Some("src/myXmod"))
            .await
            .unwrap(),
    );
    assert!(
        control > 0,
        "the fixture produces no annotations in the control directory"
    );

    for prefix in ["src/my_mod", "src/it's"] {
        let counted = total(
            cg.db()
                .get_annotation_histogram(Some(prefix))
                .await
                .unwrap(),
        );
        assert_eq!(
            counted, control,
            "the histogram for {prefix} counted annotations outside that directory"
        );
    }

    let percent = total(
        cg.db()
            .get_annotation_histogram(Some("src/%"))
            .await
            .unwrap(),
    );
    assert_eq!(percent, 0, "`%` acted as a wildcard in the histogram");
}

/// With no `path` argument the listing tools fall back to the directory the
/// server was launched from. That value reaches the same queries, so a server
/// started inside `svc/user_api` must not answer with `svc/userXapi`, even
/// though the agent passed no path at all.
#[tokio::test]
async fn a_launch_directory_scope_is_not_a_wildcard() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    for (dir, class) in [("svc/user_api", "UserApi"), ("svc/userXapi", "UserXapi")] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
        std::fs::write(
            root.join(dir).join("api.py"),
            format!("class {class}:\n    def handle(self):\n        return 1\n"),
        )
        .unwrap();
    }
    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    // The control first: a scope with no metacharacter.
    for (scope, sibling) in [
        ("svc/userXapi", "svc/user_api/"),
        ("svc/user_api", "svc/userXapi/"),
    ] {
        let result = handle_tool_call(&cg, "tokensave_largest", json!({}), None, Some(scope))
            .await
            .unwrap();
        let text = result.value["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(
            text.contains(&format!("{scope}/")),
            "largest scoped to {scope} returned nothing from it: {text}"
        );
        assert!(
            !text.contains(sibling),
            "largest scoped to {scope} leaked into {sibling}: {text}"
        );
    }
}
