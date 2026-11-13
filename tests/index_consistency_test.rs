//! The index agrees with the checkout it was built from (#865).
//!
//! Two properties. An incremental sync after the tree changed under the index
//! (a branch switch, a reset to another commit) gives the same graph as a
//! full index of that tree. And two working trees of one repository, each
//! with its own index, answer only for their own files.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use tokensave::tokensave::TokenSave;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit_all(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", message]);
}

/// The graph without row identities and timestamps: the files, the nodes by
/// kind, name, file and span, and the edges by the nodes they join.
#[derive(Debug, PartialEq, Eq)]
struct Graph {
    files: BTreeSet<String>,
    nodes: BTreeSet<String>,
    edges: BTreeSet<String>,
}

async fn graph(cg: &TokenSave) -> Graph {
    let files = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    let all_nodes = cg.get_all_nodes().await.unwrap();
    let key_of: HashMap<&str, String> = all_nodes
        .iter()
        .map(|n| {
            (
                n.id.as_str(),
                format!("{:?} {} in {}", n.kind, n.qualified_name, n.file_path),
            )
        })
        .collect();
    let nodes = all_nodes
        .iter()
        .map(|n| {
            format!(
                "{} lines {}-{} signature {:?}",
                key_of[n.id.as_str()],
                n.start_line,
                n.end_line,
                n.signature
            )
        })
        .collect();
    let end = |id: &str| key_of.get(id).cloned().unwrap_or_else(|| id.to_string());
    let edges = cg
        .get_all_edges()
        .await
        .unwrap()
        .iter()
        .map(|e| {
            format!(
                "{} -> {} {:?} line {:?} {:?}",
                end(&e.source),
                end(&e.target),
                e.kind,
                e.line,
                e.resolved_by
            )
        })
        .collect();
    Graph {
        files,
        nodes,
        edges,
    }
}

/// Syncs the checkout incrementally, then indexes it in full, and requires the
/// two graphs to be equal. Returns the graph.
async fn synced_graph_equals_full_index(root: &Path, step: &str) -> Graph {
    let cg = TokenSave::open(root).await.unwrap();
    cg.sync().await.unwrap();
    let incremental = graph(&cg).await;
    cg.index_all().await.unwrap();
    let full = graph(&cg).await;
    assert_eq!(
        incremental, full,
        "{step}: the incremental sync and the full index disagree"
    );
    full
}

#[tokio::test]
async fn an_incremental_sync_equals_a_full_index_after_the_tree_changes() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
    std::fs::write(
        root.join("lib.rs"),
        "pub fn helper() {}\npub fn caller() {\n    helper();\n}\n",
    )
    .unwrap();
    std::fs::write(root.join("user.rs"), "pub fn user() {\n    helper();\n}\n").unwrap();
    commit_all(root, "base");
    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    drop(cg);
    let on_master = synced_graph_equals_full_index(root, "master").await;
    assert!(
        !on_master.edges.is_empty(),
        "the comparison needs edges: {on_master:?}"
    );

    // A branch that renames a function, deletes a file and adds one.
    git(root, &["checkout", "-b", "feature"]);
    std::fs::write(
        root.join("lib.rs"),
        "pub fn renamed_helper() {}\npub fn caller() {\n    renamed_helper();\n}\n",
    )
    .unwrap();
    std::fs::remove_file(root.join("user.rs")).unwrap();
    std::fs::write(
        root.join("added.rs"),
        "pub fn added() {\n    renamed_helper();\n    caller();\n}\n",
    )
    .unwrap();
    commit_all(root, "feature");
    let on_feature = synced_graph_equals_full_index(root, "feature").await;
    assert!(on_feature.files.contains("added.rs"), "{on_feature:?}");
    assert!(!on_feature.files.contains("user.rs"), "{on_feature:?}");

    // Back: the index of master is again the index of master's tree.
    git(root, &["checkout", "master"]);
    let back_on_master = synced_graph_equals_full_index(root, "back on master").await;
    assert_eq!(back_on_master, on_master);

    // The branch name stays and the tree under it changes, as after a
    // rebase or a forced update.
    git(root, &["reset", "--hard", "feature"]);
    let after_reset = synced_graph_equals_full_index(root, "master reset to feature").await;
    assert_eq!(after_reset, on_feature);
}

/// A main checkout and a linked worktree beside it, each with an index.
#[tokio::test]
async fn two_working_trees_answer_only_for_their_own_files() {
    let tmp = tempdir().unwrap();
    let main = tmp.path().join("main");
    let linked = tmp.path().join("linked");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-b", "master"]);
    std::fs::write(main.join(".gitignore"), ".tokensave/\n").unwrap();
    std::fs::write(main.join("base.rs"), "pub fn base() {}\n").unwrap();
    commit_all(&main, "base");
    git(
        &main,
        &["worktree", "add", "-b", "side", linked.to_str().unwrap()],
    );

    let in_main = TokenSave::init(&main).await.unwrap();
    in_main.index_all().await.unwrap();
    let in_linked = TokenSave::init(&linked).await.unwrap();
    in_linked.index_all().await.unwrap();
    assert_eq!(in_linked.active_branch(), Some("side"));

    std::fs::write(main.join("main_only.rs"), "pub fn main_only() {}\n").unwrap();
    std::fs::write(linked.join("side_only.rs"), "pub fn side_only() {}\n").unwrap();
    in_linked.sync().await.unwrap();
    let main_before = graph(&in_main).await;
    assert!(
        !main_before.files.contains("side_only.rs"),
        "a sync in the linked worktree reached the main index: {main_before:?}"
    );

    in_main.sync().await.unwrap();
    let main_graph = graph(&in_main).await;
    let linked_graph = graph(&in_linked).await;
    assert!(main_graph.files.contains("main_only.rs"), "{main_graph:?}");
    assert!(!main_graph.files.contains("side_only.rs"), "{main_graph:?}");
    assert!(
        linked_graph.files.contains("side_only.rs"),
        "{linked_graph:?}"
    );
    assert!(
        !linked_graph.files.contains("main_only.rs"),
        "{linked_graph:?}"
    );
}
