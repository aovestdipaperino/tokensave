use tokensave::hooks::{
    evaluate_claude_pre_tool_use, evaluate_hook_decision, evaluate_hook_decision_with_env,
    evaluate_kiro_pre_tool_use, HookEnv,
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
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_allows_non_explore_agent() {
    let input = r#"{"subagent_type": "general-purpose", "prompt": "write a function"}"#;
    let result = evaluate_hook_decision(input);
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_blocks_exploration_prompt_explore() {
    let input = r#"{"prompt": "Explore the codebase and find all API endpoints"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_codebase_structure_prompt() {
    let input = r#"{"prompt": "Understand the codebase structure"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_call_graph_prompt() {
    let input = r#"{"prompt": "Show me the call graph for this function"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_who_calls_prompt() {
    let input = r#"{"prompt": "who calls the process_data function?"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_callers_of_prompt() {
    let input = r#"{"prompt": "find callers of handle_request"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_callees_of_prompt() {
    let input = r#"{"prompt": "what are the callees of main?"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_symbol_lookup_prompt() {
    let input = r#"{"prompt": "do a symbol lookup for TokenSave"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_read_every_prompt() {
    let input = r#"{"prompt": "read every file in src/"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_blocks_entire_codebase_prompt() {
    let input = r#"{"prompt": "scan the entire codebase for patterns"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_allows_normal_prompt() {
    let input = r#"{"prompt": "write a unit test for the parse function"}"#;
    let result = evaluate_hook_decision(input);
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_allows_empty_input() {
    let result = evaluate_hook_decision("");
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_allows_invalid_json() {
    let result = evaluate_hook_decision("not json at all");
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_allows_no_prompt_no_subagent() {
    let input = r#"{"foo": "bar"}"#;
    let result = evaluate_hook_decision(input);
    assert!(result.is_empty(), "allow should produce no output");
}

#[test]
fn test_case_insensitive_blocking() {
    let input = r#"{"prompt": "EXPLORE the Codebase Architecture"}"#;
    let result = evaluate_hook_decision(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_block_response_has_reason() {
    let input = r#"{"subagent_type": "Explore"}"#;
    let result = evaluate_hook_decision(input);
    let reason = get_block_reason(&result);
    assert!(reason.contains("tokensave MCP tools"));
}

#[test]
fn test_block_response_uses_correct_hook_schema() {
    let input = r#"{"subagent_type": "Explore"}"#;
    let result = evaluate_hook_decision(input);
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
    let reason = evaluate_kiro_pre_tool_use(input).unwrap();
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
    assert!(evaluate_kiro_pre_tool_use(input).is_some());
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
    assert!(evaluate_kiro_pre_tool_use(input).is_none());
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
    assert!(evaluate_kiro_pre_tool_use(input).is_none());
}

#[test]
fn test_kiro_allows_invalid_json() {
    assert!(evaluate_kiro_pre_tool_use("not json").is_none());
}

// ============================================================================
// Grep tool redirect — symbol-shaped patterns against code files should
// redirect to tokensave_search / _signature_search / _callers.
// ============================================================================

#[test]
fn test_grep_blocks_bare_symbol_on_rust_file() {
    let input = r#"{"pattern": "FooBar", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "bare symbol on .rs should redirect");
}

#[test]
fn test_grep_blocks_alternation_on_rust_file() {
    let input = r#"{"pattern": "Foo\\|Bar\\|Baz", "path": "src/main.rs"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "alternation of identifiers should redirect"
    );
}

#[test]
fn test_grep_blocks_word_boundary_symbol() {
    let input = r#"{"pattern": "\\bhandle_request\\b", "path": "src/main.rs"}"#;
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
    let input = r#"{"pattern": "FooBar", "path": "src/"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(
        is_blocked(&result),
        "symbol search in src/ dir should redirect"
    );
}

#[test]
fn test_grep_blocks_when_only_glob_set() {
    let input = r#"{"pattern": "FooBar", "glob": "**/*.rs"}"#;
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
    let input = r#"{"pattern": "FooBar", "type": "rust"}"#;
    let result = evaluate_hook_decision_with_env(input, &env_indexed());
    assert!(is_blocked(&result), "type=rust should redirect");
}

#[test]
fn test_grep_block_message_names_tokensave_tool() {
    let input = r#"{"pattern": "FooBar", "path": "src/main.rs"}"#;
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
    let result = evaluate_hook_decision(input);
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
    let result = evaluate_claude_pre_tool_use(input);
    assert!(is_blocked(&result), "nested Explore agent should redirect");
}

#[test]
fn test_claude_blocks_research_prompt_nested_stdin() {
    let input = r#"{
        "hook_event_name": "PreToolUse",
        "tool_name": "Agent",
        "tool_input": {"prompt": "who calls the process_data function?"}
    }"#;
    let result = evaluate_claude_pre_tool_use(input);
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
    let result = evaluate_claude_pre_tool_use(input);
    assert!(result.is_empty(), "normal tool call should pass through");
}

#[test]
fn test_claude_falls_back_to_flat_tool_input() {
    // If the wrapper is absent, treat the payload as a flat tool_input object.
    let input = r#"{"subagent_type": "Explore"}"#;
    let result = evaluate_claude_pre_tool_use(input);
    assert!(is_blocked(&result));
}

#[test]
fn test_claude_allows_invalid_json() {
    assert!(evaluate_claude_pre_tool_use("not json").is_empty());
}
