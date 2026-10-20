//! `tokensave_callers`, `tokensave_callees`, `tokensave_impact` and
//! `tokensave_type_hierarchy` accept a `symbol` name (#669).
//!
//! The name resolves as `tokensave_body` resolves it (#651); several matches
//! are answered per match up to `max_matches`, the rest listed without
//! results; and each match lists at most `limit` results with `total` and
//! `truncated` (#645).

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

const REFRESH_SRC: &str = r#"namespace App
{
    public interface IRefresher
    {
        Task RefreshAsync();
    }

    public class Coordinator : IRefresher
    {
        public async Task RefreshAsync()
        {
            Log();
            await Task.Delay(1);
        }

        public async Task RefreshAsync(int delay)
        {
            Log();
            await Task.Delay(delay);
        }

        private void Log() { }
    }

    public class Catalog : IRefresher
    {
        public async Task RefreshAsync()
        {
            await Task.Delay(2);
        }
    }
}
"#;

const RUNNER_SRC: &str = r#"namespace App
{
    public class Runner
    {
        private readonly Coordinator _coordinator = new Coordinator();
        private readonly Catalog _catalog = new Catalog();

        public async Task Run()
        {
            await _coordinator.RefreshAsync();
            await _catalog.RefreshAsync();
        }

        public async Task RunTwice()
        {
            await _coordinator.RefreshAsync(3);
        }

        public async Task RunLater()
        {
            await _coordinator.RefreshAsync();
        }
    }
}
"#;

const FAKE_SRC: &str = r#"namespace App.Tests
{
    public class FakeCoordinator : IRefresher
    {
        public Task RefreshAsync() => Task.CompletedTask;
    }
}
"#;

async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("tests")).unwrap();
    fs::write(project.join("src").join("Refresh.cs"), REFRESH_SRC).unwrap();
    fs::write(project.join("src").join("Runner.cs"), RUNNER_SRC).unwrap();
    fs::write(project.join("tests").join("FakeCoordinator.cs"), FAKE_SRC).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn call(cg: &TokenSave, tool: &str, args: Value) -> String {
    let result = handle_tool_call(cg, tool, args, None, None)
        .await
        .unwrap_or_else(|e| panic!("{tool} failed: {e}"));
    result.value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
        .to_string()
}

async fn call_json(cg: &TokenSave, tool: &str, args: Value) -> Value {
    let text = call(cg, tool, args).await;
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"))
}

async fn call_err(cg: &TokenSave, tool: &str, args: Value) -> String {
    match handle_tool_call(cg, tool, args, None, None).await {
        Ok(result) => panic!("{tool} should fail, got {}", result.value),
        Err(e) => e.to_string(),
    }
}

/// The id of the only node named `name`.
async fn id_of(cg: &TokenSave, name: &str) -> String {
    let nodes = cg.get_nodes_by_name(name).await.unwrap();
    assert_eq!(nodes.len(), 1, "expected one node named {name}: {nodes:?}");
    nodes[0].id.clone()
}

fn names(list: &Value) -> Vec<&str> {
    let mut names: Vec<&str> = list
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {list}"))
        .iter()
        .map(|item| item["name"].as_str().unwrap_or_default())
        .collect();
    names.sort_unstable();
    names
}

#[tokio::test]
async fn single_match_keeps_node_id_output_under_one_heading() {
    let (_dir, cg) = setup().await;
    let log_id = id_of(&cg, "Log").await;
    for (tool, key) in [
        ("tokensave_callers", "callers"),
        ("tokensave_callees", "callees"),
    ] {
        let by_id = call_json(&cg, tool, json!({"node_id": log_id})).await;
        assert!(
            by_id.is_array(),
            "{tool} by node_id stays a bare array: {by_id}"
        );
        let by_name = call_json(&cg, tool, json!({"symbol": "Log"})).await;
        assert_eq!(by_name["symbol"], "Log", "{by_name}");
        assert_eq!(by_name["match_count"], 1, "{by_name}");
        let only = &by_name["matches"][0];
        assert_eq!(only["qualified_name"], "App::Coordinator::Log", "{only}");
        assert_eq!(only["kind"], "method", "{only}");
        assert_eq!(only["file"], "src/Refresh.cs", "{only}");
        assert_eq!(only["line"], 22, "{only}");
        assert_eq!(only["end_line"], 22, "{only}");
        assert_eq!(only["node_id"].as_str(), Some(log_id.as_str()), "{only}");
        assert!(only.get("overloads").is_none(), "{only}");
        assert_eq!(only[key], by_id, "{tool}: same results as by node_id");
        assert_eq!(only["truncated"], false, "{only}");
        assert!(by_name.get("unanswered").is_none(), "{by_name}");
    }
    let callers = call_json(&cg, "tokensave_callers", json!({"symbol": "Log"})).await;
    assert_eq!(
        names(&callers["matches"][0]["callers"]),
        ["RefreshAsync", "RefreshAsync"]
    );

    let by_id = call_json(&cg, "tokensave_impact", json!({"node_id": log_id})).await;
    let by_name = call_json(&cg, "tokensave_impact", json!({"symbol": "Log"})).await;
    let only = &by_name["matches"][0];
    assert_eq!(only["node_id"].as_str(), Some(log_id.as_str()), "{only}");
    for field in [
        "node_count",
        "edge_count",
        "count",
        "total",
        "truncated",
        "nodes",
    ] {
        assert_eq!(only[field], by_id[field], "impact `{field}`");
    }
    assert_eq!(by_id["node_count"], 6, "{by_id}");

    let iface_id = id_of(&cg, "IRefresher").await;
    let by_id = call(
        &cg,
        "tokensave_type_hierarchy",
        json!({"node_id": iface_id}),
    )
    .await;
    let by_name = call(
        &cg,
        "tokensave_type_hierarchy",
        json!({"symbol": "IRefresher"}),
    )
    .await;
    assert_eq!(
        by_name,
        format!("## App::IRefresher (interface) src/Refresh.cs:3-6 id={iface_id}\n{by_id}")
    );
    for implementor in ["Coordinator", "Catalog", "FakeCoordinator"] {
        assert!(
            by_id.contains(&format!("|- implements {implementor} (class)")),
            "{by_id}"
        );
    }
}

