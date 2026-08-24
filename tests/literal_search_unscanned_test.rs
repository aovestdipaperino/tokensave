//! A literal search must say when it could not look inside every tracked
//! file — #442.
//!
//! `literal: true` reads bytes rather than symbols, so it needs no parser, but
//! it iterates the indexed files: a file reaches it only if the index holds a
//! `files` row, which happens when a language extractor handles the extension
//! or the extension is listed in `artifact_extensions` (#323). A tracked
//! `.html` template is neither, so its matches were absent from the response
//! with nothing to say so — `count` read as "these are all the occurrences",
//! which is the reported harm: an agent told to prefer tokensave over `grep`
//! got a confidently incomplete answer and no signal to fall back.
//!
//! These tests pin the signal, and pin that the remedy the signal names
//! actually works. The reporter's own repro shape: a flag key that appears in
//! both a `.js` source file and a `.html` template.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The reporter's project: `someFlag` in a JavaScript source file and in an
/// HTML template, plus a stylesheet and an extensionless tracked file, all
/// committed so `git ls-files` reports them.
async fn project_with_untracked_extensions() -> (TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    git(&root, &["init", "-b", "master"]);
    std::fs::create_dir_all(root.join("routes/templates")).unwrap();
    std::fs::write(
        root.join("index.js"),
        "const features = { someFlag: true };\n",
    )
    .unwrap();
    std::fs::write(
        root.join("routes/templates/page.html"),
        "<div>{ k: 'someFlag' }</div>\n",
    )
    .unwrap();
    std::fs::write(root.join("styles.css"), "body { color: red; }\n").unwrap();
    std::fs::write(root.join("Makefile"), "all:\n\techo someFlag\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "init"]);

    let cg = TokenSave::init(&root).await.unwrap();
    cg.index_all().await.unwrap();
    (tmp, cg)
}

async fn literal_search(cg: &TokenSave, query: &str) -> Value {
    let result = handle_tool_call(
        cg,
        "tokensave_search",
        json!({ "query": query, "literal": true }),
        None,
        None,
    )
    .await
    .expect("literal search must succeed");
    let text = result.value["content"][0]["text"]
        .as_str()
        .expect("tool result carries text");
    serde_json::from_str(text).expect("literal search returns JSON")
}

/// Returns the per-extension file count from an `unscanned` block.
fn count_for(unscanned: &Value, extension: &str) -> Option<u64> {
    unscanned["extensions"]
        .as_array()?
        .iter()
        .find(|entry| entry["extension"] == extension)?["files"]
        .as_u64()
}

/// The bug: the `.html` hit is missing. What must not also be missing is the
/// statement that something was not searched.
#[tokio::test]
async fn a_partial_literal_answer_reports_what_it_could_not_reach() {
    let (_tmp, cg) = project_with_untracked_extensions().await;
    let payload = literal_search(&cg, "someFlag").await;

    // Precondition: this is genuinely the incomplete answer, not a fixed one.
    // If `.html` ever becomes indexable by default this assertion fails
    // loudly rather than the test quietly proving nothing.
    let files: Vec<&str> = payload["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        vec!["index.js"],
        "precondition: only the indexed source file is reachable"
    );

    let unscanned = &payload["unscanned"];
    assert!(
        !unscanned.is_null(),
        "a partial answer must carry an unscanned block, got {payload:#}"
    );
    assert_eq!(
        count_for(unscanned, "html"),
        Some(1),
        "the template holding the other hit must be named: {unscanned:#}"
    );
    assert_eq!(
        count_for(unscanned, "css"),
        Some(1),
        "a stylesheet is text a literal search could have matched: {unscanned:#}"
    );
    assert_eq!(
        count_for(unscanned, "(no extension)"),
        Some(1),
        "an extensionless tracked file must not vanish from the tally: {unscanned:#}"
    );
    assert_eq!(
        unscanned["files"], 3,
        "the count must match the tally: {unscanned:#}"
    );
    // The remedy has to name the setting, or the caller learns only that the
    // answer is wrong and not what to do about it.
    let remedy = unscanned["remedy"].as_str().unwrap_or_default();
    assert!(
        remedy.contains("artifact_extensions"),
        "the remedy must name the setting, got {remedy:?}"
    );
}

/// The remedy the report names has to work, or the signal is a dead end. This
/// is the test that would fail if `artifact_extensions` only affected
/// `tokensave_files` — which is how it was documented before #442.
#[tokio::test]
async fn adding_the_extension_makes_the_file_searchable_and_shrinks_the_report() {
    let (tmp, cg) = project_with_untracked_extensions().await;
    drop(cg);

    let config_path = tmp.path().join(".tokensave/config.json");
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config["artifact_extensions"]
        .as_array_mut()
        .expect("artifact_extensions is a list")
        .push(json!("html"));
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    let cg = TokenSave::open(tmp.path()).await.unwrap();
    cg.index_all().await.unwrap();
    let payload = literal_search(&cg, "someFlag").await;

    let files: Vec<&str> = payload["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap())
        .collect();
    assert!(
        files.contains(&"routes/templates/page.html"),
        "the template's line must now be searched, got {files:?}"
    );
    // No symbol encloses a line in a file that was never parsed, and the
    // response has to say that rather than attributing it to something.
    let template_hit = payload["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["file"] == "routes/templates/page.html")
        .unwrap();
    assert!(
        template_hit["enclosing"].is_null(),
        "an artifact hit has no enclosing symbol, got {template_hit:#}"
    );

    let unscanned = &payload["unscanned"];
    assert_eq!(
        count_for(unscanned, "html"),
        None,
        "html must leave the unscanned tally once it is indexed: {unscanned:#}"
    );
    assert_eq!(
        count_for(unscanned, "css"),
        Some(1),
        "the stylesheet is still unreachable and must still be reported: {unscanned:#}"
    );
}

/// A file the caller excluded is not an omission, so it must not be reported
/// as one — otherwise the block cries wolf on every project with a vendored
/// tree and callers learn to ignore it.
#[tokio::test]
async fn a_deliberately_excluded_file_is_not_reported_as_unscanned() {
    let (tmp, cg) = project_with_untracked_extensions().await;
    drop(cg);

    let config_path = tmp.path().join(".tokensave/config.json");
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config["exclude"]
        .as_array_mut()
        .expect("exclude is a list")
        .push(json!("**/*.css"));
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    let cg = TokenSave::open(tmp.path()).await.unwrap();
    let payload = literal_search(&cg, "someFlag").await;
    let unscanned = &payload["unscanned"];

    assert_eq!(
        count_for(unscanned, "css"),
        None,
        "an excluded file is an opt-out, not a gap: {unscanned:#}"
    );
    assert_eq!(
        count_for(unscanned, "html"),
        Some(1),
        "control: the template is still reported: {unscanned:#}"
    );
}

/// With nothing missing there must be no block at all, so its presence is a
/// real signal rather than boilerplate on every response.
#[tokio::test]
async fn a_complete_literal_answer_carries_no_unscanned_block() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    git(&root, &["init", "-b", "master"]);
    std::fs::write(root.join("index.js"), "const flag = { someFlag: 1 };\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "init"]);

    let cg = TokenSave::init(&root).await.unwrap();
    cg.index_all().await.unwrap();
    let payload = literal_search(&cg, "someFlag").await;

    assert_eq!(payload["count"], 1, "precondition: the hit is found");
    assert!(
        payload["unscanned"].is_null(),
        "every tracked file was searched, so nothing should be reported: {payload:#}"
    );
}
