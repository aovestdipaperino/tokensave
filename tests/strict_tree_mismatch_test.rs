//! Opt-in strict mode: refuse rather than answer from the wrong tree — #372 §2.
//!
//! tokensave already detects two ways its index can describe a tree the user
//! is not in: a borrowed worktree index (#312, `src/worktree.rs`) and a branch
//! that drifted under a running server (#400). Both were advisory — a warning
//! prefixed to an answer the tools produced anyway.
//!
//! The reporter's argument for making it refusable: "wrong answer is worse than
//! no answer" for worktree-heavy workflows, because every downstream tool built
//! on tokensave — an agent rule saying "always check tokensave before reading
//! files", for instance — inherits the wrong-tree answer with no signal that
//! anything is off. An empty result reads as "no such symbol".
//!
//! Strict mode is opt-in via `strict_tree` in `.tokensave/config.json`, and
//! spares the diagnostic tools so the refusal is still investigable from
//! inside the session.

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

async fn call(server: &Arc<McpServer>, name: &str, arguments: Value) -> Value {
    let (mut transport, _sender, mut receiver) = ChannelTransport::new();
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
    .to_string();
    server.handle_and_write(&request, &mut transport).await;
    let raw = receiver.recv().await.expect("expected a response");
    serde_json::from_str(raw.trim()).expect("valid JSON-RPC")
}

/// A project on `master` with `feature` tracked, then checked out onto
/// `feature` under a live handle — the #400 drift condition. `strict` decides
/// whether `strict_tree` is enabled before the server opens.
async fn drifted_server(strict: bool) -> (TempDir, Arc<McpServer>) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src").join("base.rs"), "fn base() -> i32 { 1 }\n").unwrap();

    git(&root, &["init", "-b", "master"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "base"]);

    let cg = TokenSave::init(&root).await.unwrap();
    cg.index_all().await.unwrap();
    drop(cg);

    git(&root, &["checkout", "-b", "feature"]);
    tokensave::branch::track_branch_copy(&root, &root.join(".tokensave"), "feature")
        .await
        .unwrap();
    git(&root, &["checkout", "master"]);

    if strict {
        let mut config = tokensave::config::load_config(&root).unwrap();
        config.strict_tree = true;
        tokensave::config::save_config(&root, &config).unwrap();
    }

    // Open on `master`, then drift the working tree to `feature`.
    let cg = TokenSave::open(&root).await.unwrap();
    // `new_explicit_root` rather than `new`: the worktree check compares the
    // index root against the *process* CWD, which under `cargo test` is
    // tokensave's own repo, so `new` would report a worktree mismatch and mask
    // the drift condition this fixture exists to produce.
    let server = McpServer::new_explicit_root(cg, None).await;
    git(&root, &["checkout", "feature"]);

    (dir, server)
}

/// Branch changes are repaired before either the ordinary or strict gate.
#[tokio::test]
async fn branch_changes_select_the_new_context_in_both_modes() {
    for strict in [false, true] {
        let (_dir, server) = drifted_server(strict).await;
        assert!(
            server
                .wait_for_startup_catch_up(std::time::Duration::from_secs(5))
                .await
        );
        let response = call(&server, "tokensave_search", json!({"query": "base"})).await;
        assert!(response.get("error").is_none(), "{response:?}");
        let status = call(&server, "tokensave_status", json!({})).await;
        assert!(status.get("error").is_none(), "{status:?}");
        assert!(status.to_string().contains("feature"), "{status:?}");
    }
}

#[tokio::test]
async fn resource_reads_switch_before_any_tool_call() {
    let (_dir, server) = drifted_server(false).await;
    assert!(
        server
            .wait_for_startup_catch_up(std::time::Duration::from_secs(5))
            .await
    );
    let (mut transport, _sender, mut receiver) = ChannelTransport::new();
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "resources/read",
        "params": {"uri": "tokensave://branches"}})
    .to_string();
    server.handle_and_write(&request, &mut transport).await;
    let raw = receiver.recv().await.unwrap();
    let response: Value = serde_json::from_str(raw.trim()).unwrap();
    assert!(response.get("error").is_none(), "{response:?}");
    let text = response["result"]["contents"][0]["text"].as_str().unwrap();
    let branches: Value = serde_json::from_str(text).unwrap();
    let current = branches["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["is_current"] == true)
        .unwrap();
    assert_eq!(current["name"], "feature");
}

/// Strict mode must not fire when there is nothing wrong. A project with no
/// mismatch answers normally whether or not the setting is on — otherwise
/// enabling it would break every ordinary session.
#[tokio::test]
async fn strict_mode_is_inert_without_a_mismatch() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src").join("main.rs"),
        "fn main() { let _ = 1; }\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    let mut config = tokensave::config::load_config(root).unwrap();
    config.strict_tree = true;
    tokensave::config::save_config(root, &config).unwrap();
    drop(cg);

    let cg = TokenSave::open(root).await.unwrap();
    let server = McpServer::new_explicit_root(cg, None).await;

    let response = call(&server, "tokensave_search", json!({"query": "base"})).await;
    assert!(
        response.get("error").is_none(),
        "strict mode must be inert when the tree matches, got {response:?}"
    );
}
