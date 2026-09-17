# Why tokensave?

Several tools exist that help AI coding agents work more efficiently with codebases. This page explains what sets tokensave apart and why you might choose it over the alternatives.

For a neutral, detailed comparison of five of them see [COMPARABLE-TOOLS.md](COMPARABLE-TOOLS.md). Full
side-by-side pages for all eight — each stating where the other tool is better — are published at
**<https://tokensave.dev/vs>**.

> **Verified September 2026 against tokensave v7.12.1:** 87 MCP tools (86 without `ast-grep`), 60 languages.
> Two claims in earlier revisions of this page were wrong and have been corrected: CodeGraph no longer requires
> Node.js (it ships a Rust kernel and bundles its own runtime), and code-review-graph now documents 30 tools and
> 40+ languages rather than 22 and 19.

---

## The landscape at a glance

| | **tokensave** | **Serena** | **code-review-graph** | **CodeGraph** | **Graphify** | **Dual-Graph** | **LeanCTX** | **OpenWolf** |
|---|---|---|---|---|---|---|---|---|
| Approach | Queryable code graph | LSP semantic toolkit | Code graph + review focus | Code graph, one explore tool | Code + docs graph | Context prefill layer | Context gate on agent I/O | Project memory |
| MCP tools | 87 | Many (retrieval + refactor) | 30 | 1 listed by design | 10 | 5 | ~56 | 0 (hook-based) |
| Languages | 60 | 40+ (via language servers) | 40+ plus notebooks | 30+ | 36 | 12 | n/a | Language-agnostic |
| Implementation | Rust (single binary) | Python 3.13+ / `uv` | Python | Rust kernel, bundled runtime | Python | Python + Node.js | Rust (single binary) | Node.js |
| Runtime deps | None | Python, `uv`, a language server per language | Python 3.10+ | None | Python | Python 3.10+, Node.js 18+ | None | Node.js 20+ |
| License | MIT | GPL-3.0-or-later (app) | MIT | MIT | Apache 2.0 | Apache 2.0 launchers, proprietary core | Apache 2.0 | AGPL-3.0 |
| Agent support | 12+ | Many | 8 (partially overlapping) | 1 | Many | 6 | 30+ | 7 (3 deeply) |

Serena, Graphify, LeanCTX and token-savior are covered on their own pages at <https://tokensave.dev/vs>; LeanCTX and
OpenWolf are best understood as complementary layers rather than competitors.

---

## vs Dual-Graph (GrapeRoot)

Dual-Graph intercepts prompts and pre-loads ranked files before the AI sees them. The AI is passive -- it receives pre-selected context rather than querying for what it needs.

**Why tokensave is the better choice:**

**Deep code understanding vs file-level ranking.** Dual-Graph works at the file level: it knows which files exist and guesses which ones are relevant. tokensave works at the symbol level: it knows every function, struct, field, call edge, type hierarchy, and complexity metric. When the AI asks "who calls this function?" or "what breaks if I change this struct?", tokensave answers instantly. Dual-Graph can't answer those questions at all.

**87 specialized tools vs 5 generic ones.** tokensave exposes tools for call graph traversal, impact analysis, dead code detection, test mapping, rename preview, type hierarchies, circular dependency detection, and more. Each tool is purpose-built for a specific question. Dual-Graph has a file retriever, a neighbor lookup, and a token counter.

**libSQL vs JSON files.** tokensave stores its graph in libSQL with FTS5 full-text search, WAL-mode concurrent reads, and indexed queries. Dual-Graph stores everything in JSON files (`info_graph.json`, `chat_action_graph.json`, `context-store.json`) -- every lookup is a full scan. The performance difference matters on large codebases.

