//! A literal search scans the project's files in parallel chunks — #685.
//!
//! The scan reads every indexed file and matches the query against its text.
//! It used to run one file at a time, in order, on the async runtime, and it
//! asked the database for a file's nodes before it knew whether the file
//! matched at all. That made a repository-wide literal search several times
//! slower than `git grep` and queued every other tool call behind it in the
//! MCP server.
//!
//! The scan now runs in chunks, in parallel, inside the project's CPU profile
//! pool, and asks the database for a file's nodes only once the file has
//! matched. These tests pin the behaviour that could break in the process:
//! the scan still covers the whole project, it still reports the exact total
//! (#645), and it still reports matches in file order.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

/// Files in the fixture. The scan holds a chunk of files at a time, so the
/// fixture must be larger than one chunk to cross a chunk boundary.
const FILES: usize = 14;

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

/// A project whose files each hold one occurrence of the query, plus one file
/// that holds several.
async fn project() -> (TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    git(&root, &["init", "-b", "master"]);
    for i in 0..FILES {
        std::fs::write(
            root.join(format!("mod_{i:02}.py")),
            format!("def handler_{i:02}():\n    return 'needle_{i:02}'\n"),
        )
        .unwrap();
    }
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "init"]);

    let cg = TokenSave::init(&root).await.unwrap();
    cg.index_all().await.unwrap();
    (tmp, cg)
}

async fn literal_search(cg: &TokenSave, query: &str, limit: usize) -> Value {
    literal_search_with(cg, query, limit, json!({})).await
}

async fn literal_search_with(cg: &TokenSave, query: &str, limit: usize, extra: Value) -> Value {
    let mut args = json!({ "query": query, "literal": true, "format": "json", "limit": limit });
    if let (Some(object), Some(extra)) = (args.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            object.insert(key.clone(), value.clone());
        }
    }
    let result = handle_tool_call(cg, "tokensave_search", args, None, None)
        .await
        .expect("literal search must succeed");
    let text = result.value["content"][0]["text"]
        .as_str()
        .expect("tool result carries text");
    serde_json::from_str(text).expect("literal search returns JSON")
}

/// The files named by the reported matches, in the order they were reported.
fn reported_files(payload: &Value) -> Vec<&str> {
    payload["matches"]
        .as_array()
        .expect("matches is an array")
        .iter()
        .map(|m| m["file"].as_str().expect("match names a file"))
        .collect()
}

/// The scan covers the whole project, not just the first chunk: a match in a
/// file past the chunk boundary is still reported and still counted.
#[tokio::test]
async fn a_match_past_the_first_chunk_is_still_found() {
    let (_tmp, cg) = project().await;
    let payload = literal_search(&cg, "needle", FILES).await;

    assert_eq!(
        payload["total"].as_u64(),
        Some(FILES as u64),
        "every file holds one match, so the exact total is the file count"
    );
    assert_eq!(payload["count"].as_u64(), Some(FILES as u64));
    assert_eq!(payload["truncated"].as_bool(), Some(false));

    let files = reported_files(&payload);
    assert_eq!(files.len(), FILES, "every file is reported");
    assert!(
        files.contains(&"mod_00.py") && files.contains(&"mod_13.py"),
        "the scan covers the first and the last file: {files:?}"
    );
}

/// The reported matches stay in file order, so a caller that reads the first
/// few matches still sees them from the alphabetically first files.
#[tokio::test]
async fn matches_are_reported_in_file_order() {
    let (_tmp, cg) = project().await;
    let payload = literal_search(&cg, "needle", FILES).await;

    let files = reported_files(&payload);
    let mut sorted = files.clone();
    sorted.sort_unstable();
    assert_eq!(files, sorted, "matches are reported in file order");
}

/// A capped answer still reports the exact total of the whole project
/// (#645): the scan keeps counting past the cap instead of stopping at it.
#[tokio::test]
async fn a_capped_answer_still_reports_the_exact_total() {
    let (_tmp, cg) = project().await;
    let payload = literal_search(&cg, "needle", 3).await;

    assert_eq!(payload["count"].as_u64(), Some(3));
    assert_eq!(
        payload["total"].as_u64(),
        Some(FILES as u64),
        "the total counts the whole project, not only the reported matches"
    );
    assert_eq!(payload["truncated"].as_bool(), Some(true));
    assert_eq!(
        payload["total_exact"].as_bool(),
        Some(true),
        "the scan reached the end of the project, so the total is exact"
    );
}

