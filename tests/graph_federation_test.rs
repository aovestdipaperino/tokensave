//! Federating one query across several `graph_root`s — #376.
//!
//! Selecting another project already works one root at a time (#363), and
//! #375 made neighbouring roots discoverable. This is the remaining half:
//! answering a single query against several roots instead of requiring one
//! call per root.
//!
//! Two properties matter more than the fan-out itself.
//!
//! **Roots that are worktrees of one repository must collapse.** @bobbypierce42
//! reported a machine with 100+ tracked projects where a dozen roots are
//! worktrees of a single repo — near-identical trees differing by one branch's
//! changes, each with its own `.tokensave/`. Round-robin across those fills the
//! result set with copies of the same symbol at slightly different line
//! numbers, and a per-root cap does not help because each worktree *is* its own
//! root and sits under the cap. Worktrees of one repo share a
//! `git rev-parse --git-common-dir`, which is the signal used to collapse them.
//!
//! **A tool whose answer is a property of one graph must refuse an array**
//! rather than silently unioning. `circular`, `hotspots` and `dsm` describe the
//! shape of a single graph; a union of two is not a bigger answer, it is a
//! meaningless one.

#![cfg(feature = "test-transport")]

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tokensave::mcp::transport::ChannelTransport;
use tokensave::mcp::McpServer;
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

/// Indexes a standalone project containing one uniquely-named function plus a
/// shared name every project defines, so federation has both something unique
/// to find and something that appears everywhere.
async fn project(dir: &Path, unique: &str) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        format!("pub fn {unique}() -> i32 {{ 1 }}\npub fn process_batch() -> i32 {{ 2 }}\n"),
    )
    .unwrap();
    let cg = TokenSave::init(dir).await.unwrap();
    cg.sync().await.unwrap();
}

async fn call(server: &Arc<McpServer>, name: &str, arguments: Value) -> Value {
    let (mut transport, _sender, mut receiver) = ChannelTransport::new();
    let request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
    .to_string();
    server.handle_and_write(&request, &mut transport).await;
    let raw = receiver.recv().await.expect("expected a response");
    serde_json::from_str(raw.trim()).expect("valid JSON-RPC")
}

/// The concatenated text of every content block, which is where tool payloads
/// live.
fn text_of(response: &Value) -> String {
    response["result"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A served project plus two independent neighbours.
async fn served_with_two_neighbours() -> (TempDir, Arc<McpServer>, String, String) {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    let served = base.join("served");
    let a = base.join("service_a");
    let b = base.join("shared_lib");
    std::fs::create_dir_all(&served).unwrap();
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();

    project(&served, "served_only").await;
    project(&a, "alpha_only").await;
    project(&b, "beta_only").await;

    let cg = TokenSave::open(&served).await.unwrap();
    let server = McpServer::new_explicit_root(cg, None).await;
    (
        tmp,
        server,
        a.canonicalize().unwrap().to_string_lossy().to_string(),
        b.canonicalize().unwrap().to_string_lossy().to_string(),
    )
}

/// The worked example from the issue: one query, two roots, both answered.
#[tokio::test]
async fn search_federates_across_two_roots() {
    let (_tmp, server, a, b) = served_with_two_neighbours().await;

    let response = call(
        &server,
        "tokensave_search",
        json!({ "query": "process_batch", "graph_root": [a, b] }),
    )
    .await;

    assert!(
        response.get("error").is_none(),
        "an array of roots must be accepted, got {response:?}"
    );
    let text = text_of(&response);
    assert!(
        text.contains("service_a") && text.contains("shared_lib"),
        "results must carry provenance from both roots, got: {text}"
    );
}

/// A single-element array must behave exactly like the string form, so callers
/// need not special-case one root.
#[tokio::test]
async fn a_single_element_array_matches_the_string_form() {
    let (_tmp, server, a, _b) = served_with_two_neighbours().await;

    let as_string = call(
        &server,
        "tokensave_search",
        json!({ "query": "alpha_only", "graph_root": a.clone() }),
    )
    .await;
    let as_array = call(
        &server,
        "tokensave_search",
        json!({ "query": "alpha_only", "graph_root": [a] }),
    )
    .await;

    assert!(as_string.get("error").is_none(), "{as_string:?}");
    assert_eq!(
        text_of(&as_string),
        text_of(&as_array),
        "one root named as an array must answer identically to the string form"
    );
}

/// @bobbypierce42's case: a repo and its own worktree are one source, not two.
/// Without collapsing, both would occupy slots with near-identical symbols.
#[tokio::test]
async fn worktrees_of_one_repo_collapse_to_a_single_root() {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path();

    let served = base.join("served");
    let main = base.join("main_repo");
    std::fs::create_dir_all(&served).unwrap();
    std::fs::create_dir_all(&main).unwrap();
    project(&served, "served_only").await;

    // A real repo, then a real linked worktree of it, each indexed.
    std::fs::create_dir_all(main.join("src")).unwrap();
    std::fs::write(
        main.join("src/lib.rs"),
        "pub fn process_batch() -> i32 { 1 }\n",
    )
    .unwrap();
    git(&main, &["init", "-b", "master"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-m", "base"]);
    let cg = TokenSave::init(&main).await.unwrap();
    cg.sync().await.unwrap();
    drop(cg);

    let wt = base.join("main_repo_wt");
    git(
        &main,
        &["worktree", "add", "-b", "feature", wt.to_str().unwrap()],
    );
    let cg = TokenSave::init(&wt).await.unwrap();
    cg.sync().await.unwrap();
    drop(cg);

    let cg = TokenSave::open(&served).await.unwrap();
    let server = McpServer::new_explicit_root(cg, None).await;

    let roots = json!([
        main.canonicalize().unwrap().to_string_lossy(),
        wt.canonicalize().unwrap().to_string_lossy()
    ]);
    let response = call(
        &server,
        "tokensave_search",
        json!({ "query": "process_batch", "graph_root": roots }),
    )
    .await;

    assert!(response.get("error").is_none(), "{response:?}");
    let text = text_of(&response);

    assert!(
        text.contains("federated across 1 root"),
        "the worktree and its repo must count as one source, got: {text}"
    );
    assert!(
        text.contains("collapsed 1 worktree"),
        "the collapse must be reported, not silent — a caller who named a root \
         and never sees it again is owed the reason. Got: {text}"
    );

    // The repo kept is the one named first, so the caller's ordering decides
    // which checkout represents the repository.
    let main_path = main.canonicalize().unwrap().to_string_lossy().to_string();
    assert!(
        text.contains(&format!("federated across 1 root(s): {main_path}")),
        "the first-named checkout must be the one kept, got: {text}"
    );
}

/// A tool whose answer describes the shape of one graph must reject an array
/// rather than union two graphs into a meaningless third.
#[tokio::test]
async fn whole_graph_analyses_reject_multiple_roots() {
    let (_tmp, server, a, b) = served_with_two_neighbours().await;

    let response = call(
        &server,
        "tokensave_circular",
        json!({ "graph_root": [a, b] }),
    )
    .await;

    let message = response["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("single") || message.contains("one root"),
        "a whole-graph analysis must refuse an array with an explanation, got {response:?}"
    );
}

/// An empty array is a caller mistake, not a request to search nothing.
#[tokio::test]
async fn an_empty_root_array_is_rejected() {
    let (_tmp, server, _a, _b) = served_with_two_neighbours().await;

    let response = call(
        &server,
        "tokensave_search",
        json!({ "query": "process_batch", "graph_root": [] }),
    )
    .await;

    assert!(
        response.get("error").is_some(),
        "an empty graph_root array must be an error, got {response:?}"
    );
}
