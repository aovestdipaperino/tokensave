# MCP Tool Test Queries

Manual test queries for verifying all 18 tokensave MCP tools. Run these in a Claude Code session after `tokensave sync` and `tokensave claude-install`.

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
