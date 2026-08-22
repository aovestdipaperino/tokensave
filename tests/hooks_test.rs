use std::path::{Path, PathBuf};

use tokensave::hooks::{
    evaluate_claude_pre_tool_use_with_env, evaluate_droid_pre_tool_use_with_env,
    evaluate_hook_decision_with_env, evaluate_kiro_pre_tool_use_with_env, HookEnv,
};

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

fn env_disabled() -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: true,
        project_root: None,
    }
}

/// An indexed project on disk, so absolute targets can be canonicalized.
/// The returned `TempDir` must stay alive for the whole test.
fn indexed_project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".tokensave")).expect("create .tokensave");
    std::fs::write(root.join(".tokensave").join("tokensave.db"), b"").expect("write db");
    (tmp, root)
}

fn env_rooted_at(root: &Path) -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: false,
        project_root: Some(root.to_path_buf()),
    }
}

fn is_blocked(json: &str) -> bool {
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    v["hookSpecificOutput"]["permissionDecision"].as_str() == Some("deny")
}

fn get_block_reason(json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    v["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

#[test]
fn test_blocks_explore_agent() {
    let input = r#"{"subagent_type": "Explore", "prompt": "find files"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_allows_non_explore_agent() {
    let input = r#"{"subagent_type": "general-purpose", "prompt": "write a function"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_allows_typed_non_explore_agent_even_with_research_prompt() {
    // An explicitly typed non-Explore agent is a deliberate delegation (an
    // implementer, a custom agent, another harness's task type). Prompt
    // keywords must not turn it into a hard block, even when the prompt reads
    // like research — the caller chose a specific worker on purpose.
    let input = r#"{"subagent_type": "general-purpose", "prompt": "explore the codebase and find all callers of handle_request, then implement the fix"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "a typed non-Explore agent must not be blocked on prompt text, got: {result}"
    );
}

#[test]
fn test_blank_subagent_type_is_treated_as_untyped() {
    // A caller that sets subagent_type to "" is not a deliberate typed
    // delegation; a research-shaped prompt must still be steered, and it must
    // not slip past both branches.
    let research = r#"{"subagent_type": "", "prompt": "explore the codebase and map every caller of handle_request"}"#;
    assert!(is_blocked(&evaluate_hook_decision_with_env(
        research,
        &env_indexed()
    )));
    // ...while a blank type with a non-research prompt is allowed, same as any
    // untyped call.
    let impl_task = r#"{"subagent_type": "", "prompt": "write a unit test for the parser"}"#;
    assert!(evaluate_hook_decision_with_env(impl_task, &env_indexed()).is_empty());
}

#[test]
fn test_still_blocks_untyped_research_task() {
    // With no subagent_type the call is ambiguous and may be an Explore-style
    // fan-out, so the prompt heuristic still steers it to the MCP tools.
    let input = r#"{"prompt": "explore the codebase and map the call graph"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_explore_agent_respects_opt_out() {
    // The opt-out that suppresses Grep/Bash redirection now also suppresses
    // agent redirection, so an explicit "continue" override exists.
    let input = r#"{"subagent_type": "Explore", "prompt": "find files"}"#;
    assert!(is_blocked(&evaluate_hook_decision_with_env(
        input,
        &env_indexed()
    )));
    assert!(
        evaluate_hook_decision_with_env(input, &env_disabled()).is_empty(),
        "the disable opt-out must let an Explore agent through"
    );
}

#[test]
fn test_agent_block_requires_index() {
    // Like the Grep/Bash paths, the agent redirection is pointless without a
    // .tokensave index: there are no MCP tools to redirect to, so both the
    // Explore deny and the untyped-prompt deny must no-op.
    let explore = r#"{"subagent_type": "Explore", "prompt": "find files"}"#;
    assert!(
        evaluate_hook_decision_with_env(explore, &env_not_indexed()).is_empty(),
        "Explore agent must pass through when no index exists"
    );
    let untyped = r#"{"prompt": "explore the codebase and map the call graph"}"#;
    assert!(
        evaluate_hook_decision_with_env(untyped, &env_not_indexed()).is_empty(),
        "untyped research task must pass through when no index exists"
    );
}

#[test]
fn test_kiro_delegation_block_requires_index_and_honors_opt_out() {
    let input = r#"{
        "hook_event_name": "preToolUse",
        "tool_name": "delegate",
        "tool_input": {"task": "Explore the codebase architecture and call graph"}
    }"#;
    assert!(evaluate_kiro_pre_tool_use_with_env(input, &env_indexed()).is_some());
    assert!(evaluate_kiro_pre_tool_use_with_env(input, &env_not_indexed()).is_none());
    assert!(evaluate_kiro_pre_tool_use_with_env(input, &env_disabled()).is_none());
}

#[test]
fn test_blocks_exploration_prompt_explore() {
    let input = r#"{"prompt": "Explore the codebase and find all API endpoints"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_codebase_structure_prompt() {
    let input = r#"{"prompt": "Understand the codebase structure"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_call_graph_prompt() {
    let input = r#"{"prompt": "Show me the call graph for this function"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_who_calls_prompt() {
    let input = r#"{"prompt": "who calls the process_data function?"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_callers_of_prompt() {
    let input = r#"{"prompt": "find callers of handle_request"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_callees_of_prompt() {
    let input = r#"{"prompt": "what are the callees of main?"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_symbol_lookup_prompt() {
    let input = r#"{"prompt": "do a symbol lookup for TokenSave"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_read_every_prompt() {
    let input = r#"{"prompt": "read every file in src/"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_entire_codebase_prompt() {
    let input = r#"{"prompt": "scan the entire codebase for patterns"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_allows_normal_prompt() {
    let input = r#"{"prompt": "write a unit test for the parse function"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_allows_empty_input() {
    let result = evaluate_hook_decision_with_env("", &env_indexed());
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_allows_invalid_json() {
    let result = evaluate_hook_decision_with_env("not json at all", &env_indexed());
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_allows_no_prompt_no_subagent() {
    let input = r#"{"foo": "bar"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_case_insensitive_blocking() {
    let input = r#"{"prompt": "EXPLORE the Codebase Architecture"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_block_response_has_reason() {
    let input = r#"{"subagent_type": "Explore"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    let reason = get_block_reason(&result);
    assert!(reason.contains("tokensave MCP tools"));
}

#[test]
fn test_block_response_uses_correct_hook_schema() {
    let input = r#"{"subagent_type": "Explore"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        v["hookSpecificOutput"]["hookEventName"].as_str(),
        Some("PreToolUse")
    );
    assert_eq!(
        v["hookSpecificOutput"]["permissionDecision"].as_str(),
        Some("deny")
    );
    assert!(v["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .is_some());
}

#[test]
fn test_kiro_blocks_delegate_code_research_task() {
    let input = r#"{
        "hook_event_name": "preToolUse",
        "tool_name": "delegate",
        "tool_input": {
            "task": "Explore the codebase architecture and call graph"
        }
    }"#;
    let reason = evaluate_kiro_pre_tool_use_with_env(input, &env_indexed()).unwrap();
    assert!(reason.contains("tokensave MCP tools"));
}

#[test]
fn test_kiro_blocks_subagent_research_prompt() {
    let input = r#"{
        "hook_event_name": "preToolUse",
        "tool_name": "subagent",
        "tool_input": {
            "prompt": "who calls the process_data function?"
        }
    }"#;
    assert!(evaluate_kiro_pre_tool_use_with_env(input, &env_indexed()).is_some());
}

#[test]
fn test_kiro_allows_delegate_execution_task() {
    let input = r#"{
        "hook_event_name": "preToolUse",
        "tool_name": "delegate",
        "tool_input": {
            "task": "Run the full test suite and report failures"
        }
    }"#;
    assert!(evaluate_kiro_pre_tool_use_with_env(input, &env_indexed()).is_none());
}

