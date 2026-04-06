# MCP Tool Test Queries

Manual test queries for verifying all 37 tokensave MCP tools. Run these in a Claude Code session after `tokensave sync` and `tokensave install`.

### Staleness warnings

All tool responses may be prepended with staleness warnings when the index is out of date:

- **Per-file**: `WARNING: STALE INDEX — N file(s) modified since last sync: file1.rs, file2.rs. Run tokensave sync to update.`
- **Index age**: `WARNING: Index last synced Xh Ym ago. Run tokensave sync to update.`
- **Branch fallback**: `WARNING: branch 'feature-x' is not tracked — serving from 'main'. Run tokensave branch add feature-x to track it.`

To test staleness: edit a file without re-syncing, then call any tool that touches that file.
To test branch fallback: check out an untracked branch while multi-branch is active, then call any tool.

---

## tokensave_status

> What's the current status of the tokensave index?

Expected: Returns node/edge/file counts, DB size, language distribution, tokens saved. Also includes staleness info:
- `stale_commits`: number of git commits since last sync (if > 0)
- `stale_warning`: human-readable message about stale commits
- `stale_files`: count of files modified on disk since indexing (sampled up to 100)

When multi-branch is active, also includes:
- `active_branch`: the current git branch name
- `branch_fallback`: `true` if serving from an ancestor branch DB
- `branch_warning`: explanation of which branch DB is being used

To test staleness: make a git commit without running `tokensave sync`, then call status.

---

## tokensave_search

> Search for symbols named "Database" in this project.

Expected: Returns matching symbols with IDs, file paths, line numbers, and signatures.

---

## tokensave_context

> Build context for the task: "understand how the MCP server handles incoming tool calls"

Expected: Returns entry points, related symbols, relationships, and code snippets relevant to MCP tool handling.

Test with code snippets:
```
tokensave_context(task="how does the search tool work", include_code=true, max_code_blocks=3)
```
Expected: Same as above but with source code snippets embedded for the most relevant symbols.

Test plan mode:
```
tokensave_context(task="add a new MCP tool for dependency visualization", mode="plan", include_code=true)
```
Expected: Standard context plus additional sections:
- **Extension Points**: public traits/interfaces with implementor counts
- **Test Coverage**: test files covering the related modules

---

## tokensave_node

> Get detailed information about the `TokenSave` struct. First search for it, then use the node ID.

Expected: Returns full node details including qualified name, signature, docstring, visibility, line range.

---

## tokensave_callers

> What functions call `get_tokens_saved`? Search for it first to get the node ID.

Expected: Returns caller symbols with file paths and edge types.

---

## tokensave_callees

> What does the `run` function in main.rs call? Search for it first to get the node ID.

Expected: Returns callee symbols showing the call graph from `run`.

---

## tokensave_impact

> What would be affected if I changed the `Database` struct? Search for it first, then compute impact.

Expected: Returns all symbols that directly or indirectly depend on `Database`.

---

## tokensave_files

> List all indexed files under the `src/mcp/` directory.

Expected: Returns files in `src/mcp/` with symbol counts and sizes.

---

## tokensave_affected

> If I changed `src/mcp/tools.rs` and `src/tokensave.rs`, what test files would be affected?

Expected: Returns test files that transitively depend on those source files.

---

## tokensave_dead_code

> Find potentially dead code — functions and methods that nothing calls.

Expected: Returns symbols with no incoming edges. Some may be entry points (main, test functions) which are expected false positives.

---

## tokensave_diff_context

> What's the semantic context for changes to `src/cloud.rs` and `src/user_config.rs`?

Expected: Returns symbols in those files, what depends on them, and affected tests.

---

## tokensave_module_api

> Show the public API of `src/tokensave.rs`.

Expected: Returns all public symbols in that file with their signatures — the external interface of the TokenSave struct.

---

## tokensave_circular

> Are there any circular dependencies between files in this project?

Expected: Returns a list of dependency cycles (may be empty if the codebase has no circular deps).

---

## tokensave_hotspots

> What are the most connected symbols in the codebase? Show the top 5.

Expected: Returns the 5 symbols with the highest combined incoming + outgoing edge count.

---

## tokensave_similar

> Find symbols with names similar to "extract".

Expected: Returns symbols like `extract_python`, `extract_ruby`, `RustExtractor`, etc.

---

## tokensave_rename_preview

> If I rename the `search` method, what would be affected? Search for it first, then preview the rename.

Expected: Returns all edges (callers, containers, etc.) referencing that symbol.

---

## tokensave_unused_imports

> Are there any unused imports in the project?

Expected: Returns import/use nodes that have no matching references in the graph.

---

## tokensave_changelog

> What symbols changed between the last two commits? Use `HEAD~1` and `HEAD`.

Expected: Returns a structured changelog showing added/removed/modified symbols per changed file.

---

## tokensave_rank

> What is the most implemented interface? What class implements the most interfaces?

