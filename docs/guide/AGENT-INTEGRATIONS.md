# Agent Integrations

How `tokensave install` connects tokensave to your AI coding agents, and what it writes for each one.

## Configuring your agent

Run the setup command to auto-detect your installed agents:

```bash
tokensave install
```

To choose an agent, use `--agent` (for example, `codex`, `claude`, or `cursor`):

```bash
tokensave install --agent codex
```

Supported agents (use the identifier with `--agent`):

| Agent | Identifier |
|---|---|
| Antigravity | `antigravity` |
| AugmentCode | `auggie` |
| Claude Code | `claude` |
| Cline | `cline` |
| Codex CLI | `codex` |
| GitHub Copilot | `copilot` |
| Cursor | `cursor` |
| Factory Droid | `droid` |
| Gemini CLI | `gemini` |
| Grok Build | `grok` |
| Kilo CLI | `kilo` |
| Kimi CLI | `kimi` |
| Kiro | `kiro` |
| Oh My Pi (OMP) | `omp` |
| OpenCode | `opencode` |
| Pi | `pi` |
| Plank (macOS only) | `plank` |
| Qwen Code | `qwen` |
| Roo Code | `roo-code` |
| Mistral Vibe | `vibe` |
| Zed | `zed` |

Setup registers the MCP server and the agent's integration. It also offers global Git hooks to keep initialized projects up to date. You can safely run setup again after upgrading.

## What each agent gets

The Pi extension and Pi session-accounting details below describe stable builds; beta currently retains its earlier Pi integration.

Each agent gets its MCP server registered in the native config format. Claude Code additionally gets a PreToolUse hook (blocks wasteful Explore agents), a UserPromptSubmit hook, a Stop hook, prompt rules in CLAUDE.md, and auto-allowed tool permissions. Pi gets `"exposure": "direct"` on its MCP entry -- Pi otherwise defaults a server to `codemode`, which declares none of its tools to the model -- prompt rules in `AGENTS.md`, and a managed TypeScript extension that supplies the same guardrail, savings reset, and session receipt, because Pi has no shell hooks. Kiro gets global MCP config, `tokensave.md` steering loaded as a resource, and a tokensave-managed default agent with permissive built-in/tokensave tool approval, delegation guardrail hooks, and post-write sync; user-managed Kiro agents are preserved.

Global OMP installs target the profile reported by bare `omp config path`, writing `<resolved-agent-dir>/mcp.json` and `<resolved-agent-dir>/rules/tokensave.md`. Export `OMP_PROFILE` or OMP's compatible `PI_PROFILE` when installing into a named profile; OMP's resolver also honors `PI_CONFIG_DIR` and `PI_CODING_AGENT_DIR`. Tokensave trusts that native resolver rather than duplicating OMP's profile logic. Tokensave installs MCP and advisory rules for OMP; it does not install OMP hook enforcement.

All changes are idempotent -- safe to run again after upgrading. After agent setup, you'll be offered global git post-commit and post-checkout hooks. `tokensave uninstall` removes those hooks along with the agent integrations; pass `--keep-git-hooks` to leave them, or manage them on their own with `tokensave githooks`.

## Installing for several agents at once

Repeat `--agent` to install several agents in a single run. The permission grant, git-hook offer, and global config write each happen once, no matter how many agents you list:

```bash
tokensave install --agent claude --agent cursor --agent droid
```

## Project-local install

By default `tokensave install` registers the MCP server in your **global** agent config (e.g. `~/.claude.json`). To register tokensave for just the current project instead, add `--local`:

```bash
tokensave install --local --agent claude
tokensave install --local --agent omp
```

