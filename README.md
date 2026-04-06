<p align="center">
  <img src="src/resources/logo.png" alt="TokenSave" width="300">
</p>

<h3 align="center">Supercharge Claude Code with Semantic Code Intelligence</h3>

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

When Claude Code works on a complex task, it spawns **Explore agents** that scan your codebase using grep, glob, and file reads. Every tool call consumes tokens.

**tokensave gives Claude a pre-indexed semantic knowledge graph.** Instead of scanning files, Claude queries the graph instantly — fewer API calls, less token usage, same code understanding.

### How It Works

```
┌──────────────────────────────────────────────────────────────┐
│  Claude Code                                                 │
│                                                              │
│  "Implement user authentication"                             │
│        │                                                     │
│        ▼                                                     │
│  ┌─────────────────┐       ┌─────────────────┐               │
│  │  Explore Agent  │ ───── │  Explore Agent  │               │
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
│              │   • Vector embeddings │                       │
│              └───────────────────────┘                       │
└──────────────────────────────────────────────────────────────┘
```

**Without tokensave:** Explore agents use `grep`, `glob`, and `Read` to scan files — many API calls, high token usage.

**With tokensave:** Agents query the graph via MCP tools — instant results, local processing, fewer tokens.

---

## Key Features

| | | |
|---|---|---|
| **Smart Context Building** | **Semantic Search** | **Impact Analysis** |
| One tool call returns everything Claude needs — entry points, related symbols, and code snippets. | Find code by meaning, not just text. Search for "authentication" and find `login`, `validateToken`, `AuthService`. | Know exactly what breaks before you change it. Trace callers, callees, and the full impact radius of any symbol. |
| **31 Languages** | **100% Local** | **Always Fresh** |
| Rust, Go, Java, Python, TypeScript, C, C++, Swift, and 22 more — all with the same API. Three tiers (lite/medium/full) let you control binary size. | No data leaves your machine. No API keys. No external services. Everything runs on a local libSQL database. | Git hooks automatically sync the index as you work. Your code intelligence is always up to date. |

---

## What's New in v3.4

### Multi-Branch Indexing

tokensave can now keep a separate code graph per git branch, so switching branches never gives you stale results and never re-indexes files you already parsed on another branch. Multi-branch is opt-in:

```bash
tokensave branch add          # track the current branch
tokensave branch list         # see tracked branches and DB sizes
tokensave branch gc           # clean up branches deleted from git
```

Two new MCP tools enable cross-branch queries without switching your checkout:

- **`tokensave_branch_search`** — search symbols in another branch's graph
- **`tokensave_branch_diff`** — compare symbols between branches (added/removed/changed)

See [docs/BRANCHING-USER-GUIDE.md](docs/BRANCHING-USER-GUIDE.md) for the full guide.

### 37 MCP Tools

<details>
<summary>Previous: v3.3 tools</summary>

Five tools added in v3.3:

- **`tokensave_commit_context`** — semantic summary of uncommitted changes for drafting commit messages
- **`tokensave_pr_context`** — semantic diff between git refs for pull request descriptions
- **`tokensave_simplify_scan`** — quality analysis of changed files (duplications, dead code, complexity, coupling)
- **`tokensave_test_map`** — source-to-test mapping at the symbol level, with uncovered symbol detection
- **`tokensave_type_hierarchy`** — recursive type hierarchy tree for traits, interfaces, and classes

</details>

All 37 tools include MCP annotations (`readOnlyHint`, `title`) and the three core tools (`tokensave_context`, `tokensave_search`, `tokensave_status`) are marked `anthropic/alwaysLoad` so they bypass the client's tool-search round-trip.

### Upgrade-Aware Daemon

The background daemon now detects when its binary has been replaced (via `brew upgrade`, `cargo install`, `scoop update`, or any package manager) and automatically restarts with the new version. On Windows, the service is configured with SCM failure recovery actions for automatic restart.

### MCP Resources

Three resources are exposed via `resources/list` and `resources/read`: `tokensave://status`, `tokensave://files`, and `tokensave://overview`.

<details>
<summary>Previous releases</summary>

#### v3.2 — 13-26x Faster Indexing

Full index performance improved through rayon parallel extraction, prepared-statement DB writes, suffix-indexed reference resolution, and bulk-load mode. A 1,782-file codebase now indexes in 1.2s (down from 14.8s). A 28K-file monorepo indexes in 22s (down from 565s). New databases get the final schema in one shot instead of running migrations sequentially.

#### v3.1 — Edge Deduplication

Incremental syncs could accumulate duplicate edges over time. v3.1 adds a unique index on edges and deduplicates existing databases on upgrade. Also adds concurrent sync prevention via PID-based lockfile, and `tokensave doctor` now runs VACUUM.

#### v3.0 — Bundled Grammars & Daemon

All 31 language grammars ship as a single bundled dependency. `tokensave install` offers to set up the background daemon as an autostart service (launchd/systemd/Windows Service).

