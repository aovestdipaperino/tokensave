<p align="center">
  <img src="src/resources/logo.png" alt="TokenSave" width="300">
</p>

<h3 align="center">Semantic Code Intelligence for AI Coding Agents</h3>

<p align="center"><strong>Fewer tokens &bull; Fewer tool calls &bull; 100% local</strong></p>

<p align="center">
  <a href="https://crates.io/crates/tokensave"><img src="https://img.shields.io/crates/v/tokensave.svg" alt="crates.io"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.70+-orange.svg" alt="Rust"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/macOS-supported-blue.svg" alt="macOS">
  <img src="https://img.shields.io/badge/Linux-supported-blue.svg" alt="Linux">
  <img src="https://img.shields.io/badge/Windows-supported-blue.svg" alt="Windows">
  <a href="https://hypercommit.com/tokensave"><img src="https://img.shields.io/badge/Hypercommit-DB2475" alt="Hypercommit"></a>
</p>

---

## Why tokensave?

AI coding agents waste tokens exploring codebases. Every grep, glob, and file read costs money. On complex tasks, agents spawn multiple Explore sub-agents that scan hundreds of files just to build context.

**tokensave gives agents a pre-indexed semantic knowledge graph.** Instead of scanning files, the agent queries the graph and gets instant, structured answers -- the right symbols, their relationships, and source code, in one call.

### How It Works

```
┌──────────────────────────────────────────────────────────────┐
│  AI Coding Agent (Claude Code, Codex, Gemini, Cursor, ...)   │
│                                                              │
│  "Implement user authentication"                             │
│        │                                                     │
│        ▼                                                     │
│  ┌─────────────────┐       ┌─────────────────┐               │
│  │  Sub-agent      │ ───── │  Sub-agent      │               │
│  └────────┬────────┘       └─────────┬───────┘               │
└───────────┼──────────────────────────┼───────────────────────┘
            │                          │
            ▼                          ▼
┌──────────────────────────────────────────────────────────────┐
│  tokensave MCP Server                                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │
│  │   Search    │  │   Callers   │  │   Context   │           │
│  │   "auth"    │  │  "login()"  │  │   for task  │           │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘           │
│         └────────────────┼────────────────┘                  │
│                          ▼                                   │
│              ┌───────────────────────┐                       │
│              │   libSQL Graph DB     │                       │
│              │   • Instant lookups   │                       │
│              │   • FTS5 search       │                       │
│              └───────────────────────┘                       │
└──────────────────────────────────────────────────────────────┘
```

**Without tokensave:** Agents use `grep`, `glob`, and `Read` to scan files -- many API calls, high token usage.

**With tokensave:** Agents query the graph via MCP tools -- instant results, local processing, fewer tokens.

---

## Key Features

| | | |
|---|---|---|
| **Smart Context Building** | **Semantic Search** | **Impact Analysis** |
| One tool call returns everything the agent needs -- entry points, related symbols, and code snippets. | Find code by meaning, not just text. Search for "authentication" and find `login`, `validateToken`, `AuthService`. | Know exactly what breaks before you change it. Trace callers, callees, and the full impact radius of any symbol. |
| **37 MCP Tools** | **31 Languages** | **9 Agent Integrations** |
| From call graph traversal to dead code detection, test mapping, rename preview, and complexity analysis. | Rust, Go, Java, Python, TypeScript, C, C++, Swift, and 22 more. Three tiers (lite/medium/full) control binary size. | Claude Code, Codex CLI, Gemini CLI, Cursor, OpenCode, Copilot, Cline, Roo Code, Zed. |
| **Multi-Branch Indexing (opt-in)** | **100% Local** | **Always Fresh** |
| Optional per-branch databases. Cross-branch diff and search without switching your checkout. | No data leaves your machine. No API keys. No external services. Everything runs on a local libSQL database. | Background daemon syncs the index automatically. Survives reboots. Restarts after upgrades. |

---

## Quick Start

