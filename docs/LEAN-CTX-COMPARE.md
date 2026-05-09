# tokensave vs lean-ctx

Both projects compress context for AI coding agents but with different centers of gravity:

- **tokensave** — code-graph engine (libSQL + tree-sitter, 34 languages). 48 MCP tools focused on symbol-level intelligence (callers/callees, impact, complexity, DSM, test_risk, code-health composite, branch diffs, atomic edit primitives). Cost tracking, daemon, monitor TUI.
- **lean-ctx** — context runtime that *also* compresses arbitrary file reads and shell output. ~56 MCP tools plus 95+ shell-hook patterns, multi-mode file reads, hybrid search with embeddings, portable `.lctxpkg` bundles, persistent knowledge facts.

The two overlap in graph/impact analysis but diverge on read modes, shell-output compression, and persistent knowledge. Tokensave is deeper on graph quality metrics; lean-ctx is broader on the I/O surface.

---

## Useful features to import (ranked)

### High value

1. **Mode-aware file read primitive** — `tokensave_read` with modes `full | map | signatures | diff | lines:N-M | entropy | auto`. tokensave already has the symbol graph (`tokensave_node`, `tokensave_module_api`) but no whole-file `Read` replacement. Exposing one would let agents skip raw `Read` for huge files. The `signatures` and `map` modes can be served almost for free from the existing graph.

2. **Shell-output compression patterns** — lean-ctx's 95+ declarative patterns for `git` / `cargo` / `npm` / `docker` output. Orthogonal to tokensave's graph; addresses the *other half* of agent token spend (Bash tool results). Could ship as `tokensave compress -c <cmd>` plus a Claude Code Bash post-tool hook. Pattern registry stays declarative and easy to extend.

3. **Hybrid search with RRF** — extend `tokensave_search` / `tokensave_context` with Reciprocal Rank Fusion over (FTS5 BM25, graph proximity, optional local embeddings). tokensave already has the first two; adding a small embedding model behind a feature flag would meaningfully improve recall on conceptual queries (the `keywords` arg on `tokensave_context` is the manual workaround for the same problem).

4. **Persistent knowledge facts** — `knowledge remember / recall / search / export / import` with category/key. Distinct from tokensave's `session_start` / `session_end` (which are *health-metric* snapshots, not free-form facts). Useful for "the test command is X", "this module owner is Y" — survives across sessions and could be exposed as an MCP tool plus a `tokensave://knowledge` resource.

5. **Read/result caching layer** — lean-ctx's "cached re-reads compress to ~13 tokens." For MCP responses keyed by `(file, mtime, args)`, return a tiny "unchanged since last call" stub. Lowers token cost on revisits without changing tool semantics.

### Medium value

6. **Portable context packages (`.lctxpkg`)** — SHA-256-stamped bundle of `{knowledge, graph subset, session, gotchas}`. tokensave already produces per-branch DBs; a portable export/import format would help team sharing and CI ("seed the cache"). Naturally pairs with #4.

7. **PR context packs as artifacts** — wrap the existing `tokensave_pr_context` output into a saveable bundle (changed files + related tests + impact + diff context) so it can be attached to PR descriptions or CI artifacts.

8. **Weekly "wrapped" report** — `tokensave wrapped --week`. Light addition over `tokensave cost` / `monitor` that surfaces top files, top tools, peak-savings days. Strong UX hook for users.

9. **Compaction-survival session recovery** — structured queries the agent can run after Claude's auto-compaction to rehydrate task state. tokensave's `session_*` could grow a `tokensave_session_recover` companion that emits a deterministic "what was I doing" summary.

10. **Cross-file block dedup** — `tokensave_simplify_scan` finds duplications in *changed* files; lean-ctx's `ctx_dedup` does it cross-repo. Could be added as `tokensave_dedup` over the existing AST data.

11. **Directory tree tool** — `tokensave_tree` returning a compact directory outline. Cheap to add from the existing `files` index; saves an agent from running `find` / `ls`.

### Lower value / situational

12. **Streamable HTTP MCP transport** — `tokensave serve --http` for clients that don't speak stdio. Useful for browser-based or remote agents; less urgent for the current CLI-agent userbase.

13. **API route extraction** — surface HTTP endpoints (e.g., axum / express / flask handlers) as a first-class node kind. Niche but high-leverage when present.

14. **Smart-read intent routing** — `auto` mode that picks `signatures` vs `full` vs `diff` from task hints. Pairs with #1; not worth adding alone.

---

## Things to skip

- **Multi-agent handoff / share / workflow tools** — tokensave is deliberately a backend, not an orchestrator. Adding these would blur scope.
- **Sandboxed shell execution (`ctx_execute`)** — Claude Code already runs Bash; duplicating it inside the MCP server invites support burden without obvious payoff.
- **`ctx_heatmap` / agent telemetry tools** — `tokensave monitor` and `tokensave cost` already cover this lane.

---

## Sources

- tokensave: `README.md`, `src/mcp/tools/definitions.rs`
- lean-ctx: <https://github.com/yvgude/lean-ctx> (`README.md`, `LEANCTX_FEATURE_CATALOG.md`)