</details>

### 31 Languages

tokensave supports **31 programming languages** with deep extraction: functions, classes, methods, fields, imports, call graphs, inheritance chains, docstrings, complexity metrics, and cross-file dependency tracking.

### Feature Flag Tiers

Three compilation tiers control which extractors are compiled:

```
Full (default)  ── 31 languages
Medium          ── 20 languages
Lite            ── 11 languages
```

All grammars are always present via the bundled crate; the `lang-*` feature flags control which extractors are included.

See [Supported Languages](#supported-languages) for the full tier breakdown.

### Deep Nix Support

The Nix extractor goes beyond basic function/variable extraction:

- **Derivation field extraction** — `mkDerivation`, `mkShell`, `buildPythonPackage`, `buildGoModule`, and other builder calls have their attrset arguments extracted as Field nodes. `tokensave_search("my-app")` finds the `pname` field, and `tokensave_search("buildInputs")` shows what each derivation depends on.

- **Import path resolution** — `import ./path.nix` now creates a file-level dependency edge. `tokensave_callers` and `tokensave_impact` can trace which Nix files depend on which, enabling cross-file change impact analysis.

- **Flake output schema awareness** — In `flake.nix` files, standard output attributes (`packages`, `devShells`, `apps`, `nixosModules`, `checks`, `overlays`, `lib`, `formatter`) are recognized as semantic Module nodes. `tokensave_search("devShells")` finds your dev shell definition with its `buildInputs` as child Field nodes.

### Protobuf Schema Graphs

Protobuf files are now first-class citizens with dedicated node kinds:

- `message` definitions become `ProtoMessage` nodes with their fields as children
- `service` definitions become `ProtoService` nodes
- `rpc` methods become `ProtoRpc` nodes
- Nested messages, enums, `oneof` fields, and `import` statements are all tracked

This enables queries like "what services depend on this message type?" via `tokensave_callers`.

### Porting Assessment Tools

Two new MCP tools help assess and plan cross-language porting:

- **`tokensave_port_status`** — compare symbols between a source and target directory in the same project. Matches by name with cross-language kind compatibility (`class` ↔ `struct`, `interface` ↔ `trait`). Reports coverage percentage, unmatched symbols grouped by file, and target-only symbols.

- **`tokensave_port_order`** — topological sort of source symbols for porting. Returns symbols in dependency-leaf-first order: utility functions and constants at level 0, classes that use them at level 1, services that depend on those classes at level 2, etc. Detects and flags dependency cycles that should be ported together.

Both tools assume source and target live in the same indexed project (e.g., `src/python/` and `src/rust/` side by side).

### SQLite Fallback & Feedback Loop

Agent prompt rules now include two new instructions:

1. **SQLite fallback** — when MCP tools can't answer a code analysis question, the agent is instructed to query `.tokensave/tokensave.db` directly via SQL (tables: `nodes`, `edges`, `files`). This handles edge cases and complex structural queries that go beyond the built-in tools.

2. **Improvement feedback** — when the agent discovers a gap where an extractor, schema, or tool could be improved, it proposes to the user to open an issue at [github.com/aovestdipaperino/tokensave](https://github.com/aovestdipaperino/tokensave), reminding them to strip sensitive data from the description.

### Upgrading

```bash
cargo install tokensave          # or: brew upgrade tokensave / scoop update tokensave
tokensave install                # re-run for updated prompt rules + daemon offer
tokensave sync --force           # re-index to rebuild the graph (recommended after major upgrades)
```

---

## tokensave vs CodeGraph

tokensave is a ground-up Rust rewrite of [CodeGraph](https://www.npmjs.com/package/@colbymchenry/codegraph) (Node.js/TypeScript). Both build semantic code graphs for AI coding agents, but they diverge significantly in scope and capabilities.

| | **tokensave** | **CodeGraph** |
|---|---|---|
| **Runtime** | Native binary (Rust) | Node.js 18+ |
| **Install** | `brew install`, `cargo install`, `scoop install` | `npx @colbymchenry/codegraph` |
| **Languages** | 31 (3 tiers: lite/medium/full) | 17 |
| **MCP tools** | 37 | 9 |
| **Agent integrations** | 9 (Claude, Codex, Gemini, OpenCode, Cursor, Cline, Copilot, Roo Code, Zed) | 1 (Claude Code) |
| **Background daemon** | Yes (launchd/systemd/Windows Service) | No (hook-based sync only) |
| **Multi-branch indexing** | Yes (per-branch DBs, cross-branch diff/search) | No |
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
| **DB engine** | libsql (SQLite fork, WAL, async) | better-sqlite3 / wa-sqlite (WASM) |
| **Indexing speed** | ~1.2s for 1,782 files | ~4s for 1,782 files |
| **Binary size** | ~25 MB (all grammars bundled) | ~80 MB (node_modules + WASM) |

**Semantic search: two different approaches.** CodeGraph runs a local embedding model (nomic-embed-text-v1.5, 768-dim, ONNX) during indexing, storing a vector per symbol. Queries are embedded and matched by cosine similarity, so "authentication" finds `login()` even with zero lexical overlap. The cost is ~30s extra indexing per 1,000 nodes, a ~50MB model download, and ~200ms per query.

tokensave takes a different path: the `keywords` parameter on `tokensave_context` lets the calling agent provide synonyms directly. When you ask "how does authentication work?", the agent passes `keywords: ["login", "session", "credential", "token"]` and the context builder runs an FTS5 search for each keyword. This adds zero indexing cost, zero model dependency, and ~1ms per extra keyword. The trade-off is that the agent must know the right synonyms to provide — but since the agent is an LLM, it usually does. Where this falls short: if the agent doesn't know what naming conventions the codebase uses (e.g. `guardianGateway` for auth), it can't provide the right keywords. Embeddings would catch that case because they encode distributional semantics from training data, not just lexical forms.

CodeGraph pioneered the approach and remains a solid choice if you prefer npm tooling and only need Claude Code integration. tokensave extends the concept with deeper analysis, more agents, background sync, and multi-branch support.

---

## Quick Start

### 1. Install the binary

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

Download from the [latest release](https://github.com/aovestdipaperino/tokensave/releases/latest) and place the binary in your `PATH`:

| Platform | Archive |
|---|---|
| macOS (Apple Silicon) | `tokensave-vX.Y.Z-aarch64-macos.tar.gz` |
| Linux (x86_64) | `tokensave-vX.Y.Z-x86_64-linux.tar.gz` |
| Linux (ARM64) | `tokensave-vX.Y.Z-aarch64-linux.tar.gz` |
| Windows (x86_64) | `tokensave-vX.Y.Z-x86_64-windows.zip` |

```bash
# Example: Linux x86_64
curl -LO https://github.com/aovestdipaperino/tokensave/releases/latest/download/tokensave-v3.3.2-x86_64-linux.tar.gz
tar xzf tokensave-v3.3.2-x86_64-linux.tar.gz
sudo mv tokensave /usr/local/bin/
```

### 2. Configure your agent

Run the built-in installer — no scripts, no `jq`, works on macOS/Linux/Windows:

```bash
tokensave install                    # defaults to Claude Code
tokensave install --agent claude     # Claude Code (explicit)
tokensave install --agent opencode   # OpenCode
tokensave install --agent codex      # OpenAI Codex CLI
```

What each agent gets:

| | Claude Code | OpenCode | Codex CLI |
|---|---|---|---|
| MCP server registration | `~/.claude.json` | `.opencode.json` | `~/.codex/config.toml` |
| Tool permissions | Auto-allowed in `settings.json` | Runtime approval (interactive) | Auto-approved per tool |
| Hook (blocks Explore agents) | PreToolUse hook | N/A | N/A |
| Prompt rules | `~/.claude/CLAUDE.md` | `OPENCODE.md` | `~/.codex/AGENTS.md` |

All changes are idempotent — safe to run again after upgrading. After agent setup, you'll be offered a global git post-commit hook and the background daemon service. The old `tokensave claude-install` command still works as an alias.

### 3. Index your project

```bash
cd /path/to/your/project
tokensave sync
```

This creates a `.tokensave/` directory with the knowledge graph database. Subsequent runs are incremental — only changed files are re-indexed.

<details>
<summary><strong>What install writes to settings.json (Claude Code)</strong></summary>

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

#### Tool permissions

```json
{
  "permissions": {
    "allow": [
      "mcp__tokensave__tokensave_affected",
      "mcp__tokensave__tokensave_callees",
      "mcp__tokensave__tokensave_callers",
      "mcp__tokensave__tokensave_changelog",
      "mcp__tokensave__tokensave_circular",
      "mcp__tokensave__tokensave_commit_context",
      "mcp__tokensave__tokensave_complexity",
      "mcp__tokensave__tokensave_context",
      "mcp__tokensave__tokensave_coupling",
      "mcp__tokensave__tokensave_dead_code",
      "mcp__tokensave__tokensave_diff_context",
      "mcp__tokensave__tokensave_distribution",
      "mcp__tokensave__tokensave_doc_coverage",
      "mcp__tokensave__tokensave_files",
      "mcp__tokensave__tokensave_god_class",
      "mcp__tokensave__tokensave_hotspots",
      "mcp__tokensave__tokensave_impact",
      "mcp__tokensave__tokensave_inheritance_depth",
      "mcp__tokensave__tokensave_largest",
      "mcp__tokensave__tokensave_module_api",
      "mcp__tokensave__tokensave_node",
      "mcp__tokensave__tokensave_port_order",
      "mcp__tokensave__tokensave_port_status",
      "mcp__tokensave__tokensave_pr_context",
      "mcp__tokensave__tokensave_rank",
      "mcp__tokensave__tokensave_recursion",
      "mcp__tokensave__tokensave_rename_preview",
      "mcp__tokensave__tokensave_search",
      "mcp__tokensave__tokensave_similar",
      "mcp__tokensave__tokensave_simplify_scan",
      "mcp__tokensave__tokensave_status",
      "mcp__tokensave__tokensave_test_map",
      "mcp__tokensave__tokensave_type_hierarchy",
      "mcp__tokensave__tokensave_unused_imports"
    ]
  }
}
```

#### PreToolUse hook

The hook runs `tokensave hook-pre-tool-use` — a native Rust command (no bash or jq required). It intercepts Agent tool calls and blocks Explore agents and exploration-style prompts, redirecting Claude to use tokensave MCP tools instead.

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Agent",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/tokensave hook-pre-tool-use"
          }
        ]
      }
    ]
  }
}
```

#### CLAUDE.md rules

Appends instructions to `~/.claude/CLAUDE.md` that tell Claude to use tokensave tools before reaching for Explore agents or raw file reads.

</details>

---

## CLI Usage

```bash
tokensave sync [path]            # Sync (creates index if missing, incremental by default)
tokensave sync --force [path]    # Force a full re-index
tokensave status [path]          # Show statistics
tokensave status [path] --json   # Show statistics (JSON output)
tokensave query <search> [path]  # Search symbols
tokensave files [--filter dir] [--pattern glob] [--json]   # List indexed files
tokensave affected <files...> [--stdin] [--depth N]        # Find affected test files
tokensave install [--agent NAME] # Configure agent integration + daemon offer
tokensave uninstall [--agent NAME] # Remove agent integration (default: claude)
tokensave serve                  # Start MCP server
tokensave daemon                 # Start background file watcher
tokensave daemon --enable-autostart  # Install as launchd/systemd service
tokensave daemon --disable-autostart # Remove autostart service
tokensave daemon --status        # Show daemon status
tokensave disable-upload-counter # Opt out of worldwide counter uploads
tokensave enable-upload-counter  # Re-enable worldwide counter uploads
tokensave doctor [--agent NAME]  # Check installation health (default: all agents)
```

### `tokensave files`

List all indexed files, optionally filtering by directory or glob pattern.

```bash
tokensave files                           # List all indexed files
tokensave files --filter src/mcp          # Only files under src/mcp/
tokensave files --pattern "**/*.rs"       # Only Rust files
tokensave files --json                    # Machine-readable output
```

### `tokensave affected`

Find test files affected by source file changes. Uses BFS through the file dependency graph to discover impacted tests. Pipe from `git diff` for CI integration.

```bash
tokensave affected src/main.rs src/db/connection.rs   # Explicit file list
git diff --name-only HEAD~1 | tokensave affected --stdin   # From git diff
tokensave affected --stdin --depth 3 --json < changed.txt  # Custom depth, JSON output
tokensave affected src/lib.rs --filter "*_test.rs"    # Custom test pattern
```

### Auto-sync on commit (optional)

Keep the index up to date automatically by running `tokensave sync` after every successful git commit. The repo includes a `post-commit` hook that does this in the background.

**Global (all repos):**

```bash
# Set a global hooks directory (skip if you already have one)
git config --global core.hooksPath ~/.git-hooks
mkdir -p ~/.git-hooks

