//! `tokensave_read` line numbering (#652) and client-held digest
//! revalidation via `if_digest` (#650).

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

const SOURCE: &str = "fn main() {\n    let x = helper();\n}\n\nfn helper() -> i32 {\n    42\n}\n";

async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), SOURCE).unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn read(cg: &TokenSave, args: Value) -> String {
    let result = handle_tool_call(cg, "tokensave_read", args, None, None)
        .await
        .unwrap();
    result.value["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn read_json(cg: &TokenSave, mut args: Value) -> Value {
    args["format"] = json!("json");
    serde_json::from_str(&read(cg, args).await).unwrap()
}

/// Extracts the `digest:` header line from a text-format response.
fn text_digest(text: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("digest: "))
        .unwrap_or_else(|| panic!("no digest header in: {text}"))
        .to_string()
}

#[tokio::test]
async fn full_mode_numbers_every_line_like_read_tool() {
    let (_dir, cg) = setup().await;
    let v = read_json(&cg, json!({ "file": "src/main.rs", "mode": "full" })).await;
    let body = v["body"].as_str().unwrap();
    let expected = "     1\tfn main() {\n     2\t    let x = helper();\n     3\t}\n     4\t\n     5\tfn helper() -> i32 {\n     6\t    42\n     7\t}";
    assert_eq!(body, expected);
}

#[tokio::test]
async fn lines_mode_numbers_with_real_file_line_numbers() {
    let (_dir, cg) = setup().await;
    let v = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "lines": "5-6" }),
    )
    .await;
    assert_eq!(
        v["body"].as_str().unwrap(),
        "     5\tfn helper() -> i32 {\n     6\t    42"
    );
}

#[tokio::test]
async fn text_format_body_is_numbered() {
    let (_dir, cg) = setup().await;
    let text = read(&cg, json!({ "file": "src/main.rs", "mode": "full" })).await;
    assert!(text.contains("\n     1\tfn main() {"), "{text}");
    assert!(text.contains("digest: "), "{text}");
}

#[tokio::test]
async fn repeated_reads_always_return_the_body() {
    let (_dir, cg) = setup().await;
    for _ in 0..3 {
        let text = read(&cg, json!({ "file": "src/main.rs" })).await;
        assert!(text.contains("fn helper"), "{text}");
        assert!(!text.contains("unchanged"), "{text}");
    }
}

#[tokio::test]
async fn matching_if_digest_returns_unchanged_stub() {
    let (_dir, cg) = setup().await;
    let first = read(&cg, json!({ "file": "src/main.rs" })).await;
    let digest = text_digest(&first);

    let stub = read(&cg, json!({ "file": "src/main.rs", "if_digest": digest })).await;
    assert!(stub.contains("unchanged: true"), "{stub}");
    assert!(stub.contains(&format!("digest: {digest}")), "{stub}");
    assert!(!stub.contains("fn helper"), "{stub}");

    let stub_json = read_json(&cg, json!({ "file": "src/main.rs", "if_digest": digest })).await;
    assert_eq!(stub_json["unchanged"], json!(true));
    assert_eq!(stub_json["digest"], json!(digest));
    assert!(stub_json.get("body").is_none(), "{stub_json}");
}

#[tokio::test]
async fn mismatched_if_digest_returns_body_with_digest() {
    let (_dir, cg) = setup().await;
    let v = read_json(
        &cg,
        json!({ "file": "src/main.rs", "if_digest": "not-the-digest" }),
    )
    .await;
    assert!(v["body"].as_str().unwrap().contains("fn helper"), "{v}");
    assert!(v.get("unchanged").is_none(), "{v}");
    assert_eq!(v["digest"].as_str().unwrap().len(), 64, "{v}");
}

#[tokio::test]
async fn if_digest_is_scoped_to_mode_and_range() {
    let (_dir, cg) = setup().await;
    let slice = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "lines": "1-3" }),
    )
    .await;
    let slice_digest = slice["digest"].as_str().unwrap().to_string();

    let other = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "lines": "5-7", "if_digest": slice_digest }),
    )
    .await;
    assert!(other.get("unchanged").is_none(), "{other}");
    assert!(other["body"].as_str().unwrap().contains("fn helper"));

    let full = read_json(
        &cg,
        json!({ "file": "src/main.rs", "if_digest": slice_digest }),
    )
    .await;
    assert!(full.get("unchanged").is_none(), "{full}");
}

#[tokio::test]
async fn edited_file_invalidates_held_digest() {
    let (dir, cg) = setup().await;
    let v = read_json(&cg, json!({ "file": "src/main.rs" })).await;
    let digest = v["digest"].as_str().unwrap().to_string();

    fs::write(dir.path().join("src/main.rs"), SOURCE.replace("42", "43")).unwrap();

    let after = read_json(&cg, json!({ "file": "src/main.rs", "if_digest": digest })).await;
    assert!(after.get("unchanged").is_none(), "{after}");
    assert!(after["body"].as_str().unwrap().contains("43"), "{after}");
    assert_ne!(after["digest"].as_str().unwrap(), digest);
}