#[test]
fn test_kiro_allows_non_delegation_tool() {
    let input = r#"{
        "hook_event_name": "preToolUse",
        "tool_name": "read",
        "tool_input": {
            "prompt": "Explore the entire codebase"
        }
    }"#;
    assert!(evaluate_kiro_pre_tool_use_with_env(input, &env_indexed()).is_none());
}

#[test]
fn test_kiro_allows_invalid_json() {
    assert!(evaluate_kiro_pre_tool_use_with_env("not json", &env_indexed()).is_none());
}

// ============================================================================
// Grep tool redirect — symbol-shaped patterns against code files should
// redirect to tokensave_search / _signature_search / _callers.
// ============================================================================

#[test]
fn test_grep_blocks_bare_symbol_on_rust_file() {
    let input = r#"{"pattern": "FooBar", "path": "src/main.rs", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "bare symbol on .rs should redirect");
}

#[test]
fn test_grep_allows_omitted_output_mode() {
    let input = r#"{"pattern": "FooBar", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "the harness default is path-only, so only explicit content mode should redirect"
    );
}

#[test]
fn test_grep_blocks_alternation_on_rust_file() {
    let input =
        r#"{"pattern": "Foo\\|Bar\\|Baz", "path": "src/main.rs", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "alternation of identifiers should redirect"
    );
}

#[test]
fn test_grep_blocks_word_boundary_symbol() {
    let input =
        r#"{"pattern": "\\bhandle_request\\b", "path": "src/main.rs", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "\\bsymbol\\b should redirect");
}

#[test]
fn test_grep_allows_regex_metachar_pattern() {
    // dot-paren — a real regex sweep, not a symbol search
    let input = r#"{"pattern": "\\.split_at\\(", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "regex metachars should pass through");
}

#[test]
fn test_grep_allows_character_class() {
    let input = r#"{"pattern": "[A-Z][a-z]+", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "char class should pass through");
}

#[test]
fn test_grep_allows_non_code_extension() {
    let input = r#"{"pattern": "FooBar", "path": "README.md"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "non-code file should pass through");
}

#[test]
fn test_grep_allows_files_with_matches_mode() {
    let input = r#"{"pattern": "FooBar", "path": "src/", "output_mode": "files_with_matches"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "files_with_matches is file discovery, not symbol search"
    );
}

#[test]
fn test_grep_allows_count_mode() {
    let input = r#"{"pattern": "FooBar", "path": "src/", "output_mode": "count"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "count mode should pass through");
}

#[test]
fn test_grep_blocks_on_directory_path_when_indexed() {
    let input = r#"{"pattern": "FooBar", "path": "src/", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "symbol search in src/ dir should redirect"
    );
}

#[test]
fn test_grep_blocks_when_only_glob_set() {
    let input = r#"{"pattern": "FooBar", "glob": "**/*.rs", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "glob over .rs should redirect");
}

#[test]
fn test_grep_allows_glob_for_non_code() {
    let input = r#"{"pattern": "FooBar", "glob": "**/*.md", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "glob over .md should pass through");
}

#[test]
fn test_grep_non_code_glob_overrides_project_root_path() {
    let input =
        r#"{"pattern": "FooBar", "path": ".", "glob": "**/*.md", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "an explicit Markdown glob should narrow a broad project-root path"
    );
}

#[test]
fn test_grep_non_code_glob_overrides_code_directory_path() {
    let input =
        r#"{"pattern": "FooBar", "path": "src", "glob": "**/*.md", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "an explicit Markdown glob should narrow a code-directory path"
    );
}

#[test]
fn test_grep_code_glob_overrides_non_code_path() {
    let input =
        r#"{"pattern": "FooBar", "path": "docs", "glob": "**/*.rs", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        !result.is_empty(),
        "an explicit Rust glob should classify the effective target as code"
    );
    assert!(is_blocked(&result));
}

#[test]
fn test_grep_mixed_code_and_non_code_glob_passes() {
    let input =
        r#"{"pattern": "FooBar", "path": ".", "glob": "**/*.{rs,md}", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "a mixed glob can return documentation and must pass through"
    );
}

#[test]
fn test_grep_terminal_non_code_suffix_after_brace_passes() {
    let input = r#"{"pattern": "FooBar", "path": ".", "glob": "**/*.{ts,js}.bak", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "the terminal .bak suffix determines the effective file type"
    );
}

#[test]
fn test_grep_terminal_code_suffix_after_brace_blocks() {
    let input = r#"{"pattern": "FooBar", "path": ".", "glob": "**/*.{spec,test}.ts", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        !result.is_empty(),
        "the terminal .ts suffix determines the effective file type"
    );
    assert!(is_blocked(&result));
}

#[test]
fn test_grep_ignores_directory_brace_when_final_file_glob_is_code() {
    let input = r#"{"pattern": "FooBar", "path": ".", "glob": "dir.{a,b}/**/*.rs", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        !result.is_empty(),
        "a brace in an earlier path segment must not hide the final Rust extension"
    );
    assert!(is_blocked(&result));
}

#[test]
fn test_grep_unclassifiable_glob_falls_back_to_path() {
    let input = r#"{"pattern": "FooBar", "path": "src", "glob": "**/generated/**", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "a glob without an extension should preserve the existing path classification"
    );
}

#[test]
fn test_grep_unclassifiable_glob_without_path_preserves_pass_through() {
    let input = r#"{"pattern": "FooBar", "glob": "**/generated/**", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "an extensionless glob without a path should preserve the conservative pass-through"
    );
}