# Install the hook
cp scripts/post-commit ~/.git-hooks/post-commit
chmod +x ~/.git-hooks/post-commit
```

**Per-repo:**

```bash
cp scripts/post-commit .git/hooks/post-commit
chmod +x .git/hooks/post-commit
```

The hook checks for both the `tokensave` binary and a `.tokensave/` directory before running, so it is a no-op in repos that haven't been indexed.

---

## Network Calls & Privacy

tokensave's core functionality (indexing, search, graph queries, MCP server) is **100% local** — your code never leaves your machine. However, tokensave makes two optional network calls:

### 1. Worldwide token counter

tokensave tracks how many tokens it saves you across all your projects. On `sync` and `status` commands, it uploads the **count of tokens saved** (a single number) to an anonymous worldwide counter. No code, no file names, no project names, no identifying information is sent — just a number like `4823`.

This powers the "Worldwide ~1.0M" counter shown in `tokensave status`, which displays the total tokens saved by all tokensave users combined.

**What is sent:** A single HTTP POST to `https://tokensave-counter.enzinol.workers.dev/increment` with a JSON body like `{"amount": 4823}`. No cookies, no tracking, no user ID. The Cloudflare Worker also logs the **country of your IP address** (derived by Cloudflare from the request headers) for aggregate geographic statistics — your actual IP address is not stored.