Test incoming (default):
```
tokensave_rank(edge_kind="implements", node_kind="interface", limit=5)
```
Expected: Returns interfaces ranked by number of implementations (e.g. `Versioned` with 104).

Test outgoing:
```
tokensave_rank(edge_kind="implements", direction="outgoing", node_kind="class", limit=5)
```
Expected: Returns classes ranked by how many interfaces they implement (e.g. `PartitionData` with 16).

Other useful queries:
- Most extended class: `edge_kind="extends", node_kind="class"`
- Most called function: `edge_kind="calls", node_kind="method"`
- Most annotated class: `edge_kind="annotates", direction="outgoing", node_kind="class"`

---

## tokensave_largest

> What are the largest classes? What are the longest methods?

Test:
```
tokensave_largest(node_kind="class", limit=5)
tokensave_largest(node_kind="method", limit=5)
```
Expected: Returns nodes ranked by line count (end_line - start_line + 1) with start/end lines.

---

## tokensave_coupling

> Which files are depended on by the most other files? Which files have the most outward dependencies?

Test fan-in:
```
tokensave_coupling(direction="fan_in", limit=5)
```
Expected: Returns files ranked by how many other files depend on them.

Test fan-out:
```
tokensave_coupling(direction="fan_out", limit=5)
```
Expected: Returns files ranked by how many other files they depend on.

---

## tokensave_inheritance_depth

> What are the deepest class inheritance hierarchies?

Test:
```
tokensave_inheritance_depth(limit=5)
```
Expected: Returns classes ranked by inheritance chain depth via `extends` edges. Uses recursive CTE.

---

## tokensave_distribution

> How many classes vs interfaces vs methods are in a given package?

Test summary mode:
```
tokensave_distribution(path="kafka/clients/src/main/java/org/apache/kafka/common/config", summary=true)
```
Expected: Returns aggregated node kind counts (e.g. 355 fields, 193 methods, 20 classes).

Test per-file mode:
```
tokensave_distribution(path="src/mcp")
```
Expected: Returns per-file breakdown of node kinds.

---

## tokensave_recursion

> Are there any recursive or mutually-recursive call cycles? (NASA Power of 10, Rule 1)

Test:
```
tokensave_recursion(limit=5)
```
Expected: Returns call cycles found via DFS on the calls-only edge subgraph. Each cycle shows the chain of functions forming the loop. Self-recursive functions appear as length-1 cycles.

---

## tokensave_complexity

> What are the most complex functions in the codebase?

Test:
```
tokensave_complexity(limit=5)
tokensave_complexity(node_kind="function", limit=10)
```
Expected: Returns functions/methods ranked by composite score: `lines + (fan_out × 3) + fan_in`. Shows individual metrics (lines, fan_out, fan_in) alongside the total score. Also includes real cyclomatic complexity (`branches + 1`), branch count, loop count, return count, and max nesting depth — all extracted from the AST during indexing.

---

## tokensave_doc_coverage

> Which public symbols are missing documentation?

Test:
```
tokensave_doc_coverage(limit=20)
tokensave_doc_coverage(path="kafka/clients/src/main", limit=10)
```
Expected: Returns public functions, methods, classes, interfaces, traits, structs, and enums that have no docstring. Grouped by file with counts.

---

## tokensave_god_class

> Which classes have the most members? Are there any god classes that need decomposition?

Test:
```
tokensave_god_class(limit=5)
```
Expected: Returns classes ranked by total member count (methods + fields). Shows method count, field count, and total separately.

---

## tokensave_port_status

> Compare porting progress between `src/python/` (source) and `src/rust/` (target).

Test:
```
tokensave_port_status(source_dir="src/python/", target_dir="src/rust/")
```
Expected: Returns coverage summary with matched/unmatched/target-only counts. Matches by name (case-insensitive) with cross-language kind compatibility (`class` matches `struct`, `interface` matches `trait`). Unmatched symbols are grouped by source file. Shows `coverage_percent`.

Custom kinds filter:
```
tokensave_port_status(source_dir="lib/old/", target_dir="lib/new/", kinds=["function", "method"])
```
Expected: Only compares functions and methods between the two directories.

---

## tokensave_port_order

> What order should I port symbols from `src/python/` to minimize dependency issues?

Test:
```
tokensave_port_order(source_dir="src/python/", limit=30)
```
Expected: Returns symbols in topological dependency order, organized into levels:
- **Level 0**: No internal dependencies (utilities, constants) — port these first
- **Level 1**: Depends only on level 0 symbols
- **Level N**: Depends on levels 0 through N-1
- **Cycles**: Mutually dependent symbols flagged as "port together"

Each symbol shows its `depends_on` list (names of dependencies within the source dir).

Custom kinds:
```
tokensave_port_order(source_dir="src/legacy/", kinds=["function", "class"], limit=50)
```
Expected: Only includes functions and classes in the topological sort.

---

## tokensave_commit_context

> Summarize my uncommitted changes for a commit message.

