# Kiro integration

This documents the defaults installed by:

```bash
tokensave install --agent kiro
```

The integration configures Kiro's shared MCP and steering defaults, writes a
tokensave-owned Kiro agent, and selects that agent as the default only when doing
so does not overwrite a user's existing custom default-agent choice.

## Installed files

| File | Purpose |
|---|---|
| `~/.kiro/settings/mcp.json` | Registers the global `tokensave` MCP server with `command`, `args: ["serve"]`, and `disabled: false`. Approval policy is left to the managed Kiro agent. |
| `~/.kiro/steering/tokensave.md` | Adds global Kiro steering that tells normal Kiro sessions to prefer tokensave MCP tools for codebase research. |
| `~/.kiro/agents/tokensave.json` | Adds the tokensave-managed Kiro agent with `tools: ["*"]`, `allowedTools: ["@builtin", "@tokensave"]`, hooks for delegation guardrails, post-write sync, and an absolute `resources` entry for `~/.kiro/steering/tokensave.md`. The agent leaves `prompt` unset so Kiro's default prompt is used. |
| `~/.kiro/settings/cli.json` | Sets `chat.defaultAgent` to `tokensave` when the setting is absent or still points at Kiro's built-in default. |

If a user already has `~/.kiro/agents/tokensave.json` and it is not the file
tokensave writes, install and uninstall leave it untouched. In that case
tokensave also does not point `chat.defaultAgent` at that user-managed file.
If `chat.defaultAgent` already names another custom agent, install leaves that
choice unchanged and prints a warning.

Uninstall removes only the `tokensave.md` steering block, the global MCP server entry,
the tokensave-owned agent file, and `chat.defaultAgent` when it points at that
owned agent. User-authored steering after the installed block remains in place.

## Tool approval defaults

The tokensave-owned Kiro agent is intentionally permissive:

```json
{
  "tools": ["*"],
  "allowedTools": [
    "@builtin",
    "@tokensave"
  ]
}
```

`tools: ["*"]` keeps Kiro's built-in tools and configured MCP tools available.
`allowedTools` pre-approves Kiro's built-in tools and all tools served by the
`tokensave` MCP server, including mutating tokensave tools. This makes the
managed agent useful as a working example users can copy into their own Kiro
agents.

The global `~/.kiro/settings/mcp.json` entry does not set MCP-level
`autoApprove`. Ordinary Kiro sessions or other agents that only inherit the
global MCP server keep Kiro's normal approval prompts unless users deliberately
merge the managed agent's `allowedTools` policy.

## Workspace overrides

Kiro can also load workspace MCP settings from `.kiro/settings/mcp.json`. A
workspace `mcpServers.tokensave` entry takes precedence over the global
`~/.kiro/settings/mcp.json` entry installed by tokensave.

`tokensave doctor --agent kiro` checks the current workspace for that override.
It reports a problem when the workspace entry disables tokensave, omits the
`serve` argument, or points at a different command than the global install.

## Default-agent judgement call

The install is intentionally conservative:

- `chat.defaultAgent` absent, empty, or `kiro_default`: set it to `tokensave`.
- `chat.defaultAgent` already `tokensave`: leave it unchanged.
- `chat.defaultAgent` names another custom agent: leave it unchanged and warn.
- `~/.kiro/agents/tokensave.json` exists but is user-managed: leave it unchanged
  and do not select it as the default.

Users can still select the tokensave agent manually later, or copy the hook and
tool-policy mapping into their own agent configuration.

## Custom agents after setup

Users can still create their own Kiro custom agents after running the default
tokensave setup. Those agents can inherit the global MCP server and the same
permissive tool policy by merging:

```json
{
  "includeMcpJson": true,
  "tools": ["*"],
  "allowedTools": [
    "@builtin",
    "@tokensave"
  ]
}
```

For a custom agent, the installed steering file can also be referenced instead
of copied. Use an absolute resource URI for the global steering file so it does
not resolve relative to the current project directory:

```json
{
  "resources": ["file:///Users/<you>/.kiro/steering/tokensave.md"]
}
```

That keeps first-run setup simple and consistent with other tokensave agent
harnesses: tokensave owns its own default agent settings, while other custom
agents remain user-managed.

## Hooks

Kiro hooks are an agent-configuration field. `tokensave install --agent kiro`
writes them into the tokensave-owned agent file:

| Kiro hook | Matcher | Command | Purpose |
|---|---|---|---|
| `preToolUse` | `delegate` | `tokensave hook-kiro-pre-tool-use` | Blocks delegation when the delegated task is codebase research that should try tokensave MCP tools first. |
| `preToolUse` | `subagent` | `tokensave hook-kiro-pre-tool-use` | Applies the same guardrail to Kiro subagents. |
| `userPromptSubmit` | none | `tokensave hook-kiro-prompt-submit` | Silently resets the project-local per-turn savings counter. |
| `postToolUse` | `fs_write` | `tokensave hook-kiro-post-tool-use` | Silently runs an incremental `tokensave sync` after Kiro writes files, so the graph is re-indexed before later MCP queries. |

Kiro and Claude Code use different hook protocols. Claude's `PreToolUse` hook
expects a JSON decision on stdout. Kiro passes hook events on stdin and blocks
`preToolUse` by receiving exit code `2` with the reason on stderr, so Kiro uses
separate hidden hook subcommands.

The default steering still tells Kiro not to use `delegate` for codebase
exploration, architecture mapping, call graph work, symbol lookup, or other code
research until tokensave MCP tools have been tried. Delegation remains available
for execution-oriented work such as builds, tests, generated reports, or
independent implementation tasks.

## Deliberate non-defaults

No shell post-hook or `stop` hook is installed. The managed agent's tool
approval policy is permissive, but default hook execution is still scoped to the
known tokensave guardrail and sync events. Shell commands are too broad for
default sync triggering, and Kiro's stop event should not be used for
Claude-style accounting until Kiro's persisted session format is verified.

Kiro-specific session accounting is also held back. Claude's stop hook parses
Claude session transcripts; Kiro does not share that transcript format, so
session accounting should only be added after Kiro's persisted session format is
verified.