**When it's sent:** After `sync` or `status` (always), and during MCP sessions every 30 seconds while tools are being called. Failed uploads are silently retried on the next opportunity.

**How to opt out:**

```bash
tokensave disable-upload-counter
```

This sets `upload_enabled = false` in `~/.tokensave/config.toml`. When disabled, tokensave **never uploads** your token count but **still fetches and displays** the worldwide total in status. You can re-enable at any time:

```bash
tokensave enable-upload-counter
```

You can also manually edit the config file at `~/.tokensave/config.toml` — it's plain TOML and fully transparent:

```toml
upload_enabled = true       # set to false to stop uploading
pending_upload = 4823       # tokens waiting to be uploaded
last_upload_at = 1711375200 # last successful upload timestamp
last_worldwide_total = 1000000
last_worldwide_fetch_at = 1711375200
last_flush_attempt_at = 1711375200
cached_latest_version = "3.3.2"
last_version_check_at = 1711375200
```

### 2. Version check

tokensave checks for new releases on GitHub so it can show you an upgrade notice:

```
Update available: v3.3.1 → v3.3.2
  Run: cargo install tokensave
```

**What is sent:** A single HTTP GET to `https://api.github.com/repos/aovestdipaperino/tokensave/releases/latest` with a `User-Agent: tokensave` header. No identifying information.