#[test]
fn test_grep_type_filter_has_priority_over_non_code_glob() {
    let input = r#"{"pattern": "FooBar", "path": ".", "glob": "**/*.md", "type": "rust", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "an explicit code type remains the highest-priority filter"
    );
}

#[test]
fn test_grep_blocks_with_type_filter_rust() {
    let input = r#"{"pattern": "FooBar", "type": "rust", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "type=rust should redirect");
}

#[test]
fn test_grep_block_message_names_tokensave_tool() {
    let input = r#"{"pattern": "FooBar", "path": "src/main.rs", "output_mode": "content"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    let reason = get_block_reason(&result);
    assert!(
        reason.contains("tokensave_"),
        "block message must name a tokensave tool"
    );
}

// ============================================================================
// Bash with embedded grep/rg/ag — same redirect logic, parsing the command.
// ============================================================================

#[test]
fn test_bash_blocks_grep_on_rust_file() {
    let input = r#"{"command": "grep -n \"FooBar\" src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "grep -n symbol .rs should redirect");
}

#[test]
fn test_bash_blocks_rg_on_src_dir() {
    let input = r#"{"command": "rg -n FooBar src/"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "rg symbol src/ should redirect");
}

#[test]
fn test_bash_blocks_grep_rn_recursive() {
    let input = r#"{"command": "grep -rn handle_request /Users/me/proj/src/"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "grep -rn callers search should redirect"
    );
}

#[test]
fn test_bash_blocks_alternation_command() {
    let input = r#"{"command": "grep -n \"Foo\\|Bar\" src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "alternation in grep command should redirect"
    );
}

#[test]
fn test_bash_blocks_rtk_grep_prefix() {
    let input = r#"{"command": "rtk grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "rtk grep prefix should be unwrapped");
}

#[test]
fn test_bash_allows_git_grep() {
    let input = r#"{"command": "git grep -n FooBar"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "git grep searches history — pass through"
    );
}

#[test]
fn test_bash_redirects_find_by_name() {
    // This asserted pass-through until #294, when `find -name` gained a policy
    // of its own: `tokensave_files` can answer discovery by path now that it
    // also tracks non-code artifacts (#323). See `hook_find_glob_test.rs` for
    // the boundaries — unmodelled predicates and non-code extensions still
    // pass through.
    let input = r#"{"command": "find . -name \"*.rs\" -type f"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "find -name should redirect");
    assert!(get_block_reason(&result).contains("tokensave_files"));
}

#[test]
fn test_bash_allows_grep_regex_metachars() {
    let input = r#"{"command": "rg -n \"\\.split_at\\(\" src/"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "regex sweep should pass through");
}

#[test]
fn test_bash_allows_grep_on_markdown() {
    let input = r#"{"command": "grep -n FooBar README.md"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "grep on .md should pass through");
}

#[test]
fn test_bash_allows_grep_in_pipe_after_other_cmd() {
    // Heuristic: only intercept commands that START with grep/rg/ag (after rtk/sudo).
    // Piping ls output through grep is not a code search.
    let input = r#"{"command": "ls src/ | grep FooBar"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "piped grep should pass through (safety)");
}

#[test]
fn test_bash_allows_non_grep_command() {
    let input = r#"{"command": "cargo test"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "non-grep bash should pass through");
}

#[test]
fn test_bash_blocks_grep_after_env_prefix() {
    let input = r#"{"command": "FOO=bar grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "a leading env assignment should not hide the grep"
    );
}

#[test]
fn test_bash_blocks_grep_after_cd_prefix() {
    let input = r#"{"command": "cd src && grep -n FooBar main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "a leading `cd … &&` should not hide the grep"
    );
}

#[test]
fn test_bash_inline_disable_env_is_honored() {
    // An explicit inline TOKENSAVE_DISABLE_GREP_HOOK=<truthy> is a deliberate
    // opt-out and must be honored, not stripped and then blocked. This mirrors
    // the exported opt-out; an ordinary FOO=bar prefix is still stripped.
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "inline TOKENSAVE_DISABLE_GREP_HOOK=1 should opt out like the exported one"
    );
}

#[test]
fn test_bash_inline_disable_env_falsey_still_blocks() {
    // A falsey value is not an opt-out (same truthiness as HookEnv::from_runtime),
    // so the grep is still redirected.
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=0 grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "TOKENSAVE_DISABLE_GREP_HOOK=0 is falsey and should still redirect"
    );
}

#[test]
fn test_bash_inline_disable_after_cd_is_honored() {
    // The opt-out must be recognized wherever it sits in the leading noise, not
    // only as the very first token, so a conscious `cd … && DISABLE=1 grep …`
    // is honored rather than redirected.
    let input = r#"{"command": "cd src && TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n FooBar main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "an inline opt-out after a cd prefix should still opt out"
    );
}

#[test]
fn test_bash_inline_disable_after_sudo_is_honored() {
    let input = r#"{"command": "sudo TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "an inline opt-out after a sudo wrapper should still opt out"
    );
}

#[test]
fn test_bash_inline_disable_after_other_env_is_honored() {
    let input = r#"{"command": "FOO=1 TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "an inline opt-out after another assignment should still opt out"
    );
}

#[test]
fn test_bash_inline_disable_before_other_env_is_honored() {
    let input =
        r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=1 FOO=bar grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "an inline opt-out before another assignment should still opt out"
    );
}

#[test]
fn test_bash_inline_disable_nested_cd_and_env_is_honored() {
    let input =
        r#"{"command": "cd src && FOO=1 TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n FooBar main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "a deeply nested inline opt-out (cd + env + disable) should still opt out"
    );
}

#[test]
fn test_bash_inline_disable_quoted_is_honored() {
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=\"1\" grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "a quoted truthy opt-out value should still opt out"
    );
}

#[test]
fn test_bash_inline_disable_true_word_is_honored() {
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=true grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "value `true` should opt out");
}

#[test]
fn test_bash_inline_disable_false_word_still_blocks() {
    // Case-insensitive `false` is falsey, matching HookEnv::from_runtime.
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=FALSE grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "value `FALSE` is not an opt-out and should still redirect"
    );
}

#[test]
fn test_bash_inline_disable_empty_value_still_blocks() {
    // An empty value is not set, so it is stripped like ordinary noise and the
    // grep is still redirected.
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK= grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "an empty opt-out value is not an opt-out and should still redirect"
    );
}

#[test]
fn test_bash_inline_disable_last_assignment_wins_falsey_blocks() {
    // Shell "last assignment wins": a trailing falsey reassignment overrides an
    // earlier truthy one, so the grep is still redirected.
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=1 TOKENSAVE_DISABLE_GREP_HOOK=0 grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "a trailing falsey reassignment should win and still redirect"
    );
}

