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
    std::fs::write(root.join("src/base.rs"), "fn base() -> i32 { 1 }\n").unwrap();

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

/// Default behaviour is unchanged: the tools answer, with the advisory warning
/// #400 added. Strict mode must be something a user opts into, never a silent
/// change to how a working install behaves.
#[tokio::test]
async fn branch_drift_refuses_local_graph_tools_by_default() {
    let (_dir, server) = drifted_server(false).await;

    let response = call(&server, "tokensave_search", json!({"query": "base"})).await;
    let error = response
        .get("error")
        .expect("branch drift must refuse, not answer");
    assert_eq!(error["code"], -32600);
    let message = error["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("master") && message.contains("feature") && message.contains("Restart"),
        "refusal must name both branches and the recovery action, got {message:?}"
    );

    let status = call(&server, "tokensave_status", json!({})).await;
    assert!(
        status.get("error").is_none(),
        "tokensave_status must remain callable under branch drift, got {status:?}"
    );

    for tool in [
        "tokensave_affected",
        "tokensave_diff_context",
        "tokensave_simplify_scan",
        "tokensave_redundancy",
        "tokensave_diagnostics",
        "tokensave_diagnose",
    ] {
        let response = call(&server, tool, json!({})).await;
        assert_eq!(
            response["error"]["code"], -32600,
            "{tool} must refuse local graph work under branch drift: {response:?}"
        );
    }
}

/// With `strict_tree` enabled, a content tool refuses instead of answering
/// from the wrong tree.
#[tokio::test]
async fn strict_mode_refuses_a_content_tool_under_drift() {
    let (_dir, server) = drifted_server(true).await;

    let response = call(&server, "tokensave_search", json!({"query": "base"})).await;
    let error = response
        .get("error")
        .expect("strict mode must refuse, not answer");
    let message = error["message"].as_str().unwrap_or_default();

    assert!(
        message.contains("master") && message.contains("feature"),
        "the refusal must name both branches so it is actionable, got {message:?}"
    );
}

/// The diagnostic tools stay available, so the situation can be investigated
/// from inside the session that hit it. Refusing everything would leave an
/// agent unable to find out *why* it was refused.
#[tokio::test]
async fn strict_mode_spares_the_diagnostic_tools() {
    let (_dir, server) = drifted_server(true).await;

    // Exactly one tool is exempt. `tokensave_status` reports the served root
    // and branch, which is what a refused caller needs to understand why.
    let response = call(&server, "tokensave_status", json!({})).await;
    assert!(
        response.get("error").is_none(),
        "tokensave_status must remain callable under strict refusal, got {response:?}"
    );

    // Tools whose names suggest they are diagnostics but which read the graph
    // must still refuse: under a wrong-tree index `diagnose` and `diagnostics`
    // would attribute a real compiler error to a node from another tree, and
    // `config` reports nothing that helps explain a refusal.
    for tool in ["tokensave_config"] {
        let response = call(&server, tool, json!({})).await;
        let message = response["error"]["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("strict_tree"),
            "{tool} must be refused by the strict gate, not reach its handler; got {response:?}"
        );
    }
}

/// Strict mode must not fire when there is nothing wrong. A project with no
/// mismatch answers normally whether or not the setting is on — otherwise
/// enabling it would break every ordinary session.
#[tokio::test]
async fn strict_mode_is_inert_without_a_mismatch() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() { let _ = 1; }\n").unwrap();

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
