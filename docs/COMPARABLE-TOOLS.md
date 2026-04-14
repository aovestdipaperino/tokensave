# Comparable Tools

## tokensave v4.0 vs Dual-Graph v3.9

Dual-Graph (also known as GrapeRoot, repository: [kunal12203/Codex-CLI-Compact](https://github.com/kunal12203/Codex-CLI-Compact)) is a context engine for AI coding assistants. Both tools reduce token usage by giving AI agents structured access to codebase knowledge instead of letting them scan files ad hoc. They take fundamentally different approaches.

---

### 1. Architecture & Design Philosophy

**tokensave** is a queryable code intelligence engine. It builds a symbol-level graph (functions, structs, fields, call edges, type hierarchies, complexity metrics) in a libSQL database, then exposes 37 specialized MCP tools that let the AI ask precise, targeted questions. The AI drives the exploration -- it decides what to query and when.

**Dual-Graph** is a context prefill layer. It scans the codebase into a file/symbol/import graph stored in JSON files, then intercepts prompts and pre-loads ranked files before the AI sees them. The AI is mostly passive -- it receives pre-selected context and works with it. It also exposes a few MCP tools for deeper exploration when needed.

This is a fundamental difference. tokensave trusts the AI to ask the right questions given the right tools. Dual-Graph assumes the AI is bad at discovery and does it for the AI upfront.

---

### 2. Graph Depth & Granularity

| Dimension | **tokensave** | **Dual-Graph** |
|---|---|---|
| Unit of analysis | Symbol (function, struct, field, variant, import) | File + symbols + imports |
| Edge types | 8 distinct (calls, contains, uses, implements, extends, annotates, derives_macro, receives) | Not disclosed (file-level neighbors) |
| Call graph | Full inter-procedural (who calls what, who is called by whom) | File-level neighbors only |
| Type hierarchy | Recursive trait/interface/class inheritance trees | No |
| Complexity metrics | AST-extracted: cyclomatic, branches, loops, nesting depth, unsafe blocks, unwrap detection | No |
| Cross-file impact | `impact` + `affected` trace the full blast radius of any symbol change | No |
| Storage | libSQL with FTS5 full-text search, WAL mode, async I/O | JSON files (`info_graph.json`, `chat_action_graph.json`, `context-store.json`) |

The JSON storage choice is significant. JSON doesn't support indexed queries, FTS, or concurrent writers -- every lookup is a full scan. libSQL gives tokensave O(1) lookups, full-text search, and WAL-mode concurrent reads.

---

### 3. MCP Tool Surface Area

**tokensave (37 tools):**

| Category | Tools |
|---|---|
| Discovery | `context`, `search`, `node`, `files`, `module_api` |
| Graph traversal | `callers`, `callees`, `impact`, `affected` |
| Quality analysis | `complexity`, `dead_code`, `god_class`, `coupling`, `simplify_scan`, `unused_imports` |
| Type system | `type_hierarchy`, `inheritance_depth` |
| Git-aware | `commit_context`, `pr_context`, `diff_context`, `changelog` |
| Rankings | `rank`, `hotspots`, `largest`, `distribution` |
| Refactoring | `rename_preview`, `similar` |
| Testing | `test_map` |
| Structure | `circular`, `recursion`, `doc_coverage` |
| Porting | `port_status`, `port_order` |
| Multi-branch | `branch_search`, `branch_diff`, `branch_list` |
| Meta | `status` |

**Dual-Graph (5 tools):**

| Tool | Purpose |
|---|---|
| `graph_read` | Read the semantic graph |
| `graph_retrieve` | Retrieve ranked relevant files |
| `graph_neighbors` | Find connected symbols |
| `count_tokens` | Estimate token count of a string |
| `get_session_stats` | Session cost metrics |

tokensave has ~7x more tools, and critically, they are specialized. "What breaks if I rename this function?" is one tool call with `rename_preview`. In Dual-Graph, that question can't be answered -- there's no call graph to traverse.

---

### 4. Language Support

| | **tokensave** | **Dual-Graph** |
|---|---|---|
| Count | 31 | 11 |
| Tier system | 3 tiers (lite/medium/full) for binary size control | No |
| Deep extractors | Nix (derivation fields, flake schema), Protobuf (message/service/rpc), COBOL, Fortran, legacy BASIC variants | Standard extraction only |
| Extraction depth | Functions, classes, methods, fields, imports, call sites, type relations, complexity, docstrings, annotations | Files, symbols, imports |

---

### 5. Indexing & Freshness

| | **tokensave** | **Dual-Graph** |
|---|---|---|
| Full index speed | ~1.2s for 1,782 files; ~22s for 28K files | Not disclosed |
| Incremental sync | Yes (hash + mtime/size pre-filter) | Rescans on launch |
| Background daemon | Yes -- launchd/systemd/Windows Service, auto-restarts on upgrade | No |
| Git hooks | Global post-commit hook, auto-installed | No |
| Multi-branch | Optional per-branch databases, cross-branch diff/search | No |
| Sync doctor | `--doctor` flag shows added/modified/removed files | No |

tokensave keeps the graph warm between sessions via the daemon. Dual-Graph rebuilds its graph at the start of each session. For large codebases, this startup cost adds up.

---

### 6. Token Tracking & Cost Visibility

| | **tokensave** | **Dual-Graph** |
|---|---|---|
| Per-call savings | Yes -- every MCP tool response includes `tokensave_metrics: before=N after=M` | No per-call breakdown |
| Session counter | `tokensave current-counter` / `reset-counter` CLI commands | `get_session_stats()` MCP tool |
| Live dashboard | `tokensave monitor` -- global TUI showing MCP calls from all projects in real time via memory-mapped ring buffer | localhost:8899 web dashboard |
| Lifetime tracking | Per-project + global "All projects" counter | Session-only |
| Worldwide counter | Aggregate "tokens saved by all users" counter (opt-out) | No |
| Cost estimation | Token counts (user applies their own pricing) | Estimated cost in dollars based on Claude pricing |

Dual-Graph's dashboard shows dollar amounts, which is user-friendly. tokensave's monitor is a live TUI showing tool calls across all projects. tokensave reports raw token deltas per call, which is more accurate (pricing changes, models differ), but less immediately readable. Dual-Graph doesn't break down savings per-call, which makes it harder to know which tool calls actually saved tokens.

---

### 7. Privacy & Network

| | **tokensave** | **Dual-Graph** |
|---|---|---|
| Core processing | 100% local | 100% local |
| Network calls | Token count upload (anonymous number only), version check | Version check, random install ID heartbeat, optional feedback |
| Opt-out | `tokensave disable-upload-counter` | Not documented |
| What's sent | A single number (e.g. `4823`) + country from IP | Install ID + platform flag |
| Data stored on disk | `.tokensave/tokensave.db` (binary SQLite) | `.dual-graph/*.json` (human-readable) |

Both are reasonably private. Dual-Graph sends a persistent install ID (even if random), which is a stronger identifier than tokensave's stateless counter upload.

---

### 8. Multi-Client Support

| Agent | **tokensave** | **Dual-Graph** |
|---|---|---|
| Claude Code | Yes (deepest integration: hooks, prompt rules, tool permissions) | Yes |
| Codex CLI | Yes | Yes |
| Gemini CLI | Yes | Yes |
| Cursor | Yes | Yes |
| OpenCode | Yes | Yes |
| GitHub Copilot | Yes | Yes |
| Cline | Yes | No |
| Roo Code | Yes | No |
| Zed | Yes | No |

tokensave supports 9 agents vs Dual-Graph's 6. tokensave's Claude Code integration goes deeper -- it installs PreToolUse hooks that actively block wasteful Explore agents, plus UserPromptSubmit and Stop hooks for lifecycle tracking.

---

### 9. Distribution & Dependencies

| | **tokensave** | **Dual-Graph** |
|---|---|---|
| Core language | Rust | Python (proprietary `graperoot` on PyPI) |
| License | MIT (fully open source) | Apache 2.0 launchers, **proprietary core** |
| Binary size | ~25 MB single binary | ~80 MB+ (Python venv + Node.js) |
| Runtime deps | None | Python 3.10+ AND Node.js 18+ |
| Install methods | brew, scoop, cargo, prebuilt binaries, `tokensave upgrade` | curl \| bash, PowerShell, Scoop |
| Self-update | `tokensave upgrade` (downloads correct platform binary from GitHub) | Auto-updates launcher on every run |
| Auditable | Yes -- read every line of source | No -- core engine is a PyPI black box |

The proprietary core is the elephant in the room. You can't audit what `graperoot` does with your code graph. You can't fork it, patch it, or run it offline without trusting PyPI. tokensave is MIT-licensed Rust you can read end to end.

---

### 10. Where Dual-Graph Is Ahead

Despite the narrower graph, Dual-Graph has a few features tokensave lacks entirely or handles differently.

#### Dollar-amount cost dashboard

Dual-Graph's localhost:8899 web dashboard shows estimated session cost in dollars based on Claude's pricing tiers. tokensave reports raw token deltas per call and exposes a TUI monitor, but never converts tokens to dollars. For users who want to know "this session cost me $1.40" at a glance without doing mental math, Dual-Graph's approach is more immediately useful.

#### Passive context injection

Dual-Graph intercepts prompts and pre-loads ranked files before the AI sees them. The AI doesn't need to learn any new tools or change its behavior -- it just receives better context. This works well with less capable models that struggle to drive a large tool surface effectively. tokensave's 37 specialized tools are powerful but require the AI to know when and how to use each one. On smaller or less instruction-following models, passive prefill can outperform active querying.

#### Pre-query token estimation

The `count_tokens` MCP tool lets the AI estimate how many tokens a file or block will cost before reading it. This enables budget-aware decisions: "this file is 8,000 tokens, skip it and read the smaller one instead." tokensave provides token metrics after the fact (how many tokens were saved by this tool call) but doesn't offer a pre-read estimation tool.

#### Cross-session context persistence

Dual-Graph maintains a `context-store.json` that persists decisions, tasks, and facts across sessions, re-injected at session start. tokensave's graph is persistent but it doesn't carry forward conversational context or decisions from previous sessions -- that responsibility falls to the AI client's own memory systems.

#### Read budget controls

Dual-Graph exposes environment variables (`DG_HARD_MAX_READ_CHARS`, `DG_TURN_READ_BUDGET_CHARS`, `DG_FALLBACK_MAX_CALLS_PER_TURN`) that cap how much context is injected per turn. This gives users a hard guardrail against runaway token usage. tokensave has no equivalent per-turn budget -- it trusts the AI to self-regulate based on tool descriptions and prompt rules.

---

### 11. Where tokensave Is Ahead

tokensave's advantages are covered in detail in sections 2-9 above. The short version: 37 vs 5 MCP tools, symbol-level vs file-level granularity, full call graphs and impact analysis, 31 vs 11 languages, libSQL vs JSON storage, background daemon, optional multi-branch indexing, 9 vs 6 agent integrations, MIT-licensed Rust vs proprietary Python core, zero runtime dependencies, and per-call token savings reporting.

---

### 12. Summary

Dual-Graph is a "smart file retriever" -- it answers "which files should the AI see?" tokensave is a "code intelligence platform" -- it answers "how does this codebase work?" They share a surface-level goal (reduce AI token usage) but the depth of analysis is not in the same category. The fact that Dual-Graph's core is a proprietary Python package while tokensave is fully open-source Rust is a non-trivial factor for anyone who cares about trust, auditability, or long-term maintenance.

---
---

## tokensave v4.0 vs CodeGraph

CodeGraph ([`@colbymchenry/codegraph`](https://www.npmjs.com/package/@colbymchenry/codegraph)) is the Node.js/TypeScript project that inspired tokensave. Both build semantic code graphs for AI coding agents, but tokensave is a ground-up Rust rewrite that has diverged significantly in scope.

---

### 1. Relationship

tokensave started as a Rust port of CodeGraph and shares the core idea: parse a codebase with tree-sitter, store symbols and edges in a local database, expose the graph via MCP tools. The two projects have since evolved independently. CodeGraph continues to ship improvements (notably `codegraph_explore` and local embeddings), while tokensave has expanded into code quality analysis, optional multi-branch indexing, multi-agent support, and a background daemon.

---

### 2. Head-to-Head

| | **tokensave** | **CodeGraph** |
|---|---|---|
| Runtime | Native binary (Rust) | Node.js 18+ |
| Install | `brew install`, `cargo install`, `scoop install`, prebuilt binaries | `npx @colbymchenry/codegraph` |
| Languages | 31 (3 tiers: lite/medium/full) | 19+ (including Svelte) |
| MCP tools | 37 | 9 |
| Agent integrations | 9 (Claude, Codex, Gemini, OpenCode, Cursor, Cline, Copilot, Roo Code, Zed) | 1 (Claude Code) |
| Background daemon | Yes (launchd/systemd/Windows Service, upgrade-aware) | No (hook-based sync only) |
| Multi-branch indexing | Yes, opt-in (per-branch DBs, cross-branch diff/search) | No |
| Complexity metrics | AST-extracted (branches, loops, nesting depth, cyclomatic) | No |
| Porting tools | Yes (`port_status`, `port_order`) | No |
| Graph visualizer | Removed | Yes |
| Semantic search | Agent-driven keyword expansion via FTS5 (zero-cost) | Local embeddings (nomic-embed-text-v1.5 via ONNX) |
| MCP resources | 4 (status, files, overview, branches) | No |
| MCP annotations | Yes (readOnlyHint, alwaysLoad) | No |
| Dead code detection | Yes | No |
| Circular dependency detection | Yes | No |
| Type hierarchy | Yes | No |
| God class / coupling analysis | Yes | No |
| Commit / PR context | Yes | No |
| Test mapping | Yes | No |
| Rename preview | Yes | No |
| Annotation extraction | 13 languages (Rust, Swift, Dart, Scala, PHP, C++, VB.NET, Java, Kotlin, TypeScript, C#, Python, Zig) | No |
| DB engine | libSQL (SQLite fork, WAL, async) | better-sqlite3 / wa-sqlite (WASM) |
| Indexing speed | ~1.2s for 1,782 files | ~4s for 1,782 files |
| Binary size | ~25 MB (all grammars bundled) | ~80 MB (node_modules + WASM) |
| Test coverage | 84% (v3.4.0), 1,000+ tests | Minimal |
| Atomic config writes | Yes | No |
| License | MIT | MIT |

---

### 3. Where CodeGraph Is Ahead

#### `codegraph_explore` -- the biggest gap

CodeGraph's headline feature. A single MCP tool that accepts a natural language question and returns full source code sections for all relevant symbols in one call, internally combining FTS5 search, graph traversal, and source extraction. The agent no longer needs to chain `search` then `node` then `callers`.

The numbers are striking: CodeGraph's benchmarks jumped from "25% fewer tool calls" to 92% fewer tool calls and 71% faster across 6 real-world codebases, including VS Code (25k files) and the Swift Compiler (272k nodes, answered in 35s with 6 calls and zero file reads).

Key properties that make it work:
- **Call budget in the tool description** -- scales automatically with project size (e.g. "5 calls max for this 40k-node project"). The model reads this and self-limits.
- **`seen` parameter** -- node IDs from call N are passed to call N+1 to guarantee disjoint results. No budget wasted on duplicates.
- **Plain text output** -- fenced source blocks with file paths, matching what a `Read` tool call returns.
- **Scoped to Explore agents** -- CLAUDE.md rules route Explore agents to `codegraph_explore` and the main session to `codegraph_context`.

tokensave has `tokensave_context` with `include_code: true`, which is functionally equivalent internally. The gap is in output format, call budget signalling, session deduplication, and prompt routing rules -- not in the underlying query logic.

#### Svelte language support

CodeGraph supports Svelte (`.svelte`) among its 19+ languages. tokensave has 31 languages but Svelte is not among them. The `tree-sitter-svelte` grammar exists and is maintained. Relevant for SvelteKit projects.

#### Local embedding search

CodeGraph runs a local embedding model (nomic-embed-text-v1.5, 768-dim, ONNX) during indexing, storing a vector per symbol. Queries are embedded and matched by cosine similarity, so "authentication" finds `login()` even with zero lexical overlap.

tokensave takes a different path: the `keywords` parameter on `tokensave_context` lets the calling agent provide synonyms directly. When you ask "how does authentication work?", the agent passes `keywords: ["login", "session", "credential", "token"]` and the context builder runs FTS5 for each keyword. This adds zero indexing cost, zero model dependency, and ~1ms per keyword. The trade-off: if the agent doesn't know what naming conventions the codebase uses (e.g. `guardianGateway` for auth), it can't provide the right keywords. Embeddings would catch that case because they encode distributional semantics, not just lexical forms.

#### Native file watcher in the MCP server

CodeGraph's MCP server watches for file changes using native OS events (FSEvents/inotify/ReadDirectoryChangesW), debounced to a 2-second quiet window, and syncs automatically with no configuration.

tokensave's daemon is architecturally more capable (persists across MCP sessions, survives restarts, launchd/systemd autostart), but users who run `tokensave serve` without enabling the daemon get no auto-sync.

#### `codegraph uninit` command

Deletes the `.codegraph/` directory cleanly, with an optional `--force` to skip the confirmation prompt. tokensave has no equivalent.

#### Config toggles for extraction

CodeGraph's per-project config exposes `extractDocstrings` and `trackCallSites` booleans. Disabling call site extraction on very large codebases could meaningfully reduce index time and DB size. tokensave extracts both unconditionally.

---

### 4. Where tokensave Is Ahead

The list is long enough that a table is more useful than prose:

| Area | tokensave | CodeGraph |
|---|---|---|
| Code quality suite | `complexity`, `coupling`, `god_class`, `inheritance_depth`, `doc_coverage`, `recursion` | -- |
| Safety metrics | unsafe blocks, unchecked calls, assertions per function | -- |
| Port tracking | `port_status`, `port_order` | -- |
| Workflow context | `commit_context`, `pr_context`, `simplify_scan`, `test_map`, `type_hierarchy` | -- |
| Agent support | 9 agents with trait-based, per-agent config formats | Claude Code only |
| Self-upgrade | `tokensave upgrade` with stable + beta channels | `npm update` |
| Background daemon | launchd/systemd/Windows SCM, upgrade-aware | None |
| Multi-branch indexing | Per-branch DBs, cross-branch diff/search | No |
| Annotation extraction | 13 languages | No |
| Languages | 31 (3 tiers) | 19+ (single build) |
| Indexing speed | ~1.2s / 1,782 files | ~4s / 1,782 files |
| Binary size | ~25 MB | ~80 MB |
| Test coverage | 84% (v3.4.0), 1,000+ tests | Minimal |
| Atomic config writes | Yes (backup + staging + rename) | No |
| Binary releases | macOS ARM, Linux x86/ARM, Windows | npm package only |
| Token tracking | Per-call metrics, session counter, live TUI monitor, worldwide counter | No |
| MCP resources | 4 resources (status, files, overview, branches) | No |
| MCP annotations | readOnlyHint, alwaysLoad on core tools | No |

---

### 5. Semantic Search: Two Approaches

This deserves its own section because it's a genuine architectural trade-off, not a clear win for either side.

**CodeGraph (embeddings):** Runs nomic-embed-text-v1.5 (768-dim, ONNX) during indexing. Every symbol gets a vector. At query time, the question is embedded and matched by cosine similarity. Catches conceptual matches that share no lexical overlap (e.g. "authentication" finds `guardianGateway`). Cost: ~30s extra indexing per 1,000 nodes, ~50MB model download, ~200ms per query.

**tokensave (agent-driven keywords):** The `keywords` parameter on `tokensave_context` lets the LLM supply synonyms. The context builder runs FTS5 for each keyword. Zero indexing cost, zero model dependency, ~1ms per keyword. Fails when the agent can't guess the codebase's naming conventions.

In practice, the agent usually knows the right synonyms because it has already seen nearby code or the user's question provides enough context. The embedding approach has a genuine edge for completely unfamiliar codebases with unconventional naming.

---

### 6. Summary

CodeGraph pioneered the approach and remains a solid choice if you prefer npm tooling and only need Claude Code integration. Its `codegraph_explore` tool represents a genuinely better interaction pattern for Explore agents that tokensave should adopt. tokensave extends the core concept with deeper analysis, more agents, background sync, optional multi-branch support, and a native binary with no runtime dependencies. The two projects are MIT-licensed and share the same philosophy -- they just diverge on scope and implementation language.

---
---

## tokensave v4.0 vs code-review-graph

code-review-graph ([tirth8205/code-review-graph](https://github.com/tirth8205/code-review-graph)) is a Python-based code intelligence tool that parses repositories into ASTs, stores them in SQLite, and exposes 22 MCP tools. Of the four tools compared in this document, code-review-graph is the closest to tokensave in philosophy and scope -- both build a local symbol-level graph and let the AI query it via specialized MCP tools. The differences are in implementation language, analysis depth, and feature focus.

---

### 1. Architecture

Both tools follow the same pipeline: tree-sitter parsing, SQLite storage, MCP exposure. The core architectural differences:

| | **tokensave** | **code-review-graph** |
|---|---|---|
| Implementation | Rust (single binary) | Python 3.10+ |
| DB engine | libSQL (SQLite fork, WAL, async) | SQLite |
| Parsing | Bundled tree-sitter grammars (zero runtime deps) | tree-sitter via Python bindings |
| MCP server | Built into the binary (`tokensave serve`) | Separate process (`code-review-graph serve`) |
| Incremental sync | Hash + mtime/size pre-filter, PID-locked | SHA-256 hash diff |
| Background daemon | launchd/systemd/Windows Service, upgrade-aware | `watch` command (foreground file watcher) |

Both tools are local-only with no cloud dependency. code-review-graph's `watch` command is a foreground watcher; tokensave's daemon persists across sessions and survives reboots.

---

### 2. MCP Tool Comparison

**tokensave (37 tools) vs code-review-graph (22 tools):**

| Category | **tokensave** | **code-review-graph** |
|---|---|---|
| Discovery | `context`, `search`, `node`, `files`, `module_api` | `semantic_search_nodes_tool`, `query_graph_tool`, `get_docs_section_tool` |
| Impact analysis | `callers`, `callees`, `impact`, `affected` | `get_impact_radius_tool`, `get_review_context_tool`, `detect_changes_tool` |
| Quality analysis | `complexity`, `dead_code`, `god_class`, `coupling`, `simplify_scan`, `unused_imports` | `find_large_functions_tool` |
| Refactoring | `rename_preview`, `similar` | `refactor_tool`, `apply_refactor_tool` |
| Type system | `type_hierarchy`, `inheritance_depth` | -- |
| Git-aware | `commit_context`, `pr_context`, `diff_context`, `changelog` | `detect_changes_tool` (risk-scored) |
| Testing | `test_map` | (test edges in graph, no dedicated tool) |
| Structure | `circular`, `recursion`, `doc_coverage` | `list_communities_tool`, `get_community_tool`, `get_architecture_overview_tool` |
| Rankings | `rank`, `hotspots`, `largest`, `distribution` | `list_graph_stats_tool` |
| Porting | `port_status`, `port_order` | -- |
| Multi-branch | `branch_search`, `branch_diff`, `branch_list` | -- |
| Multi-repo | -- | `list_repos_tool`, `cross_repo_search_tool` |
| Execution flows | -- | `list_flows_tool`, `get_flow_tool`, `get_affected_flows_tool` |
| Documentation | -- | `generate_wiki_tool`, `get_wiki_page_tool` |
| Visualization | Removed | `code-review-graph visualize` (D3.js) |
| MCP prompts | -- | 5 templates (`review_changes`, `architecture_map`, `debug_issue`, `onboard_developer`, `pre_merge_check`) |
| Meta | `status` | `build_or_update_graph_tool`, `list_graph_stats_tool` |

The tool surfaces overlap substantially but each has areas the other lacks. tokensave goes deeper on code quality, type systems, porting, and multi-branch. code-review-graph has unique features in multi-repo search, execution flow analysis, community detection, wiki generation, and MCP prompts.

---

### 3. Where code-review-graph Is Ahead

#### Multi-repository support

code-review-graph maintains a registry of multiple repositories and exposes `cross_repo_search_tool` for searching across all of them. tokensave operates on one project at a time -- if you work across multiple repos, you need separate graph databases with no cross-repo querying.

#### Execution flow analysis

`list_flows_tool`, `get_flow_tool`, and `get_affected_flows_tool` trace execution paths through the codebase by criticality level. tokensave has call graph traversal (`callers`, `callees`, `impact`) but doesn't assemble them into named, ranked execution flows. The caveat: code-review-graph's own benchmarks show flow detection at 33% recall, reliable only for Python repos with framework patterns.

#### Community detection

The Leiden algorithm groups tightly-coupled code into "communities" -- clusters of files and symbols that change together or depend heavily on each other. `list_communities_tool` and `get_community_tool` expose these clusters. tokensave has `coupling` analysis but doesn't cluster symbols into named communities.

#### Wiki generation

`generate_wiki_tool` produces markdown documentation from the code graph, and `get_wiki_page_tool` retrieves individual pages. Uses ollama for generation. tokensave has `doc_coverage` to measure existing documentation but doesn't generate new docs.

#### MCP prompts

Five pre-built prompt templates (`review_changes`, `architecture_map`, `debug_issue`, `onboard_developer`, `pre_merge_check`) that structure common workflows. tokensave relies on prompt rules in CLAUDE.md rather than MCP-level prompt templates.

#### Apply refactoring

`apply_refactor_tool` can execute rename refactorings, not just preview them. tokensave's `rename_preview` shows what would change but doesn't write the changes -- it leaves that to the AI.

#### Risk-scored change detection

`detect_changes_tool` assigns risk scores to changes based on their blast radius. tokensave's `impact` and `affected` tools trace the blast radius but don't assign a risk score.

#### Notebook support

Jupyter and Databricks notebooks (`.ipynb`) are parsed with Python, R, and SQL cell support. tokensave does not handle notebooks.

#### Benchmarks with F1/precision/recall

code-review-graph publishes impact accuracy metrics (average F1 0.54, precision 0.38, recall 1.0) across 6 real repositories. tokensave does not publish accuracy benchmarks for its impact analysis.

---

### 4. Where tokensave Is Ahead

| Area | tokensave | code-review-graph |
|---|---|---|
| Languages | 31 (3 tiers) | 19 + notebooks |
| Language depth | Deep extractors (Nix derivation fields, Protobuf schema, COBOL, Fortran, legacy BASIC) | Standard tree-sitter extraction |
| Code quality suite | `complexity`, `coupling`, `god_class`, `inheritance_depth`, `doc_coverage`, `recursion`, `unused_imports`, `dead_code`, `simplify_scan` | `find_large_functions_tool` only |
| Type system | `type_hierarchy`, `inheritance_depth` | -- |
| Multi-branch | Optional per-branch DBs, cross-branch diff/search | -- |
| Porting tools | `port_status`, `port_order` | -- |
| Workflow context | `commit_context`, `pr_context`, `diff_context`, `changelog`, `test_map` | `detect_changes_tool` |
| Background daemon | launchd/systemd/Windows SCM, upgrade-aware | Foreground `watch` only |
| Agent support | 9 agents (different set: adds Gemini, Copilot, Cline, Roo Code; lacks Windsurf, Continue, Antigravity) | 8 platforms (adds Windsurf, Continue, Antigravity; lacks Gemini, Copilot, Cline, Roo Code) |
| Annotation extraction | 13 languages | -- |
| Token tracking | Per-call metrics, session counter, live TUI monitor, worldwide counter | -- |
| MCP resources | 4 (status, files, overview, branches) | -- |
| MCP annotations | readOnlyHint, alwaysLoad on core tools | -- |
| Self-update | `tokensave upgrade` with stable/beta channels | `pip install --upgrade` |
| Runtime deps | None (single Rust binary) | Python 3.10+, optional sentence-transformers/igraph/ollama |
| Indexing speed | ~1.2s / 1,782 files | ~2s / 2,900 files (incremental) |
| Binary size | ~25 MB | Python package + deps |
| Test coverage | 84% (v3.4.0), 1,000+ tests | pytest suite (coverage not published) |

---

### 5. Semantic Search: Three Approaches

code-review-graph offers the most flexibility here, supporting three embedding backends:

- **Local sentence-transformers** -- similar to CodeGraph's ONNX approach, runs a local model
- **Google Gemini embeddings** -- external API, requires API key
- **MiniMax embeddings** -- external API

tokensave uses agent-driven keyword expansion via FTS5 (zero model dependency, ~1ms per keyword). code-review-graph's `semantic_search_nodes_tool` uses whichever embedding backend is configured.

The trade-offs are the same as discussed in the CodeGraph section: embeddings catch conceptual matches with zero lexical overlap at the cost of indexing time and model dependency. FTS5 keywords are faster and lighter but require the agent to guess the right terms.

---

### 6. Benchmarks

code-review-graph publishes detailed benchmarks across 6 real repositories (express, fastapi, flask, gin, httpx, Next.js), reporting an 8.2x average token reduction. They also publish impact accuracy (F1, precision, recall) and performance numbers (flow detection latency, search latency).

tokensave publishes indexing speed benchmarks (1.2s for 1,782 files, 22s for 28K files) and per-call token savings via `tokensave_metrics`, but does not publish cross-repository accuracy or token reduction benchmarks in the same format.

Notable from code-review-graph's benchmarks: the tool performs worse than naive file reads on small single-file changes (express showed 0.7x, meaning the graph overhead exceeded the savings). This is an honest limitation that applies to any graph-based approach including tokensave -- for trivial changes, the graph lookup costs more than just reading the file.

---

### 7. Summary

code-review-graph is the most direct competitor to tokensave. Both build symbol-level graphs with tree-sitter, store them in SQLite, and expose them via MCP tools. The differences: tokensave has deeper code quality analysis, more languages, optional multi-branch indexing, a persistent daemon, and ships as a zero-dependency Rust binary. code-review-graph has multi-repo support, execution flow analysis, community detection, wiki generation, MCP prompts, notebook support, and published accuracy benchmarks. Both are MIT-licensed and fully open source.

---
---

## tokensave v4.0 vs OpenWolf

OpenWolf ([cytostack/openwolf](https://github.com/cytostack/openwolf)) is a Claude Code efficiency tool that takes a fundamentally different approach from tokensave, Dual-Graph, and CodeGraph. Rather than building a code graph, it wraps Claude Code's lifecycle with six hook scripts that monitor, log, and intervene during the session.

---

### 1. Architecture & Design Philosophy

**tokensave** builds a semantic knowledge graph and lets the AI query it via MCP tools. The AI actively drives exploration through structured queries.

**OpenWolf** doesn't analyze code structure at all. It operates as an invisible middleware layer that intercepts Claude Code's file reads and writes through lifecycle hooks. Before a file read, OpenWolf injects a summary and size estimate from a pre-built project index (`anatomy.md`). After a write, it updates the index and logs the action. Between sessions, it carries forward a "cerebrum" of corrections and preferences.

The two tools are complementary rather than competitive. tokensave answers "how does this codebase work?" OpenWolf answers "what has Claude already seen and what mistakes should it avoid repeating?"

---

### 2. Feature Comparison

| | **tokensave** | **OpenWolf** |
|---|---|---|
| Core mechanism | Semantic code graph queried via 37 MCP tools | 6 lifecycle hooks intercepting file reads/writes |
| Code understanding | Symbol-level (functions, call graphs, type hierarchies) | File-level (path, description, size estimate) |
| Languages | 31 with deep extraction | Language-agnostic (file-level only) |
| Token tracking | Per-call metrics, session counter, live TUI monitor | Lifetime ledger with read/write counts, hit/miss rates, repeated-read blocking |
| Redundancy prevention | Not addressed (the AI decides what to re-read) | Warns and blocks repeated file reads (~71% blocked) |
| Correction memory | No | `cerebrum.md` carries forward mistakes, preferences, and do-not-repeat rules across sessions |
| Bug history | No | `buglog.json` -- searchable history preventing re-discovery of known bugs |
| Action logging | `tokensave monitor` TUI shows tool calls | `memory.md` -- chronological log with token estimates |
| Design QC | No | Auto-captures dev server screenshots for visual review |
| Framework knowledge | No | Curated prompts for 12 UI frameworks with migration support |
| MCP tools | 37 specialized tools | 0 (hook-based, no MCP) |
| Agent support | 9 agents | Claude Code only |
| Implementation | Rust, single binary | Node.js 20+, optional PM2 and puppeteer-core |
| License | MIT | AGPL-3.0 |
| Privacy | 100% local (optional anonymous counter upload) | 100% local |

---

### 3. Where OpenWolf Is Ahead

#### Redundant read prevention

OpenWolf's most impactful feature. It tracks every file Claude reads during a session and warns (or blocks) when Claude attempts to re-read the same file. Their benchmarks claim 71% of repeated reads blocked. tokensave has no equivalent -- it doesn't intercept file reads at all, so Claude can (and does) re-read the same file multiple times in a session.

#### Correction memory across sessions

`cerebrum.md` stores mistakes Claude made, user corrections, and do-not-repeat rules. This is re-injected at the start of each new session, so Claude doesn't make the same mistake twice. tokensave has no session-to-session learning mechanism -- it provides code structure intelligence, not behavioral memory. Claude Code's own auto-memory system covers some of this ground, but OpenWolf's approach is more structured and project-scoped.

#### Bug history

`buglog.json` is a searchable record of bugs Claude has fixed. Before working on a new bug, the hooks can surface whether a similar fix was already attempted. tokensave has `changelog` and `diff_context` for git-level history, but no structured bug database.

#### Token savings magnitude

OpenWolf claims ~80% token reduction on their test project (425K tokens vs 2.5M baseline). tokensave's savings come from replacing Explore agent tool calls with graph queries, which is a different (and narrower) optimization surface. OpenWolf attacks a broader set of waste: redundant reads, oversized reads, and lack of project awareness. The two approaches are additive -- using both would address different sources of token waste.

#### File-size awareness before reads

Before Claude reads a file, OpenWolf injects the file's estimated token count from `anatomy.md`. This lets the AI make informed decisions about whether a large file is worth reading. tokensave provides token metrics after tool calls but doesn't intercept Claude's native `Read` tool to warn about file size.

#### Design QC

Detects the running dev server, captures full-page screenshots in viewport-height JPEG sections, and stores them for Claude to evaluate. Useful for frontend work where visual correctness matters. tokensave has no visual testing capability.

---

### 4. Where tokensave Is Ahead

OpenWolf has no code understanding. It knows files exist and how big they are, but it doesn't know what's inside them at a structural level. It can't answer "who calls this function?", "what breaks if I change this struct?", or "show me the type hierarchy of this trait." The entire code intelligence surface -- call graphs, impact analysis, dead code detection, complexity metrics, test mapping, rename preview, type hierarchies, circular dependency detection -- is absent.

| Area | tokensave | OpenWolf |
|---|---|---|
| Semantic code graph | Yes (41K+ nodes, 88K+ edges) | No |
| Call graph traversal | `callers`, `callees`, `impact`, `affected` | No |
| Quality analysis | `complexity`, `dead_code`, `god_class`, `coupling` | No |
| Refactoring support | `rename_preview`, `similar` | No |
| Git-aware context | `commit_context`, `pr_context`, `diff_context` | No |
| Multi-branch indexing | Optional per-branch DBs with cross-branch diff | No |
| Language-specific extraction | 31 languages with deep tree-sitter parsing | Language-agnostic file listing |
| MCP tools | 37 | 0 |
| Agent support | 9 agents | Claude Code only |
| Background daemon | launchd/systemd/Windows SCM | PM2 (optional) |
| Dependencies | None (single Rust binary) | Node.js 20+, optional PM2, optional puppeteer-core |

---

### 5. Complementarity

These tools solve different problems and could run side by side. tokensave replaces expensive Explore agent scans with instant graph queries. OpenWolf reduces waste from redundant reads, carries forward corrections, and blocks known-bad patterns. A combined setup where tokensave provides code intelligence and OpenWolf provides behavioral guardrails would address both sources of token waste.

The main friction point would be hook conflicts -- both tools register Claude Code hooks. tokensave uses a PreToolUse hook to redirect Explore agents; OpenWolf uses hooks for file read/write interception. These operate on different events and should not conflict in practice.

---

### 6. Summary

OpenWolf is a "behavioral efficiency layer" -- it makes Claude Code less wasteful by preventing redundant reads, carrying forward corrections, and providing file-size awareness. tokensave is a "code intelligence platform" -- it makes Claude Code smarter by giving it structured access to the codebase's symbols, relationships, and dependencies. OpenWolf's correction memory and redundant-read blocking are features tokensave genuinely lacks. tokensave's entire code analysis suite is something OpenWolf doesn't attempt. The AGPL-3.0 license is worth noting -- it's more restrictive than tokensave's MIT, requiring derivative works to be open-sourced.

---
---

## Possible Improvements

Features from Dual-Graph, CodeGraph, OpenWolf, and code-review-graph that could be ported to tokensave, ordered by estimated value.

### 1. `tokensave_explore` tool -- from CodeGraph (very high value)

The single highest-impact feature across both competitors. A unified MCP tool that accepts a natural language question and returns full source code sections for all relevant symbols in one call. Internally combines FTS5 search, graph traversal, and source extraction. The key innovations are: a call budget encoded in the tool description that scales with project size (the model reads it and self-limits), a `seen` parameter for session deduplication across calls, and prompt routing rules that direct Explore agents to this tool while the main session uses `tokensave_context`. tokensave already has the query logic via `tokensave_context` with `include_code: true` -- the gap is output format, budget signalling, and agent routing.

### 2. Dollar-amount cost dashboard -- from Dual-Graph (medium value)

`tokensave monitor` shows raw tool calls and token deltas in a TUI. Dual-Graph's localhost:8899 web dashboard converts tokens to estimated dollar amounts using Claude's pricing. Adding a `--dollars` or `--pricing` flag to `tokensave monitor` (or a companion web dashboard) that maps token counts to cost estimates would make the savings more tangible for non-technical stakeholders and teams tracking AI spend.

### 3. Pre-read token estimation tool -- from Dual-Graph (medium value)

A `tokensave_estimate_tokens` MCP tool that accepts a file path or symbol name and returns the approximate token count without actually reading the content. Lets the AI make budget-aware decisions ("this file is 8,000 tokens, read the smaller one instead"). Could be implemented cheaply using the `source_bytes` already stored in the nodes table and a bytes-to-tokens heuristic.

### 4. Svelte language support -- from CodeGraph (medium value)

CodeGraph supports `.svelte` files. The `tree-sitter-svelte` grammar is maintained and the extraction patterns (script blocks containing TypeScript/JavaScript) are well-understood. Relevant for any project in the SvelteKit ecosystem.

### 5. Per-turn read budget controls -- from Dual-Graph (low-medium value)

Dual-Graph's `DG_HARD_MAX_READ_CHARS` and `DG_TURN_READ_BUDGET_CHARS` environment variables cap how much context is injected per turn. tokensave trusts the AI to self-regulate, which works well with capable models but offers no hard guardrail. A per-project config option (e.g. `max_context_tokens_per_call`) could cap the output size of tools like `tokensave_context` and `tokensave_explore`, giving users a safety valve for cost control.

### 6. Embedded file watcher in `tokensave serve` -- from CodeGraph (low-medium value)

CodeGraph's MCP server watches for file changes using native OS events, debounced to a 2-second quiet window. tokensave's daemon is more capable but requires separate setup. Users who run `tokensave serve` without the daemon get no auto-sync. A minimal fix: have `tokensave serve` detect whether the daemon is running and either warn or embed a lightweight fallback watcher inline.

### 7. Redundant-read detection in hooks -- from OpenWolf (medium value)

OpenWolf's most impactful feature: it tracks every file Claude reads during a session and warns or blocks re-reads (~71% of repeated reads blocked). tokensave already registers a PreToolUse hook that intercepts Agent calls. Extending the hook (or adding a second hook on `Read` tool calls) to maintain a session read log and warn on duplicates would address a real source of token waste that the graph alone doesn't solve. The hook could surface a "you already read this file N tokens ago" message, letting Claude skip the re-read.

### 8. Structured bug/correction memory -- from OpenWolf (low-medium value)

OpenWolf's `cerebrum.md` carries forward mistakes and do-not-repeat rules across sessions, and `buglog.json` provides a searchable bug history. Claude Code's built-in auto-memory partially covers this, but a project-scoped, structured correction log that tokensave injects into the MCP server instructions (or exposes as an MCP resource) could improve cross-session consistency. The risk is scope creep -- this is behavioral memory, not code intelligence, and it's worth considering whether tokensave should own this or leave it to the AI client.

### 9. File-size awareness on MCP tool output -- from OpenWolf (low-medium value)

OpenWolf injects file size estimates before Claude reads a file. tokensave already stores `source_bytes` per file node. Surfacing estimated token counts alongside file paths in tools like `tokensave_context`, `tokensave_files`, and `tokensave_search` (e.g. appending `(~1,200 tokens)` next to each file path) would help the AI make budget-aware decisions without a separate estimation tool.

### 10. `trackCallSites` / `extractDocstrings` config toggles -- from CodeGraph (low value)

Per-project booleans to disable call site extraction or docstring extraction. On very large codebases, disabling call sites could meaningfully reduce initial index time and DB size. Maps naturally to tokensave's existing per-project config.

### 11. Multi-repository registry and cross-repo search -- from code-review-graph (medium value)

code-review-graph maintains a registry of multiple repositories and exposes `cross_repo_search_tool` for searching symbols across all of them. tokensave operates on one project at a time. For teams working across multiple microservices or monorepo-adjacent setups, a `tokensave register` / `tokensave repos` mechanism that lets the MCP server query multiple project databases would be valuable. The daemon already tracks multiple projects for file watching -- extending this to cross-project queries is a natural progression.

### 12. Execution flow analysis -- from code-review-graph (low-medium value)

`list_flows_tool`, `get_flow_tool`, and `get_affected_flows_tool` trace execution paths through the codebase and rank them by criticality. tokensave has the underlying call graph data (`callers`, `callees`, `impact`) but doesn't assemble these into named, ranked execution flows. Worth noting: code-review-graph's own benchmarks show flow detection at 33% recall, reliable only for Python repos with framework patterns. An implementation in tokensave should be scoped carefully to languages where entry points are well-defined.

### 13. Community detection -- from code-review-graph (low-medium value)

The Leiden algorithm groups tightly-coupled code into clusters of files and symbols that depend heavily on each other. tokensave has `coupling` analysis that identifies high-coupling pairs, but doesn't cluster them into named communities. A `tokensave_communities` tool that runs a clustering algorithm over the edge graph and returns named groups would help with architecture understanding on large unfamiliar codebases.

### 14. MCP prompt templates -- from code-review-graph (low-medium value)

Five pre-built MCP prompts (`review_changes`, `architecture_map`, `debug_issue`, `onboard_developer`, `pre_merge_check`) that structure common workflows. tokensave relies on CLAUDE.md prompt rules. MCP prompts are a standard part of the protocol and could provide structured entry points for common tasks without requiring prompt rule injection.

### 15. Risk-scored change detection -- from code-review-graph (low value)

`detect_changes_tool` assigns a risk score to changes based on blast radius. tokensave's `impact` and `affected` trace the blast radius but don't assign a numeric risk score. Adding a risk heuristic (based on number of affected symbols, test coverage of affected area, complexity of changed code) to `tokensave_diff_context` or a new `tokensave_risk` tool would be straightforward given the existing data.

### 16. Notebook support -- from code-review-graph (low value)

Jupyter and Databricks notebooks (`.ipynb`) parsed with Python, R, and SQL cell support. Relevant for data science projects. Would require a notebook-aware pre-processor that extracts code cells before passing them to existing language extractors.

### 17. Published accuracy benchmarks -- from code-review-graph (low value, high credibility)

code-review-graph publishes F1/precision/recall metrics for impact analysis across 6 real repositories. tokensave publishes indexing speed and per-call token savings but no accuracy benchmarks. Running a similar evaluation suite against tokensave's `impact` and `affected` tools and publishing results would strengthen credibility and identify areas where the blast radius analysis over-predicts or under-predicts.

### 18. `tokensave uninit` command -- from CodeGraph (low value)

Deletes the `.tokensave/` directory cleanly with an optional `--force` to skip confirmation. Occasionally useful during troubleshooting. Trivial to implement.
