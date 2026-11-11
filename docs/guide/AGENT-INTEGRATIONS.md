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

## Working alongside Caveman

[Caveman](https://github.com/JuliusBrussee/caveman) is a separate project that makes an agent write shorter prose. tokensave reduces what the agent reads; Caveman reduces what it writes. The two can run together without any setting. tokensave never installs, configures or runs Caveman, and its tool results are the same whether Caveman is installed or not.

To ask the agent to keep tokensave results exact while Caveman is on, opt in:

```toml
# ~/.tokensave/config.toml
caveman_compatibility = true
```

`TOKENSAVE_CAVEMAN_COMPATIBILITY` overrides the setting for one run: `0`, `false`, `no`, `off` or empty turn it off, and any other value turns it on. When cooperation is on and a Caveman skill is found for a supported agent, the MCP `initialize` instructions gain one paragraph. It tells the agent to keep using targeted tokensave queries, to quote code, identifiers, paths, line numbers, signatures, errors and numbers from tokensave results exactly, and that user instructions take precedence. Restart the agent's MCP connection after changing the setting or installing or removing Caveman. With the setting off, which is the default, no agent file is read and the instructions are unchanged.

tokensave looks for a skill named `caveman` in these places, for Claude Code, Codex, Cursor, Copilot, Gemini, OpenCode, Pi and OMP:

- `.agents/skills/caveman/` and `.<agent>/skills/caveman/` in the project;
- `skills/caveman/` and `caveman/skills/caveman/` in the agent's home, which honors `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and OpenCode's `XDG_CONFIG_HOME`;
- `~/.agents/skills/caveman/`;
- for Claude Code, the plugin path that `plugins/installed_plugins.json` records for `caveman@caveman`.

Check what tokensave sees with:

```bash
tokensave doctor --caveman
tokensave doctor --caveman --agent claude --session-id <claude-session-id>
```

This runs the normal `doctor` checks and adds a **Caveman compatibility** section. It also prints a JSON report per agent on stdout, so `tokensave doctor --caveman 2>/dev/null` gives only the JSON. The Caveman check reads files and changes nothing. A report separates installation (`absent`, `installed`, `partial` or `unsupported`) from activation in a session (`active`, `inactive` or `unknown`). A skill on disk only means that Caveman is available. Activation is reported only for a Claude Code session named with `--session-id`, and only when the plugin manifest names a Caveman release tokensave was checked against (4.0.1 and 4.1.0). tokensave then reads that session's `.caveman-sessions/<id>.mode` file. Caveman's machine-wide `.caveman-active` flag and its `defaultMode` setting never count as activation.

Caveman's optional proxy or input compression can rewrite tool results after tokensave has returned them, and tokensave cannot prevent that. If you need exact results in a run, turn off lossy input compression with Caveman's own controls. `doctor --caveman` says so whenever a Caveman skill is found. The design notes and findings are in [Caveman compatibility](../CAVEMAN-COMPATIBILITY.md).