Test all changes:
```
tokensave_commit_context()
```
Expected: Returns changed files with semantic roles (source/test/config/docs), symbols in each file, a suggested commit category (feature/fix/refactor/test/chore), and the 5 most recent commit subjects for style matching.

Test staged only:
```
tokensave_commit_context(staged_only=true)
```
Expected: Same as above but only includes staged changes (git index vs HEAD).

If no changes: returns "No changes detected."
If not a git repo: returns a git error message.

---

## tokensave_pr_context

> Summarize changes for a pull request from the current branch against main.

Test with defaults:
```
tokensave_pr_context()
```
Expected: Returns semantic diff between `main` and `HEAD`:
- Commit log (hash + subject for each commit)
- Symbols added (new symbols with no external callers)
- Symbols modified (existing symbols with external callers)
- Test files changed directly
- Affected tests (transitively impacted via dependency graph)
- Impacted modules (directories containing dependents of modified symbols)

Test with custom refs:
```
tokensave_pr_context(base_ref="develop", head_ref="feature-branch")
```
Expected: Same structure but comparing the specified refs.

---

## tokensave_simplify_scan

> Analyze changed files for quality issues.

Test:
```
tokensave_simplify_scan(files=["src/mcp/tools/handlers.rs", "src/mcp/tools/definitions.rs"])
```
Expected: Returns four categories of findings:
- **duplications**: symbols with >0.8 name similarity to symbols in other files
- **dead_introductions**: private functions/methods with no incoming edges (unreferenced)
- **complexity_warnings**: functions exceeding composite score threshold (lines + fan_out*3 > 100)
- **coupling_warnings**: files with fan_in > 15 (many dependents)

Each finding includes the symbol name, file, line number, and reason.

---

## tokensave_test_map

> Which tests cover the functions in `src/tokensave.rs`?

Test by file:
```
tokensave_test_map(file="src/tokensave.rs")
```
Expected: Returns:
- **coverage**: list of source functions/methods paired with their test callers (test name, file, line)
- **uncovered**: source functions/methods with no test callers found (up to depth 3)
- **test_files**: deduplicated list of all test files providing coverage
- **covered_symbols** / **uncovered_symbols**: counts

Test by node ID:
```
tokensave_test_map(node_id="fn:search_nodes")
```
Expected: Same structure but for a single symbol. If it's not a function/method, no coverage data is returned.

---

## tokensave_type_hierarchy

> Show the full type hierarchy for a trait. Search for the trait first, then use its node ID.

Test:
```
tokensave_type_hierarchy(node_id="trait:McpTransport")
```
Expected: Returns an indented tree showing the root type and all implementors/extenders recursively:
```
McpTransport (trait) -- src/mcp/transport.rs:191
|- implements StdioTransport (struct) -- src/mcp/transport.rs:203
|- implements ChannelTransport (struct) -- src/mcp/transport.rs:236
```

Test with depth limit:
```
tokensave_type_hierarchy(node_id="interface:Serializable", max_depth=2)
```
Expected: Same tree structure but stops at depth 2 (no grandchildren of grandchildren).

---

## tokensave_branch_search

> Search for a symbol in another branch's graph without switching your checkout.

**Prerequisites:** Multi-branch must be active. Run `tokensave branch add main` and `tokensave branch add feature-x` first.

Test:
```
tokensave_branch_search(branch="main", query="Database", limit=5)
```
Expected: Returns matching symbols from `main`'s graph, each tagged with `"branch": "main"`. Results may differ from the current branch if the symbol was modified or removed.

Test with untracked branch:
```
tokensave_branch_search(branch="nonexistent-branch", query="test")
```
Expected: Returns an error: `branch 'nonexistent-branch' is not tracked`.

---

## tokensave_branch_diff

> Compare code graphs between two branches to see what symbols were added, removed, or changed.

**Prerequisites:** Both branches must be tracked via `tokensave branch add`.

Test with defaults (current branch vs main):
```
tokensave_branch_diff()
```
Expected: Returns a JSON object with:
- `base`: the default branch name (e.g. "main")
- `head`: the current branch name
- `summary`: counts of added/removed/changed symbols
- `added`: symbols in head but not base (with name, kind, file, line, signature)
- `removed`: symbols in base but not head
- `changed`: symbols in both but with different signatures (shows both `base_signature` and `head_signature`)

Test with explicit branches:
```
tokensave_branch_diff(base="main", head="feature/foo")
```
Expected: Same structure comparing the specified branches.

Test with file filter:
```
tokensave_branch_diff(base="main", head="feature/foo", file="src/tokensave.rs")
```
Expected: Only symbols from `src/tokensave.rs` appear in the diff.

Test with kind filter:
```
tokensave_branch_diff(base="main", head="feature/foo", kind="function")
```
Expected: Only function symbols appear in the diff.

Test same branch error:
```
tokensave_branch_diff(base="main", head="main")
```
Expected: Returns an error: `base and head are the same branch: 'main'`.