**When it's sent:** During `status` (cached for 5 minutes) and during `sync` (always, runs in parallel with indexing so it adds no latency). The upgrade command is auto-detected from your install method (cargo or brew).

**There is no way to disable the version check**, but it has a 1-second timeout and failures are silently ignored — it never blocks your workflow.

### Summary

| Call | Data sent | When | Opt-out |
|------|-----------|------|---------|
| Worldwide counter upload | Token count (a number) + country (from IP) | sync, status, stale commands | `tokensave disable-upload-counter` |
| Worldwide counter read | Nothing (GET request) | status | N/A (read-only, 1s timeout) |
| Version check | Nothing (GET request) | status (cached 5m), sync (parallel) | N/A (1s timeout, no-op on failure) |

---

## MCP Tools Reference

These tools are exposed via the MCP server and available to Claude Code when `.tokensave/` exists in the project.

| Tool | Use For |
|------|---------|
| `tokensave_search` | Find symbols by name (functions, classes, types) |
| `tokensave_context` | Get relevant code context for a task |
| `tokensave_callers` | Find what calls a function |
| `tokensave_callees` | Find what a function calls |
| `tokensave_impact` | See what's affected by changing a symbol |
| `tokensave_node` | Get details + source code for a symbol |
| `tokensave_files` | List indexed project files with filtering |
| `tokensave_affected` | Find test files affected by source changes |
| `tokensave_status` | Get index status, statistics, and global tokens saved |
| `tokensave_dead_code` | Find unreachable symbols (no incoming edges) |
| `tokensave_diff_context` | Semantic context for changed files — modified symbols, dependencies, affected tests |
| `tokensave_module_api` | Public API surface of a file or directory |
| `tokensave_circular` | Detect circular file dependencies |
| `tokensave_hotspots` | Most connected symbols (highest call count) |
| `tokensave_similar` | Find symbols with similar names |
| `tokensave_rename_preview` | All references to a symbol (preview rename impact) |
| `tokensave_unused_imports` | Import statements that are never referenced |
| `tokensave_changelog` | Semantic diff between two git refs |
| `tokensave_rank` | Rank nodes by relationship count (most implemented interface, most extended class, etc.) |
| `tokensave_largest` | Rank nodes by size — largest classes, longest methods |
| `tokensave_coupling` | Rank files by fan-in (most depended on) or fan-out (most dependencies) |
| `tokensave_inheritance_depth` | Find the deepest class inheritance hierarchies |
| `tokensave_distribution` | Node kind breakdown (classes, methods, fields) per file or directory |
| `tokensave_recursion` | Detect recursive/mutually-recursive call cycles (NASA Power of 10, Rule 1) |
| `tokensave_complexity` | Rank functions by composite complexity with cyclomatic complexity, safety metrics (unsafe, unchecked, assertions) from AST |
| `tokensave_doc_coverage` | Find public symbols missing documentation |
| `tokensave_god_class` | Find classes with the most members (methods + fields) |
| `tokensave_port_status` | Compare symbols between source/target directories to track porting progress |
| `tokensave_port_order` | Topological sort of symbols for porting — port leaves first, then dependents |

### `tokensave_context`

Get relevant code context for a task using semantic search and graph traversal.

- **`task`** (string, required): The task description
- **`max_nodes`** (number, optional): Maximum number of nodes to include (default: 20)

Returns structured code context with entry points, related symbols, and code snippets.

### `tokensave_search`

Search for symbols by name in the codebase.

- **`query`** (string, required): Symbol name to search for
- **`kind`** (string, optional): Filter by node kind (function, class, method, etc.)
- **`limit`** (number, optional): Maximum results (default: 10)

Returns matching symbols with locations and signatures.

