# MCP Tools

The tools and resources the tokensave MCP server exposes to agents, grouped by category.

The server exposes more than 80 tools (one fewer when the optional `ast-grep` binary is not on `PATH`). Since 7.14.0 only the 11 core tools plus `tokensave_more` are listed by default; the rest are listed on demand through `tokensave_more`, stay callable by name, and are all listed with `"tools": "full"`. The tables below group the most commonly used ones by category. Most are read-only, safe to call in parallel, and annotated with `readOnlyHint`. The edit primitives are scoped to single files and re-index in place; session baseline and memory-recording tools also mutate local `.tokensave` state and are annotated as non-read-only. Five core tools (`tokensave_context`, `tokensave_search`, `tokensave_status`, `tokensave_callees` and `tokensave_impact`) are marked `anthropic/alwaysLoad` so they bypass the client's tool-search round-trip.

## Listing fewer tools

**Only the core tools are listed by default.** An MCP client sends the schema of every listed tool on every turn, before any tool is called. The full tokensave surface is about 97 KB of schema, which on a small-context model can be more than half of the window (#576). So `tools/list` sends only the core tools: `context`, `search`, `status`, `read`, `body`, `files`, `callers`, `callees`, `impact`, `str_replace` and `multi_str_replace` (about 22 KB with `tokensave_more`). These sizes are measured in the [MCP footprint baseline](../MCP-FOOTPRINT-BASELINE.md), and a test keeps the default list and instructions within a recorded byte budget. The `initialize` instructions name these tools and the areas `tokensave_more` can list, so a client that defers tool schemas still knows the rest exist. The setting selects what the server lists, not what it can run: a tool that is not listed still answers a call by name, so agent permission lists and hooks keep working. To list every tool, set `"tools": "full"` in `.tokensave/config.json`, or `TOKENSAVE_TOOLS=full` in the environment of the MCP server; the environment variable wins over the file. A `"tools": "full"` written by 7.13.0, which wrote it into every config it saved, is read as the old default; set it again after upgrading to keep the full list.

With the core toolset the server also lists `tokensave_more`. A call with an `area` (`analysis`, `edit`, `git`, `memory`, `navigate` or `all`) lists the tools of that area for the rest of the session, and the server sends `notifications/tools/list_changed` so the client fetches the list again. A session pays only for the areas it uses: the `git` area adds about 6 KB. A client that ignores `list_changed` keeps the core list. `tokensave install` grants `tokensave_more` along with every other tool, so it does not prompt; an install from before this release prompts once, until `tokensave reinstall` or the upgrade resync refreshes the permission list.

To keep a whole area out of every list, name it in `disabled_tool_areas` in `~/.tokensave/config.toml`, for example `disabled_tool_areas = ["memory"]` when another MCP server already keeps session memory. The values are the area names: `analysis`, `edit`, `git`, `memory` and `navigate`. The server then leaves the tools of that area out of `tools/list` with the core toolset and with `"tools": "full"`, and `tokensave_more` and the `initialize` instructions do not offer the area. When every area is named, `tokensave_more` is not listed. The core tools are in no area and are always listed. Unlike the toolset, this setting also stops the tools: a call by name to a tool of such an area does not run, and the answer says that the tool is disabled. A call of `tokensave_more` for such an area gets the same answer. The `tokensave tool` command line is not affected. Tool results, tool descriptions, the server instructions and the `tokensave://schema` resource do not send the agent to a tool of such an area. The server reads the setting when it starts and prints a warning on stderr for a name that is not an area. `tokensave doctor` shows each area that is off, with its tools.

## Which project a call answers

When `tokensave serve` starts, it picks a default project:

1. With `--path <dir>`, the project at that directory, or none if it has no index.
2. Otherwise, the project that contains the working directory (the nearest `.tokensave/` above it). A git repository without an index is initialized first when [`auto_init`](CLI-REFERENCE.md#settings-for-automatic-initialization) allows it.
3. Failing that, the index of the main checkout when the working directory is a linked worktree. Next comes a registered project (the only one, or the one that contains or sits under the working directory). Last is a `roots` list that some clients put in their `initialize` request.
4. If nothing matches, the server starts with **no default project** and logs `serving with no default project`.

Each tool call then answers from the first of these that applies:

1. **`graph_root`**, when the call passes it. An explicit `graph_root` always wins. See [Query another initialized project](#query-another-initialized-project).
2. **The default project.** A server that has one never looks at the client's roots.
3. **The client's roots**, when there is no default project and the client declared the MCP `roots` capability in `initialize`. During the call the server sends `roots/list` to the client. A root can be a project root or any folder inside one, and the server answers from the indexed project that contains the roots. The call then behaves as if it had passed that project as `graph_root`: the graph is opened read-only, node IDs are graph-qualified, no savings are recorded, and only tools that take `graph_root` can answer this way. The call is refused, with the reason, when the roots lie in no indexed project or in several, or when the client reports no roots. Pass `graph_root` in those cases.
4. **Otherwise** the call is refused with the list of registered projects to pass as `graph_root`. `tokensave_status` answers with the same explanation instead of refusing.

### Behind an MCP proxy or gateway

An MCP proxy (aggregator or gateway), such as [MCPProxy](https://github.com/smart-mcp-proxy/mcpproxy-go), can run one tokensave process for every client: several Claude Code sessions in different repositories, an editor, and so on. They share one MCP session, so the server cannot have a single right default project. Instead, start it with no default project and let each call's roots choose:

- Configure the proxy to run `tokensave serve` without `--path`, from a folder that is in no indexed project. The proxy's install folder is a common choice.
- The proxy must declare the `roots` capability to tokensave and forward a `roots/list` request sent during a tool call to the client that made that call. tokensave asks again on every call and caches nothing, so the proxy does not need to forward `notifications/roots/list_changed`. tokensave never asks outside a tool call, where a proxy has no caller to forward to.

Limits:

- A call whose caller has no roots is refused unless it passes `graph_root`. This covers REST or CLI calls through the proxy, a client without the `roots` capability, and any call the proxy cannot attribute to a client (MCPProxy cannot when several calls to the same stdio server are in flight). The proxy answers such calls with an empty list. tokensave waits up to 5 seconds for an answer and then refuses the call the same way.
- A call answered from the roots reads its project like a `graph_root` call. The [on-demand staleness check and catch-up sync](INDEXING.md#index-freshness) run only for a default project, so keep these projects fresh with the git hooks that `tokensave install` offers, or with `tokensave sync`.
- Tools without a `graph_root` parameter, such as the edit, git and session-memory tools, need a default project.
- If only one project is registered, `serve` started outside every project uses it as the default project (step 3 above), and roots are not consulted.

A client that launches tokensave inside its own project is unaffected: that project is the default, and the server never sends `roots/list`.

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

`tokensave_search` and `tokensave_files` also accept an array of roots to
federate one request across several graphs. A multi-root array may include the
project already served by the MCP server; that project participates as a
read-only member of the federation. Unusable roots are reported as skipped
when at least one root succeeds, and worktrees of the same repository are
collapsed with the omitted paths reported. A scalar root or one-element array
that names the served project remains invalid; omit `graph_root` for that
single-project call.

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
| `tokensave_vuln_reachability` | Report cached OSV advisories and indexed call evidence. Set `TOKENSAVE_OSV_ONLINE=1` in the server environment to allow refreshes. |
| `tokensave_status` | Index status, statistics, tokens saved |

Call `tokensave_context` with a non-empty `task` string, for example `{"task":"Find authentication tests"}`. The `query` alias also works. When both are supplied, their values must match.

### Call Graph & Impact

| Tool | Purpose |
|------|---------|
| `tokensave_callers` | Find every extracted call sharing a function's trailing name, labelled by what resolution established |
| `tokensave_callees` | Find what a function calls |
| `tokensave_impact` | See what's affected by changing a symbol |
| `tokensave_call_chain` | Shortest path between two symbols, named by symbol (`from`, `to`) or node ID. Default: a directed call chain. `follow: "any"`: the shortest connection over every edge kind in either direction |
| `tokensave_affected` | Find test files affected by source changes |
| `tokensave_rename` | Graph-based rename: every site with a confidence class (`exact`, `heuristic`, `ambiguous`, `text_only`), a diff preview, and an all-or-nothing apply. Not binding-aware |
| `tokensave_hotspots` | Most connected symbols (highest call count) |

Symbol-based callers and impact lookups can show fewer rows per match when the response reaches its size limit. Each shortened match reports the displayed `count`, the original `total`, and `truncated: true`.

**Edge provenance.** `tokensave_callers`, `tokensave_callees` and `tokensave_call_chain` show how the resolver bound each edge they return. `resolved_by` names the resolver path, for example `exact-match`, `qualified-match` or `simple-name-match`. `resolved_exact` is `false` when the resolver picked the target by name alone, so a symbol of the same name elsewhere could be the real target; read the call site before you rely on such an edge. When one function reaches a target from several call sites, the strongest provenance is reported. Both fields are absent for an edge that an extractor read directly from the syntax tree. `tokensave_impact` reports `heuristic_edge_count`: the number of edges in the radius with `resolved_exact: false`.

**Complete caller evidence.** Direct `tokensave_callers` results do not stop at resolved edges. For every extracted call whose trailing name matches the requested symbol, the containing caller has a `references` entry labelled `confirmed`, `other`, `ambiguous`, or `unresolved`. `other` names the same-named definition the resolver selected. An ambiguous entry reports its candidate count and whether the requested definition is among them. Ambiguous and unresolved entries carry `to_read: true`; their receiver text is included when the reference had one. The tool keeps the existing caller grouping, graph depth, dispatch and provenance fields. `limit` also bounds the reference evidence; a cut reports `reference_count`, `reference_total`, and `references_truncated` instead of silently omitting sites.

**Paths between two symbols.** `tokensave_call_chain` answers "how does A reach B?". Pass `from` and `to` as symbol names, or `from_id` and `to_id` as node IDs; an ID wins over a name. A name must match one symbol. When it matches several, or none, the tool does not pick one: it returns `found: false` and lists the candidates under `unresolved`, and you call again with the node ID of one of them. With `follow: "calls"` (the default) the path follows outgoing `calls` edges only, so it is a chain of execution. With `follow: "any"` the path follows every stored edge kind in either direction, which shows how two symbols are connected when neither calls the other; each step then reports `edge_direction` (`forward`: from the previous step to this one, `reverse`: the opposite). `max_depth` limits the path length (default 8, maximum 20). A `follow: "any"` search runs from both ends and meets in the middle, so a well-connected node near one end costs little. It also stops after it has read the edges of a fixed number of nodes. The result then has `budget_exhausted: true` and a message that gives that number; this means "not found within the budget" and not "not connected".

### Code Quality

| Tool | Purpose |
|------|---------|
| `tokensave_complexity` | Rank functions by cyclomatic & cognitive complexity, nesting depth, Halstead metrics, maintainability index, CRAP, and safety metrics |
| `tokensave_dead_code` | Find unreachable symbols (no incoming edges; symbols named as an ambiguity candidate are excluded) |
| `tokensave_ambiguous_calls` | Call sites the resolver could not pin to one target, with every tied candidate |
| `tokensave_god_class` | Find classes/structs with too many members (methods + fields; supports optional `kinds` and `exclude_data_structs`) |
| `tokensave_coupling` | Rank files by fan-in/fan-out |
| `tokensave_inheritance_depth` | Find the deepest inheritance hierarchies |
| `tokensave_circular` | Detect circular file dependencies |
| `tokensave_imports` | Module-level import dependencies, cycles, and cut simulation |
| `tokensave_recursion` | Detect recursive/mutually-recursive call cycles |
| `tokensave_unused_imports` | Import statements never referenced |
| `tokensave_doc_coverage` | Public code symbols missing documentation (excludes non-code docs/config extensions such as `.md`, `.txt`, `.json`, `.yaml`, `.toml`, `.canvas`) |
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

The acyclicity score is one minus the fraction of dependency edges whose endpoints
belong to the same strongly connected component of two or more files. One-way
links between separate cycles contribute only to the total edge count.

### Sessions

Snapshot health metrics at the start of an AI coding session, then diff at the end to see what improved or regressed.

| Tool | Purpose |
|------|---------|
| `tokensave_session_start` | Save current health metrics as a JSON baseline for later comparison |
| `tokensave_session_end` | Recompute and diff against the baseline -- per-dimension deltas, pass/fail, automatic cleanup |

### Edit Primitives

Writer tools that let agents modify files without regex or shell-quoting hazards. Each is single-file, anchored, and triggers an in-place re-index after writing so the graph never goes stale. The four below are the most common; `tokensave_create_file` is described after the table.

| Tool | Purpose |
|------|---------|
| `tokensave_str_replace` | Replace a unique `old_str` with `new_str`; fails if 0 or >1 matches (protects against multi-edit bugs) |
| `tokensave_multi_str_replace` | Apply N `(old, new)` replacements atomically -- all-or-nothing transaction |
| `tokensave_insert_at` | Insert content before or after a unique anchor string or line number |
| `tokensave_ast_grep_rewrite` | Structural code rewrite via the `ast-grep` CLI in `--rewrite` mode |

**Where the edit tools write.** Every edit tool (the ones above, `tokensave_replace_lines`, the symbol edits `tokensave_replace_symbol`, `tokensave_insert_at_symbol` and `tokensave_delete_symbol`, and `tokensave_rename`) resolves its target and refuses it unless it lies inside one of these:

- the indexed project root, or
- another git worktree of the same repository, as `git worktree list` reports it from the project root. This is how an agent working in a worktree edits its own checkout: pass an absolute path there, or a relative `path` with `project_root` (alias `cwd`) set to the worktree.

Symlinks and `..` are resolved before the check, so neither can point an edit outside those directories. There is one exception. The indexer follows a symlink inside the root into a directory outside it, so a file the index tracks under such a link (a plain path with no `..`) stays editable through it. Paths inside `.git` and tokensave's own state directory are refused as well. A refused edit fails with a `refusing to edit` error and leaves the file untouched. A rename that would touch a refused file is blocked as a whole. The check and the write are not atomic, so a symlink swapped in between them is followed.

**Read arguments.** `tokensave_read` accepts `file` (alias `path`), `mode`, `lines`, `start`, `end`, `if_digest`, `force`, `format`, and the graph selectors `graph_root`/`graph_branch`. For a 1-based inclusive range, pass `lines: "40-75"` or `start: 40, end: 75`; omitting `end` reads to the end of the file. A range infers `mode: "lines"` unless a mode is explicit. Unknown parameters are rejected by name, including `start_line`/`end_line` and `offset`/`limit`; they are not range aliases.

**What `tokensave_read` reads.** `tokensave_read` reads any file inside the project root, whether the index tracks it or not. That includes files matched by `.gitignore` or by the `exclude` list, such as `vendor/`. The index-backed tools (search, context, symbol, graph and file-listing tools) never return those files, because they are never indexed. `tokensave_read` refuses paths inside `.git`, since `.git/config` can hold credentials, and paths that resolve outside the root (#636, #866).

**Repository content is data.** Source, comments and documentation in tool results come from the repository, and anyone who can commit to it controls them. The `initialize` instructions and the rules file `tokensave install` writes tell the agent to treat that content as data and never follow instructions that appear inside it (#866). See [SECURITY.md](../../SECURITY.md#indexed-content-is-data-not-instructions) for what that guidance can and cannot prevent.

`tokensave_create_file` (`path`, `content`, optional `overwrite`) creates a new file and indexes it at once, so it is searchable right after the call. It is in the `edit` area, so it is listed after `tokensave_more` with `edit` or `all`, and is not marked read-only. Its path rules are stricter than the other edit tools', which also accept another worktree of the repository:

- The file must land inside the indexed project root. An absolute path elsewhere, any `..` component, and a path through a symlinked directory that leaves the root are refused. Symlinks are resolved on the nearest existing directory above the file.
- Paths inside `.git` and tokensave's own state directory are refused.
- An existing file is replaced only with `overwrite: true`. A directory or a symlink is never replaced. `project_root`/`cwd` retargeting is not accepted.
- `content` is limited to the project's `max_file_size` (1 MiB by default).
- The checks and the write are not atomic. A symlink created in a parent directory between them is followed; without `overwrite` the write still never replaces an existing file.

The result reports the project-relative `file`, `resolved_path`, `bytes`, `overwritten`, `digest`, and `indexed`. `indexed` is false for a file kind tokensave does not index.

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

### Imported knowledge (OKF)

| Tool | Purpose |
|------|---------|
| `tokensave_knowledge` | Search and read [Open Knowledge Format](OKF.md) bundles imported with `tokensave okf import`; lists the project files each concept refers to and whether they are indexed, and each concept's status, trust tier and freshness; `current_only` follows deprecated concepts to their replacements |

It is read-only, is listed in the `memory` area of `tokensave_more`, and
returns only a hint until a bundle is imported. See
[Using imported knowledge](OKF.md#using-imported-knowledge).

## MCP resources

Four resources are exposed via `resources/list` and `resources/read`:

- `tokensave://status` -- graph statistics as JSON
- `tokensave://files` -- indexed file tree grouped by directory
- `tokensave://overview` -- project summary with language distribution and symbol kinds
- `tokensave://branches` -- tracked branches with DB sizes and parent info
