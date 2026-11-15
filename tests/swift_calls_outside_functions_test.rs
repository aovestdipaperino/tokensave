//! Swift calls outside function bodies are recorded.
//!
//! Call sites were only read from function and init bodies, so a callee used
//! only from top-level `main.swift` code, a property initializer or a
//! computed property had no incoming `calls` edge and looked dead. A
//! property's calls now belong to the property; any other top-level
//! statement's calls belong to the file.
//!
//! The project is `tests/fixtures/swift_calls_outside_functions/`: the
//! callees in one file, top-level code and a computed property in
//! `main.swift`.

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
        .join("swift_calls_outside_functions");
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

/// Catches: calls in top-level `main.swift` code and in property
/// initializers or computed properties producing no `calls` edge, so their
/// callees look dead. A top-level statement's call belongs to the file; a
/// property's belongs to the property.
#[tokio::test]
async fn swift_top_level_and_property_calls_are_recorded() {
    let (_dir, cg) = setup().await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    assert_eq!(
        caller_names(&cg, &nodes, method(&nodes, "Launcher", "boot")).await,
        vec!["booted"],
        "a top-level `let` initializer"
    );
    assert_eq!(
        caller_names(&cg, &nodes, method(&nodes, "Reporter", "summarize")).await,
        vec!["main.swift"],
        "a top-level `if` statement"
    );
    assert_eq!(
        caller_names(&cg, &nodes, method(&nodes, "Formatter", "title")).await,
        vec!["heading"],
        "a computed property"
    );
}