#[test]
fn test_bash_inline_disable_last_assignment_wins_truthy_allows() {
    let input = r#"{"command": "TOKENSAVE_DISABLE_GREP_HOOK=0 TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "a trailing truthy reassignment should win and opt out"
    );
}

#[test]
fn test_bash_blocks_grep_after_cd_and_env_prefix() {
    // Nested non-opt-out prefixes still unwrap to reveal the grep.
    let input = r#"{"command": "cd src && FOO=bar grep -n FooBar main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "cd + ordinary env prefix should not hide the grep"
    );
}

#[test]
fn test_bash_allows_pipe_after_cd_prefix() {
    let input = r#"{"command": "cd src && ls | grep FooBar"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "piped grep after a cd should still pass through"
    );
}

#[test]
fn test_bash_blocks_grep_on_python_file() {
    let input = r#"{"command": "grep -n FooBar src/app.py"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "grep on .py should redirect");
}

#[test]
fn test_bash_blocks_grep_on_typescript_file() {
    let input = r#"{"command": "grep -n FooBar src/index.tsx"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "grep on .tsx should redirect");
}

// ============================================================================
// Safety guards
// ============================================================================

#[test]
fn test_grep_allows_when_not_indexed() {
    let input = r#"{"pattern": "FooBar", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_not_indexed());
    assert!(
        result.is_empty(),
        "no .tokensave/tokensave.db → pass through"
    );
}

#[test]
fn test_grep_allows_when_env_override() {
    let input = r#"{"pattern": "FooBar", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_disabled());
    assert!(
        result.is_empty(),
        "TOKENSAVE_DISABLE_GREP_HOOK=1 → pass through"
    );
}

#[test]
fn test_bash_allows_when_env_override() {
    let input = r#"{"command": "grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_disabled());
    assert!(
        result.is_empty(),
        "TOKENSAVE_DISABLE_GREP_HOOK=1 → bash grep passes through"
    );
}

/// Regression for #248: `TOKENSAVE_DISABLE_GREP_HOOK` must be a *complete*
/// bypass for the binary hook, across every redirect path — Grep, Bash, a
/// typed `Explore` agent, and an untyped research-shaped prompt. This is the
/// documented escape hatch for headless / subagent (`claude -p`) dispatch,
/// where a child that legitimately needs raw search must be able to opt out
/// without stripping all hooks. `HookEnv::from_runtime` maps the env var onto
/// `disable_grep_hook`, so exercising the flag here covers the runtime path.
#[test]
fn test_disable_env_bypasses_every_redirect_path() {
    let cases = [
        r#"{"pattern": "FooBar", "path": "src/main.rs", "output_mode": "content"}"#,
        r#"{"command": "grep -n FooBar src/main.rs"}"#,
        r#"{"subagent_type": "Explore", "prompt": "find all API endpoints"}"#,
        r#"{"prompt": "explore the codebase and map the call graph"}"#,
    ];
    for input in cases {
        // Sanity: each case IS redirected with the guardrail active...
        assert!(
            is_blocked(&evaluate_hook_decision_with_env(input, &env_indexed())),
            "expected redirect with guardrail active: {input}"
        );
        // ...and the opt-out lets every one of them through.
        assert!(
            evaluate_hook_decision_with_env(input, &env_disabled()).is_empty(),
            "TOKENSAVE_DISABLE_GREP_HOOK must fully bypass this path: {input}"
        );
    }
}

#[test]
fn test_bash_allows_when_not_indexed() {
    let input = r#"{"command": "grep -n FooBar src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_not_indexed());
    assert!(result.is_empty(), "bash redirect requires indexed project");
}

#[test]
fn test_grep_allows_long_pattern() {
    // Very long patterns are unlikely to be simple symbol searches
    let huge = "A".repeat(300);
    let input = format!(r#"{{"pattern": "{huge}", "path": "src/main.rs"}}"#);
    let result = evaluate_hook_decision_with_env(&input, &env_indexed());
    assert!(result.is_empty(), "pattern over 200 chars should pass");
}

#[test]
fn test_grep_allows_empty_pattern() {
    let input = r#"{"pattern": "", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "empty pattern should pass");
}

#[test]
fn test_grep_existing_evaluate_hook_decision_still_works_for_agent() {
    // Sanity: the legacy entrypoint should still handle Agent
    let input = r#"{"subagent_type": "Explore"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

// ============================================================================
// Claude Code PreToolUse stdin contract — event arrives as JSON on stdin with
// the tool arguments nested under `tool_input` (no TOOL_INPUT env var).
// ============================================================================

#[test]
fn test_claude_blocks_explore_agent_nested_stdin() {
    let input = r#"{
        "hook_event_name": "PreToolUse",
        "tool_name": "Agent",
        "tool_input": {"subagent_type": "Explore", "prompt": "find files"}
    }"#;
    let result = evaluate_claude_pre_tool_use_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "nested Explore agent should redirect");
}

#[test]
fn test_claude_lowercase_explorer_subagent_passes() {
    let input = r#"{
        "hook_event_name": "PreToolUse",
        "tool_name": "Agent",
        "tool_input": {"subagent_type": "explorer", "prompt": "find files"}
    }"#;
    let result = evaluate_claude_pre_tool_use_with_env(input, &env_indexed());
    assert!(
        result.is_empty(),
        "Claude behavior must remain exact-case Explore only"
    );
}

#[test]
fn test_claude_blocks_research_prompt_nested_stdin() {
    let input = r#"{
        "hook_event_name": "PreToolUse",
        "tool_name": "Agent",
        "tool_input": {"prompt": "who calls the process_data function?"}
    }"#;
    let result = evaluate_claude_pre_tool_use_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
    assert!(get_block_reason(&result).contains("tokensave MCP tools"));
}

#[test]
fn test_claude_allows_normal_tool_nested_stdin() {
    let input = r#"{
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test"}
    }"#;
    let result = evaluate_claude_pre_tool_use_with_env(input, &env_indexed());
    assert!(result.is_empty(), "normal tool call should pass through");
}

#[test]
fn test_claude_falls_back_to_flat_tool_input() {
    // If the wrapper is absent, treat the payload as a flat tool_input object.
    let input = r#"{"subagent_type": "Explore"}"#;
    let result = evaluate_claude_pre_tool_use_with_env(input, &env_indexed());
    assert!(is_blocked(&result));
}

#[test]
fn test_claude_allows_invalid_json() {
    assert!(evaluate_claude_pre_tool_use_with_env("not json", &env_indexed()).is_empty());
}

