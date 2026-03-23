#!/bin/bash
# Block Explore agents and exploration-style general-purpose agents
# when codegraph MCP tools should be used instead.
# This is a PreToolUse hook for the "Agent" tool matcher.
#
# Install: copy to ~/.claude/hooks/ and add to settings.json hooks.

BLOCK_MSG='{"decision": "block", "reason": "STOP: Use codegraph MCP tools (codegraph_context, codegraph_search, codegraph_callees, codegraph_callers, codegraph_impact) instead of agents for code research. Codegraph is faster and more precise for symbol relationships, call paths, and code structure. Only use agents for code exploration if you have already tried codegraph and it cannot answer the question."}'

SUBAGENT_TYPE=$(echo "$TOOL_INPUT" | jq -r '.subagent_type // empty' 2>/dev/null)

# Block Explore agents outright
if [ "$SUBAGENT_TYPE" = "Explore" ]; then
    echo "$BLOCK_MSG"
    exit 0
fi

# For any agent type, check if the prompt is exploration/research work
# that codegraph can handle
PROMPT=$(echo "$TOOL_INPUT" | jq -r '.prompt // empty' 2>/dev/null)
if [ -n "$PROMPT" ]; then
    if echo "$PROMPT" | grep -qiE 'explore.*(code|repo|project|crate)|codebase.*(structure|architecture|overview)|all.*\.rs.*files|source.*files.*contents|read every|full contents|entire codebase|architecture and structure|call.*(graph|path|chain)|symbol.*(relat|lookup)|who calls|callers of|callees of'; then
        echo "$BLOCK_MSG"
        exit 0
    fi
fi

echo '{"decision": "allow"}'
