//! Regression tests for #671: TypeScript `implements I<T>` and interface
//! `extends I<T>` recorded no implementation.
//!
//! * In tree-sitter-typescript a heritage type with type arguments is a
//!   `generic_type` node, not a `type_identifier`, so the extractor skipped
//!   `implements IRenderer<Row>` entirely and only `implements IRenderer`
//!   produced an edge.
//! * An interface's `extends` clause (`extends_type_clause`) was not read at
//!   all, so `interface IValidatingRenderer<R, V> extends IRenderer<R, V>`
//!   was invisible to `tokensave_implementations` and
//!   `tokensave_type_hierarchy`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::extraction::{LanguageExtractor, TypeScriptExtractor};
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

const RENDERER_SRC: &str = r#"
export interface Row { id: string }

export interface IRenderer<R = object, V = any> {
    render(row: R, value: V): string;
}

export class PlainRenderer implements IRenderer {
    render(row: object, value: any): string { return String(value); }
}

export class TypedRenderer implements IRenderer<Row> {
    render(row: Row, value: any): string { return row.id + value; }
}

export abstract class BaseRenderer<R extends object, V> implements IRenderer<R, V> {
    abstract render(row: R, value: V): string;
}

export interface IValidatingRenderer<R, V> extends IRenderer<R, V> {
    validate(value: V): boolean;
}
"#;

const ALL_IMPLEMENTERS: [&str; 4] = [
    "BaseRenderer",
    "IValidatingRenderer",
    "PlainRenderer",
    "TypedRenderer",
];

fn extract_text(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

#[test]
fn generic_heritage_types_are_recorded_without_type_arguments() {
    for file in ["renderer.ts", "renderer.tsx"] {
        let result = TypeScriptExtractor.extract(file, RENDERER_SRC);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let bases: Vec<&str> = result
            .unresolved_refs
            .iter()
            .filter(|r| matches!(r.reference_kind, EdgeKind::Extends | EdgeKind::Implements))
            .map(|r| r.reference_name.as_str())
            .collect();
        assert!(
            bases.iter().all(|b| !b.contains('<')),
            "{file}: heritage refs must not carry type arguments: {bases:?}"
        );
        assert_eq!(
            bases.iter().filter(|b| **b == "IRenderer").count(),
            4,
            "{file}: all four implementers must reference `IRenderer`: {bases:?}"
        );
    }
}

async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/renderer.ts"), RENDERER_SRC).unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

#[tokio::test]
async fn implementations_finds_generic_typescript_implementers() {
    let (_dir, cg) = setup().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_implementations",
        json!({ "trait": "IRenderer" }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    let output: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({ "raw": text }));
    let mut names: Vec<String> = output["implementations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| e["type"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    assert_eq!(names, ALL_IMPLEMENTERS, "implementations output: {text}");
}

#[tokio::test]
async fn type_hierarchy_lists_generic_typescript_implementers() {
    let (_dir, cg) = setup().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let iface = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Interface && n.name == "IRenderer")
        .expect("IRenderer node");
    let result = handle_tool_call(
        &cg,
        "tokensave_type_hierarchy",
        json!({ "node_id": iface.id }),
        None,
        None,
    )
    .await
    .unwrap();
    let text = extract_text(&result.value);
    for name in ALL_IMPLEMENTERS {
        assert!(
            text.contains(name),
            "type_hierarchy(IRenderer) should list {name}: {text}"
        );
    }
}