### `tokensave_callers` / `tokensave_callees`

Find functions that call a symbol, or functions called by a symbol.

- **`symbol`** (string, required): The symbol to analyze
- **`depth`** (number, optional): Traversal depth (default: 1)
- **`limit`** (number, optional): Maximum results (default: 20)

Returns related symbols with relationship types.

### `tokensave_impact`

Analyze the impact of changing a symbol. Returns all symbols affected by modifications.

- **`symbol`** (string, required): The symbol to analyze
- **`max_depth`** (number, optional): Maximum traversal depth (default: 3)

Returns impact map showing affected symbols and their relationships.

### `tokensave_node`

Get detailed information about a specific symbol including source code.

- **`symbol`** (string, required): The symbol name
- **`file`** (string, optional): Filter by file path

Returns complete symbol details with source code, location, relationships, and for function/method nodes: complexity metrics (`branches`, `loops`, `returns`, `max_nesting`, `cyclomatic_complexity`) and safety metrics (`unsafe_blocks`, `unchecked_calls`, `assertions`).

### `tokensave_files`

List indexed project files. Use for file/folder exploration without reading file contents.

- **`path`** (string, optional): Filter to files under this directory
- **`pattern`** (string, optional): Filter files matching a glob pattern (e.g. `**/*.rs`)
- **`format`** (string, optional): Output format — `flat` or `grouped` (default: grouped)

Returns file listing with symbol counts, grouped by directory.

### `tokensave_affected`

Find test files affected by changed source files. BFS through the file dependency graph to discover impacted tests.

- **`files`** (array of strings, required): Changed file paths to analyze
- **`depth`** (number, optional): Maximum dependency traversal depth (default: 5)
- **`filter`** (string, optional): Custom glob pattern for test files (default: common test patterns)

Returns the list of affected test files and count.

### `tokensave_status`

Get index status and project statistics. Returns index metadata, symbol counts, language distribution, and pending changes. Also reports global tokens saved across all tracked projects (from the user-level database at `~/.tokensave/global.db`).

### `tokensave_dead_code`

Find unreachable symbols — functions or methods with no incoming edges (nothing calls them).

- **`kinds`** (array of strings, optional): Node kinds to check (default: `["function", "method"]`)

Returns a list of potentially dead code symbols with file paths and line numbers.

### `tokensave_diff_context`

Get semantic context for changed files. Given a list of file paths, returns the symbols in those files, what depends on them, and which tests are affected.

- **`files`** (array of strings, required): Changed file paths to analyze
- **`depth`** (number, optional): Impact traversal depth (default: 2)

Returns: modified symbols, impacted downstream symbols, and affected test files.

### `tokensave_module_api`

Show the public API surface of a file or directory — all exported symbols with their signatures.

- **`path`** (string, required): File path or directory prefix to inspect

Returns public symbols sorted by file and line number.

### `tokensave_circular`

Detect circular dependencies between files in the project.

- **`max_depth`** (number, optional): Maximum search depth (default: 10)

Returns a list of dependency cycles (each cycle is a list of file paths).

### `tokensave_hotspots`

Find the most connected symbols — the ones with the highest combined incoming + outgoing edge count.

- **`limit`** (number, optional): Maximum results (default: 10)

Returns symbols ranked by connectivity, useful for identifying high-risk code.

### `tokensave_similar`

Find symbols with names similar to a given query.

- **`symbol`** (string, required): Symbol name to match against
- **`limit`** (number, optional): Maximum results (default: 10)

Returns matching symbols sorted by relevance. Useful for finding patterns, naming inconsistencies, or related code.

### `tokensave_rename_preview`

Preview the impact of renaming a symbol. Shows all edges referencing it — callers, callees, containers, and other relationships.

- **`node_id`** (string, required): The symbol's node ID

Returns all referencing symbols with their locations and edge types.

### `tokensave_unused_imports`

Find import/use statements that are never referenced anywhere in the graph.

Returns a list of unused import nodes with file paths and line numbers.

### `tokensave_changelog`

Generate a semantic diff between two git refs. Shows which symbols were added, removed, or exist in changed files.

- **`from_ref`** (string, required): Starting git ref (e.g., `HEAD~5`, `v1.4.0`)
- **`to_ref`** (string, required): Ending git ref (e.g., `HEAD`, `v1.5.0`)

Returns a structured changelog with added/removed/modified symbols per file.

### `tokensave_rank`

Rank nodes by relationship count. Supports both incoming (default) and outgoing direction.

- **`edge_kind`** (string, required): Relationship type — `implements`, `extends`, `calls`, `uses`, `contains`, `annotates`, `derives_macro`
- **`direction`** (string, optional): `incoming` (default, e.g. most-implemented interface) or `outgoing` (e.g. class that implements the most interfaces)
- **`node_kind`** (string, optional): Filter by node kind (e.g. `interface`, `class`, `method`)
- **`limit`** (number, optional): Maximum results (default: 10)

