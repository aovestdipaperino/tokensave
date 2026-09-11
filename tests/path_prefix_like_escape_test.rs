//! `--path` scoping in `coupling`, `inheritance_depth` and `god_class` — #524.
//!
//! These three queries built their path filter by interpolating the caller's
//! path straight into the SQL string, where every sibling query binds or
//! quote-escapes it. Three consequences, all silent: `_` and `%` acted as
//! `LIKE` wildcards, so a filter for `src/my_mod` also matched `src/myXmod`
//! and `src/%` matched everything; and a path containing `'` closed the
//! literal and failed the query outright.
//!
//! Escaping alone is not enough to catch this — binding a parameter fixes the
//! apostrophe and leaves both wildcards live — so each case below is asserted
//! separately against all three queries.

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;

/// Two sibling directories whose names differ only where `LIKE` would treat
/// `_` as a wildcard, plus one holding an apostrophe. Each carries a small
/// class hierarchy with a cross-file reference, so all three queries under
/// test have rows to return.
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
    }

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

/// Every path the three queries return for `path_prefix`, as plain strings.
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

    vec![
        ("coupling", coupling),
        ("inheritance_depth", inheritance),
        ("god_class", god),
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

/// A path containing `'` must be quoted, not close the SQL literal. Before
/// the fix each of these failed with a SQLite syntax error.
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
