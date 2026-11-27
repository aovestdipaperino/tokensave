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

Each agent gets its MCP server registered in the native config format. Claude Code additionally gets a PreToolUse hook (blocks wasteful Explore agents), a UserPromptSubmit hook, a Stop hook, prompt rules in CLAUDE.md, and auto-allowed tool permissions. Pi gets `"exposure": "direct"` on its MCP entry -- Pi otherwise defaults a server to `codemode`, which declares none of its tools to the model -- prompt rules in `AGENTS.md`, and a managed TypeScript extension that supplies the same guardrail, savings reset, and session receipt, because Pi has no shell hooks. Kiro gets global MCP config, `tokensave.md` steering loaded as a resource, and a tokensave-managed default agent with per-tool read-only approvals, delegation guardrail hooks, and post-write sync; user-managed Kiro agents are preserved.

Global OMP installs target the profile reported by bare `omp config path`, writing `<resolved-agent-dir>/mcp.json` and `<resolved-agent-dir>/rules/tokensave.md`. Export `OMP_PROFILE` or OMP's compatible `PI_PROFILE` when installing into a named profile; OMP's resolver also honors `PI_CONFIG_DIR` and `PI_CODING_AGENT_DIR`. Tokensave trusts that native resolver rather than duplicating OMP's profile logic. Tokensave installs MCP and advisory rules for OMP; it does not install OMP hook enforcement.

All changes are idempotent -- safe to run again after upgrading. After agent setup, you'll be offered global git post-commit and post-checkout hooks. `tokensave uninstall` removes those hooks along with the agent integrations; pass `--keep-git-hooks` to leave them, or manage them on their own with `tokensave githooks`. `uninstall` keeps every index and setting; to delete those as well, see [Uninstalling](INSTALLATION.md#uninstalling).

## Tool approvals

Fresh installations auto-approve only classified read-only tools where per-tool approval is available. Write-capable tools remain available through the agent's confirmation flow.

| Agent | Default policy on a fresh install |
|---|---|
| Claude Code | Exact read-only entries in `permissions.allow` |
| Codex CLI | `approval_mode = "auto"` for exact read-only tools |
| Kiro CLI 2.x | Exact read-only MCP selectors in the TokenSave profile's `allowedTools` |
| Gemini CLI and Qwen Code | Leave whole-server trust unset so the agent can prompt |
| Other integrations | Leave approvals to the agent's policy |

Kiro CLI V3 uses capability-based permissions. Follow Kiro's profile migration instructions when converting a CLI 2.x profile. [Kiro configuration reference](https://kiro.dev/docs/cli/v3/agent-config/)

To auto-approve a particular write tool in a supported integration, use its exact MCP name. Repeat the option to select multiple tools. An invocation can approve only tools available in that build.

```bash
tokensave install --agent claude --approve-write-tool tokensave_str_replace
tokensave install --agent claude --reset-tool-approvals
```

Regular install, reinstall and automatic upgrade maintenance add missing read-only grants while preserving existing approval choices. Older wildcard, whole-server trust and write grants can continue to approve writes. Use `--reset-tool-approvals` to replace TokenSave-specific write grants and old blanket trust with safe defaults while preserving other agents' and tools' settings. The old `--explicit-permissions` spelling is an alias for reset. `--wildcard-permissions` is rejected because it would automatically cover future write tools.

The core capability registry supplies exact approval candidates and the MCP `readOnlyHint`. Source edits, session baseline and persistent memory changes, the diagnostics compiler and the test runner require write opt-in. Read-only classification describes the requested operation. Routine index freshness and bookkeeping can still occur, and listing more tools changes only the session's discovery state. An unclassified tool receives no generated approval.

Set `manage_permissions = false` in `~/.tokensave/config.toml`, or `TOKENSAVE_MANAGE_PERMISSIONS=0` for one run, to manage policies yourself. Install and upgrade leave existing approvals as found. Uninstall preserves a server entry or Kiro profile when removing it would delete attached user-managed approvals. Remove those entries separately in the agent if you want to disconnect them too.

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

Preview a project-local uninstall with `tokensave uninstall --local --dry-run`. Run it again without `--dry-run` to disconnect those integrations.

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

The native `tokensave hook-pre-tool-use` command handles Agent, Grep, Glob, and Bash tool calls without requiring bash or jq. It blocks Explore agents, provides guidance for eligible Grep searches, and redirects path-shaped discovery (`Glob`, `find -name`, `fd --extension`) over code extensions to `tokensave_files`.

An Agent call with an explicit non-Explore `subagent_type` always passes. An untyped Agent call is redirected only when its prompt reads as code research. Code-specific phrases such as "call graph", "callers of" or "codebase structure" are enough on their own. Generic phrases such as "explore", "read every" or "full contents" also need a code word in the prompt ("code", "source", "src", "repo", "function", "class", "module" and similar), so an agent that reads plain-text or OCR files is not blocked.