/// `exact_total: false` trades the exact count for a scan that stops at the
/// limit. The answer then says the total is a lower bound rather than
/// passing a partial count off as the number of matches in the project.
#[tokio::test]
async fn an_inexact_total_scan_stops_at_the_limit_and_says_so() {
    let (_tmp, cg) = project().await;
    let payload = literal_search_with(&cg, "needle", 3, json!({ "exact_total": false })).await;

    assert_eq!(payload["count"].as_u64(), Some(3));
    assert_eq!(
        payload["total_exact"].as_bool(),
        Some(false),
        "the scan stopped at the limit, so the total is a lower bound"
    );
    assert_eq!(payload["truncated"].as_bool(), Some(true));
    assert!(
        payload["total"].as_u64().unwrap_or(0) >= 3,
        "the lower bound still covers the matches that were reported"
    );
}

/// A scan that finds fewer matches than the limit still reaches the end of
/// the project, so its total is exact even with `exact_total: false`.
#[tokio::test]
async fn an_inexact_total_scan_that_finishes_is_still_exact() {
    let (_tmp, cg) = project().await;
    let payload =
        literal_search_with(&cg, "needle", FILES * 4, json!({ "exact_total": false })).await;

    assert_eq!(payload["count"].as_u64(), Some(FILES as u64));
    assert_eq!(payload["total"].as_u64(), Some(FILES as u64));
    assert_eq!(payload["total_exact"].as_bool(), Some(true));
    assert_eq!(payload["truncated"].as_bool(), Some(false));
}

/// A project big enough that a scan cannot finish inside a millisecond.
///
/// The files are tracked as artifacts, so they are indexed without a parser
/// and the fixture stays cheap, but the literal scan still reads their text.
async fn large_project() -> (TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    git(&root, &["init", "-b", "master"]);
    let filler: String = (0..2_000)
        .map(|i| format!("  \"field_{i}\": \"filler value for the scan\",\n"))
        .collect();
    for i in 0..64 {
        std::fs::write(
            root.join(format!("data_{i:02}.json")),
            format!("{{\n{filler}  \"needle\": true\n}}\n"),
        )
        .unwrap();
    }
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "init"]);

    let cg = TokenSave::init(&root).await.unwrap();
    cg.index_all().await.unwrap();
    (tmp, cg)
}

/// A scan that runs out of time answers with the matches it found and says
/// the answer is partial, instead of holding the call until the whole project
/// has been read.
#[tokio::test]
async fn a_scan_that_runs_out_of_time_reports_a_partial_answer() {
    let (_tmp, cg) = large_project().await;
    let payload = literal_search_with(&cg, "needle", 500, json!({ "timeout_ms": 1 })).await;

    assert_eq!(
        payload["timed_out"].as_bool(),
        Some(true),
        "a scan that cannot finish in a millisecond stops early"
    );
    assert_eq!(
        payload["total_exact"].as_bool(),
        Some(false),
        "a scan that stopped early cannot report an exact total"
    );
    assert_eq!(payload["truncated"].as_bool(), Some(true));
    assert!(
        payload["count"].as_u64().unwrap_or(0) >= 1,
        "the matches found before the deadline are still reported"
    );
    assert!(
        payload["note"]
            .as_str()
            .is_some_and(|note| !note.is_empty()),
        "a partial answer explains itself"
    );
}

/// A scan with the budget turned off runs to the end of the project, so it
/// reports an exact total even though it took longer than a millisecond.
#[tokio::test]
async fn a_scan_without_a_budget_reports_an_exact_total() {
    let (_tmp, cg) = large_project().await;
    let payload = literal_search_with(&cg, "needle", 500, json!({ "timeout_ms": 0 })).await;

    assert_eq!(payload["timed_out"].as_bool(), None);
    assert_eq!(payload["total_exact"].as_bool(), Some(true));
    assert_eq!(payload["total"].as_u64(), Some(64));
}