#[tokio::test]
async fn qualified_type_member_names_one_match() {
    let (_dir, cg) = setup().await;
    for symbol in [
        "Catalog.RefreshAsync",
        "Catalog::RefreshAsync",
        "App.Catalog.RefreshAsync",
    ] {
        let out = call_json(&cg, "tokensave_callers", json!({"symbol": symbol})).await;
        assert_eq!(out["match_count"], 1, "{symbol}: {out}");
        let only = &out["matches"][0];
        assert_eq!(
            only["qualified_name"], "App::Catalog::RefreshAsync",
            "{only}"
        );
        assert_eq!(names(&only["callers"]), ["Run"], "{symbol}: {only}");
    }
}

#[tokio::test]
async fn overloads_are_answered_together() {
    let (_dir, cg) = setup().await;
    let out = call_json(
        &cg,
        "tokensave_callers",
        json!({"symbol": "Coordinator.RefreshAsync"}),
    )
    .await;
    assert_eq!(out["match_count"], 1, "{out}");
    let only = &out["matches"][0];
    assert_eq!(
        only["overloads"].as_array().map(Vec::len),
        Some(2),
        "{only}"
    );
    assert_eq!(
        names(&only["callers"]),
        ["Run", "RunLater", "RunTwice"],
        "callers of either overload, each once: {only}"
    );

    // Both overloads call `Log`; it is listed once.
    let out = call_json(
        &cg,
        "tokensave_callees",
        json!({"symbol": "Coordinator.RefreshAsync"}),
    )
    .await;
    let callees = &out["matches"][0]["callees"];
    assert_eq!(
        names(callees).iter().filter(|n| **n == "Log").count(),
        1,
        "{callees}"
    );
}

#[tokio::test]
async fn several_matches_answered_up_to_max_matches_then_listed() {
    let (_dir, cg) = setup().await;
    // Four `RefreshAsync` qualified names: the interface's, Catalog's,
    // Coordinator's (two overloads, one match) and the test fake's.
    let out = call_json(&cg, "tokensave_callers", json!({"symbol": "RefreshAsync"})).await;
    assert_eq!(out["match_count"], 4, "{out}");
    assert_eq!(
        out["matches"].as_array().map(Vec::len),
        Some(3),
        "default max_matches is 3: {out}"
    );
    assert_eq!(out["unanswered"].as_array().map(Vec::len), Some(1), "{out}");

    let out = call_json(
        &cg,
        "tokensave_callers",
        json!({"symbol": "RefreshAsync", "max_matches": 2}),
    )
    .await;
    assert_eq!(out["match_count"], 4, "{out}");
    let matches = out["matches"].as_array().expect("matches");
    assert_eq!(matches.len(), 2, "{out}");
    for m in matches {
        assert!(
            m["callers"].is_array(),
            "answered match carries results: {m}"
        );
        for field in ["qualified_name", "kind", "file", "line", "node_id"] {
            assert!(!m[field].is_null(), "heading lacks `{field}`: {m}");
        }
    }
    let unanswered = out["unanswered"].as_array().expect("unanswered");
    assert_eq!(unanswered.len(), 2, "{out}");
    for m in unanswered {
        assert!(m.get("callers").is_none(), "listed without results: {m}");
        assert!(m["node_id"].as_str().is_some_and(|s| !s.is_empty()), "{m}");
        assert!(
            m["qualified_name"]
                .as_str()
                .is_some_and(|q| q.ends_with("RefreshAsync")),
            "{m}"
        );
    }
    assert!(
        out["hint"].as_str().is_some_and(|h| h.contains("node_id")),
        "{out}"
    );

    // Every match appears exactly once across both lists.
    let mut all: Vec<&str> = matches
        .iter()
        .chain(unanswered)
        .map(|m| m["qualified_name"].as_str().unwrap_or_default())
        .collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 4, "{all:?}");

    // A listed match is answerable by its node id.
    let follow_up_id = unanswered[0]["node_id"].as_str().unwrap();
    let follow_up = call_json(&cg, "tokensave_callers", json!({"node_id": follow_up_id})).await;
    assert!(follow_up.is_array(), "{follow_up}");

    // The text tool groups the same way.
    let text = call(
        &cg,
        "tokensave_type_hierarchy",
        json!({"symbol": "RefreshAsync", "max_matches": 2}),
    )
    .await;
    assert_eq!(
        text.lines().filter(|l| l.starts_with("## ")).count(),
        2,
        "{text}"
    );
    assert!(text.contains("past max_matches=2"), "{text}");
}