**Claude Code native Grep.** Claude's hook event does not report whether the session's TokenSave MCP connection is ready. Eligible native Grep calls receive optional search advice and continue through Claude's normal permissions. Use the native results during startup or a timeout, and call `tokensave_search` when it becomes available. Set `TOKENSAVE_CLAUDE_STRICT_GREP=1` in Claude Code's launch environment to restore redirects for eligible native Grep calls. Bash advice is unaffected.

**Grep redirects.** A denial suggests `tokensave_search` for indexed definitions. For text matches such as references and comments, keep the original pattern, paths, and filters. Run a supported shell search through [`tokensave grep`](CLI-REFERENCE.md#index-facts-on-search-output) to add index facts, or use the bypass below to run the original search.

**Bash text searches.** The Bash hook can suggest a scoped `tokensave_search` lookup for definitions when a simple `grep`, `rg`, or `ag` command names one indexed symbol and one file or directory. Scope is expressed using the MCP tool's path substring filter. The original Bash command runs with Claude Code's usual permissions and supplies the exact matches, line numbers, and shell output. When the MCP tool is missing, disconnected, or returns stale or incomplete results, use the Bash results directly. Explicit fixed-string searches, structural patterns, and compound Bash commands run without a symbol suggestion.

Searches within a single JavaScript or TypeScript file can include a `const`, `let`, `var`, or `function` declaration prefix when the name is indexed.

To suppress the suggestion for one invocation, prefix the search in the Bash tool's command field:

```bash
TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n 'fn verify' src/auth.rs
```

The hook reads the inline assignment from the command before the shell runs. For a session-wide opt-out, set the variable in the environment that launches Claude Code.

Grep patterns containing a pipe pass through, including extended regex alternation (`|`) and basic regex alternation (`\|`). This preserves matches in comments, strings, and other source text even when an alternative matches an indexed symbol. Other unrecognized regex patterns, searches outside the index, searches over non-code extensions, and `find` predicates such as `-exec`, `-delete`, and `-mtime` also pass through.

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

`tokensave init` searches these places too. When it finds a Caveman skill, it names the agents and, if the setting is off, shows how to turn it on. It never turns it on itself, and it says nothing when no skill is found and the setting is off. Auto-initialization by hooks or `serve` does not search.

Check what tokensave sees with:

```bash
tokensave doctor --caveman
tokensave doctor --caveman --agent claude --session-id <claude-session-id>
```

This runs the normal `doctor` checks and adds a **Caveman compatibility** section. It also prints a JSON report per agent on stdout, so `tokensave doctor --caveman 2>/dev/null` gives only the JSON. The Caveman check reads files and changes nothing. A report separates installation (`absent`, `installed`, `partial` or `unsupported`) from activation in a session (`active`, `inactive` or `unknown`). A skill on disk only means that Caveman is available. Activation is reported only for a Claude Code session named with `--session-id`, and only when the plugin manifest names a Caveman release tokensave was checked against (4.0.1 and 4.1.0). tokensave then reads that session's `.caveman-sessions/<id>.mode` file. Caveman's machine-wide `.caveman-active` flag and its `defaultMode` setting never count as activation.

Caveman's optional proxy or input compression can rewrite tool results after tokensave has returned them, and tokensave cannot prevent that. If you need exact results in a run, turn off lossy input compression with Caveman's own controls. `doctor --caveman` says so whenever a Caveman skill is found. The design notes and findings are in [Caveman compatibility](../CAVEMAN-COMPATIBILITY.md).

## Using Auto Ontology with an agent

The optional launcher in a TokenSave source checkout starts one interactive
Claude Code or Codex CLI session with both MCP servers. It uses local TokenSave stdio for
symbols and source. Auto Ontology runs separately and serves the catalog over
HTTP, with the selected client handling OAuth. Install Python 3 and put
`tokensave` and either `claude` or `codex` on `PATH`. Run this from your
indexed project.

```sh
python3 /path/to/tokensave/scripts/auto-ontology/compose.py \
  --url https://catalog.example.com/mcp
```

Supply the Auto Ontology MCP endpoint, including `/mcp`. Use HTTPS for a remote
server or HTTP for a loopback test server. The launcher prefers Claude Code
when both clients are present. Pass `--agent codex` or `--agent claude` to select
a client explicitly.

- In Claude Code, open `/mcp` and sign in under `auto-ontology`.
- For Codex CLI, run the same command with `--agent codex --login` once to sign
  in, then start the session with `--agent codex`.

For a mixed question, ask the agent to locate the
implementation and explain the corresponding business term. The launcher
provides routing instructions and configures the client to ask before
executing Auto Ontology tools. Review the arguments and approve any transfer
of local source or metadata explicitly. Your agent's model provider and
organization-managed settings still apply.

The launcher's MCP settings apply to this session. It invokes TokenSave in the
project you started from and passes the two server definitions directly to the
client. See the [interoperability evaluation](../AUTO-ONTOLOGY-EVALUATION.md)
for deployment requirements and the offline OKF workflow.