// ============================================================================
// Factory Droid PreToolUse stdin contract — event arrives as JSON on stdin
// with the tool payload nested under `tool_input` (the Claude/Kiro shape),
// but the block is signaled via the raw reason text (`hook_droid_pre_tool_use`
// prints it to stderr and exits 2 — the Kiro mechanism), not a stdout JSON
// object. The `^(Execute|Grep|Task)$` matcher is installed, so grep/bash-shaped
// `command` payloads, Droid's native `Grep` `pattern`, and typed subagent
// payloads all reach this handler.
// ============================================================================

#[test]
fn test_droid_blocks_grep_shaped_execute_command_on_rust_file() {
    let input = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "grep -rn FooBar src/main.rs"}
    }"#;
    let reason = evaluate_droid_pre_tool_use_with_env(input, &env_indexed());
    assert!(
        reason.is_some(),
        "a symbol-shaped grep on a Rust file should redirect"
    );
    assert!(reason.unwrap().contains("tokensave"));
}

#[test]
fn test_droid_allows_terminal_launched_tools() {
    // Regression: tools the owner runs via a shell (Plannotator, builds, git)
    // are ordinary Execute commands that don't start with grep/rg/ag and must
    // pass untouched.
    let plannotator = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "npx plannotator review"}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(plannotator, &env_indexed()).is_none());

    let build = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "cargo build --release"}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(build, &env_indexed()).is_none());

    let git_commit = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "git commit -am \"fix bug\""}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(git_commit, &env_indexed()).is_none());
}

#[test]
fn test_droid_allows_git_grep() {
    // git grep searches history, which tokensave does not index.
    let input = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "git grep FooBar"}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none());
}

#[test]
fn test_droid_allows_when_not_indexed() {
    let input = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "grep -rn FooBar src/main.rs"}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_not_indexed()).is_none());
}

#[test]
fn test_droid_respects_disable_grep_hook_escape_hatch() {
    let input = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "grep -rn FooBar src/main.rs"}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_some());
    assert!(
        evaluate_droid_pre_tool_use_with_env(input, &env_disabled()).is_none(),
        "TOKENSAVE_DISABLE_GREP_HOOK=1 must let the grep call through"
    );
}

#[test]
fn test_droid_specialized_subagent_with_normal_task_passes() {
    // A specialized sub-agent given a normal (non-research) task must not be
    // blocked. The installed Task matcher routes this through the shared core,
    // but an exact non-research type is still a deliberate delegation.
    let input = r#"{
        "tool_name": "Task",
        "tool_input": {
            "subagent_type": "worker",
            "prompt": "Implement the retry logic for the sync client and add tests"
        }
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none());
}

#[test]
fn test_droid_explorer_subagent_blocks_with_tokensave_context_hint() {
    let input = r#"{
        "tool_name": "Task",
        "tool_input": {
            "subagent_type": "explorer",
            "description": "Map request flow",
            "prompt": "Find every caller of handle_request"
        }
    }"#;
    let reason = evaluate_droid_pre_tool_use_with_env(input, &env_indexed())
        .expect("Droid explorer should redirect to tokensave");
    assert!(reason.contains("tokensave_context"));
}

#[test]
fn test_droid_explorer_subagent_requires_index_and_honors_opt_out() {
    let input = r#"{
        "tool_name": "Task",
        "tool_input": {
            "subagent_type": "explorer",
            "prompt": "Map the codebase"
        }
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_some());
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_not_indexed()).is_none());
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_disabled()).is_none());
}

#[test]
fn test_droid_custom_explorer_named_subagent_passes() {
    let input = r#"{
        "tool_name": "Task",
        "tool_input": {
            "subagent_type": "explorer-writer",
            "prompt": "Inspect and update the documentation"
        }
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none());
}

#[test]
fn test_droid_untyped_research_task_passes() {
    let input = r#"{
        "tool_name": "Task",
        "tool_input": {
            "prompt": "who calls the process_data function?"
        }
    }"#;
    assert!(
        evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none(),
        "Droid Task blocking must be limited to exact lowercase explorer"
    );
}

#[test]
fn test_droid_falls_back_to_flat_tool_input() {
    // If the wrapper is absent, treat the payload as a flat tool_input object
    // (matches the Claude adapter's fallback for the same reason).
    let input = r#"{"command": "grep -rn FooBar src/main.rs"}"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_some());
}

#[test]
fn test_droid_allows_empty_input() {
    assert!(evaluate_droid_pre_tool_use_with_env("", &env_indexed()).is_none());
}

#[test]
fn test_droid_allows_invalid_json() {
    assert!(evaluate_droid_pre_tool_use_with_env("not json", &env_indexed()).is_none());
}

#[test]
fn test_droid_block_reason_documents_escape_hatch() {
    let input = r#"{
        "tool_name": "Execute",
        "tool_input": {"command": "grep -rn FooBar src/main.rs"}
    }"#;
    let reason = evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).unwrap();
    assert!(reason.contains("TOKENSAVE_DISABLE_GREP_HOOK"));
}

// ---------------------------------------------------------------------------
// Droid native `Grep` tool payloads. The `Grep` matcher routes these through
// the same shared decision core as the Claude `Grep` tool, but Droid names two
// fields differently (`glob_pattern` not `glob`; `file_paths` not
// `files_with_matches`), so these guard that the classifier reads both shapes.
// ---------------------------------------------------------------------------

#[test]
fn test_droid_native_grep_omitted_output_mode_passes() {
    let input = r#"{
        "tool_name": "Grep",
        "tool_input": {"pattern": "handle_request", "path": "src"}
    }"#;
    let reason = evaluate_droid_pre_tool_use_with_env(input, &env_indexed());
    assert!(
        reason.is_none(),
        "omitted output_mode uses Droid's path-only default and should pass"
    );
}

#[test]
fn test_droid_native_grep_uses_glob_pattern_field() {
    // Droid's Grep field is `glob_pattern`, not Claude's `glob`.
    let on_code = r#"{
        "tool_name": "Grep",
        "tool_input": {"pattern": "FooBar", "glob_pattern": "**/*.rs", "output_mode": "content"}
    }"#;
    assert!(
        evaluate_droid_pre_tool_use_with_env(on_code, &env_indexed()).is_some(),
        "glob_pattern over .rs should redirect"
    );

    let on_docs = r#"{
        "tool_name": "Grep",
        "tool_input": {"pattern": "FooBar", "glob_pattern": "**/*.md", "output_mode": "content"}
    }"#;
    assert!(
        evaluate_droid_pre_tool_use_with_env(on_docs, &env_indexed()).is_none(),
        "glob_pattern over .md should pass through"
    );
}

