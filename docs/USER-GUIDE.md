# Tokensave User Guide

Thanks for downloading Tokensave!

Tokensave is a code intelligence tool that builds a semantic knowledge graph of your codebase. It gives AI coding agents (like Claude Code) instant, structured access to your code's symbols, relationships, and dependencies — so they spend fewer tokens scanning files and more time writing code.

Everything runs locally. Your code never leaves your machine.

---

## Table of Contents

1. [Installing Tokensave](#installing-tokensave)
2. [Your First Index](#your-first-index)
3. [Connecting to Your Agent](#connecting-to-your-agent)
4. [Exploring Your Codebase from the CLI](#exploring-your-codebase-from-the-cli)
5. [Keeping the Index Fresh](#keeping-the-index-fresh)
6. [The Background Daemon](#the-background-daemon)
7. [Checking Your Setup with Doctor](#checking-your-setup-with-doctor)
8. [Finding Affected Tests](#finding-affected-tests)
9. [MCP Tools for AI Agents](#mcp-tools-for-ai-agents)
10. [Supported Languages](#supported-languages)
11. [Privacy and Network](#privacy-and-network)
12. [Updating Tokensave](#updating-tokensave)
13. [Configuration Files](#configuration-files)
14. [Troubleshooting](#troubleshooting)

---

## Installing Tokensave

Pick whichever method suits your platform.

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

If you only work with a subset of languages, you can install a smaller binary:

```bash
cargo install tokensave --features medium        # 20 languages
cargo install tokensave --no-default-features    # 11 languages (lite)
```

**Prebuilt binaries:**

Download from the [latest release](https://github.com/aovestdipaperino/tokensave/releases/latest) and place the binary somewhere on your `PATH`. Archives are available for macOS (Apple Silicon), Linux (x86_64 and ARM64), and Windows (x86_64).

---

## Your First Index

Navigate to any project directory and run:

```bash
cd /path/to/your/project
tokensave sync
```

Tokensave will scan every supported source file, extract symbols (functions, classes, methods, imports, type relationships, complexity metrics), and store everything in a local database at `.tokensave/tokensave.db`. You'll see a spinner with file-by-file progress and an ETA.

Once it finishes, run `tokensave status` to see what was indexed:

```bash
tokensave status
```

This prints an overview of your project: the number of files, symbols, edges (relationships between symbols), language distribution, and how many tokens the index has saved you so far. If you just want the summary line without the ASCII art, pass `--short`:

```bash
tokensave status --short
```

For machine-readable output, use `--json`.

### Incremental syncs

After the initial full index, every subsequent `tokensave sync` is incremental. It detects which files changed since the last sync (via content hashing) and only re-indexes those files. On a typical commit-sized change, this takes under a second.

### Force re-index

If you ever need to rebuild the entire index from scratch (for example, after a major Tokensave upgrade), pass `--force`:

```bash
tokensave sync --force
```

### Skipping folders

If there are directories you never want indexed (vendored code, generated output, etc.), pass `--skip-folder`:

```bash
tokensave sync --skip-folder vendor --skip-folder generated
```

### Seeing what changed

The `--doctor` flag lists every file that was added, modified, or removed during the sync, so you can verify exactly what the index updated:

```bash
tokensave sync --doctor
```

### Respecting .gitignore

By default, tokensave respects your `.gitignore` rules and skips ignored files during indexing. You can check the current setting or toggle it:

```bash
tokensave gitignore              # show current setting
tokensave gitignore on           # enable (default)
tokensave gitignore off          # disable — index everything
```

Don't forget to add `.tokensave` to your `.gitignore` so the database doesn't get committed:

```bash
echo .tokensave >> .gitignore
```

---

## Connecting to Your Agent

Tokensave works as an MCP (Model Context Protocol) server. AI coding agents connect to it to query your codebase instead of scanning files directly. The `install` command sets everything up automatically.

### Claude Code

```bash
tokensave install
```

This is the default. It registers the MCP server in `~/.claude/settings.json`, grants tool permissions so Claude doesn't have to ask you every time, installs a `PreToolUse` hook that redirects Claude away from spawning expensive Explore agents, and adds prompt rules to `~/.claude/CLAUDE.md` that tell Claude to prefer tokensave tools.

### Other agents

Tokensave supports nine agents. Pass `--agent` to install for a specific one:

```bash
tokensave install --agent claude      # Claude Code (default)
tokensave install --agent opencode    # OpenCode
tokensave install --agent codex       # OpenAI Codex CLI
tokensave install --agent gemini      # Gemini CLI
tokensave install --agent copilot     # GitHub Copilot CLI
tokensave install --agent cursor      # Cursor
tokensave install --agent zed         # Zed
tokensave install --agent cline       # Cline
tokensave install --agent roo-code    # Roo Code
```

Each agent gets an appropriate configuration: MCP server registration, tool permissions (where the agent supports them), and prompt rules in the agent's instruction file.

The install is idempotent — safe to run again after upgrading tokensave. You'll also be offered the option to set up a global git post-commit hook and the background daemon (more on those below).

### Removing an integration

```bash
tokensave uninstall                   # remove Claude Code integration
tokensave uninstall --agent codex     # remove Codex integration
```

---

## Exploring Your Codebase from the CLI

You don't need an AI agent to use tokensave. The CLI has several commands for direct exploration.

### Searching for symbols

```bash
tokensave query "authenticate"
```

This searches the full-text index for symbols matching your query. It returns function names, class names, method names, and their file locations and signatures. Limit results with `-l`:

```bash
tokensave query "authenticate" -l 5
```

### Building task context

```bash
tokensave context "implement user authentication"
```

This is the same context builder that AI agents use. Given a natural language task description, it finds the most relevant entry points, related symbols, and code structure. Output defaults to Markdown; use `--format json` for structured output.

```bash
tokensave context "implement user authentication" --format json -n 30
```

The `-n` flag controls how many symbols are included (default: 20).

### Listing indexed files

```bash
tokensave files                           # all files
tokensave files --filter src/mcp          # only files under src/mcp/
tokensave files --pattern "**/*.rs"       # only Rust files
tokensave files --json                    # machine-readable output
```

### Running the MCP server directly

```bash
tokensave serve
```

This starts the MCP server over stdio. You normally don't need to run this yourself — the agent integration handles it. But it's useful for debugging or connecting custom tools.

---

## Keeping the Index Fresh

Tokensave gives you three ways to keep the index up to date.

### Manual sync

Run `tokensave sync` whenever you want. It's incremental and fast.

### Post-commit hook

During `tokensave install`, you'll be offered a global git `post-commit` hook. If you accept, tokensave will automatically sync in the background after every git commit across all your repos. The hook is a no-op in repos that don't have a `.tokensave/` directory.

You can also set it up manually:

**Global (all repos):**

```bash
git config --global core.hooksPath ~/.git-hooks
mkdir -p ~/.git-hooks
cp scripts/post-commit ~/.git-hooks/post-commit
chmod +x ~/.git-hooks/post-commit
```

**Per-repo:**

```bash
cp scripts/post-commit .git/hooks/post-commit
chmod +x .git/hooks/post-commit
```

### Background daemon

The daemon watches all your tracked projects for file changes and syncs automatically. See the next section.

---

## The Background Daemon

The background daemon monitors all projects you've indexed and runs incremental syncs whenever files change on disk. This means your index is always up to date, even between commits.

### Starting the daemon

```bash
tokensave daemon
```

This forks into the background. To run it in the foreground (useful for debugging), pass `--foreground`:

```bash
tokensave daemon --foreground
```

### Checking daemon status

```bash
tokensave daemon --status
```

### Stopping the daemon

```bash
tokensave daemon --stop
```

### Autostart on boot

You can register the daemon as a system service so it starts automatically when you log in:

```bash
tokensave daemon --enable-autostart    # install launchd (macOS) / systemd (Linux) / Windows Service
tokensave daemon --disable-autostart   # remove the autostart service
```

### Upgrade-aware restarts

When you upgrade tokensave (via `brew upgrade`, `cargo install`, `scoop update`, or any other method), the running daemon detects that its binary has been replaced and automatically restarts with the new version. You don't need to manually stop and start it.

---

## Checking Your Setup with Doctor

The `doctor` command runs a comprehensive health check:

```bash
tokensave doctor
```

It verifies:

- **Binary** — location and version
- **Current project** — whether a `.tokensave/` index exists and the database is healthy
- **Global database** — the cross-project database at `~/.tokensave/global.db`
- **User config** — `~/.tokensave/config.toml` and upload settings
- **Agent integrations** — MCP server registration, hook installation, tool permissions, prompt rules
- **Network** — connectivity to the worldwide counter and GitHub releases API

If any tool permissions are missing after an upgrade, doctor will tell you to run `tokensave install` again.

To check only a specific agent:

```bash
tokensave doctor --agent claude
tokensave doctor --agent codex
```

---

## Finding Affected Tests

When you change source files, you often want to know which tests might be affected. The `affected` command traces through the file dependency graph to find them.

```bash
tokensave affected src/main.rs src/db/connection.rs
```

This performs a breadth-first search from the changed files through import/dependency edges to find test files that directly or transitively depend on those files.

### Piping from git

This is especially useful in CI pipelines:

```bash
git diff --name-only HEAD~1 | tokensave affected --stdin
```

### Options

```bash
tokensave affected src/lib.rs --depth 3         # limit traversal depth (default: 5)
tokensave affected src/lib.rs --filter "*_test.rs"  # custom test file pattern
tokensave affected src/lib.rs --json             # JSON output
tokensave affected src/lib.rs --quiet            # just file paths, no decoration
```

---

## MCP Tools for AI Agents

When running as an MCP server, tokensave exposes 34 tools that AI agents can call. Here's what they do, grouped by purpose.

### Core exploration

| Tool | What it does |
|------|-------------|
| `tokensave_context` | Given a task description, returns relevant symbols, relationships, and code snippets. This is the go-to starting point for any coding task. |
| `tokensave_search` | Find symbols by name. Supports filtering by kind (function, class, method, etc.). |
| `tokensave_node` | Get full details for a specific symbol: source code, location, complexity metrics, and relationships. |
| `tokensave_files` | List indexed files, optionally filtered by directory or glob pattern. |
| `tokensave_status` | Index statistics: file counts, symbol counts, language distribution, and tokens saved. |

### Navigating relationships

| Tool | What it does |
|------|-------------|
| `tokensave_callers` | Find what calls a given function or method. Configurable traversal depth. |
| `tokensave_callees` | Find what a function or method calls. |
| `tokensave_impact` | Trace the full blast radius of changing a symbol — everything that could be affected. |
| `tokensave_affected` | Find test files affected by source file changes. |
| `tokensave_similar` | Find symbols with similar names (useful for naming patterns or related code). |
| `tokensave_rename_preview` | Preview all references to a symbol before renaming it. |

### Code quality analysis

| Tool | What it does |
|------|-------------|
| `tokensave_dead_code` | Find unreachable symbols — functions with no callers. |
| `tokensave_unused_imports` | Find import statements that are never referenced. |
| `tokensave_circular` | Detect circular file dependencies. |
| `tokensave_recursion` | Detect recursive and mutually-recursive call cycles. |
| `tokensave_complexity` | Rank functions by composite complexity score, including cyclomatic complexity from the AST. |
| `tokensave_god_class` | Find classes with the most members — candidates for decomposition. |
| `tokensave_hotspots` | Find the most connected symbols (highest call count). These are high-risk areas. |
| `tokensave_doc_coverage` | Find public symbols missing documentation. |
| `tokensave_simplify_scan` | Quality analysis of changed files: duplications, dead code, complexity, coupling. |

### Structural analysis

| Tool | What it does |
|------|-------------|
| `tokensave_module_api` | Public API surface of a file or directory. |
| `tokensave_coupling` | Rank files by coupling (fan-in or fan-out). |
| `tokensave_inheritance_depth` | Find the deepest class inheritance hierarchies. |
| `tokensave_type_hierarchy` | Recursive type hierarchy tree for traits, interfaces, and classes. |
| `tokensave_distribution` | Node kind breakdown (classes, methods, fields) per file or directory. |
| `tokensave_rank` | Rank nodes by relationship count (most-implemented interface, most-extended class, etc.). |
| `tokensave_largest` | Rank nodes by size — largest classes, longest methods. |

### Git-aware tools

| Tool | What it does |
|------|-------------|
| `tokensave_diff_context` | Semantic context for changed files: modified symbols, dependencies, and affected tests. |
| `tokensave_changelog` | Semantic diff between two git refs — which symbols were added, removed, or modified. |
| `tokensave_commit_context` | Semantic summary of uncommitted changes, useful for drafting commit messages. |
| `tokensave_pr_context` | Semantic diff between git refs for pull request descriptions. |
| `tokensave_test_map` | Source-to-test mapping at the symbol level, with uncovered symbol detection. |

### Porting tools

| Tool | What it does |
|------|-------------|
| `tokensave_port_status` | Compare symbols between source/target directories to track cross-language porting progress. |
| `tokensave_port_order` | Topological sort of symbols for porting — tells you what to port first based on dependencies. |

All tools are read-only and safe to call in parallel.

---

## Supported Languages

Tokensave supports 31 languages, organized into three tiers. Each tier includes all the languages from the tier below it.

### Lite (11 languages)

Always compiled. The smallest binary for the most popular languages.

Rust, Go, Java, Scala, TypeScript, JavaScript, Python, C, C++, Kotlin, C#, Swift

### Medium (Lite + 9 = 20 languages)

Adds scripting, config, and additional systems languages.

Dart, Pascal, PHP, Ruby, Bash, Protobuf, PowerShell, Nix, VB.NET

### Full (Medium + 11 = 31 languages)

Everything, including legacy and niche languages.

Lua, Zig, Objective-C, Perl, Batch/CMD, Fortran, COBOL, MS BASIC 2.0, GW-BASIC, QBasic, QuickBASIC 4.5

### Mixing individual languages

You can also cherry-pick individual languages without taking a full tier:

```bash
cargo install tokensave --no-default-features --features lang-nix,lang-bash
```

### What gets extracted

For each supported language, tokensave extracts:

- Function and method definitions (with signatures)
- Class, struct, trait, interface, and enum definitions
- Fields and properties
- Import and export statements
- Call relationships and type references
- Docstrings and annotations
- Complexity metrics (branches, loops, returns, max nesting, cyclomatic complexity)
- Cross-file dependency edges

---

## Privacy and Network

Tokensave's core functionality is 100% local. Indexing, search, graph queries, and the MCP server all run on your machine against a local database. No API keys are needed.

There are two optional network calls.

### Worldwide token counter

Tokensave tracks how many tokens it has saved you. During `sync` and `status`, it uploads that count (a single number like `4823`) to an anonymous worldwide counter. No code, file names, project names, or identifying information is sent. The Cloudflare Worker also logs the country derived from your IP for aggregate geographic statistics — your actual IP is not stored.

This powers the "Worldwide" counter shown in `tokensave status`.

**To opt out:**

```bash
tokensave disable-upload-counter
```

When disabled, tokensave never uploads your count but still fetches and displays the worldwide total. Re-enable at any time:

```bash
tokensave enable-upload-counter
```

### Version check

Tokensave checks GitHub for new releases so it can show you an upgrade notice. This is a single GET request to the GitHub API with no identifying information. It has a 1-second timeout and failures are silently ignored. This check cannot be disabled, but it never blocks your workflow.

---

## Updating Tokensave

When a new version is available, tokensave tells you during `sync` and `status`:

```
Update available: v3.3.3 -> v3.4.0
  Run: tokensave upgrade
```

The `upgrade` command downloads the latest release from GitHub and replaces the binary in place:

```bash
tokensave upgrade
```

It automatically stops the background daemon before replacing the binary and restarts it afterwards if it was running. Beta and stable are separate update channels — a beta build only sees beta releases and vice versa.

You can also update through your package manager:

```bash
brew upgrade tokensave          # Homebrew
scoop update tokensave          # Scoop
cargo install tokensave         # Cargo
```

After upgrading, it's good practice to re-run install (to pick up any new tool permissions or prompt rules) and force a re-index:

```bash
tokensave install
tokensave sync --force
```

---

## Configuration Files

Tokensave stores data in two places.

### Per-project: `.tokensave/`

Created inside each project you index. Contains:

- `tokensave.db` — the libSQL database with all symbols, edges, files, and vector embeddings

Add `.tokensave` to your `.gitignore`.

### Per-user: `~/.tokensave/`

Created in your home directory. Contains:

- `config.toml` — user preferences (upload opt-in/out, cached version info, pending upload count)
- `global.db` — cross-project database that tracks tokens saved across all your projects

The `config.toml` is plain TOML and fully transparent:

```toml
upload_enabled = true       # set to false to stop uploading
pending_upload = 4823       # tokens waiting to be uploaded
last_upload_at = 1711375200 # last successful upload timestamp
last_worldwide_total = 1000000
last_worldwide_fetch_at = 1711375200
```

---

## Troubleshooting

### "tokensave not initialized"

The `.tokensave/` directory doesn't exist in your current project. Run:

```bash
tokensave sync
```

### MCP server not connecting

Your AI agent doesn't see tokensave tools.

1. Run `tokensave doctor` to check the integration
2. Verify `tokensave` is on your PATH: `which tokensave`
3. Re-run `tokensave install` and restart your agent completely

### Missing symbols in search

Some symbols aren't showing up.

- Run `tokensave sync` to update the index
- Check that the language is supported (see the tiers above)
- Verify the file isn't being skipped by `.gitignore` (`tokensave gitignore` to check)

### Indexing is slow on first run

The initial full index of a large project can take a few seconds. This is normal.

- Subsequent syncs are incremental and much faster
- Use `tokensave sync` (not `--force`) for day-to-day updates
- The post-commit hook and daemon run in the background so they never block you

### Stale install warning

If you see a warning about your install being stale after an upgrade, run:

```bash
tokensave install
```

This updates tool permissions, hooks, and prompt rules to match the new version.

### Getting help

If you run into something not covered here, check the [GitHub repository](https://github.com/aovestdipaperino/tokensave) or open an issue.
