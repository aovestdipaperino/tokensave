# MCP Tool Test Queries

Manual test queries for verifying all 27 tokensave MCP tools. Run these in a Claude Code session after `tokensave sync` and `tokensave claude-install`.

---

## tokensave_status

> What's the current status of the tokensave index?

Expected: Returns node/edge/file counts, DB size, language distribution, tokens saved.

---

## tokensave_search

> Search for symbols named "Database" in this project.

Expected: Returns matching symbols with IDs, file paths, line numbers, and signatures.

---

## tokensave_context

> Build context for the task: "understand how the MCP server handles incoming tool calls"

Expected: Returns entry points, related symbols, relationships, and code snippets relevant to MCP tool handling.

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