**60 languages vs 12.** tokensave supports 60 languages with deep extraction including niche languages like Nix (with derivation field extraction and flake schema awareness), Protobuf (message/service/rpc as first-class nodes), COBOL, Fortran, and legacy BASIC variants. Dual-Graph covers 12 mainstream languages (TypeScript, JavaScript, Python, Go, Swift, Rust, Java, Kotlin, Scala, C#, Ruby, PHP) and nothing more.

**Fully open source vs proprietary core.** tokensave is MIT-licensed Rust you can read, audit, fork, and patch. Dual-Graph's launcher scripts are Apache 2.0, but the core engine (`graperoot` on PyPI) is proprietary. You can't see what it does with your code graph. You can't run it offline without trusting a closed-source PyPI package.

**Zero runtime dependencies.** tokensave ships as a single ~25 MB binary with all 60 tree-sitter grammars bundled. Dual-Graph requires both Python 3.10+ and Node.js 18+, totaling ~80 MB+ across a Python venv and Node.js installation.

**Persistent index, no rebuild.** tokensave keeps its graph on disk and refreshes it incrementally — an on-demand staleness check on each MCP call, a catch-up sync when the server connects, and an optional git post-commit hook — so it never rebuilds from scratch. Dual-Graph rebuilds its graph at the start of every session.

**Optional multi-branch indexing.** tokensave can optionally maintain per-branch databases with cross-branch diff and search. Dual-Graph has no branch awareness.

**More agent integrations.** tokensave supports more than a dozen AI coding agents (Claude Code, Codex CLI, Gemini CLI, Cursor, OpenCode, Copilot, Cline, Roo Code, Zed, Antigravity, Kilo, Kiro, Kimi, Vibe) with per-agent configuration formats. Dual-Graph supports 6.

**Per-call token savings.** Every tokensave MCP tool response includes `tokensave_metrics: before=N after=M` showing exactly how many tokens that specific call saved. Dual-Graph reports session-level totals but can't tell you which calls helped and which didn't.

**Stronger privacy.** tokensave's only optional network call is an anonymous token count (a single number like `4823`) with documented opt-out. Dual-Graph sends a persistent install ID on every launch.

---

## vs CodeGraph

CodeGraph is the Node.js/TypeScript project that originally inspired tokensave. Both build semantic code graphs with tree-sitter and expose them via MCP tools. tokensave is a ground-up Rust rewrite that has diverged significantly.

**Why tokensave is the better choice:**

**3.3x faster indexing.** tokensave indexes 1,782 files in ~1.2s; CodeGraph takes ~4s for the same codebase. The gap widens on larger projects thanks to rayon parallel extraction and prepared-statement DB writes.

**Comparable footprint now.** tokensave is a ~25 MB binary with zero runtime dependencies. CodeGraph used to require Node.js and ship ~80 MB of `node_modules` and WASM; it now has a Rust kernel and bundles its own runtime, so packaging is no longer a differentiator between the two.

**87 tools vs one, by choice.** CodeGraph now deliberately lists a single tool, `codegraph_explore`, intended to answer structural questions in one call, with the others still functional but unlisted. That is a real design argument against a large surface — 87 tool definitions cost tokens on every request — and not simply a missing feature. tokensave adds an entire code quality suite (complexity, coupling, god class detection, inheritance depth, doc coverage, recursion analysis), workflow tools (commit context, PR context, test mapping, diff context), refactoring support (rename preview, similar symbol detection), structural analysis (circular dependencies, unused imports, dead code), and porting tools (port status, port order).

**60 languages vs 30+.** tokensave supports 60 — including Svelte and Astro — with deep extractors for Nix, Protobuf, COBOL, Fortran, VB.NET, and legacy BASIC variants that CodeGraph doesn't cover.

**12+ agent integrations vs 1.** CodeGraph supports Claude Code only. tokensave integrates with Claude Code, Codex CLI, Gemini CLI, Cursor, OpenCode, Copilot, Cline, Roo Code, Zed, Antigravity, Kilo, Kiro, Kimi, and Vibe -- each with native configuration format support.

**Optional multi-branch indexing.** tokensave can optionally maintain per-branch databases with cross-branch diff and search. CodeGraph indexes only the current checkout.

**Annotation extraction.** tokensave extracts annotations and attributes across 13 languages (Rust, Swift, Dart, Scala, PHP, C++, VB.NET, Java, Kotlin, TypeScript, C#, Python, Zig). CodeGraph doesn't track annotations.

**Per-call token tracking.** tokensave reports token savings on every MCP tool response plus a live TUI monitor and session/lifetime counters. CodeGraph has no token tracking.

**MCP resources and annotations.** tokensave exposes 4 MCP resources (status, files, overview, branches) and marks core tools with `readOnlyHint` and `anthropic/alwaysLoad` annotations. CodeGraph has neither.

**Extensive test suite.** tokensave has 1,000+ tests with 84% line coverage (measured at v3.4.0; more tests have been added since). CodeGraph has minimal test coverage.

**Atomic config writes.** tokensave creates backups before modifying agent config files and uses atomic staging + rename. A crash during install can't corrupt your settings. CodeGraph writes configs directly.

**Self-update.** `tokensave upgrade` downloads the correct platform binary from GitHub with stable/beta channel support. CodeGraph relies on `npm update`.

**Where CodeGraph still leads:** CodeGraph's `codegraph_explore` tool (a unified natural-language query tool with call budgets and session deduplication) is a genuinely better interaction pattern for Explore agents. CodeGraph also offers local embedding search via nomic-embed-text-v1.5; tokensave uses agent-driven keyword expansion via FTS5, which is faster and lighter but doesn't catch conceptual matches with zero lexical overlap.

---

## vs code-review-graph

code-review-graph is the closest competitor in philosophy -- both build symbol-level graphs with tree-sitter, store them in SQLite, and expose them via MCP tools. The differences are in implementation depth and feature focus.

**Why tokensave is the better choice:**

**Rust vs Python.** tokensave is a single native binary with zero runtime dependencies. code-review-graph requires Python 3.10+ and optional dependencies (sentence-transformers, igraph, ollama) that can total hundreds of megabytes.

**Deeper code quality analysis.** tokensave has 9 quality/structure tools: `complexity`, `coupling`, `god_class`, `inheritance_depth`, `doc_coverage`, `recursion`, `unused_imports`, `dead_code`, and `simplify_scan`. code-review-graph has `find_large_functions_tool` -- one tool that checks function length.

**Type system awareness.** `type_hierarchy` and `inheritance_depth` provide recursive trait/interface/class inheritance trees. code-review-graph doesn't track type relationships.

**Richer git integration.** tokensave offers `commit_context`, `pr_context`, `diff_context`, `changelog`, and `test_map` for workflow automation. code-review-graph has `detect_changes_tool` with risk scoring, but no commit/PR context generation or test mapping.

**Optional multi-branch indexing.** tokensave can optionally maintain per-branch databases with cross-branch diff and search via `branch_search`, `branch_diff`, and `branch_list`. code-review-graph has no branch awareness.

**More languages with deeper extraction.** 60 languages with deep extractors (Nix derivation fields, Protobuf message/service/rpc, COBOL, Fortran, legacy BASIC) vs 40+ with standard tree-sitter extraction.

**No long-lived watcher process.** tokensave refreshes its index on demand — a staleness check on each MCP call plus a catch-up sync when the server connects — so there is no separate process to manage. code-review-graph has a foreground `watch` command that stops when you close the terminal.

**Porting tools.** `port_status` and `port_order` help assess and plan cross-language porting with topological dependency ordering. code-review-graph has no equivalent.

**Per-call token tracking.** Every tool response includes `tokensave_metrics: before=N after=M`. Plus a live TUI monitor, session counters, and a worldwide aggregate counter. code-review-graph has no token tracking.

**MCP resources and annotations.** 4 MCP resources and `readOnlyHint`/`alwaysLoad` annotations on core tools. code-review-graph has neither.

**Faster indexing.** ~1.2s for 1,782 files (full index) vs ~2s for 2,900 files (incremental, not directly comparable but indicative of similar performance class with tokensave handling more extraction depth).

**Extensive test suite.** 1,000+ tests with 84% line coverage (measured at v3.4.0; more tests added since) vs unpublished coverage.

**Where code-review-graph still leads:** Multi-repository registry with cross-repo search, execution flow analysis with criticality ranking, community detection via the Leiden algorithm, wiki generation from the code graph, 5 MCP prompt templates for common workflows, apply-refactoring (not just preview), notebook support (Jupyter/Databricks), published accuracy benchmarks (F1/precision/recall across 6 repos), and support for Windsurf, Continue, and Antigravity (which tokensave lacks, while tokensave supports Gemini CLI, Copilot, Cline, and Roo Code which code-review-graph lacks).

---

## vs OpenWolf

OpenWolf takes a fundamentally different approach. It doesn't build a code graph at all -- it keeps portable project memory in a local `.wolf/` directory, attaches to whatever session and tool events each agent exposes, identifies redundant reads, and records token usage read from the harness transcript. Integration spans seven agents at three depths: full lifecycle hooks for Claude Code and Codex CLI, a native plugin for OpenCode, compatible hook discovery for Grok Build, and context-file injection for Cursor, Gemini CLI and Antigravity.

**Why tokensave is the better choice:**

**Code intelligence vs behavioral guardrails.** tokensave understands your code: it knows every function, every call edge, every type hierarchy, every dependency chain. OpenWolf knows files exist and how big they are, but has zero understanding of what's inside them. It can't answer "who calls this function?", "what breaks if I change this?", or "show me the type hierarchy."

**87 MCP tools vs zero.** OpenWolf is entirely hook-based -- it has no MCP tools at all. The AI can't query it. It can only intercept and annotate the AI's existing tool calls. tokensave gives the AI 87 structured tools to actively explore the codebase.

**60 languages with deep extraction.** tokensave parses 60 languages at the symbol level. OpenWolf is language-agnostic because it only tracks files, not code structure.

**12+ agent integrations vs 7.** OpenWolf reaches seven agents but only three of them deeply (Claude Code, Codex CLI, OpenCode); the rest get a context file. tokensave works with more than a dozen, each with native MCP registration.

**Zero runtime dependencies.** tokensave is a single Rust binary. OpenWolf is TypeScript and requires Node.js 20+.

**MIT vs AGPL-3.0.** tokensave's MIT license imposes no restrictions. OpenWolf's AGPL-3.0 requires derivative works to be open-sourced -- a concern for commercial tooling built on top of it.

**Where OpenWolf still leads:** Redundant-read identification, memory that survives across sessions *and across agents* via a portable `.wolf/` directory with explicit handover packets, searchable bug history, file-size awareness before reads, design QC with dev server screenshot capture, and -- notably -- token accounting measured from the harness transcript rather than estimated, grouped by agent and model. These address a different class of waste than tokensave does. The two are complementary and run side by side; see <https://tokensave.dev/vs-openwolf>.

---

## Cross-cutting advantages

Several of tokensave's advantages apply across all four comparisons:

**Single native binary, zero dependencies.** Most alternatives require a runtime: Python, Node.js, or both. tokensave installs and runs with nothing else on the machine. Two tools now match it here — CodeGraph bundles its own runtime, and LeanCTX is also a single Rust binary — so this is no longer a universal differentiator. It remains a real one against LSP-backed tools such as Serena, which additionally need a language server installed, started and warmed for every language in the repo.

**Broad language support.** 60 languages with three compilation tiers (lite/medium/full) for binary size control, and deep extractors for languages most tools skip entirely.

**Broadest agent support.** More than a dozen AI coding agent integrations with per-agent native configuration formats. code-review-graph supports 8 platforms with partial overlap (it adds Windsurf, Continue; tokensave adds Gemini CLI, Copilot, Cline, Roo Code). No other tool covers as many agents with as deep an integration (hooks, prompt rules, tool permissions).

**Optional multi-branch indexing.** The only tool with optional per-branch graph databases and cross-branch diff and search.

**Per-call token tracking.** The only tool that reports exactly how many tokens each individual MCP tool call saved, plus a live TUI monitor across all projects.

**Permissively licensed and fully open.** MIT-licensed Rust, auditable end to end. Dual-Graph's core is proprietary. OpenWolf is AGPL-3.0 and Serena's application is GPL-3.0-or-later with a CLA — both worth checking before embedding in a commercial workflow. CodeGraph, code-review-graph and LeanCTX are permissively licensed open source.

**Atomic, safe configuration.** tokensave is the only tool that creates backups before modifying agent config files and uses atomic writes. A crash or interruption during install can't corrupt your settings.