#[test]
fn test_droid_native_grep_non_code_glob_overrides_project_root_path() {
    let input = r#"{
        "tool_name": "Grep",
        "tool_input": {
            "pattern": "FooBar",
            "path": ".",
            "glob_pattern": "**/*.md",
            "output_mode": "content"
        }
    }"#;
    assert!(
        evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none(),
        "Droid's explicit Markdown glob should narrow its broad path"
    );
}

#[test]
fn test_droid_native_grep_unclassifiable_glob_without_path_passes() {
    let input = r#"{
        "tool_name": "Grep",
        "tool_input": {
            "pattern": "FooBar",
            "glob_pattern": "**/generated/**",
            "output_mode": "content"
        }
    }"#;
    assert!(
        evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none(),
        "Droid's extensionless glob should preserve the conservative pass-through"
    );
}

#[test]
fn test_droid_native_grep_file_paths_mode_passes() {
    // `file_paths` returns only names (Droid's cheap mode) — nothing to save.
    let input = r#"{
        "tool_name": "Grep",
        "tool_input": {"pattern": "handle_request", "path": "src", "output_mode": "file_paths"}
    }"#;
    assert!(
        evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none(),
        "file_paths output mode should pass through"
    );
}

#[test]
fn test_droid_native_grep_content_mode_blocks() {
    let input = r#"{
        "tool_name": "Grep",
        "tool_input": {"pattern": "handle_request", "path": "src", "output_mode": "content"}
    }"#;
    assert!(
        evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_some(),
        "content output mode over code should redirect"
    );
}

#[test]
fn test_droid_native_grep_non_code_target_passes() {
    let input = r#"{
        "tool_name": "Grep",
        "tool_input": {"pattern": "TODO", "glob_pattern": "**/*.md", "output_mode": "content"}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none());
}

#[test]
fn test_droid_native_grep_respects_escape_hatch() {
    let input = r#"{
        "tool_name": "Grep",
        "tool_input": {"pattern": "handle_request", "path": "src", "output_mode": "content"}
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_some());
    assert!(
        evaluate_droid_pre_tool_use_with_env(input, &env_disabled()).is_none(),
        "TOKENSAVE_DISABLE_GREP_HOOK=1 must let the native Grep call through"
    );
}

// --- absolute / home-rooted project-root targets -------------------------

#[test]
fn test_bash_blocks_grep_on_absolute_project_root() {
    let (_tmp, root) = indexed_project();
    let input =
        serde_json::json!({"command": format!("grep -rn handle_request {}", root.display())})
            .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        is_blocked(&result),
        "an absolute spelling of the indexed root is the same whole-project search as `.`"
    );
}

#[test]
fn test_bash_blocks_grep_on_absolute_project_root_with_trailing_slash() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({"command": format!("rg -n handle_request {}/", root.display())})
        .to_string();
    assert!(
        is_blocked(&evaluate_hook_decision_with_env(
            &input,
            &env_rooted_at(&root)
        )),
        "a trailing slash must not change the classification"
    );
}

#[test]
fn test_bash_allows_grep_on_sibling_sharing_root_prefix() {
    let (tmp, root) = indexed_project();
    let sibling = tmp.path().join("project-old");
    std::fs::create_dir_all(&sibling).expect("create sibling");
    let input =
        serde_json::json!({"command": format!("grep -rn handle_request {}", sibling.display())})
            .to_string();
    assert!(
        evaluate_hook_decision_with_env(&input, &env_rooted_at(&root)).is_empty(),
        "a sibling sharing the root's string prefix is a different tree"
    );
}

#[test]
fn test_bash_allows_grep_on_absolute_non_code_subdirectory() {
    let (_tmp, root) = indexed_project();
    let docs = root.join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let input =
        serde_json::json!({"command": format!("grep -rn handle_request {}", docs.display())})
            .to_string();
    assert!(
        evaluate_hook_decision_with_env(&input, &env_rooted_at(&root)).is_empty(),
        "only the root itself is the whole-project bucket, not every path inside it"
    );
}

#[test]
fn test_grep_blocks_absolute_project_root_path() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "pattern": "handle_request",
        "path": root.to_string_lossy(),
        "output_mode": "content",
    })
    .to_string();
    assert!(
        is_blocked(&evaluate_hook_decision_with_env(
            &input,
            &env_rooted_at(&root)
        )),
        "structured Grep with an absolute root path should redirect"
    );
}

#[test]
fn test_grep_non_code_glob_overrides_absolute_project_root_path() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "pattern": "handle_request",
        "path": root.to_string_lossy(),
        "glob": "**/*.md",
        "output_mode": "content",
    })
    .to_string();
    assert!(
        evaluate_hook_decision_with_env(&input, &env_rooted_at(&root)).is_empty(),
        "an explicit Markdown glob still narrows an absolute project-root path"
    );
}

#[test]
fn test_bash_allows_absolute_root_when_root_is_unknown() {
    let (_tmp, root) = indexed_project();
    let input =
        serde_json::json!({"command": format!("grep -rn handle_request {}", root.display())})
            .to_string();
    assert!(
        evaluate_hook_decision_with_env(&input, &env_indexed()).is_empty(),
        "without a known root the classifier must fail open"
    );
}

#[test]
fn test_bash_allows_absolute_root_when_root_does_not_exist() {
    let (tmp, _root) = indexed_project();
    let missing = tmp.path().join("gone");
    let input =
        serde_json::json!({"command": format!("grep -rn handle_request {}", missing.display())})
            .to_string();
    assert!(
        evaluate_hook_decision_with_env(&input, &env_rooted_at(&missing)).is_empty(),
        "a root that cannot be canonicalized must fail open"
    );
}

#[test]
fn test_bash_absolute_project_root_respects_opt_out() {
    let (_tmp, root) = indexed_project();
    let input =
        serde_json::json!({"command": format!("grep -rn handle_request {}", root.display())})
            .to_string();
    let disabled = HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: true,
        project_root: Some(root.clone()),
    };
    assert!(
        evaluate_hook_decision_with_env(&input, &disabled).is_empty(),
        "TOKENSAVE_DISABLE_GREP_HOOK=1 must bypass the root-target redirect too"
    );
}

#[test]
fn test_droid_execute_blocks_absolute_project_root() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "tool_name": "Execute",
        "tool_input": {"command": format!("rg -n handle_request {}", root.display())},
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_rooted_at(&root)).is_some(),
        "the Droid adapter shares the classifier, so the root target redirects there too"
    );
}

#[test]
fn test_droid_native_grep_blocks_absolute_project_root_path() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "tool_name": "Grep",
        "tool_input": {
            "pattern": "handle_request",
            "path": root.to_string_lossy(),
            "output_mode": "content",
        },
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_rooted_at(&root)).is_some(),
        "Droid's native Grep takes the same absolute-root path as its Execute tool"
    );
}

