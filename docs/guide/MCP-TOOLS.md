# MCP Tools

The tools and resources the tokensave MCP server exposes to agents, grouped by category.

The server exposes more than 80 tools (one fewer when the optional `ast-grep` binary is not on `PATH`). Since 7.14.0 only the 11 core tools plus `tokensave_more` are listed by default; the rest are listed on demand through `tokensave_more`, stay callable by name, and are all listed with `"tools": "full"`. The tables below group the most commonly used ones by category. Most are read-only, safe to call in parallel, and annotated with `readOnlyHint`. The edit primitives are scoped to single files and re-index in place; session baseline and memory-recording tools also mutate local `.tokensave` state and are annotated as non-read-only. The three core tools (`tokensave_context`, `tokensave_search`, `tokensave_status`) are marked `anthropic/alwaysLoad` so they bypass the client's tool-search round-trip.

## Listing fewer tools

**Only the core tools are listed by default.** An MCP client sends the schema of every listed tool on every turn, before any tool is called. The full tokensave surface is about 89 KB of schema, which on a small-context model can be more than half of the window (#576). So `tools/list` sends only the core tools: `context`, `search`, `status`, `read`, `body`, `files`, `callers`, `callees`, `impact`, `str_replace` and `multi_str_replace` (about 17 KB with `tokensave_more`). The `initialize` instructions name these tools and the areas `tokensave_more` can list, so a client that defers tool schemas still knows the rest exist. The setting selects what the server lists, not what it can run: a tool that is not listed still answers a call by name, so agent permission lists and hooks keep working. To list every tool, set `"tools": "full"` in `.tokensave/config.json`, or `TOKENSAVE_TOOLS=full` in the environment of the MCP server; the environment variable wins over the file. A `"tools": "full"` written by 7.13.0, which wrote it into every config it saved, is read as the old default; set it again after upgrading to keep the full list.

With the core toolset the server also lists `tokensave_more`. A call with an `area` (`analysis`, `edit`, `git`, `memory`, `navigate` or `all`) lists the tools of that area for the rest of the session, and the server sends `notifications/tools/list_changed` so the client fetches the list again. A session pays only for the areas it uses: the `git` area adds about 6 KB. A client that ignores `list_changed` keeps the core list. `tokensave install` grants `tokensave_more` along with every other tool, so it does not prompt; an install from before this release prompts once, until `tokensave reinstall` or the upgrade resync refreshes the permission list.

## Query another initialized project

Semantic read tools can query an explicitly selected local graph without
restarting the MCP server:

```json
{
  "query": "screenGate",
  "graph_root": "/absolute/path/to/typewhisper"
}
```

Selected results include canonical root/branch provenance. Node IDs are
namespaced to that graph, and the matching selectors must be repeated on
follow-up calls. For example, a follow-up to a branch-selected query includes
both values:

```json
{
  "node_id": "graph:<fingerprint>:function:<raw-id>",
  "graph_root": "/absolute/path/to/typewhisper",
  "graph_branch": "feature/auth"
}
```

`graph_root` must be the exact absolute root of an already initialized project.
`graph_branch` is optional and, when supplied, must name a tracked branch.
Selected opens are read-only: they never initialize, sync, migrate, auto-track,
or write graph/source data. They also do not contribute to savings accounting.
Calls without selectors behave exactly as before.

`graph_root` is only useful if you know the other project exists, so the server
tells you: initialized projects sitting directly beside the served root are
named in the MCP `instructions`, in `tokensave_status`, and in empty
`tokensave_search` / `tokensave_context` results — the point at which a session
would otherwise conclude a symbol does not exist rather than look next door
(#375). Only immediate siblings are offered, at most five, and nothing is opened
or indexed on their behalf; querying one still requires an explicit `graph_root`.

Selectors are intentionally unavailable on tools that write, shell out, or
depend on the current checkout: the edit primitives, VCS and branch tools,
diagnostics and test execution, dependency and runtime introspection, workflow
and session-memory tools, and server administration. Those tools reject a
selector instead of silently ignoring it.

`tokensave_simplify_scan` and `tokensave_redundancy` accept selectors. On a
selected graph, `tokensave_redundancy` reads function bodies from the selected
project's working tree and does not write its fingerprint cache, so it refuses
unless the selected branch is the one checked out there (not a detached HEAD,
and not a fallback to a parent branch's index).

## Tools by category

### Discovery

| Tool | Purpose |
|------|---------|
| `tokensave_context` | Get relevant code context for a task -- entry points, related symbols, code snippets |
| `tokensave_search` | Find symbols by name (functions, classes, types) |
| `tokensave_node` | Get details + source code for a specific symbol |
| `tokensave_files` | List indexed project files (source and tracked artifacts) with filtering |
| `tokensave_module_api` | Public API surface of a file or directory |
| `tokensave_similar` | Find symbols with similar names |
| `tokensave_annotations` | Attribute/annotation/decorator introspection -- histogram of all annotations or per-site listings with target filters |
| `tokensave_doc` | Companion Markdown documentation for a source file -- doc content, the files it covers, and a staleness signal |
| `tokensave_dependencies` | Package-manifest introspection across 17 ecosystems -- workspace summary, per-package lookup, license surface, version drift |
| `tokensave_status` | Index status, statistics, tokens saved |

### Call Graph & Impact

| Tool | Purpose |
|------|---------|
| `tokensave_callers` | Find what calls a function |
| `tokensave_callees` | Find what a function calls |
| `tokensave_impact` | See what's affected by changing a symbol |
| `tokensave_affected` | Find test files affected by source changes |
| `tokensave_rename` | Graph-based rename: every site with a confidence class (`exact`, `heuristic`, `ambiguous`, `text_only`), a diff preview, and an all-or-nothing apply. Not binding-aware |
| `tokensave_hotspots` | Most connected symbols (highest call count) |

### Code Quality

| Tool | Purpose |
|------|---------|
| `tokensave_complexity` | Rank functions by cyclomatic & cognitive complexity, nesting depth, Halstead metrics, maintainability index, CRAP, and safety metrics |
| `tokensave_dead_code` | Find unreachable symbols (no incoming edges; symbols named as an ambiguity candidate are excluded) |
| `tokensave_ambiguous_calls` | Call sites the resolver could not pin to one target, with every tied candidate |
| `tokensave_god_class` | Find classes with too many members |
| `tokensave_coupling` | Rank files by fan-in/fan-out |
| `tokensave_inheritance_depth` | Find the deepest inheritance hierarchies |
| `tokensave_circular` | Detect circular file dependencies |
| `tokensave_imports` | Module-level import dependencies, cycles, and cut simulation |
| `tokensave_recursion` | Detect recursive/mutually-recursive call cycles |
| `tokensave_unused_imports` | Import statements never referenced |
| `tokensave_doc_coverage` | Public symbols missing documentation |
| `tokensave_simplify_scan` | Quality analysis of changed files (duplications, dead code, complexity) |

### Code-Health Analytics

Five tools surface structural quality signals from the existing graph. The composite score uses a geometric mean over independent dimensions so no single one can be gamed.

| Tool | Purpose |
|------|---------|
| `tokensave_health` | Composite quality signal (0-10000) from acyclicity, depth, equality, redundancy, and modularity |
| `tokensave_gini` | Gini inequality coefficient for any metric (complexity, lines, fan-in/out, members) -- finds god files and uneven distribution |
| `tokensave_dependency_depth` | Longest file-level dependency chains (Lakos levelization) with full chain reconstruction after Tarjan SCC cycle-breaking |
| `tokensave_dsm` | Design Structure Matrix in `stats`, `clusters`, or `matrix` form -- reveals layering violations and hidden coupling |
| `tokensave_test_risk` | Risk-weighted test-gap analysis combining complexity, fan-in, coverage, and 90-day git churn into a single score |

### Sessions

Snapshot health metrics at the start of an AI coding session, then diff at the end to see what improved or regressed.

| Tool | Purpose |
|------|---------|
| `tokensave_session_start` | Save current health metrics as a JSON baseline for later comparison |
| `tokensave_session_end` | Recompute and diff against the baseline -- per-dimension deltas, pass/fail, automatic cleanup |

### Edit Primitives

Four writer tools that let agents modify files without regex or shell-quoting hazards. Each is single-file, anchored, and triggers an in-place re-index after writing so the graph never goes stale.

| Tool | Purpose |
|------|---------|
| `tokensave_str_replace` | Replace a unique `old_str` with `new_str`; fails if 0 or >1 matches (protects against multi-edit bugs) |
| `tokensave_multi_str_replace` | Apply N `(old, new)` replacements atomically -- all-or-nothing transaction |
| `tokensave_insert_at` | Insert content before or after a unique anchor string or line number |
| `tokensave_ast_grep_rewrite` | Structural code rewrite via the `ast-grep` CLI in `--rewrite` mode |

### Git & Workflow

| Tool | Purpose |
|------|---------|
| `tokensave_diff_context` | Semantic context for changed files -- modified symbols, dependencies, affected tests |
| `tokensave_commit_context` | Semantic summary of uncommitted changes for commit message drafting |
| `tokensave_pr_context` | Semantic diff between git refs for pull request descriptions |
| `tokensave_changelog` | Semantic diff between two git refs |
| `tokensave_test_map` | Source-to-test mapping at the symbol level, with uncovered symbol detection |
| `tokensave_test_coverage` | Per-file/symbol/test-fn coverage rollup with transitive call-edge expansion |

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
| `tokensave_branch_diff` | Compare symbols between branches; `quality: true` compares structural quality |
| `tokensave_branch_list` | List tracked branches with DB sizes and sync times |

See [Multi-Branch Indexing](MULTI-BRANCH.md) for branch tracking and
[branch quality regressions](MULTI-BRANCH.md#branch-quality-regressions).

### Cross-session memory

Three MCP tools persist decisions and code-area context across sessions, stored in the per-project `.tokensave/tokensave.db`.

| Tool | Purpose |
|------|---------|
| `tokensave_record_decision` | Save a design/architecture decision with optional reason, files, and tags |
| `tokensave_record_code_area` | Mark a path the agent has worked in (touch counter + last_touched_at) |
| `tokensave_session_recall` | FTS5 query over saved decisions; pair with the two write tools |

Use these so the agent doesn't have to re-explain architecture choices session-to-session.

## MCP resources

Four resources are exposed via `resources/list` and `resources/read`:

- `tokensave://status` -- graph statistics as JSON
- `tokensave://files` -- indexed file tree grouped by directory
- `tokensave://overview` -- project summary with language distribution and symbol kinds
- `tokensave://branches` -- tracked branches with DB sizes and parent info
