//! `tokensave_body` disambiguation (#651).
//!
//! A bare member name shared by several symbols must not dump every body:
//! the tool returns a candidate list instead, and the caller re-asks with a
//! qualified name (`Type::Member` or `Type.Member`) or a node id.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

/// C# project mirroring the report: a long `RefreshAsync`, a one-line test
/// fake, and the wanted short one on `Coordinator`.
async fn setup_csharp() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("tests")).unwrap();
    fs::write(
        project.join("src/Catalog.cs"),
        r#"namespace App
{
    public class Catalog
    {
        public async Task RefreshAsync()
        {
            var a = 1;
            var b = 2;
            var c = a + b;
            await Task.Delay(c);
        }
    }
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("src/Coordinator.cs"),
        r#"namespace App
{
    public class Coordinator
    {
        public async Task RefreshAsync()
        {
            await Task.Delay(1);
        }
    }
}
"#,
    )
    .unwrap();
    fs::write(
        project.join("tests/FakeCoordinator.cs"),
        r#"namespace App.Tests
{
    public class FakeCoordinator
    {
        public Task RefreshAsync() => Task.CompletedTask;
    }
}
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

fn text_of(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

async fn body(cg: &TokenSave, args: Value) -> String {
    let result = handle_tool_call(cg, "tokensave_body", args, None, None)
        .await
        .unwrap();
    text_of(&result.value).to_string()
}

async fn body_json(cg: &TokenSave, mut args: Value) -> Value {
    args["format"] = json!("json");
    let text = body(cg, args).await;
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"))
}

#[tokio::test]
async fn ambiguous_name_returns_candidates_not_bodies() {
    let (_dir, cg) = setup_csharp().await;
    let out = body_json(&cg, json!({"symbol": "RefreshAsync"})).await;
    assert_eq!(out["match_count"].as_u64(), Some(0), "{out}");
    assert!(out["matches"].as_array().is_none_or(Vec::is_empty), "{out}");
    let candidates = out["candidates"].as_array().expect("candidates array");
    assert_eq!(candidates.len(), 3, "{out}");
    let names: Vec<&str> = candidates
        .iter()
        .map(|c| c["qualified_name"].as_str().unwrap_or_default())
        .collect();
    for want in [
        "App::Coordinator::RefreshAsync",
        "App::Catalog::RefreshAsync",
        "App.Tests::FakeCoordinator::RefreshAsync",
    ] {
        assert!(
            names.iter().any(|n| n.ends_with(want)),
            "missing candidate {want} in {names:?}"
        );
    }
    for c in candidates {
        assert!(c["body"].is_null(), "candidates carry no body: {c}");
        assert!(c["id"].as_str().is_some_and(|s| !s.is_empty()), "{c}");
        assert_eq!(c["kind"].as_str(), Some("method"), "{c}");
        assert!(
            c["file"].as_str().is_some_and(|f| f.ends_with(".cs")),
            "{c}"
        );
        assert!(c["start_line"].as_u64().unwrap_or(0) >= 1, "{c}");
        assert!(c["end_line"].as_u64() >= c["start_line"].as_u64(), "{c}");
    }
    // The displayed qualified name drops the file-path prefix.
    assert!(
        names.iter().all(|n| !n.contains(".cs")),
        "file path should be stripped: {names:?}"
    );
    assert!(out["hint"]
        .as_str()
        .is_some_and(|h| h.contains("qualified")));
}

#[tokio::test]
async fn ambiguous_name_text_format_lists_candidates() {
    let (_dir, cg) = setup_csharp().await;
    let text = body(&cg, json!({"symbol": "RefreshAsync"})).await;
    assert!(text.contains("ambiguous"), "{text}");
    assert!(text.contains("Coordinator::RefreshAsync"), "{text}");
    assert!(text.contains("FakeCoordinator::RefreshAsync"), "{text}");
    assert!(
        !text.contains("Task.Delay"),
        "no bodies for an ambiguous name: {text}"
    );
}

#[tokio::test]
async fn double_colon_qualified_name_returns_single_body() {
    let (_dir, cg) = setup_csharp().await;
    let out = body_json(&cg, json!({"symbol": "Coordinator::RefreshAsync"})).await;
    assert_eq!(out["match_count"].as_u64(), Some(1), "{out}");
    let b = out["matches"][0]["body"].as_str().unwrap_or_default();
    assert!(b.contains("Task.Delay(1)"), "{b}");
}

#[tokio::test]
async fn dot_separator_is_accepted() {
    let (_dir, cg) = setup_csharp().await;
    let out = body_json(&cg, json!({"symbol": "Coordinator.RefreshAsync"})).await;
    assert_eq!(out["match_count"].as_u64(), Some(1), "{out}");
    let m = &out["matches"][0];
    assert!(
        m["file"]
            .as_str()
            .is_some_and(|f| f.ends_with("Coordinator.cs")),
        "{m}"
    );
    let b = m["body"].as_str().unwrap_or_default();
    assert!(b.contains("Task.Delay(1)"), "{b}");
}

#[tokio::test]
async fn nested_dot_separator_is_accepted() {
    let (_dir, cg) = setup_csharp().await;
    let out = body_json(&cg, json!({"symbol": "App.Coordinator.RefreshAsync"})).await;
    assert_eq!(out["match_count"].as_u64(), Some(1), "{out}");
    let text = body(&cg, json!({"symbol": "Tests.FakeCoordinator.RefreshAsync"})).await;
    assert!(text.contains("Task.CompletedTask"), "{text}");
}

#[tokio::test]
async fn literal_dotted_name_wins_over_separator_fallback() {
    // A file path contains a dot; its literal qualified-name suffix must
    // still resolve without being rewritten to `Coordinator::cs`.
    let (_dir, cg) = setup_csharp().await;
    let out = body_json(
        &cg,
        json!({"symbol": "src/Coordinator.cs::App::Coordinator::RefreshAsync"}),
    )
    .await;
    assert_eq!(out["match_count"].as_u64(), Some(1), "{out}");
}

#[tokio::test]
async fn candidate_id_resolves_to_body() {
    let (_dir, cg) = setup_csharp().await;
    let out = body_json(&cg, json!({"symbol": "RefreshAsync"})).await;
    let id = out["candidates"]
        .as_array()
        .and_then(|cs| {
            cs.iter().find(|c| {
                c["file"]
                    .as_str()
                    .is_some_and(|f| f.ends_with("src/Coordinator.cs"))
            })
        })
        .and_then(|c| c["id"].as_str())
        .expect("Coordinator candidate id")
        .to_string();
    let out = body_json(&cg, json!({"node_id": id})).await;
    assert_eq!(out["match_count"].as_u64(), Some(1), "{out}");
    let b = out["matches"][0]["body"].as_str().unwrap_or_default();
    assert!(b.contains("Task.Delay(1)"), "{b}");
}

#[tokio::test]
async fn unknown_id_reports_not_found() {
    let (_dir, cg) = setup_csharp().await;
    let text = body(&cg, json!({"id": "no-such-node-id"})).await;
    assert!(text.contains("No symbol"), "{text}");
}

#[tokio::test]
async fn missing_symbol_and_id_is_an_error() {
    let (_dir, cg) = setup_csharp().await;
    let result = handle_tool_call(&cg, "tokensave_body", json!({}), None, None).await;
    assert!(result.is_err(), "neither symbol nor id should be an error");
}

#[tokio::test]
async fn unique_bare_name_returns_body() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "pub fn only_one(x: u32) -> u32 {\n    x * 2\n}\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let out = body_json(&cg, json!({"symbol": "only_one"})).await;
    assert_eq!(out["match_count"].as_u64(), Some(1), "{out}");
    assert!(out["matches"][0]["body"]
        .as_str()
        .is_some_and(|b| b.contains("x * 2")));
}