#[test]
fn test_bash_blocks_grep_on_root_reached_through_parent_segments() {
    let (_tmp, root) = indexed_project();
    let docs = root.join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let input = serde_json::json!({
        "command": format!("grep -rn handle_request {}/..", docs.display()),
    })
    .to_string();
    assert!(
        is_blocked(&evaluate_hook_decision_with_env(
            &input,
            &env_rooted_at(&root)
        )),
        "canonicalization sees through `..`, so a detour still names the root"
    );
}

#[test]
fn test_bash_allows_grep_on_the_parent_of_the_project_root() {
    let (tmp, root) = indexed_project();
    let input = serde_json::json!({
        "command": format!("grep -rn handle_request {}", tmp.path().display()),
    })
    .to_string();
    assert!(
        evaluate_hook_decision_with_env(&input, &env_rooted_at(&root)).is_empty(),
        "a search wider than the index cannot be served by the graph"
    );
}

#[test]
fn test_bash_classifies_only_the_first_target() {
    let (_tmp, root) = indexed_project();
    let docs = root.join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let input = serde_json::json!({
        "command": format!("grep -rn handle_request {} {}", docs.display(), root.display()),
    })
    .to_string();
    assert!(
        evaluate_hook_decision_with_env(&input, &env_rooted_at(&root)).is_empty(),
        "multi-target commands keep the pre-existing first-target-only classification"
    );
}

// --- activation scope: ancestor discovery and event cwd ------------------

#[test]
fn test_from_runtime_at_discovers_the_root_from_a_subdirectory() {
    let (_tmp, root) = indexed_project();
    let nested = root.join("crates").join("api").join("src");
    std::fs::create_dir_all(&nested).expect("create nested dirs");
    let env = HookEnv::from_runtime_at(Some(&nested));
    assert!(
        env.in_tokensave_project,
        "a subdirectory of an indexed project is still the indexed project"
    );
    assert_eq!(env.project_root.as_deref(), Some(root.as_path()));
}

#[test]
fn test_from_runtime_at_outside_any_project_is_inactive() {
    let (tmp, _root) = indexed_project();
    let outside = tmp.path().join("elsewhere");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let env = HookEnv::from_runtime_at(Some(&outside));
    assert!(!env.in_tokensave_project);
    assert!(env.project_root.is_none());
}

#[test]
fn test_droid_event_cwd_activates_the_guardrail() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "tool_name": "Execute",
        "tool_input": {"command": "rg -n handle_request src/"},
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_not_indexed()).is_some(),
        "the event's own cwd must decide the project, not the hook process cwd"
    );
}

#[test]
fn test_droid_event_cwd_below_the_root_activates_the_guardrail() {
    let (_tmp, root) = indexed_project();
    let nested = root.join("crates").join("api");
    std::fs::create_dir_all(&nested).expect("create nested dirs");
    let input = serde_json::json!({
        "cwd": nested.to_string_lossy(),
        "tool_name": "Execute",
        "tool_input": {"command": "rg -n handle_request src/"},
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_not_indexed()).is_some(),
        "a session started in a subdirectory is still inside the indexed project"
    );
}

#[test]
fn test_droid_event_cwd_outside_a_project_deactivates_the_guardrail() {
    let (tmp, _root) = indexed_project();
    let outside = tmp.path().join("elsewhere");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let input = serde_json::json!({
        "cwd": outside.to_string_lossy(),
        "tool_name": "Execute",
        "tool_input": {"command": "rg -n handle_request src/"},
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_indexed()).is_none(),
        "an event cwd outside any index has nothing to redirect to"
    );
}

#[test]
fn test_droid_event_cwd_still_honors_the_opt_out() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "tool_name": "Execute",
        "tool_input": {"command": "rg -n handle_request src/"},
    })
    .to_string();
    // Opted out where the hook runs, but the event points at an indexed
    // project: the override must not resurrect the guardrail.
    let disabled_outside = HookEnv {
        in_tokensave_project: false,
        disable_grep_hook: true,
        project_root: None,
    };
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &disabled_outside).is_none(),
        "TOKENSAVE_DISABLE_GREP_HOOK=1 outranks any discovered project"
    );
}

#[test]
fn test_droid_explorer_task_with_event_cwd_still_blocks() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "tool_name": "Task",
        "tool_input": {"subagent_type": "explorer", "prompt": "map the codebase"},
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_not_indexed()).is_some(),
        "the built-in explorer guard must survive event-cwd resolution"
    );
}

#[test]
fn test_droid_worker_task_with_event_cwd_still_passes() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "tool_name": "Task",
        "tool_input": {"subagent_type": "worker", "prompt": "explore the codebase structure"},
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_indexed()).is_none(),
        "a typed non-explorer task stays a deliberate delegation"
    );
}

#[test]
fn test_kiro_event_cwd_activates_the_delegation_block() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "tool_name": "delegate",
        "tool_input": {"task": "explore the codebase and map the call graph"},
    })
    .to_string();
    assert!(
        evaluate_kiro_pre_tool_use_with_env(&input, &env_not_indexed()).is_some(),
        "Kiro's preToolUse hook should resolve the project like its postToolUse hook"
    );
}

#[test]
fn test_kiro_event_cwd_outside_a_project_deactivates_the_delegation_block() {
    let (tmp, _root) = indexed_project();
    let outside = tmp.path().join("elsewhere");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let input = serde_json::json!({
        "cwd": outside.to_string_lossy(),
        "tool_name": "delegate",
        "tool_input": {"task": "explore the codebase and map the call graph"},
    })
    .to_string();
    assert!(
        evaluate_kiro_pre_tool_use_with_env(&input, &env_indexed()).is_none(),
        "the event cwd decides in both directions, for Kiro too"
    );
}

#[test]
fn test_claude_event_cwd_activates_the_guardrail() {
    let (_tmp, root) = indexed_project();
    let input = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "tool_name": "Grep",
        "tool_input": {"pattern": "handle_request", "path": "src", "output_mode": "content"},
    })
    .to_string();
    assert!(
        is_blocked(&evaluate_claude_pre_tool_use_with_env(
            &input,
            &env_not_indexed()
        )),
        "Claude's event cwd should decide the project too"
    );
}

#[test]
fn test_claude_event_cwd_outside_a_project_deactivates_the_guardrail() {
    let (tmp, _root) = indexed_project();
    let outside = tmp.path().join("elsewhere");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let input = serde_json::json!({
        "cwd": outside.to_string_lossy(),
        "tool_name": "Grep",
        "tool_input": {"pattern": "handle_request", "path": "src", "output_mode": "content"},
    })
    .to_string();
    assert!(
        evaluate_claude_pre_tool_use_with_env(&input, &env_indexed()).is_empty(),
        "the event cwd decides in both directions, for Claude too"
    );
}

