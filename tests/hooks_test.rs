use tokensave::hooks::{
    evaluate_claude_pre_tool_use_with_env, evaluate_droid_pre_tool_use_with_env,
    evaluate_hook_decision_with_env, evaluate_kiro_pre_tool_use_with_env, HookEnv,
};

fn env_indexed() -> HookEnv {
    HookEnv {
        cwd_has_tokensave_db: true,
        disable_grep_hook: false,
    }
}

fn env_not_indexed() -> HookEnv {
    HookEnv {
        cwd_has_tokensave_db: false,
        disable_grep_hook: false,
    }
}

fn env_disabled() -> HookEnv {
    HookEnv {
        cwd_has_tokensave_db: true,
        disable_grep_hook: true,
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
    let input = r#"{"pattern": "FooBar", "glob": "**/*.md"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "glob over .md should pass through");
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
fn test_bash_allows_find_without_grep() {
    let input = r#"{"command": "find . -name \"*.rs\" -type f"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(result.is_empty(), "find alone should pass through");
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
// object. The `^(Execute|Grep)$` matcher is installed, so grep/bash-shaped
// `command` payloads and Droid's native `Grep` `pattern` payloads reach this
// handler; the shared decision core is still exercised directly below for the
// sub-agent-shaped payload, which no installed matcher routes here.
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
    // blocked. Droid's own sub-agent/task launch tool name is unconfirmed in
    // Factory's public docs, so today such a call never reaches this hook
    // (`^(Execute|Grep)$` is the registered matcher). This test guards the
    // shared decision core directly in case that matcher scope widens to cover
    // a delegation tool.
    let input = r#"{
        "subagent_type": "implementer",
        "prompt": "Implement the retry logic for the sync client and add tests"
    }"#;
    assert!(evaluate_droid_pre_tool_use_with_env(input, &env_indexed()).is_none());
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
