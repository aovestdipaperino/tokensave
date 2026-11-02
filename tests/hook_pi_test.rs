//! The Pi adapter for the search guardrail.
//!
//! Pi has no hook contract of its own: its extension relays `{ toolName,
//! input, cwd }` to `tokensave hook-pi-pre-tool-use` and blocks the call on
//! exit code 2. These tests pin the translation that happens on the way in,
//! because each Pi tool has a different shape from its Claude Code namesake:
//! `grep` always returns matching lines but carries no `output_mode`, `find`
//! is a glob search, and `bash` is already shaped like Claude's `Bash`.

use std::path::{Path, PathBuf};
use tokensave::config::{save_config, TokenSaveConfig};
use tokensave::hooks::{evaluate_pi_pre_tool_use_with_env, HookEnv};

fn env_rooted_at(root: &Path) -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: false,
        project_root: Some(root.to_path_buf()),
        cwd: Some(root.to_path_buf()),
    }
}

fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::Builder::new()
        .prefix("tspi")
        .tempdir()
        .expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(tokensave::config::get_tokensave_dir(&root))
        .expect("create .tokensave");
    std::fs::write(
        tokensave::config::get_tokensave_dir(&root).join("tokensave.db"),
        b"",
    )
    .expect("write db");
    let config = TokenSaveConfig {
        root_dir: root.to_string_lossy().to_string(),
        ..TokenSaveConfig::default()
    };
    save_config(&root, &config).expect("save config");
    std::fs::create_dir_all(root.join("src")).expect("create dir");
    std::fs::write(root.join("src").join("mod.rs"), "fn my_function() {}\n").expect("write source");
    let root = root.canonicalize().expect("canonicalize root");
    (tmp, root)
}

/// A Pi `tool_call` envelope, the shape the extension sends.
fn pi_event(tool_name: &str, input: serde_json::Value, cwd: &Path) -> String {
    serde_json::json!({
        "toolName": tool_name,
        "input": input,
        "cwd": cwd.to_string_lossy(),
    })
    .to_string()
}

#[test]
fn a_symbol_grep_is_blocked_without_claude_output_mode() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    let event = pi_event(
        "grep",
        serde_json::json!({ "pattern": "my_function", "path": "src" }),
        &root,
    );
    let decision = evaluate_pi_pre_tool_use_with_env(&event, &env);
    assert!(
        decision.is_some_and(|reason| reason.contains("tokensave_search")),
        "a bare symbol grep must be redirected"
    );
}

#[test]
fn a_glob_find_is_redirected_to_tokensave_files() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    let event = pi_event(
        "find",
        serde_json::json!({ "pattern": "*.rs", "path": "src" }),
        &root,
    );
    let decision = evaluate_pi_pre_tool_use_with_env(&event, &env);
    assert!(
        decision.is_some_and(|reason| reason.contains("tokensave_files")),
        "a code-glob find must be redirected"
    );
}

#[test]
fn a_symbol_shaped_bash_grep_is_blocked() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    let event = pi_event(
        "bash",
        serde_json::json!({ "command": "rg 'my_function' src" }),
        &root,
    );
    assert!(evaluate_pi_pre_tool_use_with_env(&event, &env).is_some());
}

/// Non-code searches keep working: the guardrail only steers symbol lookups.
#[test]
fn a_documentation_search_passes() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    let event = pi_event(
        "grep",
        serde_json::json!({ "pattern": "usage", "glob": "*.md" }),
        &root,
    );
    assert!(evaluate_pi_pre_tool_use_with_env(&event, &env).is_none());
}

/// Tools the guardrail does not classify never reach the shared decision core.
#[test]
fn unguarded_tools_pass_through() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    let event = pi_event(
        "read",
        serde_json::json!({ "pattern": "my_function", "path": "src" }),
        &root,
    );
    assert!(evaluate_pi_pre_tool_use_with_env(&event, &env).is_none());
}

/// A payload that is not Pi's envelope is treated as a bare tool input, so a
/// mis-versioned extension degrades to the shared classifier instead of
/// failing closed on everything.
#[test]
fn a_non_envelope_payload_falls_back_to_the_shared_decision() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    let flat = serde_json::json!({
        "pattern": "my_function",
        "output_mode": "content",
        "path": "src",
    })
    .to_string();
    let decision = evaluate_pi_pre_tool_use_with_env(&flat, &env);
    assert!(
        decision.is_some_and(|reason| reason.contains("tokensave_search")),
        "an envelope-less payload still reaches the shared classifier"
    );
}