### `tokensave_largest`

Rank nodes by line count (end_line - start_line + 1). Find the largest classes, longest methods, biggest enums.

- **`node_kind`** (string, optional): Filter by kind (e.g. `class`, `method`, `function`)
- **`limit`** (number, optional): Maximum results (default: 10)

### `tokensave_coupling`

Rank files by coupling — how many other files they depend on or are depended on by.

- **`direction`** (string, optional): `fan_in` (default, most depended-on) or `fan_out` (most outward dependencies)
- **`limit`** (number, optional): Maximum results (default: 10)

Only considers `calls`, `uses`, `implements`, and `extends` edges across file boundaries.

### `tokensave_inheritance_depth`

Find the deepest class/interface inheritance hierarchies by walking `extends` chains.

- **`limit`** (number, optional): Maximum results (default: 10)

Uses a recursive CTE to compute the maximum depth for each class in the hierarchy.

### `tokensave_distribution`

Show node kind distribution (classes, methods, fields, etc.) per file or directory.

- **`path`** (string, optional): Directory or file path prefix to filter
- **`summary`** (boolean, optional): If true, aggregate counts across all matching files instead of per-file breakdown (default: false)

### `tokensave_recursion`

Detect recursive and mutually-recursive call cycles in the call graph. Addresses NASA Power of 10 Rule 1 ("no recursion — call graph must be acyclic").

- **`limit`** (number, optional): Maximum number of cycles to return (default: 10)

Returns call cycles with full node details. Self-recursive functions appear as length-1 cycles.

### `tokensave_complexity`

Rank functions/methods by a composite complexity score: `lines + (fan_out × 3) + fan_in`. Also includes real cyclomatic complexity and structural metrics extracted from the AST during indexing.

- **`node_kind`** (string, optional): Filter by kind (default: function and method)
- **`limit`** (number, optional): Maximum results (default: 10)

Returns per-function: lines, fan_out, fan_in, composite score, plus AST-derived metrics: `cyclomatic_complexity` (branches + 1), `branches`, `loops`, `returns`, `max_nesting`.

### `tokensave_doc_coverage`

Find public symbols missing documentation (docstrings). Checks functions, methods, classes, interfaces, traits, structs, enums, and modules.

- **`path`** (string, optional): Directory or file path prefix to filter
- **`limit`** (number, optional): Maximum results (default: 50)

Returns undocumented symbols grouped by file with counts.

### `tokensave_god_class`

Find classes with the most members (methods + fields). Identifies "god classes" that may have too much responsibility and need decomposition.

- **`limit`** (number, optional): Maximum results (default: 10)

Returns classes ranked by total member count with method and field counts shown separately.

### `tokensave_port_status`

Compare symbols between a source directory and a target directory to track porting progress. Both directories must be within the same indexed project.

- **`source_dir`** (string, required): Path prefix for the source code (e.g., `src/python/`)
- **`target_dir`** (string, required): Path prefix for the target code (e.g., `src/rust/`)
- **`kinds`** (array of strings, optional): Node kinds to compare (default: function, method, class, struct, interface, trait, enum, module)

Matches symbols by name (case-insensitive) with cross-language kind compatibility — `class` matches `struct`, `interface` matches `trait`. Returns:

- **`matched`**: symbols found in both source and target (with kind mapping)
- **`unmatched`**: source symbols not yet ported (grouped by file)
- **`target_only`**: new symbols in target with no source equivalent
- **`coverage_percent`**: percentage of source symbols matched in target

Example: "We've ported 45 of 87 functions (51.7%). Here are the 42 remaining, grouped by source file."

### `tokensave_port_order`

Return symbols from a source directory in topological dependency order for porting. Symbols with no internal dependencies come first (safe to port immediately), then symbols whose dependencies are all earlier in the list.

- **`source_dir`** (string, required): Path prefix for the source code
- **`kinds`** (array of strings, optional): Node kinds to include (default: function, method, class, struct, interface, trait, enum, module)
- **`limit`** (number, optional): Maximum symbols to return (default: 50)

Uses Kahn's algorithm on the internal call/uses/extends/implements graph. Output is organized into levels:

- **Level 0**: No internal dependencies — port these first (utilities, constants, base types)
- **Level 1**: Depends only on level 0 — port these next
- **Level N**: Depends only on levels 0 through N-1
- **Cycles**: Mutually dependent symbols that should be ported together

Example: "Port `log_message` and `MAX_RETRIES` first (no deps), then `Connection` (uses both), then `Pool` (uses `Connection`)."

---

## `tokensave doctor`

Run a comprehensive health check of your tokensave installation:

```bash
tokensave doctor
```

