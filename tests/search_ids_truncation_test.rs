//! `tokensave_search` output contracts for #645 and #646.
//!
//! #646: `ids: true` only surfaced node ids in `format: "json"`; the default
//! text format dropped them, so the ids could not feed callers/callees/impact.
//!
//! #645: a literal search stopped at `limit` and reported `count: <limit>`
//! with no sign that more matches existed, so a capped answer read as the
//! complete set.

use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

async fn project() -> (TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn refresh_one() {\n    call_refresh_async();\n}\n\n\
         pub fn refresh_two() {\n    call_refresh_async();\n    call_refresh_async();\n}\n\n\
         pub fn refresh_three() {\n    call_refresh_async();\n    call_refresh_async();\n}\n\n\
         pub fn call_refresh_async() {}\n",
    )
    .unwrap();
    let cg = TokenSave::init(&root).await.unwrap();
    cg.index_all().await.unwrap();
    (tmp, cg)
}

async fn search_text(cg: &TokenSave, args: Value) -> String {
    let result = handle_tool_call(cg, "tokensave_search", args, None, None)
        .await
        .expect("search must succeed");
    result.value["content"][0]["text"]
        .as_str()
        .expect("tool result carries text")
        .to_string()
}

#[tokio::test]
async fn ranked_text_format_prints_ids_when_requested() {
    let (_tmp, cg) = project().await;
    let json_text = search_text(
        &cg,
        json!({ "query": "refresh_two", "ids": true, "format": "json" }),
    )
    .await;
    let items: Value = serde_json::from_str(&json_text).unwrap();
    let id = items
        .as_array()
        .and_then(|a| a.iter().find(|i| i["name"] == "refresh_two"))
        .and_then(|i| i["id"].as_str())
        .expect("json carries the id")
        .to_string();

    let text = search_text(&cg, json!({ "query": "refresh_two", "ids": true })).await;
    assert!(
        text.contains(&id),
        "text format with ids: true must include node id {id}:\n{text}"
    );

    let plain = search_text(&cg, json!({ "query": "refresh_two" })).await;
    assert!(
        !plain.contains(&id),
        "ids must stay out of the text format unless requested:\n{plain}"
    );
}

#[tokio::test]
async fn literal_text_format_prints_enclosing_ids_when_requested() {
    let (_tmp, cg) = project().await;
    let json_text = search_text(
        &cg,
        json!({ "query": "call_refresh_async();", "literal": true, "ids": true,
                "format": "json", "limit": 1 }),
    )
    .await;
    let payload: Value = serde_json::from_str(&json_text).unwrap();
    let enclosing_id = payload["matches"][0]["enclosing_id"]
        .as_str()
        .expect("json carries enclosing_id")
        .to_string();

    let text = search_text(
        &cg,
        json!({ "query": "call_refresh_async();", "literal": true, "ids": true, "limit": 1 }),
    )
    .await;
    assert!(
        text.contains(&enclosing_id),
        "literal text with ids: true must include enclosing id {enclosing_id}:\n{text}"
    );
}

#[tokio::test]
async fn literal_search_reports_truncation_and_total() {
    let (_tmp, cg) = project().await;
    let json_text = search_text(
        &cg,
        json!({ "query": "call_refresh_async();", "literal": true,
                "format": "json", "limit": 2 }),
    )
    .await;
    let payload: Value = serde_json::from_str(&json_text).unwrap();
    assert_eq!(payload["count"], 2, "{payload}");
    assert_eq!(payload["total"], 5, "{payload}");
    assert_eq!(payload["truncated"], true, "{payload}");

    let text = search_text(
        &cg,
        json!({ "query": "call_refresh_async();", "literal": true, "limit": 2 }),
    )
    .await;
    assert!(
        text.starts_with("count: 2 of 5 (truncated"),
        "text header must say the result was cut:\n{text}"
    );
}

#[tokio::test]
async fn literal_search_under_limit_is_not_truncated() {
    let (_tmp, cg) = project().await;
    let json_text = search_text(
        &cg,
        json!({ "query": "call_refresh_async();", "literal": true,
                "format": "json", "limit": 50 }),
    )
    .await;
    let payload: Value = serde_json::from_str(&json_text).unwrap();
    assert_eq!(payload["count"], 5, "{payload}");
    assert_eq!(payload["total"], 5, "{payload}");
    assert_eq!(payload["truncated"], false, "{payload}");

    let text = search_text(
        &cg,
        json!({ "query": "call_refresh_async();", "literal": true, "limit": 50 }),
    )
    .await;
    assert!(text.starts_with("count: 5\n"), "{text}");
}

#[tokio::test]
async fn ranked_text_format_signals_more_results() {
    let (_tmp, cg) = project().await;
    let text = search_text(&cg, json!({ "query": "refresh", "limit": 1 })).await;
    assert!(
        text.starts_with("count: 1 (truncated"),
        "ranked text header must say more results exist:\n{text}"
    );

    let text = search_text(&cg, json!({ "query": "refresh", "limit": 200 })).await;
    assert!(
        !text.contains("truncated"),
        "a complete ranked result must not claim truncation:\n{text}"
    );
}
