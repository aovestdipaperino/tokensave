//! #458, second half: `tokensave_constructors` must refuse a language it
//! cannot scan, rather than report a confident zero.
//!
//! The scan looks for `Name { ... }` literal syntax, which is how Rust and Go
//! construct values and nobody else does — Python builds with `Name(...)`,
//! Java with `new Name(...)`. Against those it finds nothing and used to
//! return `{"match_count": 0, "sites": []}`, which is indistinguishable from
//! "this type is never constructed". The caveat in the tool description did
//! not help: a clean zero is the reading a caller takes, and this tool's name
//! promises very nearly what an impact review needs.

use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

async fn project(file: &str, source: &str) -> (TempDir, TokenSave) {
    let tmp = tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(file), source).expect("write source");
    let cg = TokenSave::init(tmp.path()).await.expect("init");
    cg.index_all().await.expect("index");
    (tmp, cg)
}

async fn constructors(cg: &TokenSave, name: &str) -> Value {
    let result = handle_tool_call(
        cg,
        "tokensave_constructors",
        json!({ "struct": name }),
        None,
        None,
    )
    .await
    .expect("constructors must succeed");
    let text = result.value["content"][0]["text"]
        .as_str()
        .expect("tool result carries text");
    serde_json::from_str(text).expect("constructors returns JSON")
}

#[tokio::test]
async fn a_python_class_is_refused_rather_than_reported_as_never_constructed() {
    let (_tmp, cg) = project(
        "models.py",
        "class Settings:\n\
         \x20   def __init__(self, host):\n\
         \x20       self.host = host\n\
         \n\
         def make():\n\
         \x20   return Settings(\"localhost\")\n",
    )
    .await;

    let result = constructors(&cg, "Settings").await;
    assert_eq!(result["language_supported"], false);
    // The whole point: no zero to misread.
    assert!(
        result.get("match_count").is_none(),
        "a count that can only ever be zero must not be reported at all: {result}"
    );
    assert!(result.get("sites").is_none(), "nor an empty site list");
    let note = result["note"].as_str().expect("a note explaining why");
    assert!(
        note.contains("Settings") && note.contains("models.py"),
        "the note must name the type and where it is declared: {note}"
    );
}

#[tokio::test]
async fn a_rust_struct_still_gets_a_real_answer() {
    let (_tmp, cg) = project(
        "lib.rs",
        "pub struct Settings { pub host: String, pub port: u16 }\n\
         pub fn make() -> Settings {\n\
         \x20   Settings { host: String::new(), port: 0 }\n\
         }\n",
    )
    .await;

    let result = constructors(&cg, "Settings").await;
    assert_eq!(result["language_supported"], true);
    assert_eq!(
        result["match_count"], 1,
        "the Rust path must be untouched: {result}"
    );
}

/// Go is the other language with this syntax, so it must not be swept up in
/// the refusal — and its function signatures must not be mistaken for
/// construction sites. Go writes a return type where Rust writes `-> T`, so
/// `func make() Settings {` put the type between a `)` and the body brace and
/// was counted as a literal, with a `missing_fields` list naming every field:
/// advice to "fix" a declaration that constructs nothing.
#[tokio::test]
async fn a_go_struct_still_gets_a_real_answer() {
    let (_tmp, cg) = project(
        "main.go",
        "package main\n\n\
         type Settings struct {\n\
         \x20   Host string\n\
         }\n\n\
         func make() Settings {\n\
         \x20   return Settings{Host: \"localhost\"}\n\
         }\n",
    )
    .await;

    let result = constructors(&cg, "Settings").await;
    assert_eq!(result["language_supported"], true, "got: {result}");
    assert_eq!(
        result["match_count"], 1,
        "only the literal counts, not the function's return type: {result}"
    );
    assert_eq!(result["sites"][0]["line"], 8, "got: {result}");
}
