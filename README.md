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
| **13 Languages** | **100% Local** | **Always Fresh** |
| Rust, Go, Java, Scala, TypeScript, JavaScript, Python, C, C++, Kotlin, Dart, C#, Pascal — all with the same API. | No data leaves your machine. No API keys. No external services. Everything runs on a local libSQL database. | Git hooks automatically sync the index as you work. Your code intelligence is always up to date. |

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
cargo install tokensave
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
curl -LO https://github.com/aovestdipaperino/tokensave/releases/latest/download/tokensave-v1.4.0-x86_64-linux.tar.gz
tar xzf tokensave-v1.4.0-x86_64-linux.tar.gz
sudo mv tokensave /usr/local/bin/
```

### 2. Configure Claude Code

Run the built-in installer — no scripts, no `jq`, works on macOS/Linux/Windows:

```bash
tokensave claude-install
```

This single command:

- Registers tokensave as an MCP server in `~/.claude/settings.json`
- Adds a native PreToolUse hook that blocks Explore agents in favor of tokensave
- Adds tool permissions so Claude can call all 9 tokensave tools without prompting
- Appends rules to `~/.claude/CLAUDE.md` that instruct Claude to prefer tokensave over file reads

All changes are idempotent — safe to run again after upgrading.

### 3. Index your project

```bash
cd /path/to/your/project
tokensave sync
```

This creates a `.tokensave/` directory with the knowledge graph database. Subsequent runs are incremental — only changed files are re-indexed.

<details>
<summary><strong>What claude-install writes to settings.json</strong></summary>

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
      "mcp__tokensave__tokensave_context",
      "mcp__tokensave__tokensave_files",
      "mcp__tokensave__tokensave_impact",
      "mcp__tokensave__tokensave_node",
      "mcp__tokensave__tokensave_search",
      "mcp__tokensave__tokensave_status"
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
tokensave query <search> [path]  # Search symbols
tokensave files [--filter dir] [--pattern glob] [--json]   # List indexed files
tokensave affected <files...> [--stdin] [--depth N]        # Find affected test files
tokensave claude-install         # Configure Claude Code integration
tokensave serve                  # Start MCP server
tokensave disable-upload-counter # Opt out of worldwide counter uploads
tokensave enable-upload-counter  # Re-enable worldwide counter uploads
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

tokensave's core functionality (indexing, search, graph queries, MCP server) is **100% local** — your code never leaves your machine. However, starting with v1.4.0, tokensave makes two optional network calls:

### 1. Worldwide token counter

tokensave tracks how many tokens it saves you across all your projects. On `sync` and `status` commands, it uploads the **count of tokens saved** (a single number) to an anonymous worldwide counter. No code, no file names, no project names, no identifying information is sent — just a number like `4823`.

This powers the "Worldwide ~1.0M" counter shown in `tokensave status`, which displays the total tokens saved by all tokensave users combined.

**What is sent:** A single HTTP POST to `https://tokensave-counter.enzinol.workers.dev/increment` with a JSON body like `{"amount": 4823}`. No cookies, no tracking, no user ID. The Cloudflare Worker also logs the **country of your IP address** (derived by Cloudflare from the request headers) for aggregate geographic statistics — your actual IP address is not stored.

**When it's sent:** After `sync` or `status` (always), or after other commands if the last upload was more than 30 seconds ago. Failed uploads are silently retried on the next command with a 60-second cooldown.

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
cached_latest_version = "1.4.0"
last_version_check_at = 1711375200
```

### 2. Version check

tokensave checks for new releases on GitHub so it can show you an upgrade notice:

```
Update available: v1.3.0 → v1.4.0
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

Returns complete symbol details with source code, location, and relationships.

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

tokensave uses language-specific Tree-sitter grammars (native Rust bindings) to extract:
- Function and class definitions
- Variable and type declarations
- Import and export statements
- Method calls and references

### 2. Storage

Extracted symbols are stored in a local libSQL (Turso) database with:
- Symbol metadata (name, kind, location, signature)
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

| Language | Extensions | Since |
|----------|-----------|-------|
| Rust | `.rs` | 0.4.0 |
| Go | `.go` | 0.5.0 |
| Java | `.java` | 0.5.0 |
| Scala | `.scala`, `.sc` | 0.6.0 |
| TypeScript | `.ts`, `.tsx` | 0.7.0 |
| JavaScript | `.js`, `.jsx` | 0.7.0 |
| Python | `.py` | 0.7.0 |
| C | `.c`, `.h` | 0.7.0 |
| C++ | `.cpp`, `.hpp`, `.cc`, `.cxx`, `.hh` | 0.7.0 |
| Kotlin | `.kt`, `.kts` | 0.7.0 |
| Dart | `.dart` | 0.7.0 |
| C# | `.cs` | 0.7.0 |
| Pascal | `.pas`, `.pp`, `.dpr` | 0.7.0 |

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

---

## Building

```bash
cargo build --release
cargo test
cargo clippy --all
```

## License

MIT License — see [LICENSE](LICENSE) for details.
