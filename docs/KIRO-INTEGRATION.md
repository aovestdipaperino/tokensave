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
| `~/.kiro/settings/mcp.json` | Registers the global `tokensave` MCP server with `command`, `args: ["serve"]`, `disabled: false`, and `autoApprove` for read-only `tokensave_*` tools. |
| `~/.kiro/steering/AGENTS.md` | Adds a bounded global Kiro steering block that tells normal Kiro sessions to prefer tokensave MCP tools for codebase research. |
| `~/.kiro/agents/tokensave.json` | Adds the tokensave-managed Kiro agent with hooks for delegation guardrails and post-write sync. |
| `~/.kiro/settings/cli.json` | Sets `chat.defaultAgent` to `tokensave` when the setting is absent or still points at Kiro's built-in default. |

If a user already has `~/.kiro/agents/tokensave.json` and it is not the file
tokensave writes, install and uninstall leave it untouched. In that case
tokensave also does not point `chat.defaultAgent` at that user-managed file.
If `chat.defaultAgent` already names another custom agent, install leaves that
choice unchanged and prints a warning.

Uninstall removes only the steering block bounded by tokensave's installed
marker, the global MCP server entry, the tokensave-owned agent file, and
`chat.defaultAgent` when it points at that owned agent. User-authored steering
after the installed block remains in place.

## Tool approval defaults

Kiro supports MCP-level `autoApprove`. The installed default uses it only for
tools whose MCP annotation sets `readOnlyHint: true`.

Read-only graph, search, health, branch, body, and session-recall tools run
without a prompt. Tools that can mutate files or local tokensave state remain
available, but Kiro asks before using them. Today that means these tools are
not auto-approved:

- `tokensave_str_replace`
- `tokensave_multi_str_replace`
- `tokensave_insert_at`
- `tokensave_ast_grep_rewrite`
- `tokensave_session_start`
- `tokensave_session_end`
- `tokensave_record_decision`
- `tokensave_record_code_area`

This is intentionally more conservative than Claude Code's permission model.
Kiro's MCP security model supports auto-approval, but default setup should only
pre-approve frequent, read-only, limited-scope tools.

## Workspace overrides

Kiro can also load workspace MCP settings from `.kiro/settings/mcp.json`. A
workspace `mcpServers.tokensave` entry takes precedence over the global
`~/.kiro/settings/mcp.json` entry installed by tokensave.

`tokensave doctor --agent kiro` checks the current workspace for that override.
It reports a problem when the workspace entry disables tokensave, omits the
`serve` argument, or points at a different command than the global install. It
also warns when the workspace entry shadows the global read-only `autoApprove`
defaults.

## Default-agent judgement call

The install is intentionally conservative:

- `chat.defaultAgent` absent, empty, or `kiro_default`: set it to `tokensave`.
- `chat.defaultAgent` already `tokensave`: leave it unchanged.
- `chat.defaultAgent` names another custom agent: leave it unchanged and warn.
- `~/.kiro/agents/tokensave.json` exists but is user-managed: leave it unchanged
  and do not select it as the default.

Users can still select the tokensave agent manually later, or copy the hook
mapping into their own agent configuration.

## Custom agents after setup

Users can still create their own Kiro custom agents after running the default
tokensave setup. Those agents can inherit the global MCP server by setting:

```json
{
  "includeMcpJson": true
}
```

For a global custom agent under `~/.kiro/agents/`, the installed steering file
can also be referenced instead of copied:

```json
{
  "prompt": "file://../steering/AGENTS.md"
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

No `execute_bash`, `fs_write` pre-approval, shell post-hook, or `stop` hook is
installed. Shell commands are too broad for default sync triggering, and Kiro's
stop event should not be used for Claude-style accounting until Kiro's persisted
session format is verified.

Kiro-specific session accounting is also held back. Claude's stop hook parses
Claude session transcripts; Kiro does not share that transcript format, so
session accounting should only be added after Kiro's persisted session format is
verified.