#[tokio::test]
async fn node_id_takes_precedence_over_symbol() {
    let (_dir, cg) = setup().await;
    let log_id = id_of(&cg, "Log").await;
    let both = call_json(
        &cg,
        "tokensave_callers",
        json!({"node_id": log_id, "symbol": "RefreshAsync"}),
    )
    .await;
    let by_id = call_json(&cg, "tokensave_callers", json!({"node_id": log_id})).await;
    assert_eq!(both, by_id);

    let both = call_json(
        &cg,
        "tokensave_impact",
        json!({"node_id": log_id, "symbol": "RefreshAsync"}),
    )
    .await;
    assert!(both.get("matches").is_none(), "{both}");
    assert_eq!(both["node_count"], 6, "{both}");
}

#[tokio::test]
async fn neither_symbol_nor_node_id_is_an_error() {
    let (_dir, cg) = setup().await;
    for tool in [
        "tokensave_callers",
        "tokensave_callees",
        "tokensave_impact",
        "tokensave_type_hierarchy",
    ] {
        let err = call_err(&cg, tool, json!({"max_depth": 2})).await;
        assert!(
            err.contains("symbol") && err.contains("node_id"),
            "{tool}: {err}"
        );
    }
}

#[tokio::test]
async fn unknown_symbol_lists_no_results() {
    let (_dir, cg) = setup().await;
    let out = call_json(&cg, "tokensave_callers", json!({"symbol": "NoSuchMethod"})).await;
    assert_eq!(out["match_count"], 0, "{out}");
    assert_eq!(out["matches"], json!([]), "{out}");
    assert!(
        out["hint"]
            .as_str()
            .is_some_and(|h| h.contains("No symbol named 'NoSuchMethod'")),
        "{out}"
    );
}

#[tokio::test]
async fn limit_caps_each_match_and_reports_total() {
    let (_dir, cg) = setup().await;
    let out = call_json(
        &cg,
        "tokensave_callers",
        json!({"symbol": "Coordinator.RefreshAsync", "limit": 1}),
    )
    .await;
    let only = &out["matches"][0];
    assert_eq!(only["count"], 1, "{only}");
    assert_eq!(only["total"], 3, "{only}");
    assert_eq!(only["truncated"], true, "{only}");
    assert_eq!(only["callers"].as_array().map(Vec::len), Some(1), "{only}");

    // With node_id alone, the bare array is wrapped only when cut.
    let log_id = id_of(&cg, "Log").await;
    let cut = call_json(
        &cg,
        "tokensave_callers",
        json!({"node_id": log_id, "limit": 1}),
    )
    .await;
    assert_eq!(cut["count"], 1, "{cut}");
    assert_eq!(cut["total"], 2, "{cut}");
    assert_eq!(cut["truncated"], true, "{cut}");
    assert_eq!(cut["callers"].as_array().map(Vec::len), Some(1), "{cut}");
    let fits = call_json(
        &cg,
        "tokensave_callers",
        json!({"node_id": log_id, "limit": 2}),
    )
    .await;
    assert_eq!(fits.as_array().map(Vec::len), Some(2), "{fits}");

    let impact = call_json(
        &cg,
        "tokensave_impact",
        json!({"node_id": log_id, "limit": 2}),
    )
    .await;
    assert_eq!(impact["node_count"], 6, "{impact}");
    assert_eq!(impact["count"], 2, "{impact}");
    assert_eq!(impact["total"], 6, "{impact}");
    assert_eq!(impact["truncated"], true, "{impact}");
    assert_eq!(
        impact["nodes"].as_array().map(Vec::len),
        Some(2),
        "{impact}"
    );

    let tree = call(
        &cg,
        "tokensave_type_hierarchy",
        json!({"symbol": "IRefresher", "limit": 1}),
    )
    .await;
    assert_eq!(tree.matches("|- implements").count(), 1, "{tree}");
    assert!(tree.contains("count: 1 of 3 (truncated"), "{tree}");
}
