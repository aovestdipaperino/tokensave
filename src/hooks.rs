//! Hook handlers for Claude Code integration.
//!
//! These functions are invoked by Claude Code's hook system to intercept
//! tool calls and redirect exploration work to tokensave MCP tools.

/// PreToolUse hook handler for Claude Code's Agent tool matcher.
///
/// Reads the `TOOL_INPUT` environment variable (JSON), inspects the
/// `subagent_type` and `prompt` fields, and prints a JSON decision to
/// stdout. Blocks Explore agents and exploration-style prompts, directing
/// Claude to use tokensave MCP tools instead.
pub fn hook_pre_tool_use() {
    let tool_input = std::env::var("TOOL_INPUT").unwrap_or_default();
    println!("{}", evaluate_hook_decision(&tool_input));
}

/// Pure decision logic for the PreToolUse hook.
///
/// Takes the raw `TOOL_INPUT` JSON string and returns the JSON decision
/// string to print to stdout.
pub fn evaluate_hook_decision(tool_input: &str) -> String {
    let block_msg = serde_json::json!({
        "decision": "block",
        "reason": "STOP: Use tokensave MCP tools (tokensave_context, tokensave_search, \
                   tokensave_callees, tokensave_callers, tokensave_impact, tokensave_files, \
                   tokensave_affected) instead of agents for code research. Tokensave is \
                   faster and more precise for symbol relationships, call paths, and code \
                   structure. Only use agents for code exploration if you have already tried \
                   tokensave and it cannot answer the question."
    });

    let parsed: serde_json::Value =
        serde_json::from_str(tool_input).unwrap_or_else(|_| serde_json::json!({}));

    // Block Explore agents outright
    if parsed.get("subagent_type").and_then(|v| v.as_str()) == Some("Explore") {
        return block_msg.to_string();
    }

    // Check if the prompt is exploration/research work that tokensave can handle
    if let Some(prompt) = parsed.get("prompt").and_then(|v| v.as_str()) {
        let lower = prompt.to_ascii_lowercase();
        let exploration_patterns = [
            "explore", "codebase structure", "codebase architecture", "codebase overview",
            "source files contents", "read every", "full contents", "entire codebase",
            "architecture and structure", "call graph", "call path", "call chain",
            "symbol relat", "symbol lookup", "who calls", "callers of", "callees of",
        ];
        if exploration_patterns.iter().any(|pat| lower.contains(pat)) {
            return block_msg.to_string();
        }
    }

    r#"{"decision": "allow"}"#.to_string()
}
