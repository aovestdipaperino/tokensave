//! `tokensave_callers`, `tokensave_callees`, `tokensave_call_chain` and
//! `tokensave_impact` report how the resolver bound the edges they return.
//!
//! The index stores `edges.resolved_by` (#544). These tests pin that the
//! traversal tools show it, so a reader can tell an edge the resolver could
//! justify from one it picked by name alone.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::{Edge, EdgeKind, ResolvedBy};

/// A project whose call edges are written with a known provenance.
///
/// The functions do not call each other in the source. The tests insert the
/// edges themselves, so they pin what the tools report and not how the
/// resolver binds a given call.
///
/// - `driver` calls `free_fn` through an exact edge.
/// - `driver` calls `bump` through a heuristic edge.
/// - `both` calls `bump` from two call sites, one of each class.
async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("lib.rs"),
        "pub fn bump() {}\npub fn free_fn() {}\npub fn driver() {}\npub fn both() {}\n",
    )
    .unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();

    let mut edges = Vec::new();
    for (source, target, line, resolved_by) in [
        ("driver", "free_fn", 10, ResolvedBy::ExactMatch),
        ("driver", "bump", 11, ResolvedBy::SimpleNameMatch),
        ("both", "bump", 20, ResolvedBy::SimpleNameMatch),
        ("both", "bump", 21, ResolvedBy::QualifiedMatch),
    ] {
        edges.push(Edge {
            source: node_id(&cg, source).await,
            target: node_id(&cg, target).await,
            kind: EdgeKind::Calls,
            line: Some(line),
            resolved_by: Some(resolved_by),
        });
    }
    cg.db().insert_edges(&edges).await.unwrap();
    (dir, cg)
}

async fn node_id(cg: &TokenSave, name: &str) -> String {
    cg.get_nodes_by_name(name).await.unwrap()[0].id.clone()
}

async fn call(cg: &TokenSave, tool: &str, args: Value) -> Value {
    let result = handle_tool_call(cg, tool, args, None, None).await.unwrap();
    let text = result.value["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("{tool} did not return JSON ({e}): {text}"))
}

/// The items of the single match a `symbol` call answers.
fn items<'a>(parsed: &'a Value, key: &str) -> &'a Vec<Value> {
    parsed["matches"][0][key]
        .as_array()
        .unwrap_or_else(|| panic!("no `{key}` list in {parsed}"))
}

fn named<'a>(items: &'a [Value], name: &str) -> &'a Value {
    items
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("no item named `{name}` in {items:?}"))
}

#[tokio::test]
async fn callees_report_exact_and_heuristic_resolution() {
    let (_dir, cg) = setup().await;
    let parsed = call(
        &cg,
        "tokensave_callees",
        json!({"symbol": "driver", "max_depth": 1}),
    )
    .await;
    let callees = items(&parsed, "callees");

    let free_fn = named(callees, "free_fn");
    assert_eq!(free_fn["resolved_by"], "exact-match", "{free_fn}");
    assert_eq!(free_fn["resolved_exact"], true, "{free_fn}");

    let bump = named(callees, "bump");
    assert_eq!(bump["resolved_by"], "simple-name-match", "{bump}");
    assert_eq!(bump["resolved_exact"], false, "{bump}");
}

#[tokio::test]
async fn callers_report_resolution() {
    let (_dir, cg) = setup().await;
    let parsed = call(&cg, "tokensave_callers", json!({"symbol": "free_fn"})).await;
    let driver = named(items(&parsed, "callers"), "driver");
    assert_eq!(driver["resolved_by"], "exact-match", "{driver}");
    assert_eq!(driver["resolved_exact"], true, "{driver}");

    let parsed = call(&cg, "tokensave_callers", json!({"symbol": "bump"})).await;
    let driver = named(items(&parsed, "callers"), "driver");
    assert_eq!(driver["resolved_by"], "simple-name-match", "{driver}");
    assert_eq!(driver["resolved_exact"], false, "{driver}");
}

/// `both` calls `bump` from a heuristic site and from an exact site. The
/// relationship is as well established as its best site, in both directions.
#[tokio::test]
async fn several_call_sites_report_the_strongest_resolution() {
    let (_dir, cg) = setup().await;

    let parsed = call(&cg, "tokensave_callers", json!({"symbol": "bump"})).await;
    let both = named(items(&parsed, "callers"), "both");
    assert_eq!(both["lines"].as_array().unwrap().len(), 2, "{both}");
    assert_eq!(both["resolved_by"], "qualified-match", "{both}");
    assert_eq!(both["resolved_exact"], true, "{both}");

    let parsed = call(
        &cg,
        "tokensave_callees",
        json!({"symbol": "both", "max_depth": 1}),
    )
    .await;
    let bump = named(items(&parsed, "callees"), "bump");
    assert_eq!(bump["resolved_by"], "qualified-match", "{bump}");
    assert_eq!(bump["resolved_exact"], true, "{bump}");
}

#[tokio::test]
async fn call_chain_steps_report_resolution() {
    let (_dir, cg) = setup().await;
    let driver = node_id(&cg, "driver").await;
    let bump = node_id(&cg, "bump").await;

    let parsed = call(
        &cg,
        "tokensave_call_chain",
        json!({"from_id": driver, "to_id": bump}),
    )
    .await;
    assert_eq!(parsed["found"], true, "{parsed}");
    let steps = parsed["steps"].as_array().unwrap();
    // The first step is the start node: no edge leads to it.
    assert!(steps[0].get("resolved_by").is_none(), "{parsed}");
    assert_eq!(steps[1]["resolved_by"], "simple-name-match", "{parsed}");
    assert_eq!(steps[1]["resolved_exact"], false, "{parsed}");
}

#[tokio::test]
async fn impact_counts_heuristic_edges() {
    let (_dir, cg) = setup().await;

    // `free_fn` has one dependent, through an exact edge.
    let parsed = call(&cg, "tokensave_impact", json!({"symbol": "free_fn"})).await;
    assert_eq!(parsed["matches"][0]["heuristic_edge_count"], 0, "{parsed}");

    // `bump` has two dependents. `driver` reaches it by name alone, and one
    // of the two call sites in `both` does too.
    let parsed = call(&cg, "tokensave_impact", json!({"symbol": "bump"})).await;
    assert_eq!(parsed["matches"][0]["heuristic_edge_count"], 2, "{parsed}");
}
