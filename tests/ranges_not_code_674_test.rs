//! Ranges, not code (#674).
//!
//! * Every symbol in the results of `tokensave_search`, `tokensave_callers`,
//!   `tokensave_callees`, `tokensave_impact`, `tokensave_implementations` and
//!   `tokensave_type_hierarchy` carries `end_line` next to `line`, so a caller
//!   can read exactly the span it needs instead of a body or a whole file.
//! * `tokensave_implementations` returns locations and signatures by default;
//!   `bodies: true` inlines the source bodies as before.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

/// Line numbers below are 1-based editor lines into this source.
const SHAPES_SRC: &str = "export interface Shape {
    area(): number;
}

export class Circle implements Shape {
    radius = 2;

    area(): number {
        const r = this.radius;
        return r * r * 3;
    }
}

export function total(s: Shape): number {
    const a = s.area();
    return a;
}

export function report(s: Shape): number {
    const t = total(s);
    return t + 1;
}
";

/// The source of `Circle.area`, lines 8..=11.
const AREA_BODY: &str = "    area(): number {
        const r = this.radius;
        return r * r * 3;
    }";

async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/shapes.ts"), SHAPES_SRC).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

fn extract_text(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

async fn call(cg: &TokenSave, tool: &str, args: Value) -> String {
    let result = handle_tool_call(cg, tool, args, None, None)
        .await
        .unwrap_or_else(|e| panic!("{tool} failed: {e}"));
    extract_text(&result.value).to_string()
}

async fn call_json(cg: &TokenSave, tool: &str, args: Value) -> Value {
    let text = call(cg, tool, args).await;
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{tool}: {e}: {text}"))
}

async fn node_id(cg: &TokenSave, name: &str) -> String {
    let items = call_json(
        cg,
        "tokensave_search",
        json!({ "query": name, "format": "json", "ids": true }),
    )
    .await;
    items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("'{name}' not found: {items}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn find<'a>(items: &'a Value, key: &str, name: &str) -> &'a Value {
    items
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {items}"))
        .iter()
        .find(|item| item[key].as_str() == Some(name))
        .unwrap_or_else(|| panic!("no {key}={name} in {items}"))
}

#[tokio::test]
async fn search_reports_end_line_in_json_and_text() {
    let (_dir, cg) = setup().await;
    let items = call_json(
        &cg,
        "tokensave_search",
        json!({ "query": "total", "format": "json" }),
    )
    .await;
    let total = find(&items, "name", "total");
    assert_eq!(total["line"], 14, "{total}");
    assert_eq!(total["end_line"], 17, "{total}");

    let text = call(&cg, "tokensave_search", json!({ "query": "total" })).await;
    assert!(
        text.contains("src/shapes.ts:14-17: total (function)"),
        "text output must carry line-end_line: {text}"
    );
}

#[tokio::test]
async fn callers_report_the_callers_end_line() {
    let (_dir, cg) = setup().await;
    let id = node_id(&cg, "total").await;
    let items = call_json(&cg, "tokensave_callers", json!({ "node_id": id })).await;
    let report = find(&items, "name", "report");
    assert_eq!(report["line"], 20, "call-site line unchanged: {report}");
    assert_eq!(report["def_line"], 19, "{report}");
    assert_eq!(report["end_line"], 22, "{report}");
}

#[tokio::test]
async fn callees_report_end_line() {
    let (_dir, cg) = setup().await;
    let id = node_id(&cg, "report").await;
    let items = call_json(&cg, "tokensave_callees", json!({ "node_id": id })).await;
    let total = find(&items, "name", "total");
    assert_eq!(total["line"], 14, "{total}");
    assert_eq!(total["end_line"], 17, "{total}");
}

#[tokio::test]
async fn impact_reports_end_line() {
    let (_dir, cg) = setup().await;
    let id = node_id(&cg, "total").await;
    let out = call_json(&cg, "tokensave_impact", json!({ "node_id": id })).await;
    let report = find(&out["nodes"], "name", "report");
    assert_eq!(report["line"], 19, "{report}");
    assert_eq!(report["end_line"], 22, "{report}");
}

#[tokio::test]
async fn type_hierarchy_reports_line_ranges() {
    let (_dir, cg) = setup().await;
    let id = node_id(&cg, "Shape").await;
    let text = call(&cg, "tokensave_type_hierarchy", json!({ "node_id": id })).await;
    assert!(
        text.starts_with("Shape (interface) -- src/shapes.ts:1-3\n"),
        "root carries its range: {text}"
    );
    assert!(
        text.contains("Circle (class) -- src/shapes.ts:5-12"),
        "implementer carries its range: {text}"
    );
}

#[tokio::test]
async fn implementations_default_is_locations_and_signatures_without_bodies() {
    let (_dir, cg) = setup().await;
    let out = call_json(
        &cg,
        "tokensave_implementations",
        json!({ "trait": "Shape" }),
    )
    .await;
    let circle = find(&out["implementations"], "type", "Circle");
    assert_eq!(circle["file"], "src/shapes.ts", "{circle}");
    assert_eq!(circle["line"], 5, "{circle}");
    assert_eq!(circle["end_line"], 12, "{circle}");
    let area = find(&circle["methods"], "name", "area");
    assert_eq!(area["line"], 8, "{area}");
    assert_eq!(area["end_line"], 11, "{area}");
    assert!(
        area["signature"]
            .as_str()
            .is_some_and(|s| s.contains("area")),
        "method keeps its signature: {area}"
    );
    assert!(area.get("body").is_none(), "no body by default: {area}");

    let out = call_json(
        &cg,
        "tokensave_implementations",
        json!({ "method": "area" }),
    )
    .await;
    let entries = out["implementations"].as_array().unwrap();
    let area = entries
        .iter()
        .find(|e| e["line"] == 8)
        .unwrap_or_else(|| panic!("Circle.area missing: {out}"));
    assert_eq!(area["end_line"], 11, "{area}");
    assert!(area["signature"].is_string(), "{area}");
    assert!(
        entries.iter().all(|e| e.get("body").is_none()),
        "no bodies by default: {out}"
    );
}

#[tokio::test]
async fn implementations_bodies_true_inlines_the_source() {
    let (_dir, cg) = setup().await;
    let out = call_json(
        &cg,
        "tokensave_implementations",
        json!({ "trait": "Shape", "bodies": true }),
    )
    .await;
    let circle = find(&out["implementations"], "type", "Circle");
    let area = find(&circle["methods"], "name", "area");
    assert_eq!(area["body"], AREA_BODY, "{area}");
    assert_eq!(area["line"], 8, "{area}");
    assert!(area["signature"].is_string(), "{area}");

    let out = call_json(
        &cg,
        "tokensave_implementations",
        json!({ "method": "area", "bodies": true }),
    )
    .await;
    let area = out["implementations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["line"] == 8)
        .unwrap_or_else(|| panic!("Circle.area missing: {out}"))
        .clone();
    assert_eq!(area["body"], AREA_BODY, "{area}");
    assert_eq!(area["end_line"], 11, "{area}");
}
