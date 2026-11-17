//! Swift calls through a type name resolve across files.
//!
//! `Cache.open(...)` was recorded as the bare name `open`. Common names
//! (`open`, `wait`, `stop`, `create`, `start`) are on the cross-file
//! blocklist, so a static call through a type name in another file never
//! resolved. The receiver type now travels with the reference as
//! `Cache::open` and resolves through the qualified-name match.
//!
//! The project is `tests/fixtures/swift_type_calls/`: the called types in
//! one file, the caller in another.

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, Node, NodeKind};

/// Copies the fixture project into a temporary directory and indexes it.
async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().expect("temp dir");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("swift_type_calls");
    for entry in fs::read_dir(&fixture).expect("fixture dir is readable") {
        let entry = entry.expect("fixture dir entry");
        fs::copy(entry.path(), dir.path().join(entry.file_name())).expect("copy fixture file");
    }
    let cg = TokenSave::init(dir.path()).await.expect("init");
    cg.index_all().await.expect("index");
    (dir, cg)
}

fn method<'n>(nodes: &'n [Node], owner: &str, name: &str) -> &'n Node {
    let suffix = format!("{owner}::{name}");
    nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.qualified_name.ends_with(&suffix))
        .unwrap_or_else(|| panic!("method {suffix}"))
}

async fn caller_names(cg: &TokenSave, nodes: &[Node], target: &Node) -> Vec<String> {
    let incoming = cg.get_incoming_edges(&target.id).await.expect("edges");
    let mut names = Vec::new();
    for edge in incoming {
        if edge.kind != EdgeKind::Calls {
            continue;
        }
        if let Some(caller) = nodes.iter().find(|n| n.id == edge.source) {
            names.push(caller.name.clone());
        }
    }
    names.sort();
    names
}

/// Catches: `Type.method(...)` calls in another file going unresolved when
/// the method name is common (`open`, `wait`, `stop`, `create`), whether the
/// call spans lines, takes a trailing closure, or sits under `try await`.
#[tokio::test]
async fn swift_type_qualified_calls_resolve_across_files() {
    let (_dir, cg) = setup().await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    for (owner, name) in [
        ("Cache", "open"),
        ("Cache", "wait"),
        ("Cache", "stop"),
        ("Registry", "create"),
    ] {
        let target = method(&nodes, owner, name);
        assert_eq!(
            caller_names(&cg, &nodes, target).await,
            vec!["run"],
            "callers of {owner}.{name}"
        );
    }
}

/// Catches: a call through an instance receiver (`worker.start()`) being
/// bound by name alone to a same-named method in another file.
#[tokio::test]
async fn swift_instance_receiver_call_is_not_bound_by_name_alone() {
    let (_dir, cg) = setup().await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    let start = method(&nodes, "Worker", "start");
    assert_eq!(caller_names(&cg, &nodes, start).await, Vec::<String>::new());
}