This writes project-scoped config you can commit and share with your team. For Claude that's `./.mcp.json`, `./.claude/settings.json`, and `./CLAUDE.md`; OMP uses `./.omp/mcp.json` and `./.omp/rules/tokensave.md` without invoking the OMP CLI. Supported agents: **claude, cursor, droid, gemini, zed, opencode, roo-code, kiro, auggie, omp, plank, pi** (each writes its own project file, e.g. `.cursor/mcp.json`, `.factory/mcp.json`, `.gemini/settings.json`, `.zed/settings.json`, `opencode.json`, `.roo/mcp.json`, `.kiro/settings/mcp.json`, `.augment/settings.json`, `.omp/mcp.json`, `.mcp.json` for plank, `.pi/mcp.json` and `./AGENTS.md` for pi). Other agents have no project-scoped config and report an error with `--local`.

Remove a project-local install with `tokensave uninstall --local`.

To accept or skip the Git hooks without a prompt, add `--git-hook yes` or `--git-hook no`. Use `tokensave githooks` to inspect them, or `tokensave githooks off` to remove them.

For agent-specific instructions, see [Connecting to Your Agent](../USER-GUIDE.md#connecting-to-your-agent).

## What install writes for Claude Code

### MCP server

```json
{
  "mcpServers": {
    "tokensave": {
      "command": "/path/to/tokensave",
      "args": ["serve"]
    }
  }
}
```

### PreToolUse hook

The hook runs `tokensave hook-pre-tool-use` -- a native Rust command (no bash or jq required). It intercepts Agent, Grep, Glob, and Bash tool calls: Explore agents are blocked outright, symbol-shaped grep/rg/ag invocations (plain identifiers, alternations, `\b`-wrapped names) are redirected to the matching tokensave MCP tool, and path-shaped discovery (`Glob`, `find -name`, `fd --extension`) over code extensions is redirected to `tokensave_files`. Regex patterns, `git grep`, piped commands, non-code extensions, search roots outside the index, and `find` predicates that change what the command does (`-exec`, `-delete`, `-mtime`) all pass through untouched; set `TOKENSAVE_DISABLE_GREP_HOOK=1` to opt out per shell.

Filters are read most-specific-first: an explicit `type` is authoritative, then an explicit file glob, then the search path. A documentation search such as `path: "."` with `glob: "**/*.md"` therefore passes through rather than being treated as a code search on the broad path, while a code-only glob (`**/*.rs`) still redirects even under a non-code path. Mixed globs (`**/*.{rs,md}`) pass through, since they can return documentation.

**Headless / subagent dispatch (`claude -p`).** Child processes dispatched by an orchestrating session inherit its `~/.claude/settings.json`, including this hook. To let a child run raw searches, set `TOKENSAVE_DISABLE_GREP_HOOK=1` in the child's environment -- the native binary honors it and passes every path (Grep, Glob, Bash, Agent) through, so there is no need for the blunt `--settings '{"hooks": {}}'` that strips *all* hooks. The guardrail is stateless: it never consults citation history, so it only ever redirects the symbol-shaped searches described above and steers untyped research fan-out; ordinary commands are unaffected whether the session is interactive or headless.

### CLAUDE.md rules

Appends instructions to `~/.claude/CLAUDE.md` that tell Claude to use tokensave tools before reaching for Explore agents or raw file reads.

## How it works with Claude Code

Once configured, Claude Code automatically uses tokensave instead of reading raw files when it needs to understand your codebase. These layers reinforce each other:

| Layer | What it does | Why it matters |
|-------|-------------|----------------|
| **MCP server** | Exposes 80+ `tokensave_*` tools to Claude | Claude can query the graph directly |
| **CLAUDE.md rules** | Tells Claude to prefer tokensave over agents/file reads | Prevents the model from falling back to expensive patterns |
| **PreToolUse hook** | Native Rust hook blocks Explore agents | Catches cases where the model ignores the CLAUDE.md rules |
| **UserPromptSubmit hook** | Runs at prompt submission | Lifecycle tracking for token accounting |
| **Stop hook** | Runs when the session ends | Flushes token counters |

The result: Claude gets the same code understanding with far fewer tokens. A typical Explore agent reads 20-50 files; tokensave returns the relevant symbols, relationships, and code snippets from its pre-built index.