#[tokio::test]
async fn force_is_still_accepted_and_returns_the_body() {
    let (_dir, cg) = setup().await;
    let first = read(&cg, json!({ "file": "src/main.rs" })).await;
    let digest = text_digest(&first);
    // `force` is deprecated and a no-op on its own; it also overrides a
    // matching `if_digest` so old callers that pass it keep getting bodies.
    let text = read(
        &cg,
        json!({ "file": "src/main.rs", "force": true, "if_digest": digest }),
    )
    .await;
    assert!(text.contains("fn helper"), "{text}");
    assert!(!text.contains("unchanged"), "{text}");
}

#[tokio::test]
async fn map_mode_body_carries_digest_and_honours_if_digest() {
    let (_dir, cg) = setup().await;
    let v = read_json(&cg, json!({ "file": "src/main.rs", "mode": "map" })).await;
    let digest = v["digest"].as_str().unwrap().to_string();
    assert!(v["body"].as_str().unwrap().contains("helper"), "{v}");
    let stub = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "map", "if_digest": digest }),
    )
    .await;
    assert_eq!(stub["unchanged"], json!(true), "{stub}");
}

// ---------------------------------------------------------------------------
// Argument aliases: `path` for `file`, `start`/`end` for `lines`. Most tools
// take `path`, and `tokensave_replace_lines` takes `path`/`start`/`end` for
// the same line-range concept, so agents send that shape here too.
// ---------------------------------------------------------------------------

async fn read_err(cg: &TokenSave, args: Value) -> String {
    match handle_tool_call(cg, "tokensave_read", args, None, None).await {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    }
}

#[tokio::test]
async fn path_alias_reads_file() {
    let (_dir, cg) = setup().await;
    let by_file = read_json(&cg, json!({ "file": "src/main.rs" })).await;
    let by_path = read_json(&cg, json!({ "path": "src/main.rs" })).await;
    assert_eq!(by_path["body"], by_file["body"], "{by_path}");
}

#[tokio::test]
async fn start_end_strings_slice_lines() {
    // Regression: the exact argument shape a session sent ten times and got
    // two errors per call for.
    let (_dir, cg) = setup().await;
    let by_lines = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "lines": "5-6" }),
    )
    .await;
    let by_start_end = read_json(
        &cg,
        json!({ "path": "src/main.rs", "mode": "lines", "start": "5", "end": "6" }),
    )
    .await;
    assert_eq!(by_start_end["body"], by_lines["body"], "{by_start_end}");
    assert_eq!(by_start_end["digest"], by_lines["digest"], "{by_start_end}");
}

#[tokio::test]
async fn start_end_numbers_slice_lines() {
    let (_dir, cg) = setup().await;
    let v = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "start": 5, "end": 6 }),
    )
    .await;
    assert_eq!(
        v["body"].as_str().unwrap(),
        "     5\tfn helper() -> i32 {\n     6\t    42"
    );
}

#[tokio::test]
async fn start_without_end_reads_to_end_of_file() {
    let (_dir, cg) = setup().await;
    let v = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "start": 6 }),
    )
    .await;
    assert_eq!(v["body"].as_str().unwrap(), "     6\t    42\n     7\t}");
}

#[tokio::test]
async fn range_argument_infers_lines_mode() {
    let (_dir, cg) = setup().await;
    let v = read_json(&cg, json!({ "file": "src/main.rs", "lines": "6-7" })).await;
    assert_eq!(v["body"].as_str().unwrap(), "     6\t    42\n     7\t}");
    assert_eq!(v["mode"], json!("lines"), "{v}");
    let v = read_json(&cg, json!({ "file": "src/main.rs", "start": 6, "end": 7 })).await;
    assert_eq!(v["body"].as_str().unwrap(), "     6\t    42\n     7\t}");
    // An explicit mode still wins.
    let v = read_json(
        &cg,
        json!({ "file": "src/main.rs", "mode": "full", "start": 6 }),
    )
    .await;
    assert!(
        v["body"].as_str().unwrap().starts_with("     1\tfn main()"),
        "{v}"
    );
}

#[tokio::test]
async fn invalid_start_end_is_reported_like_an_invalid_lines_value() {
    let (_dir, cg) = setup().await;
    let msg = read_err(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "start": 0 }),
    )
    .await;
    assert!(msg.contains("invalid"), "{msg}");
    let msg = read_err(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "start": 6, "end": 5 }),
    )
    .await;
    assert!(msg.contains("invalid"), "{msg}");
    let msg = read_err(
        &cg,
        json!({ "file": "src/main.rs", "mode": "lines", "start": "six" }),
    )
    .await;
    assert!(msg.contains("invalid"), "{msg}");
}

#[tokio::test]
async fn missing_arguments_name_both_spellings() {
    let (_dir, cg) = setup().await;
    let msg = read_err(&cg, json!({ "mode": "full" })).await;
    assert!(msg.contains("file") && msg.contains("path"), "{msg}");
    let msg = read_err(&cg, json!({ "file": "src/main.rs", "mode": "lines" })).await;
    assert!(msg.contains("lines") && msg.contains("start"), "{msg}");
}