#[test]
fn test_unusable_event_cwd_keeps_the_process_environment() {
    let (tmp, root) = indexed_project();
    let missing = root.join("gone");
    let file = root.join("Cargo.toml");
    std::fs::write(&file, "[package]\n").expect("write file");
    let cases = vec![
        ("no key".to_string(), None),
        ("blank".to_string(), Some(serde_json::json!(""))),
        ("whitespace".to_string(), Some(serde_json::json!("   "))),
        (
            "relative".to_string(),
            Some(serde_json::json!("crates/api")),
        ),
        (
            "non-existent".to_string(),
            Some(serde_json::json!(missing.to_string_lossy())),
        ),
        (
            "a file, not a directory".to_string(),
            Some(serde_json::json!(file.to_string_lossy())),
        ),
        ("null".to_string(), Some(serde_json::Value::Null)),
        ("a number".to_string(), Some(serde_json::json!(7))),
        (
            "an object".to_string(),
            Some(serde_json::json!({"path": tmp.path().to_string_lossy()})),
        ),
    ];

    for (label, cwd) in cases {
        let mut event = serde_json::json!({
            "tool_name": "Execute",
            "tool_input": {"command": "rg -n handle_request src/"},
        });
        if let Some(cwd) = cwd {
            event["cwd"] = cwd;
        }
        let input = event.to_string();
        assert!(
            evaluate_droid_pre_tool_use_with_env(&input, &env_indexed()).is_some(),
            "{label}: an unusable cwd must fall back to the hook process environment"
        );
        assert!(
            evaluate_droid_pre_tool_use_with_env(&input, &env_not_indexed()).is_none(),
            "{label}: the fallback must not invent a project either"
        );
    }
}

#[test]
fn test_absolute_ancestor_root_target_redirects_from_a_subdirectory() {
    let (_tmp, root) = indexed_project();
    let nested = root.join("crates").join("api");
    std::fs::create_dir_all(&nested).expect("create nested dirs");
    let input = serde_json::json!({
        "cwd": nested.to_string_lossy(),
        "tool_name": "Execute",
        "tool_input": {"command": format!("rg -n handle_request {}", root.display())},
    })
    .to_string();
    assert!(
        evaluate_droid_pre_tool_use_with_env(&input, &env_not_indexed()).is_some(),
        "the discovered ancestor root is the whole-project target"
    );
}

// ============================================================================
// #435: grep / find targets outside the indexed project must pass through.
// These tests require a real project root on disk because containment is
// checked by canonicalizing the resolved path.
// ============================================================================

#[test]
fn test_bash_allows_grep_after_cd_to_outside_project() {
    let (tmp, root) = indexed_project();
    let outside = tmp.path().join("other");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let input = serde_json::json!({
        "command": format!("cd {} && grep -rn Foo --include=*.cs .", outside.display()),
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        result.is_empty(),
        "grep after cd outside the project should pass through: {result}"
    );
}

#[test]
fn test_bash_allows_grep_on_absolute_outside_path() {
    let (tmp, root) = indexed_project();
    let outside = tmp.path().join("other");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let file = outside.join("file.cs");
    std::fs::write(&file, "class Foo {}\n").expect("write file");
    let input = serde_json::json!({
        "command": format!("grep -rn Foo {}", file.display()),
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        result.is_empty(),
        "absolute outside path should pass through: {result}"
    );
}

#[test]
fn test_bash_blocks_grep_after_cd_to_inside_project_with_existing_file() {
    let (_tmp, root) = indexed_project();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");
    std::fs::write(src.join("lib.rs"), "pub fn Foo() {}\n").expect("write file");
    let input = serde_json::json!({
        "command": "cd src && grep -rn Foo lib.rs",
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        is_blocked(&result),
        "grep after cd inside the project should still redirect: {result}"
    );
}

#[test]
fn test_bash_blocks_grep_after_cd_to_inside_project_with_missing_file() {
    let (_tmp, root) = indexed_project();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");
    let input = serde_json::json!({
        "command": "cd src && grep -rn Foo missing.rs",
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        is_blocked(&result),
        "grep after cd inside the project should still redirect even when the file is missing: {result}"
    );
}

#[test]
fn test_bash_allows_find_after_cd_to_outside_project() {
    let (tmp, root) = indexed_project();
    let outside = tmp.path().join("other");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    let input = serde_json::json!({
        "command": format!("cd {} && find . -name '*.rs'", outside.display()),
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        result.is_empty(),
        "find after cd outside the project should pass through: {result}"
    );
}

#[test]
fn test_bash_blocks_find_after_cd_to_inside_project() {
    let (_tmp, root) = indexed_project();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");
    let input = serde_json::json!({
        "command": "cd src && find . -name '*.rs'",
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        is_blocked(&result),
        "find after cd inside the project should still redirect: {result}"
    );
}

#[cfg(unix)]
#[test]
fn test_bash_allows_grep_after_cd_to_escaped_outside_directory() {
    let (tmp, root) = indexed_project();
    let outside = tmp.path().join("dir with spaces");
    std::fs::create_dir_all(&outside).expect("create outside dir with spaces");
    let input = serde_json::json!({
        "command": format!("cd {}\\ with\\ spaces && grep -rn Foo --include=*.cs .", tmp.path().join("dir").display()),
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        result.is_empty(),
        "backslash-escaped cd to an outside directory should pass through: {result}"
    );
}

#[cfg(unix)]
#[test]
fn test_bash_blocks_grep_after_cd_to_symlink_to_code_directory() {
    use std::os::unix::fs::symlink;

    let (_tmp, root) = indexed_project();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");
    std::fs::write(src.join("lib.rs"), "pub fn Foo() {}\n").expect("write file");
    let sym = root.join("sym-src");
    symlink(&src, &sym).expect("create symlink to src");
    let input = serde_json::json!({
        "command": "cd sym-src && grep -rn Foo .",
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        is_blocked(&result),
        "cd into a symlink that points to a code directory should still redirect: {result}"
    );
}

#[test]
fn test_bash_allows_grep_after_chained_cd_uses_first_directory() {
    let (tmp, root) = indexed_project();
    let outside = tmp.path().join("src");
    std::fs::create_dir_all(&outside).expect("create outside src dir");
    let input = serde_json::json!({
        "command": format!(
            "cd {} && cd src && grep -rn Foo lib.rs",
            tmp.path().display()
        ),
    })
    .to_string();
    let result = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        result.is_empty(),
        "only the first leading cd is modeled; a second cd must not override it: {result}"
    );
}
