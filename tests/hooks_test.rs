use tokensave::hooks::evaluate_hook_decision;

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