```
tokensave doctor v1.8.1

Binary
  ✔ Binary: /Users/you/.cargo/bin/tokensave
  ✔ Version: 1.8.1

Current project
  ✔ Index found: /Users/you/project/.tokensave/

Global database
  ✔ Global DB: /Users/you/.tokensave/global.db

User config
  ✔ Config: /Users/you/.tokensave/config.toml
  ✔ Upload enabled

Claude Code integration
  ✔ Settings: /Users/you/.claude/settings.json
  ✔ MCP server registered
  ✔ PreToolUse hook installed
  ✔ All 27 tool permissions granted
  ✔ CLAUDE.md contains tokensave rules

Network
  ✔ Worldwide counter reachable (total: 1.0M)
  ✔ GitHub releases API reachable

All checks passed.
```

Checks: binary location, project index, global DB, user config, agent integration (MCP server, hook, permissions, prompt rules), and network connectivity. If any tool permissions are missing after an upgrade, it tells you to run `tokensave install`. Use `--agent` to check a specific agent only.

---

## How It Works with Claude Code

Once configured, Claude Code automatically uses tokensave instead of reading raw files when it needs to understand your codebase. The three layers reinforce each other:

| Layer | What it does | Why it matters |
|-------|-------------|----------------|
| **MCP server** | Exposes `tokensave_*` tools to Claude | Claude can query the graph directly |
| **CLAUDE.md rules** | Tells Claude to prefer tokensave over agents/file reads | Prevents the model from falling back to expensive patterns |
| **PreToolUse hook** | Native Rust hook (`tokensave hook-pre-tool-use`) blocks Explore agents | Catches cases where the model ignores the CLAUDE.md rules — no bash/jq needed |

The result: Claude gets the same code understanding with far fewer tokens. A typical Explore agent reads 20-50 files; tokensave returns the relevant symbols, relationships, and code snippets from its pre-built index.

---

## How It Works (Technical)

### 1. Extraction

tokensave uses language-specific Tree-sitter grammars (bundled via `tokensave-large-treesitters`) to extract:
- Function and class definitions
- Variable and type declarations
- Import and export statements
- Method calls and references
- Complexity metrics (branches, loops, returns, max nesting depth)

### 2. Storage

Extracted symbols are stored in a local libSQL (Turso) database with:
- Symbol metadata (name, kind, location, signature, complexity metrics)
- File information and language classification
- FTS5 full-text search index
- Vector embeddings for semantic search

### 3. Reference Resolution

The system resolves references between symbols:
- Import chains
- Function calls
- Type relationships
- Cross-file dependencies

### 4. Graph Queries

The graph supports complex queries:
- Find callers/callees at configurable depth
- Trace impact of changes across the codebase
- Build contextual symbol sets for a given task
- Semantic search via vector embeddings

---

## Supported Languages

tokensave supports 31 languages organized into three tiers controlled by Cargo feature flags. Each tier includes all languages from the tier below it.

### Lite (11 languages) — `--no-default-features`

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

### Medium (Lite + 9 = 20 languages) — `--features medium`

Adds scripting, config, and additional systems languages.

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

### Full (Medium + 11 = 31 languages) — default

Everything. Includes legacy, niche, and domain-specific languages.

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

### Mixing individual languages

You can also enable individual languages without a full tier:

```bash
# Lite + just Nix and Bash
cargo install tokensave --no-default-features --features lang-nix,lang-bash
```

---

## Troubleshooting

### "tokensave not initialized"

The `.tokensave/` directory doesn't exist in your project.

```bash
tokensave sync
```

### MCP server not connecting

Claude Code doesn't see tokensave tools.

1. Ensure `~/.claude/settings.json` includes the tokensave MCP server config
2. Restart Claude Code completely
3. Check that `tokensave` is in your PATH: `which tokensave`

### Missing symbols in search

Some symbols aren't showing up in search results.

- Run `tokensave sync` to update the index
- Check that the language is supported (see table above)
- Verify the file isn't excluded by `.gitignore`

### Indexing is slow

Large projects take longer on the first full index.

- Subsequent runs use incremental sync and are much faster
- Use `tokensave sync` (not `--force`) for day-to-day updates
- The post-commit hook runs in the background to avoid blocking

---

## Origin

This project is a Rust port of the original [CodeGraph](https://github.com/colbymchenry/codegraph) TypeScript implementation by [@colbymchenry](https://github.com/colbymchenry). The port maintains the same architecture and MCP tool interface while leveraging Rust for performance and native tree-sitter bindings.

### Performance

Full-index benchmark on a 1,782-file mixed Rust/Java/Scala codebase (57K nodes, 103K edges):

| Tool | Time | Speedup |
|---|---|---|
| CodeGraph (TypeScript, v0.6.8) | 31.2s | 1x |
| **tokensave (Rust, v3.2.1)** | **1.2s** | **26x** |

Key optimizations: rayon parallel extraction, prepared-statement DB writes, suffix-indexed reference resolution, and bulk-load mode with deferred index creation.

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

## License

MIT License — see [LICENSE](LICENSE) for details.
