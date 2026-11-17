//! `tokensave_call_chain` takes symbol names, and can follow every edge kind
//! in either direction.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

/// `top` calls `mid`, `mid` calls `leaf`, and `other` calls `leaf`. Two
/// methods are named `dup`.
async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("lib.rs"),
        r"
pub fn leaf() {}

pub fn mid() {
    leaf();
}

pub fn top() {
    mid();
}

pub fn other() {
    leaf();
}

pub struct A;
impl A {
    pub fn dup(&self) {}
}

pub struct B;
impl B {
    pub fn dup(&self) {}
}
",
    )
    .unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn chain(cg: &TokenSave, args: Value) -> Value {
    let result = handle_tool_call(cg, "tokensave_call_chain", args, None, None)
        .await
        .unwrap();
    let text = result.value["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"))
}

async fn node_id(cg: &TokenSave, name: &str) -> String {
    cg.get_nodes_by_name(name).await.unwrap()[0].id.clone()
}

fn step_names(parsed: &Value) -> Vec<&str> {
    parsed["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn symbol_names_find_the_chain() {
    let (_dir, cg) = setup().await;
    let parsed = chain(&cg, json!({"from": "top", "to": "leaf"})).await;
    assert_eq!(parsed["found"], true, "{parsed}");
    assert_eq!(step_names(&parsed), ["top", "mid", "leaf"], "{parsed}");
}

#[tokio::test]
async fn node_ids_still_work_and_mix_with_names() {
    let (_dir, cg) = setup().await;
    let top = node_id(&cg, "top").await;
    let leaf = node_id(&cg, "leaf").await;

    let by_id = chain(&cg, json!({"from_id": top, "to_id": leaf})).await;
    assert_eq!(step_names(&by_id), ["top", "mid", "leaf"], "{by_id}");

    let mixed = chain(&cg, json!({"from_id": top, "to": "leaf"})).await;
    assert_eq!(mixed, by_id);
}

/// A node id wins over a name, as `node_id` wins over `symbol` elsewhere.
#[tokio::test]
async fn an_id_takes_precedence_over_a_name() {
    let (_dir, cg) = setup().await;
    let mid = node_id(&cg, "mid").await;
    let parsed = chain(&cg, json!({"from_id": mid, "from": "top", "to": "leaf"})).await;
    assert_eq!(step_names(&parsed), ["mid", "leaf"], "{parsed}");
}

#[tokio::test]
async fn an_ambiguous_name_lists_candidates_and_picks_none() {
    let (_dir, cg) = setup().await;
    let parsed = chain(&cg, json!({"from": "top", "to": "dup"})).await;
    assert_eq!(parsed["found"], false, "{parsed}");
    assert!(parsed["steps"].as_array().unwrap().is_empty(), "{parsed}");
    let to = &parsed["unresolved"]["to"];
    assert_eq!(to["match_count"], 2, "{parsed}");
    let candidates = to["matches"].as_array().unwrap();
    assert_eq!(candidates.len(), 2, "{parsed}");
    assert!(
        candidates.iter().all(|c| c["node_id"].is_string()),
        "{parsed}"
    );
    // `from` resolved, so it is not reported.
    assert!(parsed["unresolved"].get("from").is_none(), "{parsed}");

    // A listed candidate, passed back by id, is accepted.
    let id = candidates[0]["node_id"].as_str().unwrap();
    let retry = chain(&cg, json!({"from": "top", "to_id": id})).await;
    assert!(retry.get("unresolved").is_none(), "{retry}");
}

#[tokio::test]
async fn an_unknown_name_is_reported() {
    let (_dir, cg) = setup().await;
    let parsed = chain(&cg, json!({"from": "no_such_symbol_zz", "to": "leaf"})).await;
    assert_eq!(parsed["found"], false, "{parsed}");
    let from = &parsed["unresolved"]["from"];
    assert_eq!(from["match_count"], 0, "{parsed}");
    assert!(
        from["hint"].as_str().unwrap().contains("No symbol named"),
        "{parsed}"
    );
}

#[tokio::test]
async fn a_missing_end_is_an_error() {
    let (_dir, cg) = setup().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_call_chain",
        json!({"to": "leaf"}),
        None,
        None,
    )
    .await;
    let message = result.err().expect("an error").to_string();
    assert!(message.contains("from (or from_id)"), "{message}");

    let result = handle_tool_call(
        &cg,
        "tokensave_call_chain",
        json!({"from": "top", "to": "leaf", "follow": "sideways"}),
        None,
        None,
    )
    .await;
    let message = result.err().expect("an error").to_string();
    assert!(message.contains("invalid follow"), "{message}");
}

/// `top` and `other` both reach `leaf`, but neither calls the other.
#[tokio::test]
async fn follow_any_connects_what_a_directed_chain_does_not() {
    let (_dir, cg) = setup().await;

    let directed = chain(&cg, json!({"from": "top", "to": "other"})).await;
    assert_eq!(directed["found"], false, "{directed}");
    assert!(directed["steps"][0].get("edge_direction").is_none());

    let any = chain(&cg, json!({"from": "top", "to": "other", "follow": "any"})).await;
    assert_eq!(any["found"], true, "{any}");
    assert_eq!(step_names(&any), ["top", "mid", "leaf", "other"], "{any}");
    let directions: Vec<&Value> = any["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| &step["edge_direction"])
        .collect();
    // top -> mid -> leaf, then leaf <- other.
    assert_eq!(
        directions,
        [
            &Value::Null,
            &json!("forward"),
            &json!("forward"),
            &json!("reverse")
        ],
        "{any}"
    );
}

#[tokio::test]
async fn follow_any_respects_max_depth() {
    let (_dir, cg) = setup().await;
    let parsed = chain(
        &cg,
        json!({"from": "top", "to": "other", "follow": "any", "max_depth": 2}),
    )
    .await;
    assert_eq!(parsed["found"], false, "{parsed}");
    assert!(parsed.get("budget_exhausted").is_none(), "{parsed}");
}

/// The node budget stops the search and the result says so: "not found" must
/// not read as "not connected".
#[tokio::test]
async fn a_spent_node_budget_is_reported() {
    let (_dir, cg) = setup().await;
    let top = node_id(&cg, "top").await;
    let other = node_id(&cg, "other").await;

    let spent = cg.get_connection_path(&top, &other, 8, 1).await.unwrap();
    assert!(spent.path.is_none());
    assert!(spent.budget_exhausted);

    let enough = cg.get_connection_path(&top, &other, 8, 100).await.unwrap();
    assert_eq!(enough.path.unwrap().len(), 4);
    assert!(!enough.budget_exhausted);
}
