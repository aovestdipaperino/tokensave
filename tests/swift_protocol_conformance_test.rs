//! Swift protocol conformances become `implements` edges.
//!
//! A conformance list (`struct X: P`, `actor Y: P`, `extension Z: P`) was
//! only read for classes, and always as `Extends`, so no Swift type ever
//! reached `tokensave_implementations`. As in C# (#643), a Swift inheritance
//! clause cannot tell a superclass from a protocol, so the resolver decides
//! the edge kind once the target is known.
//!
//! The project is `tests/fixtures/swift_conformance/`: protocols and a base
//! class in one file, the conforming types in another.

use std::fs;
use std::path::Path;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, Node, NodeKind};

/// Copies the fixture project into a temporary directory and indexes it.
async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().expect("temp dir");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("swift_conformance");
    for entry in fs::read_dir(&fixture).expect("fixture dir is readable") {
        let entry = entry.expect("fixture dir entry");
        fs::copy(entry.path(), dir.path().join(entry.file_name())).expect("copy fixture file");
    }
    let cg = TokenSave::init(dir.path()).await.expect("init");
    cg.index_all().await.expect("index");
    (dir, cg)
}

async fn implementer_names(cg: &TokenSave, protocol: &str) -> Vec<String> {
    let result = handle_tool_call(
        cg,
        "tokensave_implementations",
        json!({ "trait": protocol }),
        None,
        None,
    )
    .await
    .expect("tokensave_implementations call");
    let text = result.value["content"][0]["text"]
        .as_str()
        .expect("tool result has text");
    let output: Value = serde_json::from_str(text).expect("tool result is JSON");
    let mut names = Vec::new();
    if let Some(implementations) = output["implementations"].as_array() {
        for implementation in implementations {
            if let Some(name) = implementation["type"].as_str() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names
}

fn type_node<'n>(nodes: &'n [Node], kind: NodeKind, name: &str) -> &'n Node {
    nodes
        .iter()
        .find(|n| n.kind == kind && n.name == name)
        .unwrap_or_else(|| panic!("{kind:?} {name}"))
}

/// Catches: struct, actor, enum and extension conformances producing no
/// `implements` edge, so `tokensave_implementations` finds no Swift type.
#[tokio::test]
async fn implementations_finds_swift_protocol_conformers() {
    let (_dir, cg) = setup().await;
    assert_eq!(
        implementer_names(&cg, "DataSource").await,
        vec!["CachedSource", "RemoteSource"]
    );
    assert_eq!(
        implementer_names(&cg, "Loadable").await,
        vec!["FileLoader", "Mode", "Plain"]
    );
}

/// Catches: a superclass, or a protocol a protocol inherits, being turned
/// into `implements` along with the conformances in the same clause.
#[tokio::test]
async fn swift_superclass_and_protocol_inheritance_stay_extends() {
    let (_dir, cg) = setup().await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    let base = type_node(&nodes, NodeKind::Class, "SourceBase");
    let loadable = type_node(&nodes, NodeKind::Interface, "Loadable");
    let reloadable = type_node(&nodes, NodeKind::Interface, "Reloadable");

    let into_base = cg.get_incoming_edges(&base.id).await.expect("edges");
    let base_kinds: Vec<EdgeKind> = into_base
        .iter()
        .map(|e| e.kind)
        .filter(|k| matches!(k, EdgeKind::Extends | EdgeKind::Implements))
        .collect();
    assert_eq!(base_kinds, vec![EdgeKind::Extends], "into SourceBase");

    let into_loadable = cg.get_incoming_edges(&loadable.id).await.expect("edges");
    let from_reloadable: Vec<EdgeKind> = into_loadable
        .iter()
        .filter(|e| e.source == reloadable.id)
        .map(|e| e.kind)
        .collect();
    assert_eq!(
        from_reloadable,
        vec![EdgeKind::Extends],
        "Reloadable -> Loadable"
    );
}
