//! Path-shaped discovery is redirected to `tokensave_files`.
//!
//! Regression test for #294. The hook covered symbol-shaped Grep and Bash
//! grep/rg/ag, but not `find -name`, `fd --extension`, or the `Glob` tool, so a
//! separate Python/PS1 shim had to replicate policy for those — the
//! two-implementation problem, where whichever layer is stricter or staler
//! wins.
//!
//! This could not land before #323: `tokensave_files` was populated only by
//! symbol extraction, so redirecting `find -name "*.feature"` to it would have
//! traded a working command for an empty result.
//!
//! The through-line of these tests is that a redirect is a promise the graph
//! can answer the question. Anything ambiguous — an unmodelled `find`
//! predicate, a non-code extension, a search root outside the index, `fd`'s
//! regex form — must pass through untouched.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::{json, Value};
use tokensave::hooks::{evaluate_hook_decision_with_env, HookEnv};

fn env_indexed() -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: false,
        project_root: None,
    }
}

fn env_not_indexed() -> HookEnv {
    HookEnv {
        in_tokensave_project: false,
        disable_grep_hook: false,
        project_root: None,
    }
}

/// Returns the block reason, or `None` when the call passes through.
///
/// The hook signals "allow" with empty output rather than a decision object.
fn decide(input: &Value, env: &HookEnv) -> Option<String> {
    let out = evaluate_hook_decision_with_env(&input.to_string(), env);
    if out.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_str(&out).unwrap();
    Some(
        value["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    )
}

/// Block reason for a Bash command in an indexed project.
fn bash(command: &str) -> Option<String> {
    decide(&json!({ "command": command }), &env_indexed())
}

/// Block reason for a tool input in an indexed project.
fn tool(input: Value) -> Option<String> {
    decide(&input, &env_indexed())
}

// ---------------------------------------------------------------------------
// find / fd
// ---------------------------------------------------------------------------

#[test]
fn find_by_name_for_a_code_extension_is_redirected() {
    let reason = bash("find . -name \"*.rs\"").expect("find -name must redirect");
    assert!(
        reason.contains("tokensave_files"),
        "must name the replacement tool, got: {reason}"
    );
}

#[test]
fn fd_extension_form_is_redirected() {
    // `fd -e py` means the same thing as `find . -name '*.py'`; the shim
    // covered both, so retiring it requires both.
    assert!(bash("fd -e py").is_some());
    assert!(bash("fd --extension py").is_some());
}

#[test]
fn fd_glob_form_is_redirected() {
    assert!(bash("fd -g '*.ts'").is_some());
}

#[test]
fn fd_regex_form_passes_through() {
    // `fd '\.py$'` is a regex, not a glob. tokensave_files takes a glob, so
    // redirecting here would send the caller to a tool that cannot express
    // what they asked for.
    assert!(bash("fd '\\.py$'").is_none());
}

#[test]
fn find_with_an_unmodelled_predicate_passes_through() {
    // `-exec` and `-delete` change what the command *does*. Redirecting a
    // command we have only partly parsed is how a guardrail starts breaking
    // things it was never meant to touch.
    assert!(bash("find . -name '*.rs' -exec rm {} ;").is_none());
    assert!(bash("find . -name '*.rs' -mtime -1").is_none());
    assert!(bash("find . -name '*.rs' -delete").is_none());
}

#[test]
fn find_for_a_non_code_extension_passes_through() {
    assert!(bash("find . -name '*.bin'").is_none());
    assert!(bash("find . -name '*.png'").is_none());
}

#[test]
fn a_mixed_search_passes_through() {
    // Partly answerable is not answerable: redirecting would silently drop the
    // half of the search the index does not cover.
    assert!(bash("fd -e rs -e bin").is_none());
}

#[test]
fn narrowing_flags_do_not_defeat_the_redirect() {
    // `find . -type f -name '*.py'` is the ordinary spelling. These flags only
    // shrink the result set, so treating them as unmodelled would mean the
    // redirect almost never fired in practice.
    assert!(bash("find . -type f -name '*.py'").is_some());
    assert!(bash("find . -maxdepth 3 -name '*.rs' -print").is_some());
    assert!(bash("fd -e py -t f").is_some());
}

#[test]
fn a_search_root_outside_the_index_passes_through() {
    assert!(bash("find /var/log -name '*.rs'").is_none());
}

#[test]
fn a_code_search_root_is_redirected() {
    assert!(bash("find src -name '*.rs'").is_some());
}

#[test]
fn find_without_a_name_predicate_passes_through() {
    // `find . -type f` is not discovery by name and has no files-tool analogue.
    assert!(bash("find .").is_none());
}

#[test]
fn the_inline_opt_out_is_honored() {
    // The escape hatch has to work on the new paths too, or the only way out
    // of a wrong block is to disable the hook wholesale.
    assert!(bash("TOKENSAVE_DISABLE_GREP_HOOK=1 find . -name '*.rs'").is_none());
}

// ---------------------------------------------------------------------------
// Glob tool
// ---------------------------------------------------------------------------

#[test]
fn a_code_glob_is_redirected() {
    let reason = tool(json!({ "pattern": "**/*.py" })).expect("Glob must redirect");
    assert!(reason.contains("tokensave_files"), "got: {reason}");
}

#[test]
fn a_non_code_glob_passes_through() {
    assert!(tool(json!({ "pattern": "**/*.png" })).is_none());
}

#[test]
fn a_grep_call_is_not_mistaken_for_a_glob() {
    // Grep and Glob both carry `pattern`. Misreading a content search as a path
    // search is the one error that sends the caller to a tool that cannot
    // answer them at all, so the Grep-only fields are the discriminator.
    assert!(tool(json!({
        "pattern": "*.py",
        "output_mode": "files_with_matches",
        "path": "src",
    }))
    .is_none());

    assert!(tool(json!({ "pattern": "*.py", "type": "py" })).is_none());
}

#[test]
fn a_wildcardless_pattern_passes_through() {
    // `config.py` as a Grep pattern is an ordinary literal search. Without a
    // wildcard there is nothing to say it was meant as a path.
    assert!(tool(json!({ "pattern": "config.py" })).is_none());
}

#[test]
fn a_glob_scoped_outside_the_index_passes_through() {
    assert!(tool(json!({ "pattern": "**/*.rs", "path": "/var/log" })).is_none());
}

#[test]
fn a_brace_glob_over_code_extensions_is_redirected() {
    assert!(tool(json!({ "pattern": "src/**/*.{ts,tsx}" })).is_some());
}

#[test]
fn a_brace_glob_mixing_code_and_assets_passes_through() {
    assert!(tool(json!({ "pattern": "src/**/*.{ts,png}" })).is_none());
}

// ---------------------------------------------------------------------------
// Gating
// ---------------------------------------------------------------------------

#[test]
fn nothing_is_redirected_outside_a_tokensave_project() {
    // Without an index there is no tool to redirect to, so the hook must be
    // inert rather than block the only means of discovery available.
    let env = env_not_indexed();
    assert!(decide(&json!({ "command": "find . -name '*.rs'" }), &env).is_none());
    assert!(decide(&json!({ "pattern": "**/*.rs" }), &env).is_none());
}
