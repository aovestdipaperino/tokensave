# Comparisons

How tokensave relates to CodeGraph and other tools that reduce agent token usage.

## tokensave vs CodeGraph

tokensave is a ground-up Rust rewrite of [CodeGraph](https://www.npmjs.com/package/@colbymchenry/codegraph) (Node.js/TypeScript). Both build semantic code graphs for AI coding agents, but they diverge significantly in scope and capabilities.

| | **tokensave** | **CodeGraph** |
|---|---|---|
| **Runtime** | Native binary (Rust) | Node.js 18+ |
| **Install** | `brew install`, `cargo install`, `scoop install` | `npx @colbymchenry/codegraph` |
| **Languages** | 50+ (3 tiers: lite/medium/full) | 19+ |
| **MCP tools** | 80+ | 9 |
| **Agent integrations** | 12+ (Claude, Codex, Gemini, Qwen, OpenCode, Cursor, Cline, Copilot, Roo Code, Zed, Antigravity, Kilo, Kiro, Kimi, Vibe, Grok, OMP, Pi, Plank, Factory Droid) | 1 (Claude Code) |
| **Index freshness** | On-demand staleness check on every MCP call; catch-up sync on connect; multi-agent work expected to use git worktrees | Native OS-level file watcher (FSEvents/inotify/ReadDirectoryChangesW, 2 s debounce); catch-up sync on connect |
| **Multi-branch indexing** | Yes, opt-in (per-branch DBs, cross-branch diff/search) | No |
| **Complexity metrics** | AST-extracted (branches, loops, nesting depth, cyclomatic & cognitive complexity, Halstead, maintainability index, CRAP) | No |
| **Porting tools** | Yes (`port_status`, `port_order`) | No |
| **Graph visualizer** | Removed (v4.0.1) | Yes |
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
| **Code-health analytics** | Composite score, Gini, dependency depth, DSM, risk-weighted test gaps, session deltas | No |
| **Edit primitives** | 4 atomic writers (`str_replace`, `multi_str_replace`, `insert_at`, `ast_grep_rewrite`) with auto re-indexing | No |
| **Crash resilience** | Subprocess-isolated extraction; native grammar aborts skip the file, sync continues | No |
| **Self-upgrade** | `tokensave upgrade` with stable/beta channels | `npm update` |
| **DB engine** | libsql (SQLite fork, WAL, async) | better-sqlite3 / wa-sqlite (WASM) |
| **Indexing speed** | ~1.2s for 1,782 files | ~4s for 1,782 files |
| **Binary size** | ~25 MB compressed download (all grammars bundled) | ~80 MB (node_modules + WASM) |

CodeGraph pioneered the approach and remains a solid choice if you prefer npm tooling and only need Claude Code integration. tokensave extends the concept with deeper analysis, more agents, multi-branch support, and a native binary with no runtime dependencies.

For detailed comparisons against CodeGraph, Dual-Graph (GrapeRoot), code-review-graph, and OpenWolf, see [docs/COMPARABLE-TOOLS.md](../COMPARABLE-TOOLS.md).

**Side-by-side comparison pages** (each one states where the other tool is better):
[Serena](https://tokensave.dev/vs-serena) ·
[code-review-graph](https://tokensave.dev/vs-code-review-graph) ·
[Graphify](https://tokensave.dev/vs-graphify) ·
[CodeGraph](https://tokensave.dev/vs-codegraph) ·
[token-savior](https://tokensave.dev/vs-token-savior) ·
[GrapeRoot](https://tokensave.dev/vs-graperoot) ·
[LeanCTX](https://tokensave.dev/vs-leanctx) ·
[OpenWolf](https://tokensave.dev/vs-openwolf) —
index at [tokensave.dev/vs](https://tokensave.dev/vs).

## Why tokensave over the alternatives

Several tools reduce token usage for AI coding agents. Here's why tokensave stands apart.

### Single native binary, zero dependencies

Most alternatives require a runtime: Python, Node.js, or both. tokensave ships as a single Rust binary with all 50+ tree-sitter grammars bundled (~25 MB compressed download). Nothing else to install -- and unlike LSP-backed tools such as [Serena](https://tokensave.dev/vs-serena), no language server to install, start or warm up per language. Two tools now match tokensave on packaging: [CodeGraph](https://tokensave.dev/vs-codegraph) bundles its own runtime, and [LeanCTX](https://tokensave.dev/vs-leanctx) is also a single Rust binary.

### Deepest code intelligence

tokensave works at the symbol level: functions, structs, fields, call edges, type hierarchies, complexity metrics. Alternatives like Dual-Graph (GrapeRoot) work at the file level -- they know which files exist but can't answer "who calls this function?" or "what breaks if I change this struct?" tokensave's 80+ specialized MCP tools cover call graph traversal, impact analysis, dead code detection, test mapping, rename preview, type hierarchies, circular dependency detection, complexity ranking, code-health analytics (Gini, DSM, dependency depth, risk-weighted test gaps), atomic edit primitives, and more. The closest comparable graph tool, [code-review-graph](https://tokensave.dev/vs-code-review-graph), exposes 30; several others expose fewer than 10 by deliberate design.

### Broadest agent support

More than a dozen AI coding agent integrations with per-agent native configuration formats. No other tool covers as many agents with as deep an integration. Claude Code gets hooks, prompt rules, and auto-allowed tool permissions. Kiro gets global MCP config, `tokensave.md` steering loaded as a resource, a managed agent with permissive built-in/tokensave tool approval, and hooks for delegation guardrails plus post-write sync. Pi gets MCP registration with the tool exposure its rules depend on, plus a managed extension for the guardrail, savings reset, and session receipt. Other agents get MCP server registration in their native config format.

### Multi-branch indexing

The only tool in this space with optional per-branch graph databases and cross-branch diff and search. When enabled, switching branches is instant -- no re-indexing required.

### Per-call token tracking

The only tool that reports exactly how many tokens each individual MCP tool call saved, plus a live TUI monitor across all projects and lifetime counters.

### Fully open source

MIT-licensed Rust, auditable end to end. Dual-Graph's core engine (`graperoot` on PyPI) is proprietary -- you can't see what it does with your code graph. OpenWolf is AGPL-3.0, which requires derivative works to be open-sourced.

### Performance

Full-index benchmark on a 1,782-file mixed Rust/Java/Scala codebase (57K nodes, 103K edges):

| Tool | Time | Speedup |
|---|---|---|
| CodeGraph (TypeScript) | 31.2s | 1x |
| **tokensave (Rust)** | **1.2s** | **26x** |