### 1. Install

**Homebrew (macOS):**

```bash
brew install aovestdipaperino/tap/tokensave
```

**Scoop (Windows):**

```powershell
scoop bucket add tokensave https://github.com/aovestdipaperino/scoop-tokensave
scoop install tokensave
```

**Cargo (any platform):**

```bash
cargo install tokensave                          # full (31 languages, default)
cargo install tokensave --features medium        # medium (20 languages)
cargo install tokensave --no-default-features    # lite (11 languages, smallest binary)
```

**Prebuilt binaries (Linux, Windows, macOS):**

Download from the [latest release](https://github.com/aovestdipaperino/tokensave/releases/latest) and place the binary in your `PATH`.

| Platform | Archive |
|---|---|
| macOS (Apple Silicon) | `tokensave-vX.Y.Z-aarch64-macos.tar.gz` |
| Linux (x86_64) | `tokensave-vX.Y.Z-x86_64-linux.tar.gz` |
| Linux (ARM64) | `tokensave-vX.Y.Z-aarch64-linux.tar.gz` |
| Windows (x86_64) | `tokensave-vX.Y.Z-x86_64-windows.zip` |

### 2. Configure your agent

```bash
tokensave install                    # auto-detects installed agents
tokensave install --agent claude     # Claude Code (explicit)
tokensave install --agent codex      # OpenAI Codex CLI
tokensave install --agent gemini     # Gemini CLI
tokensave install --agent opencode   # OpenCode
tokensave install --agent cursor     # Cursor
tokensave install --agent copilot    # GitHub Copilot
tokensave install --agent cline      # Cline
tokensave install --agent roo-code   # Roo Code
tokensave install --agent zed        # Zed
```

Each agent gets its MCP server registered in the native config format. Claude Code additionally gets a PreToolUse hook (blocks wasteful Explore agents), a UserPromptSubmit hook, a Stop hook, prompt rules in CLAUDE.md, and auto-allowed tool permissions.

All changes are idempotent -- safe to run again after upgrading. After agent setup, you'll be offered a global git post-commit hook and the background daemon service.

### 3. Index your project

```bash
cd /path/to/your/project
tokensave sync
```

This creates a `.tokensave/` directory with the knowledge graph database. Subsequent runs are incremental -- only changed files are re-indexed.

<details>
<summary><strong>What install writes for Claude Code</strong></summary>

#### MCP server

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

#### PreToolUse hook

The hook runs `tokensave hook-pre-tool-use` -- a native Rust command (no bash or jq required). It intercepts Agent tool calls and blocks Explore agents, redirecting Claude to use tokensave MCP tools instead.

#### CLAUDE.md rules

Appends instructions to `~/.claude/CLAUDE.md` that tell Claude to use tokensave tools before reaching for Explore agents or raw file reads.

</details>

---

## Multi-Branch Indexing (Optional)

tokensave can optionally maintain a separate code graph per git branch. When enabled, switching branches never gives you stale results and never re-indexes files you already parsed on another branch. Multi-branch tracking is opt-in -- without it, tokensave uses a single database for all branches.

### How it works

When you track a branch, tokensave copies the nearest ancestor DB and syncs only the files that differ. This means tracking a feature branch off `main` is nearly instant -- it only parses the files you've changed.

### CLI commands

```bash
tokensave branch add              # track the current branch
tokensave branch list             # see tracked branches and DB sizes
tokensave branch remove <name>    # stop tracking a branch
tokensave branch removeall        # remove all tracked branches except default
tokensave branch gc               # clean up branches deleted from git
```

### Cross-branch MCP tools

Three MCP tools enable cross-branch queries without switching your checkout:

- **`tokensave_branch_search`** -- search symbols in another branch's graph
- **`tokensave_branch_diff`** -- compare code graphs between two branches: symbols added, removed, and changed (signature differs). Supports file and kind filters.
- **`tokensave_branch_list`** -- list tracked branches with DB sizes, parent branch, and sync times

### Branch fallback

When the MCP server can't find a database for the current branch, it serves from the nearest ancestor branch's DB and includes a warning in every tool response suggesting you run `tokensave branch add`.

See [docs/BRANCHING-USER-GUIDE.md](docs/BRANCHING-USER-GUIDE.md) for the full guide.

---

## 37 MCP Tools

Every tool is read-only, safe to call in parallel, and annotated with `readOnlyHint`. The three core tools (`tokensave_context`, `tokensave_search`, `tokensave_status`) are marked `anthropic/alwaysLoad` so they bypass the client's tool-search round-trip.

### Discovery

| Tool | Purpose |
|------|---------|
| `tokensave_context` | Get relevant code context for a task -- entry points, related symbols, code snippets |
| `tokensave_search` | Find symbols by name (functions, classes, types) |
| `tokensave_node` | Get details + source code for a specific symbol |
| `tokensave_files` | List indexed project files with filtering |
| `tokensave_module_api` | Public API surface of a file or directory |
| `tokensave_similar` | Find symbols with similar names |
| `tokensave_status` | Index status, statistics, tokens saved |

### Call Graph & Impact

| Tool | Purpose |
|------|---------|
| `tokensave_callers` | Find what calls a function |
| `tokensave_callees` | Find what a function calls |
| `tokensave_impact` | See what's affected by changing a symbol |
| `tokensave_affected` | Find test files affected by source changes |
| `tokensave_rename_preview` | All references to a symbol (preview rename impact) |
| `tokensave_hotspots` | Most connected symbols (highest call count) |

### Code Quality

| Tool | Purpose |
|------|---------|
| `tokensave_complexity` | Rank functions by cyclomatic complexity, nesting depth, safety metrics |
| `tokensave_dead_code` | Find unreachable symbols (no incoming edges) |
| `tokensave_god_class` | Find classes with too many members |
| `tokensave_coupling` | Rank files by fan-in/fan-out |
| `tokensave_inheritance_depth` | Find the deepest inheritance hierarchies |
| `tokensave_circular` | Detect circular file dependencies |
| `tokensave_recursion` | Detect recursive/mutually-recursive call cycles |
| `tokensave_unused_imports` | Import statements never referenced |
| `tokensave_doc_coverage` | Public symbols missing documentation |
| `tokensave_simplify_scan` | Quality analysis of changed files (duplications, dead code, complexity) |

### Git & Workflow

| Tool | Purpose |
|------|---------|
| `tokensave_diff_context` | Semantic context for changed files -- modified symbols, dependencies, affected tests |
| `tokensave_commit_context` | Semantic summary of uncommitted changes for commit message drafting |
| `tokensave_pr_context` | Semantic diff between git refs for pull request descriptions |
| `tokensave_changelog` | Semantic diff between two git refs |
| `tokensave_test_map` | Source-to-test mapping at the symbol level, with uncovered symbol detection |

### Type System

| Tool | Purpose |
|------|---------|
| `tokensave_type_hierarchy` | Recursive type hierarchy tree for traits, interfaces, and classes |
| `tokensave_rank` | Rank nodes by relationship count (most implemented interface, most extended class) |
| `tokensave_distribution` | Node kind breakdown per file or directory |
| `tokensave_largest` | Rank nodes by size -- largest classes, longest methods |

### Porting

| Tool | Purpose |
|------|---------|
| `tokensave_port_status` | Compare symbols between source/target directories to track porting progress |
| `tokensave_port_order` | Topological sort of symbols for porting -- port leaves first, then dependents |

### Multi-Branch

| Tool | Purpose |
|------|---------|
| `tokensave_branch_search` | Search symbols in another branch's graph |
| `tokensave_branch_diff` | Compare symbols between branches (added/removed/changed) |
| `tokensave_branch_list` | List tracked branches with DB sizes and sync times |

### MCP Resources

Four resources are exposed via `resources/list` and `resources/read`:

- `tokensave://status` -- graph statistics as JSON
- `tokensave://files` -- indexed file tree grouped by directory
- `tokensave://overview` -- project summary with language distribution and symbol kinds
- `tokensave://branches` -- tracked branches with DB sizes and parent info

---

## Token Tracking

tokensave measures the tokens it saves on every MCP tool call. Each tool response includes a `tokensave_metrics: before=N after=M` line showing how many raw-file tokens were avoided by that specific call.

### Live monitor

```bash
tokensave monitor
```

A global TUI that shows MCP tool calls from all projects in real time, via a shared memory-mapped ring buffer at `~/.tokensave/monitor.mmap`. Each entry shows the project name, tool name, and token delta.

### Session and lifetime counters

```bash
tokensave current-counter          # show per-project session counter
tokensave reset-counter            # reset the session counter
tokensave status                   # shows project + global lifetime totals
```

### Worldwide counter

All tokensave users contribute to an anonymous aggregate counter. `tokensave status` shows both your project total and the worldwide total. The upload sends only a single number (e.g. `4823`) with no identifying information. Opt out with `tokensave disable-upload-counter`.

---

## Background Daemon

The daemon watches all tracked projects for file changes and runs incremental syncs automatically.

```bash
tokensave daemon                       # start in foreground
tokensave daemon --enable-autostart    # install as launchd/systemd/Windows Service
tokensave daemon --disable-autostart   # remove autostart service
tokensave daemon --status              # show daemon status
```

The daemon is upgrade-aware: it snapshots its own binary's mtime and size at startup and checks every 60 seconds. When a package manager replaces the binary (`brew upgrade`, `cargo install`, `scoop update`), the daemon flushes pending syncs and exits. The service manager (launchd `KeepAlive`, systemd `Restart=on-failure`, Windows SCM failure actions) automatically relaunches with the new version.

---

## Graph Visualizer

```bash
tokensave visualize
```

Opens an interactive, browser-based code graph visualization powered by Cytoscape.js. Right-click any node to explore callers, callees, the call graph, or impact analysis.

---

## Self-Upgrade

```bash
tokensave upgrade                  # upgrade to latest in current channel
tokensave channel                  # show current channel (stable/beta)
tokensave channel beta             # switch to beta channel
tokensave channel stable           # switch back to stable
```

`tokensave upgrade` downloads the correct platform binary from GitHub releases, stops the daemon, replaces the binary, and restarts the daemon. Supports stable and beta channels independently.

---

## CLI Reference

```bash
tokensave sync [path]              # Sync (creates index if missing, incremental by default)
tokensave sync --force [path]      # Force a full re-index
tokensave sync --doctor [path]     # Sync and list added/modified/removed files
tokensave status [path]            # Show statistics
tokensave status [path] --json     # Show statistics (JSON output)
tokensave query <search> [path]    # Search symbols
tokensave files [--filter dir] [--pattern glob] [--json]   # List indexed files
tokensave affected <files...> [--stdin] [--depth N]        # Find affected test files
tokensave install [--agent NAME]   # Configure agent integration + daemon offer
tokensave uninstall [--agent NAME] # Remove agent integration
tokensave serve                    # Start MCP server
tokensave visualize                # Open graph visualizer
tokensave monitor                  # Live TUI showing MCP calls across all projects
tokensave upgrade                  # Self-update to latest version
tokensave channel [stable|beta]    # Show or switch update channel
tokensave doctor [--agent NAME]    # Check installation health
tokensave branch add|list|remove|removeall|gc   # Multi-branch management
tokensave daemon [--enable-autostart|--disable-autostart|--status]
tokensave current-counter          # Show per-project token counter
tokensave reset-counter            # Reset per-project token counter
tokensave disable-upload-counter   # Opt out of worldwide counter uploads
tokensave enable-upload-counter    # Re-enable worldwide counter uploads
```

---

## `tokensave doctor`

Run a comprehensive health check of your tokensave installation:

```bash
tokensave doctor
```

Checks: binary location, project index, global DB, user config, daemon status, agent integration (MCP server, hooks, permissions, prompt rules), and network connectivity. If any tool permissions are missing after an upgrade, it tells you to run `tokensave install`. Use `--agent` to check a specific agent only.

Doctor also validates that each installed hook uses the correct tokensave subcommand and auto-repairs broken hooks.

---

## How It Works with Claude Code

Once configured, Claude Code automatically uses tokensave instead of reading raw files when it needs to understand your codebase. Three layers reinforce each other:

| Layer | What it does | Why it matters |
|-------|-------------|----------------|
| **MCP server** | Exposes 37 `tokensave_*` tools to Claude | Claude can query the graph directly |
| **CLAUDE.md rules** | Tells Claude to prefer tokensave over agents/file reads | Prevents the model from falling back to expensive patterns |
| **PreToolUse hook** | Native Rust hook blocks Explore agents | Catches cases where the model ignores the CLAUDE.md rules |
| **UserPromptSubmit hook** | Runs at prompt submission | Lifecycle tracking for token accounting |
| **Stop hook** | Runs when the session ends | Flushes token counters |

The result: Claude gets the same code understanding with far fewer tokens. A typical Explore agent reads 20-50 files; tokensave returns the relevant symbols, relationships, and code snippets from its pre-built index.

---

## Network Calls & Privacy

tokensave's core functionality (indexing, search, graph queries, MCP server) is **100% local** -- your code never leaves your machine.

| Call | Data sent | When | Opt-out |
|------|-----------|------|---------|
| Worldwide counter upload | Token count (a number) + country (from IP) | sync, status, MCP sessions | `tokensave disable-upload-counter` |
| Worldwide counter read | Nothing (GET request) | status | N/A (read-only, 1s timeout) |
| Version check | Nothing (GET request) | status (cached 5m), sync (parallel) | N/A (1s timeout, no-op on failure) |

The worldwide counter upload sends a single HTTP POST with a JSON body like `{"amount": 4823}`. No cookies, no tracking, no user ID. The Cloudflare Worker logs the country of your IP address (derived from request headers) for aggregate geographic statistics -- your actual IP address is not stored.

---

## 31 Languages

tokensave supports 31 programming languages organized into three tiers controlled by Cargo feature flags. Each tier includes all languages from the tier below it.

### Lite (11 languages) -- `--no-default-features`

Always compiled. The smallest binary for the most popular languages.

| Language | Extensions |
|----------|-----------|
| Rust | `.rs` |
| Go | `.go` |
| Java | `.java` |
| Scala | `.scala`, `.sc` |
| TypeScript | `.ts`, `.tsx` |
| JavaScript | `.js`, `.jsx` |
| Python | `.py` |
| C | `.c`, `.h` |
| C++ | `.cpp`, `.hpp`, `.cc`, `.cxx`, `.hh` |
| Kotlin | `.kt`, `.kts` |
| C# | `.cs` |
| Swift | `.swift` |

### Medium (Lite + 9 = 20 languages) -- `--features medium`

| Language | Extensions | Feature flag |
|----------|-----------|-------------|
| Dart | `.dart` | `lang-dart` |
| Pascal | `.pas`, `.pp`, `.dpr` | `lang-pascal` |
| PHP | `.php` | `lang-php` |
| Ruby | `.rb` | `lang-ruby` |
| Bash | `.sh`, `.bash` | `lang-bash` |
| Protobuf | `.proto` | `lang-protobuf` |
| PowerShell | `.ps1`, `.psm1` | `lang-powershell` |
| Nix | `.nix` | `lang-nix` |
| VB.NET | `.vb` | `lang-vbnet` |

### Full (Medium + 11 = 31 languages) -- default

| Language | Extensions | Feature flag |
|----------|-----------|-------------|
| Lua | `.lua` | `lang-lua` |
| Zig | `.zig` | `lang-zig` |
| Objective-C | `.m`, `.mm` | `lang-objc` |
| Perl | `.pl`, `.pm` | `lang-perl` |
| Batch/CMD | `.bat`, `.cmd` | `lang-batch` |
| Fortran | `.f90`, `.f95`, `.f03`, `.f08`, `.f18`, `.f`, `.for` | `lang-fortran` |
| COBOL | `.cob`, `.cbl`, `.cpy` | `lang-cobol` |
| MS BASIC 2.0 | `.bas` | `lang-msbasic2` |
| GW-BASIC | `.gw` | `lang-gwbasic` |
| QBasic | `.qb` | `lang-qbasic` |
| QuickBASIC 4.5 | `.bi`, `.bm` | `lang-qbasic` |

Individual languages can also be cherry-picked without a full tier:

```bash
cargo install tokensave --no-default-features --features lang-nix,lang-bash
```

All extractors share the same depth: functions, classes, methods, fields, imports, call graphs, inheritance chains, docstrings, complexity metrics, decorator/annotation extraction, and cross-file dependency tracking.

---

## tokensave vs CodeGraph

tokensave is a ground-up Rust rewrite of [CodeGraph](https://www.npmjs.com/package/@colbymchenry/codegraph) (Node.js/TypeScript). Both build semantic code graphs for AI coding agents, but they diverge significantly in scope and capabilities.

| | **tokensave** | **CodeGraph** |
|---|---|---|
| **Runtime** | Native binary (Rust) | Node.js 18+ |
| **Install** | `brew install`, `cargo install`, `scoop install` | `npx @colbymchenry/codegraph` |
| **Languages** | 31 (3 tiers: lite/medium/full) | 19+ |
| **MCP tools** | 37 | 9 |
| **Agent integrations** | 9 (Claude, Codex, Gemini, OpenCode, Cursor, Cline, Copilot, Roo Code, Zed) | 1 (Claude Code) |
| **Background daemon** | Yes (launchd/systemd/Windows Service) | No (hook-based sync only) |
| **Multi-branch indexing** | Yes, opt-in (per-branch DBs, cross-branch diff/search) | No |
| **Complexity metrics** | AST-extracted (branches, loops, nesting depth, cyclomatic) | No |
| **Porting tools** | Yes (`port_status`, `port_order`) | No |
| **Graph visualizer** | Yes (interactive browser-based) | Yes |
| **Semantic search** | Agent-driven keyword expansion (zero-cost) | Local embeddings (nomic-embed-text-v1.5 via ONNX) |
| **MCP resources** | 4 (status, files, overview, branches) | No |
| **MCP annotations** | Yes (readOnlyHint, alwaysLoad) | No |
| **Dead code detection** | Yes | No |
| **Circular dependency detection** | Yes | No |
| **Type hierarchy** | Yes | No |
| **God class / coupling analysis** | Yes | No |
| **Commit / PR context** | Yes | No |
| **Test mapping** | Yes | No |
| **Rename preview** | Yes | No |
| **Token tracking** | Per-call metrics, live TUI monitor, session + lifetime counters | No |
| **Self-upgrade** | `tokensave upgrade` with stable/beta channels | `npm update` |
| **DB engine** | libsql (SQLite fork, WAL, async) | better-sqlite3 / wa-sqlite (WASM) |
| **Indexing speed** | ~1.2s for 1,782 files | ~4s for 1,782 files |
| **Binary size** | ~25 MB (all grammars bundled) | ~80 MB (node_modules + WASM) |

CodeGraph pioneered the approach and remains a solid choice if you prefer npm tooling and only need Claude Code integration. tokensave extends the concept with deeper analysis, more agents, background sync, multi-branch support, and a native binary with no runtime dependencies.

For detailed comparisons against CodeGraph, Dual-Graph (GrapeRoot), code-review-graph, and OpenWolf, see [docs/COMPARABLE-TOOLS.md](docs/COMPARABLE-TOOLS.md).

---

## Why tokensave Over the Alternatives

Several tools reduce token usage for AI coding agents. Here's why tokensave stands apart.

### Single native binary, zero dependencies

Every alternative requires a runtime: Python, Node.js, or both. tokensave ships as a single ~25 MB Rust binary with all 31 tree-sitter grammars bundled. Nothing else to install.

### Deepest code intelligence

tokensave works at the symbol level: functions, structs, fields, call edges, type hierarchies, complexity metrics. Alternatives like Dual-Graph (GrapeRoot) work at the file level -- they know which files exist but can't answer "who calls this function?" or "what breaks if I change this struct?" tokensave's 37 specialized MCP tools cover call graph traversal, impact analysis, dead code detection, test mapping, rename preview, type hierarchies, circular dependency detection, complexity ranking, and more. The closest competitor (code-review-graph) has 22 tools; others have 5-9.

### Broadest agent support

9 AI coding agent integrations with per-agent native configuration formats. No other tool covers as many agents with as deep an integration. Claude Code gets hooks, prompt rules, and auto-allowed tool permissions. Other agents get MCP server registration in their native config format.

### Multi-branch indexing

The only tool in this space with optional per-branch graph databases and cross-branch diff and search. When enabled, switching branches is instant -- no re-indexing required.

### Per-call token tracking

The only tool that reports exactly how many tokens each individual MCP tool call saved, plus a live TUI monitor across all projects and lifetime counters.

### Fully open source

MIT-licensed Rust, auditable end to end. Dual-Graph's core engine (`graperoot` on PyPI) is proprietary -- you can't see what it does with your code graph. OpenWolf is AGPL-3.0, which requires derivative works to be open-sourced.

### Background daemon with upgrade awareness

The only tool that runs as a system service (launchd/systemd/Windows SCM), persists across sessions and reboots, and automatically restarts when the binary is upgraded by any package manager.

### Performance

Full-index benchmark on a 1,782-file mixed Rust/Java/Scala codebase (57K nodes, 103K edges):

| Tool | Time | Speedup |
|---|---|---|
| CodeGraph (TypeScript) | 31.2s | 1x |
| **tokensave (Rust)** | **1.2s** | **26x** |

---

## Troubleshooting

### "tokensave not initialized"

The `.tokensave/` directory doesn't exist in your project.

```bash
tokensave sync
```

### MCP server not connecting

The AI agent doesn't see tokensave tools.

1. Ensure the agent config includes the tokensave MCP server (run `tokensave doctor`)
2. Restart the agent completely
3. Check that `tokensave` is in your PATH: `which tokensave`

### Missing symbols in search

- Run `tokensave sync` to update the index
- Check that the language is supported (see table above)
- Verify the file isn't excluded by `.gitignore`

### Indexing is slow

Large projects take longer on the first full index.

- Subsequent runs use incremental sync and are much faster
- Use `tokensave sync` (not `--force`) for day-to-day updates
- The background daemon handles sync automatically

---

## Origin

This project is a Rust port of the original [CodeGraph](https://github.com/colbymchenry/codegraph) TypeScript implementation by [@colbymchenry](https://github.com/colbymchenry). The port maintains the same architecture and MCP tool interface while leveraging Rust for performance and native tree-sitter bindings.

---

## Building

```bash
cargo build --release                          # full (31 languages, default)
cargo build --release --features medium        # medium (20 languages)
cargo build --release --no-default-features    # lite (11 languages)

cargo test                                     # run all tests (requires full)
cargo check --no-default-features              # verify lite compiles
cargo clippy --all
```

## Star History

<a href="https://www.star-history.com/#aovestdipaperino/tokensave&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=aovestdipaperino/tokensave&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=aovestdipaperino/tokensave&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=aovestdipaperino/tokensave&type=Date" />
 </picture>
</a>

## License

MIT License -- see [LICENSE](LICENSE) for details.

**[tokensave.dev](https://tokensave.dev)**
