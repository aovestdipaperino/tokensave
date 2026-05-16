# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [4.14.5] - 2026-05-16

### Fixed
- **`tokensave_inheritance_depth` no longer explodes on cyclic / near-cyclic trait-bound graphs.** The recursive CTE in `get_inheritance_depth` had a depth bound of 50 but no cycle detection, so any cycle in the `extends` graph (common in Rust workspaces where generic trait bounds form indirect cycles) made the CTE traverse the cycle up to the depth limit from every entry point. On polkadot-sdk (959 `extends` edges) the query took >60 s and timed out; smaller workspaces (sotf 89, scirs 90, sonium 5) were fine. Fix tracks visited node IDs in a path column and skips recursion when the next target is already in the path — query completes in 0.55 s on polkadot end-to-end through MCP. Regression test `test_get_inheritance_depth_terminates_on_cycle` constructs a 3-node graph with an A↔B cycle and a C→A edge, then asserts the query returns in <2 s with all three nodes reported at finite, bounded depth.

### Added
- **`scripts/mcp_probe/` — MCP test-matrix harness.** Drives a fresh `tokensave serve` MCP server over stdio against a configurable set of real repos and exercises every read-only tool with 5 query variants per language, producing a per-tool / per-repo status table that flags tools needing investigation (errors, timeouts, empty results, perf regressions). Same harness doubles as a benchmark — per-call timings are logged, repos serve as a fixed corpus for cross-version perf comparison. Pluggable per-language probe modules under `tools/`; Rust ships included (`tools/rust.py` exercises all 50 MCP tools). `repos.toml` (overridable via `$TOKENSAVE_PROBE_REPOS`) holds the repo set. JSON-RPC ids are strictly matched in `probe.py::McpClient` so a slow call cannot poison subsequent ones; `isolated.py` adds a fresh-server-per-tool retry loop for tools that already showed a real timeout. `build_matrix.py` renders the log to markdown. Used to find and prove this release's `inheritance_depth` cycle bug; same harness verifies the 4.14.4 fixes stay green across the four real repos (sotf, sonium, scirs, polkadot-sdk).

## [4.14.4] - 2026-05-16

### Fixed
- **`tokensave_doc_coverage` reports public fields, enum variants, constants, statics, type aliases, properties, …** The query previously filtered to `kind IN ('function', 'method', 'class', 'interface', 'trait', 'struct', 'enum', 'module')` — so a Rust file full of `pub` undocumented struct fields reported `total_undocumented: 0` even though `tokensave_module_api` listed dozens of public symbols on the same file. Kind allow-list expanded to include `field`, `enum_variant`, `const`, `static`, `type_alias`, `property`, `csharp_property`, `record`, `data_class`, `sealed_class`, `object`, `case_class`, `kotlin_object`, `inner_class`, `abstract_method`, `constructor`, `struct_method`, `val`, `var`, `mixin`, `extension`, `union`, `typedef`. Excludes `namespace` and `package` — those are aggregators that almost never carry their own doc and would just drown out actionable items. Single `const` deduplicates the prefix and no-prefix branches. Verified end-to-end on real DBs: `biquad.rs` in sotf went from 0 → 23 undocumented public symbols; polkadot-sdk reports ~51 K with a sensible per-kind breakdown. Regression test `test_get_undocumented_public_symbols_includes_fields_and_variants`.
- **`tokensave_dead_code` excludes `#[test]`-annotated functions whose name does NOT start with `test`.** The previous filter was name-prefix-only (`name NOT LIKE 'test%'`), so `#[test] fn from_measurement_slope_excludes_lfe()` and similar leaked through. The libtest harness is an implicit caller that never appears as a graph edge, so without this filter most Rust tests with non-`test*` names got misreported as dead. Detection now walks the `annotates` edges and excludes any node whose annotation_usage name is `'test'`, `'…::test'` (covers `tokio::test`, `async_std::test`), `'wasm_bindgen_test'`, or `'…::wasm_bindgen_test'`. The JOIN is constrained to `a.kind = 'annotation_usage'` to avoid accidental matches. Real-DB impact: sotf 1794 → 540 dead functions (-70 %), sonium 778 → 209 (-73 %), scirs 4 839 → 2 469 (-49 %), polkadot-sdk **12 136 → 2 295 (-81 %)**. Manual spot-check on polkadot confirmed every dropped name is unambiguously a `#[test]` function. Regression test `test_find_dead_code_excludes_test_annotated`.
- **`tokensave_ast_grep_rewrite` surfaces a useful message when `ast-grep` exits non-zero with empty stderr.** ast-grep returns exit 1 with completely silent stdout/stderr when its pattern matches 0 nodes or when the file's language can't be inferred from the extension (`.txt`). The previous error string was `"ast-grep failed: "` — empty trailer, no actionable info. New handler falls back through stderr → stdout → an explicit explanation listing likely causes (pattern matched 0 nodes, language not inferred, invalid pattern), plus the exit code and the file + pattern that failed. Regression test `ast_grep_rewrite_surfaces_useful_error_on_empty_stderr`.
- **`tokensave_port_status` no longer cross-matches methods that share a name but belong to different parent types.** The match key was `(name.to_lowercase(), kind_compat_group)`, so `Biquad::new` matched `Adaa::new`, `Biquad::process` matched any other `process`, and so on — useless on Rust workspaces where every type has a `new`, `process`, `fmt`, `reset`, etc. Match key now also includes the parent qualifier (stripped of generics — `Biquad<T>` and `Biquad` resolve identically) for kinds that have one (`method`, `field`, `enum_variant`, `struct_method`, `abstract_method`, `constructor`, `csharp_property`, `property`, `val`, `var`); top-level kinds (struct, function, enum, trait) keep name-only matching since their containing context in `qualified_name` is just a file path. Regression tests `port_status_does_not_match_methods_of_different_parents` (Biquad in dir A, Adaa in dir B — must NOT match) and `port_status_matches_methods_with_same_parent_type` (Biquad in both dirs — must match).
- **`tokensave_branch_diff` returns an empty diff when base == head instead of erroring.** Previous behaviour was `MCP error -32603: base and head are the same branch`, inconsistent with `tokensave_pr_context` which already handled the same case by returning empty arrays. Same-ref now returns the normal JSON shape with `summary: {added:0, removed:0, changed:0}`, empty `added`/`removed`/`changed` arrays, and a `note` field explaining the equality — so callers can rely on a single response shape. Regression test `branch_diff_returns_empty_when_base_equals_head`.

## [4.14.1] - 2026-05-15

### Fixed
- **`tokensave_search` always ranks definitions above `use` re-exports.** BM25 was scoring short `pub use crate::operator::LinearOperator;` rows highly enough that five re-exports outranked the actual `pub trait LinearOperator { … }` definition. Sort now uses a coarse `kind_tier` as the primary key (defs tier 0, impl tier 1, values/members tier 2, modules tier 3, `use`/`include`/annotation usage tier 4); BM25 score is secondary within a tier. Added a per-row exact-name match boost (+10) so a trait named exactly `Foo` beats a `Method` whose qualified name happens to contain `Foo`. Regression test `search_ranks_trait_definition_above_use_reexports` constructs a trait plus five `pub use` re-exports across sibling modules and asserts the trait is hit #1.
- **`tokensave_dead_code` no longer treats `annotates` / `derives_macro` / `contains` edges as "this function is alive" evidence.** Real-world Rust pervasively uses `#[inline]`, `#[derive(Debug)]`, and similar attributes — every annotation_usage node creates an `annotates` edge pointing at the function, which the previous `kind != 'contains'` filter accepted as a live reference. Result on the sonium codebase: 0 dead functions across 5,715. Narrowed the SQL filter to an explicit allowlist of real-use edges: `calls`, `implements`, `extends`, `type_of`, `returns`, `receives`, `uses`. Regression test `dead_code_flags_unreferenced_fn_with_attribute` exercises the `#[inline]` case.
- **`tokensave_unused_imports` handles grouped imports (`use std::collections::{HashMap, HashSet};`).** The previous parser treated the literal string `{HashMap, HashSet}` as one identifier and never matched it against the file body, so every grouped import was effectively ignored — explaining the user's "0 unused across 3,404 use nodes" report. A new `identifiers_from_use_path` helper splits grouped/aliased/nested forms (`foo::bar`, `foo::bar as baz`, `foo::{a, b as c}`, `foo::{a, nested::b}`, `foo::{self, bar}`), and the handler now reports one entry per truly-unused identifier with an `unused: <identifier>` field. Regression test `unused_imports_handles_grouped_use` verifies the unused half of a grouped use is flagged while the used half is not.
- **`tokensave_changelog` filters deleted-subtree directory entries.** When an entire subtree was removed in a diff, gix yielded a directory-mode deletion entry whose path was gone from disk by the time the post-hoc `is_dir()` check ran — so directories like `crates/sonium-bem` slipped through as `removed_or_not_indexed`. `git_diff_files` now inspects `entry_mode.is_tree()` on each gix `Change` record (addition/modification/deletion/rewrite) and never pushes a tree entry into the changed-files list. The disk-based `is_dir()` filter is kept as belt-and-suspenders for additions/modifications. Regression test `changelog_filters_deleted_directory_entries` synthesises a `git rm -r crates/` commit and asserts no non-`.rs` paths appear in `changed_files`.
- **`tokensave_diff_context.modified_symbols` dedupes by node id and dedupes the input `files` array.** Callers that synthesised the file list from upstream tooling (directory walks, multi-source mergers) sometimes passed the same path multiple times — `hmatrix.rs` was reported up to 7× in a row. Added a `modified_seen: HashSet<String>` to guard pushes and an early `files` dedup pass. Regression test `diff_context_dedupes_modified_symbols_on_duplicate_input` passes the same path three times and asserts unique node ids in the output.
- **`tokensave_pr_context` collapses Cargo.toml into a single `config_summary` entry.** Behaviour was already present; added the regression test `pr_context_collapses_cargo_toml_keys` which synthesises a real git history with a 50-dependency Cargo.toml bump and asserts at most one Cargo.toml entry surfaces (kind = `config_summary`).
- **`tokensave_circular` SCC disjointness stress test.** Added `circular_emits_disjoint_sccs_under_load` — five 3-file cycles connected by non-cyclic DAG-style tails — to guard against any future SCC implementation drift that might let a file leak into more than one cycle entry.

### Added
- **`tokensave_port_order` surfaces intra-cycle ordering signals.** Each cycle entry now reports per-symbol `in_cycle_in_degree` and `in_cycle_out_degree`, a file-level `members_in_cycle` breakdown ranked by member count, an explicit `entry_point` (the SCC member with the smallest in-cycle out-degree — leaf-most, the natural starting point), and a `break_point_candidate` (the highest in-cycle in-degree node, the hub whose call sites are the most-effective refactor target). Replaces the previous flat blob of 200+ symbols with no guidance on where to start. Regression test `port_order_provides_intra_cycle_ordering` wires a 4-node SCC with one obvious hub and asserts `break_point_candidate.name == "h"`.

### Changed
- **`tokensave_ast_grep_rewrite` is conditionally registered.** The tool is only advertised via `tools/list` when the external `ast-grep` binary is on PATH at server-startup time (cached via `OnceLock` so we don't fork on every `tools/list` request). When the binary is missing, models never see a tool that would immediately return "ast-grep is not installed" on first call. `tokensave::mcp::tools::ast_grep_available()` is now public; tests in `mcp_handler_test::test_tool_definitions_complete` and `mcp_test::test_tool_definitions_count` branch on it so they pass on hosts with or without the binary installed.

## [4.14.0] - 2026-05-15

### Fixed
- **`tokensave_run_affected_tests` dispatches directly-changed test files.** Previously the handler only walked callers of every node in `changed_paths` — `#[test]` functions are leaves with no callers, so a PR that only touched `tests/foo.rs` returned "no tests cover the changed paths" and skipped running anything. The handler now also dispatches test functions whose file is itself in `changed_paths` (either via `is_test_file` path heuristic or `#[test]` annotation), with the test recorded as covering itself in `covers_source_ids`.
- **`parse_derives_in_attr_block` handles rustfmt's multi-line derive blocks.** The previous line-bounded scanner only matched `#[derive(...)]` when the closing `)` was on the same line, so rustfmt's split form (`#[derive(\n    Debug,\n    Clone,\n)]`) dropped every derive. The parser now joins the attribute-block lines and scans for `#[derive(` ... `)` across the whole region. Two new unit tests (`parses_multiline_derive_attribute`, `parses_multiline_derive_mixed_with_single_line`) cover the split form.
- **`tokensave_diagnose` normalises absolute and backslash paths.** Cargo emits absolute spans when `--manifest-path` points outside cwd, and Windows cargo emits backslash-separated paths; neither matches the indexed forward-slash, project-relative form. `node_at_location` now calls a new `normalize_lookup_path` helper that (1) replaces `\` with `/`, (2) strips the canonicalised project-root prefix for absolutes, and (3) falls back to a raw prefix strip when canonicalisation fails. A diagnostic spanning either form now maps to the correct node.
- **Resolver kind-compatibility filter now applies to the same-file blocklist branches (bug #11 follow-up).** PR8's filter was wired into the main `try_exact_name_match` / `try_qualified_match` paths but not the two `CROSS_FILE_BLOCKLIST` branches in `try_exact_name_match` and `try_exact_name_match_simple`. Common blocklisted names (`new`, `default`, `clone`, …) could still bind a `Calls` reference to a same-file non-callable — a struct or const sharing the name. Both branches now filter candidates through `kind_compatible` before declaring a same-file match. Regression test `resolver_blocklist_branch_respects_kind_filter` reproduces the case (`struct new` + `caller() { let _ = new(); }`) and asserts callees only include callable kinds.

## [4.13.0] - 2026-05-15

### Fixed
- **Resolver kind-compatibility filter (bug #11)** — `tokensave_rank --edge-kind implements` (and every downstream tool: `tokensave_impls`, `tokensave_type_hierarchy`, `tokensave_callees`'s trait dispatch, …) was poisoned by the resolver fuzzy-binding `impl Default for X` to whatever local node happened to share the name `Default`. The sonium codebase had a parser `Token` enum with a `Default` variant; 150 manual `impl Default for X` blocks all bound to that one `enum_variant`, swamping the rank tool with junk.
- New `kind_compatible(ref_kind, target_kind)` helper in `src/resolution/resolver.rs` enforces a structural matrix:
  - `Implements` / `Extends` / `DerivesMacro` → must target trait/interface/class/abstract-method/sealed-class/annotation/type-alias kinds
  - `Calls` → must target a callable (function/method/struct-method/constructor/abstract-method/arrow-function/procedure/macro)
  - `Annotates` → must target annotation/decorator kinds
  - `Uses` / `TypeOf` / `Returns` / `Contains` / `Receives` → permissive (any kind)
- Both `try_qualified_match` and `try_exact_name_match` now apply the filter; when filtering shrinks the candidate list, a `resolve_from_filtered` helper picks the same-file candidate first then falls back to the first overall, with confidence reduced to reflect the partial match. This prevents the previous "any-name-wins" behaviour without dropping legitimate resolutions.
- Regression test `implements_refs_dont_resolve_to_enum_variants` constructs the exact sonium-style scenario (`enum Token { Default, Plus }` plus two manual `impl Default for X` blocks) and asserts that `tokensave_rank --edge-kind implements` does NOT list an `enum_variant` or `field` target. Existing DBs need `tokensave sync --force` to re-resolve refs under the new constraints.

## [4.12.0] - 2026-05-15

### Added
- **`src/graph/scc.rs` — Tarjan's strongly-connected-components algorithm.** Iterative (no recursion, no stack-blow risk on deep graphs), generic over node-id type, returns components in reverse-topological order matching what port ranking needs. Used by both `tokensave_circular` and `tokensave_port_order`. Five unit tests cover DAGs, two-node cycles, three-cycle-plus-tail, self-loops, and reverse-topo emission order.

### Fixed
- **`tokensave_circular` reports one entry per SCC, not per DFS walk (bug #10)** — the previous implementation emitted every distinct DFS path through a cycle, producing 73 overlapping cycle entries on the sonium codebase that all shared a long common tail. `find_circular_dependencies` now computes SCCs via Tarjan and emits one entry per genuine mutually-recursive group, filtering out trivial single-node components that don't have self-loops. The legacy `dfs_cycle_detect` helper and `_legacy_walk_cycles` shim were removed.
- **`tokensave_port_order` exposes per-SCC cycle groups (bug #12)** — previously, every unsorted node after Kahn's topological sort was lumped into a single "Mutual dependency — port together" entry, so two disjoint mutually-recursive pairs `(a,b)` and `(c,d)` would render as one mega-cycle and lose all signal. The handler now runs Tarjan on the subgraph of unsorted nodes and emits one cycle entry per non-trivial SCC, with the `files` set of each cycle surfaced so the user has a concrete "break this edge" target. Each entry carries `symbols`, `files`, `size`, and a refined `note`.

## [4.11.0] - 2026-05-15

### Fixed
- **`tokensave_dependency_depth` no longer follows `implements`/`extends` edges (bug #7)** — the resolver fuzzy-binds `impl Debug for T` and similar across unrelated files, producing chains of spurious file-to-file deps (the report observed a 19-level chain spanning 17 unrelated files terminating in a foreign crate). `build_file_adjacency` now follows only `calls` and `uses` edges. Existing `tokensave_health` and `tokensave_circular` callers benefit too — they share the same adjacency builder.
- **`tokensave_dead_code` no longer reports 0 on `pub`-heavy codebases (bug #8a)** — two fixes: (1) the `NOT EXISTS` subquery now excludes `Contains` edges, which previously masked every node behind its parent's bookkeeping edge; (2) new `include_public: true` argument opts into auditing pub items with no indexed callers, useful for workspace-internal cleanup. Default behaviour (no flag) still excludes pub items as before.
- **`tokensave_unused_imports` no longer returns 0 on real codebases (bug #8b)** — the previous graph-only check tested `incoming.is_empty()`, but every Use node has at least one Contains edge from its parent, so the predicate never fired. New heuristic reads the source file once (cached per file) and checks whether the imported identifier appears as a whole-word token outside the use statement itself; matches what `cargo`'s own unused-import lint does. `pub use` re-exports, glob imports, and `use self::...` are skipped (intentional aliases / out-of-scope for textual heuristics). Three regression tests cover unused-detection, the dead-code Contains-edge bug, and the new `include_public` opt-in.

### Changed
- **`TokenSave::find_dead_code` signature** — gained an `include_public: bool` parameter. Existing callers (`tokensave_health`, internal tests) updated to pass `false` to preserve previous semantics.

## [4.10.0] - 2026-05-15

### Fixed
- **`tokensave_body` prefers callable kinds over same-named fields (bug #1)** — sonium hit a case where querying `gmres` returned only a struct field literally named `gmres` and missed the obvious `pub fn gmres(...)`. The handler now does an exact-name DB lookup first (via the PR1 suffix-fallback path) so the function isn't buried under BM25 noise, then sorts matches by `body_kind_preference()`: callable (0) > type def (1) > impl (2) > value (3) > field/variant (4) > use (5).
- **`tokensave_changelog` / `commit_context` / `pr_context` no longer list directories (bug #4)** — gix's `for_each_to_obtain_tree` yields directory-level entries when an entire subtree changes. `git_diff_files` now filters out any path that resolves to a directory on disk, so callers see only file paths.
- **`tokensave_diff_context.impacted_symbols` dedupes by node id (bug #5)** — diamond dependencies caused the same downstream node to appear 6+ times consecutively. `impacted_seen: HashSet<String>` now guards inserts.
- **`tokensave_recursion` drops length-1 self-cycles (bug #6)** — single-node cycles are almost always either resolver fuzzy-binding (`self.push()` cross-bound across distinct impls of the same name) or trivial self-recursion. Cycles with `< 2` distinct nodes are now filtered out before being added to the result set.
- **`tokensave_commit_context` / `tokensave_pr_context` collapse config-file symbols (bug #3)** — Cargo.toml's 50+ dependency keys used to each enumerate as a separate "modified symbol", blowing past 50K tokens on a real diff. Both handlers now emit a single `{kind: "config_summary", file, config_keys: N}` entry per file with role `config` (`*.toml` / `*.yaml` / `*.json` / `*.ini` / `*.cfg` / `*.lock`).
- **`classify_file_role` no longer flags source files with inline tests as "test" (bug #3 follow-up)** — a `src/foo.rs` with `#[cfg(test)] mod tests` at the bottom keeps role `source`. The "test" bucket is reserved for files that exist purely to host tests (path-based check via `is_test_file`). Three unit tests in `mcp::tools::handlers::git::tests` cover the classification matrix.
- **Rust extractor emits `Extends` edges for supertrait bounds (bug #9)** — `trait Leaf: Middle + Base` now produces unresolved refs with `EdgeKind::Extends` for each bound, so `tokensave_inheritance_depth`'s recursive CTE walks Rust supertrait chains correctly. Bound extraction handles `type_identifier`, `scoped_type_identifier`, `generic_type`, and `higher_ranked_trait_bound`. Existing DBs need a re-index (`tokensave sync --force`) to pick up the new edges.

## [4.9.0] - 2026-05-15

### Added
- **`tokensave_derives` tool** — surfaces the `#[derive(...)]` macros attached to a type plus the trait + method names each one synthesizes. Closes the dead-end-search gap where calls like `.clone()`, `format!("{:?}", x)`, or `serde_json::to_string(&x)` resolve to methods that never appear in the graph (the impl is generated by the proc-macro at compile time). Accepts either `qualified_name` or `node_id`. Well-known derives carry full info (trait path, method list, source crate); unknown / proc-macro derives surface with `well_known: false` and just the derive name.
- **`derive_table` module** (`src/derive_table.rs`) — static knowledge of well-known derives (`Debug`, `Clone`, `Copy`, `Default`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`, `Serialize`, `Deserialize`, `Display`, `Error`), each mapped to its canonical trait path and method names. Five unit tests cover known + unknown derives and the `enrich` wrapper.
- **`derives` field on `tokensave_node` output for type nodes** — when the queried node is a `Struct` / `Enum` / `Union` / `Record` / `CaseClass` / `DataClass` / `PascalRecord`, the response now includes a `derives` array so callers don't need a second roundtrip just to learn what derives are present.
- **`TokenSave::get_derives_for_node(node_id)`** — public helper that re-reads the node's source-file attribute block and parses `#[derive(...)]` directly. The graph's `DerivesMacro` edges are unreliable: the resolver fuzzy-binds std-trait names like `Debug` to nonsense targets (e.g. a `Debug` enum variant in an unrelated test fixture), and the unique constraint on `(source, target, kind, line)` then collapses multiple derives on the same type onto a single edge. Re-parsing from source costs one `fs::read` per node lookup (cheap at typical Rust source sizes) and recovers the full derive list. Five unit tests in `derive_parse_tests` cover single/multi-block derives, qualified paths, mixed attribute kinds, and dedup.

### Changed
- **Total MCP tools: 59 → 60** — `tokensave_derives` added.

## [4.8.0] - 2026-05-15

### Added
- **`tokensave_diagnose` tool** — parses raw `cargo check` / `cargo clippy` / `rustc` stderr into structured diagnostics, then maps each one to the smallest containing graph node and (by default) pre-attaches up to 5 callers. Closes the today-an-agent-hand-parses gap: the response includes severity, optional error code (`E0308`, clippy lint name), message, file/line/column, the owning node (id, kind, qualified_name, span), and the call sites the broken code is reachable from. Diagnostics without a `--> file:line:col` span are dropped — they cannot be located. Accepts a `severity` filter (`error` / `warning` / `all`) and a `max_diagnostics` cap (default 50, hard cap 500).
- **`tokensave_run_affected_tests` tool** — closes the loop opened by `tokensave_test_map` / `tokensave_test_risk`. Given `changed_paths` (or, by default, `git diff --name-only HEAD`), the handler walks the graph to find every test that covers a function/method in those files, then runs `cargo test --no-fail-fast -- <names>` with `kill_on_drop` and a configurable `timeout_secs` (default 300). Parses libtest stdout into JSON `{ test, passed, covers_source_ids[] }` entries plus pass/fail counts and the cargo exit code; trailing stdout/stderr are tailed at 2 KB each so the response stays in budget. `max_tests` defaults to 100 (hard cap 500) so a refactor touching everything doesn't dispatch an unbounded list.
- **`src/diagnose.rs`** — standalone parser module. Five unit tests cover typed errors (`error[E0308]`), clippy-style headers without codes, summary lines without spans (correctly dropped), multi-diagnostic blocks, and ANSI-prefixed lines.
- **`TokenSave::node_at_location(file, line_1based)`** — public helper that returns the smallest-span node containing a 1-based source location. Used by `tokensave_diagnose`; converts to the internal 0-based representation transparently.

### Changed
- **Total MCP tools: 57 → 59** — `tokensave_diagnose` and `tokensave_run_affected_tests` added.
- **New handler module `src/mcp/tools/handlers/workflow.rs`** — keeps cargo/libtest plumbing out of `graph.rs`, which is for code-graph queries.

## [4.7.0] - 2026-05-15

### Added
- **`tokensave_impls` tool** — index of `impl Trait for Type` blocks. Accepts optional `trait` and `type` filters (both short and qualified names). With neither, returns every impl in the graph. Surfaces information that was previously buried behind the second-class `Implements` edge: which types satisfy a given trait, which traits a type implements, and the impl blocks themselves with their files and signatures.
- **Trait dispatch resolution on `tokensave_callees`** — when a callee resolves to a method whose enclosing scope is a trait, the handler walks back via `Implements` edges to surface the concrete impl methods reachable through that trait. New entries are tagged `dispatch_via_trait: true` and carry a `dispatch_from` pointer to the trait method. Pass `resolve_dispatch: false` to opt out and get only direct call edges.
- **`TokenSave::get_impls(trait, type)`** — public helper backing the new tool.
- **`TokenSave::get_trait_dispatch_targets(method)`** — public helper that returns every impl-method satisfying a given trait method, used by `handle_callees` to surface dispatch targets.

### Changed
- **Total MCP tools: 56 → 57** — `tokensave_impls` added.
- **`tokensave_callees` description and schema** updated to advertise dispatch resolution and the new `resolve_dispatch` argument.

### Fixed
- **`tokensave_search` ranks definitions above references (PR1 follow-up)** — BM25 alone was placing `use foo` statements ahead of the actual `pub fn foo()` definition because both score similarly when the symbol name matches. `TokenSave::search` now over-fetches and re-ranks: every `NodeKind` carries an explicit bonus (callable defs +3.0, type defs / proto defs +2.5, impl blocks +2.0, values / macros / enum variants +1.0, members +0.5, neutral 0.0, container modules -1.5, annotation usages -2.0, `use` / `include` -3.0). The match is exhaustive so adding a new `NodeKind` forces a re-tune here. Result: searching for `gmres` returns the function before its imports.
- **`get_nodes_by_qualified_name` falls back to suffix or bare-name match (PR1 follow-up + user feedback)** — strict equality match remains primary. On empty results: queries with `::` retry as `qualified_name LIKE '%::<query>'` (full scan, `LIMIT 50`); queries without `::` retry as `name = ?` using `idx_nodes_name`. Both forms now resolve, e.g. `get_impls`, `TokenSave::get_impls`, and the full doubled path all return the same row. `tokensave_signature` and `tokensave_by_qualified_name` share the lookup so they agree.
- **Rust extractor no longer doubles the file path in `qualified_name`** — `qualified_prefix()` prepended `self.file_path` even though the file root was already pushed onto `node_stack` at extraction start, producing qnames like `src/foo.rs::src/foo.rs::Type::method`. Now iterates the stack only, yielding `src/foo.rs::Type::method`. Existing DBs will keep the old form until re-indexed (`tokensave sync --force`).
- **`get_impls` batches the trait lookup (PR2 review follow-up)** — previously one `get_node_by_id` per impl block (N+1). Now collects every Implements-edge target then issues a single `get_nodes_by_ids` to populate the trait map.
- **`graph_stale` insertion asserts on non-object results (PR1 review follow-up)** — `handle_tools_call` now `debug_assert!`s that the wrapped tool result is a JSON object before attaching the `graph_stale` field, matching the "crash hard on unknown value" convention so a future handler returning a non-object is caught immediately instead of silently dropping the structured staleness signal.
- **`cost_to_expand` body heuristic documented as Rust-tuned (PR1 review follow-up)** — the `20 tokens/line` rate over-estimates Haskell/Python by ~2-3x; the doc comment now explicitly says so and notes the single-line floor of 20 tokens, since this number is part of the public tool contract.

## [4.6.0] - 2026-05-15

### Added
- **`tokensave_signature` tool** — signature-only lookup by `qualified_name` or `node_id`. Returns visibility, signature string (generics, params, return type, where clauses), docstring, kind, and async flag for matching nodes. No body content. Replaces most agent `Read` calls when only the public-API surface of a symbol is needed.
- **`graph_stale` field on tool results** — when files referenced by a tool result remain stale after the post-call sync attempt, the JSON-RPC response now carries a top-level `graph_stale: ["path", …]` array plus a machine-parseable `tokensave_graph_stale: [...]` text marker. The existing human-readable WARNING is preserved. Closes the silent-drift gap where renamed/deleted symbols could return phantom callers/callees without a programmatic signal.
- **`cost_to_expand` annotation on node results** — `tokensave_node` and `tokensave_signature` responses now include `cost_to_expand: { body, full_file }` (approximate tokens) so callers can decide whether to set `include_code=true` before re-querying. Body estimate uses ~20 tokens/line; `full_file` uses indexed `files.size / 4`.
- **`tokensave://schema` MCP resource** — markdown resource documenting the on-disk `.tokensave/tokensave.db` schema: tables, columns, indexes, FKs, common query recipes (impl-of-trait, top callers, largest functions), and gotchas (content-hashed IDs, trait dispatch, derive macros). Makes the SQLite escape hatch usable without trial-and-error.
- **`TokenSave::get_file_size_bytes(path)`** — public helper that returns the indexed byte size of a file (0 when unknown). Backs the `cost_to_expand` full-file estimate.

### Changed
- **Total MCP tools: 55 → 56** — `tokensave_signature` added; all existing tools unchanged.

### Fixed
- **Clippy: project-wide cleanup to restore `-D warnings`** — 43 pre-existing lib errors and 3 bin errors resolved without behavioral change: module doc comments wrap snake_case tool names in backticks; `bench.rs` uses `write!` instead of `format!(..).push_str`; `extraction_worker.rs` converted to `let…else`; redundant closures in `agents/copilot.rs`, `extraction/haskell_extractor.rs`, `mcp/tools/handlers/memory.rs` replaced with method references; `resolution/resolver.rs` merges identical match arms; `serve.rs` uses `sort_by_key`; `upgrade.rs` uses `is_ok_and`; `main.rs` drops a useless `.into()`.

## [4.5.1] - 2026-05-15

### Added
- **`tokensave monitor` highlights the last 3 updates** — the most recently active (project, tool) pair renders green, second-to-last orange, third-to-last yellow. Re-firing the same tool moves it to the front rather than duplicating. Cleared on Ctrl+R.
- **Welcome banner on fresh installs** — when `tokensave` is invoked with no subcommand and the global DB has zero registered projects, print a cyan welcome that explicitly suggests `tokensave init` before the existing "Create one now?" prompt. Returning users see no change.

### Fixed
- **CI: `clippy::items_after_test_module` denied under Rust 1.95.0** — two test modules from the v4.5.0 work (`gain_tests` in `src/commands.rs`, `gain_format_tests` in `src/display.rs`) were inserted mid-file. Rust 1.95.0 promoted this lint into `clippy::all`, which the project denies project-wide. Both moved to file end.
- **CI: `cargo fmt` drift across 11 files** — accumulated unwrapped one-line `println!` / `match` / struct literals from the v4.5.0 features; re-formatted to match `rustfmt` expectations.

## [4.5.0] - 2026-05-15

### Added
- **Cross-session memory primitives (3 new MCP tools)** — `tokensave_record_decision`, `tokensave_record_code_area`, and `tokensave_session_recall` persist agent decisions and worked-on paths in the per-project DB so they survive across sessions. `session_recall` uses FTS5 for fuzzy retrieval. Backed by two new tables and an FTS mirror added in schema migration v8.
- **`tokensave gain` CLI for the savings ledger** — every MCP tool call now writes an append-only row to a new `savings_ledger` table in the global DB. `tokensave gain [--all] [--history] [--range 7d] [--json]` reports tokens saved + dollar estimates (Sonnet input pricing, refreshed daily via LiteLLM).
- **`tokensave bench` reproducible retrieval benchmark** — runs a fixed query set through `tokensave_context` and reports retrieval savings vs a full-file baseline (CCE-style methodology). Ships with a 10-query generic default set embedded into the binary (no external file dependency); `--queries <file>` accepts a custom set. Measured **93% mean retrieval savings on tokensave's own repo** (180K → 3.4K tokens across 10 generic queries).

### Changed
- **Schema bumped from v7 to v8** — adds `memory_decisions`, `memory_code_areas`, and the `memory_decisions_fts` virtual table. Existing user DBs upgrade idempotently via `migrate_v8`; fresh installs use the mirrored DDL in `create_schema`. No breaking changes; existing tools and queries continue to work.
- **`GlobalDb::open()` refactored to delegate to `GlobalDb::open_at(path)`** — enables test isolation via `tempfile::TempDir` without process-wide `HOME` mutation. The public `open()` API is unchanged.
- **Total MCP tools: 52 → 55** — three new memory tools added; all existing tools unchanged.

### Fixed
- **`coverage_discipline` health penalty reduced from 10% to 2% (issue #76)** — annotating genuinely untestable functions with `/// skip-test-coverage` was dropping `quality_signal` despite improving `coverage_pct`, because the penalty had no positive counterbalance (coverage doesn't feed into the composite health score). Max penalty reduced so honest annotation is not punished.

## [4.4.0] - 2026-05-14

### Fixed
- **Rust extractor now detects function calls inside macro invocations (issue #72)** — `assert!(check_count(5))` previously only created a Calls edge to `assert`, missing `check_count`. The extractor now walks into `token_tree` nodes inside macros to find nested call patterns.
- **`test_risk` denominator no longer includes test functions (issue #73)** — functions with `#[test]` annotations and functions inside `::tests::` modules are now excluded from `total_functions` and the coverage percentage denominator.
- **Rust extractor resolves instance method calls (issue #74)** — `instance.method()` now emits an additional unresolved ref with just the method name, allowing the resolver to match it against `impl` method definitions. Previously only associated function calls (`Type::new()`) were resolved.

### Added
- **`/// skip-test-coverage` doc comment convention (issue #75)** — mark genuinely untestable functions to exclude them from `test_risk` coverage calculations. The `skipped` count appears in the summary. A `coverage_discipline` health dimension penalises overuse (up to 10% quality signal reduction).
- **VS Code Insiders support for the Copilot installer (issue #69)** — `tokensave install --agent copilot` now also configures `Code - Insiders/User/settings.json` alongside the regular VS Code path.
- **Copilot prompt instructions (issue #70)** — the Copilot installer now writes `copilot-instructions.md` with tokensave MCP tool guidance to VS Code (`User/prompts/`), VS Code Insiders, and Copilot CLI (`~/.copilot/`).

## [4.3.18] - 2026-05-14

### Fixed
- **Inline `#[cfg(test)]` test modules are now recognized as test coverage** — `test_map`, `health`, `test_risk`, `affected`, `impact`, and `commit_context` previously only detected tests by file path patterns (`tests/`, `_test.`, etc.). Functions annotated with `#[test]` inside inline `#[cfg(test)] mod tests { ... }` blocks in source files (226 such functions in tokensave's own codebase) were invisible to coverage analysis. The Rust extractor now emits `Annotates` edges from `#[cfg(test)]` to modules, and all test-detection handlers query `#[test]` annotations via the graph in addition to checking file paths.
- **`tokensave serve` resolves the correct project in multi-folder workspaces (issue #66 reopened)** — when multiple projects are registered in the global DB, the `serve` fallback now picks the project closest to cwd (ancestor match first, then descendant match) instead of failing with an ambiguity error. As a last resort, the server peeks at the MCP `initialize` request's `roots` array to discover the workspace folder the client is working in.

## [4.3.17] - 2026-05-14

### Fixed
- **`tokensave upgrade` no longer breaks Homebrew installs (issue #67)** — previously, self-upgrading a Homebrew-managed install mutated the Cellar directly, leaving Homebrew's recorded keg state inconsistent and causing later `brew upgrade` to fail. `tokensave upgrade` now detects Homebrew installs and delegates to `brew update && brew upgrade tokensave`. (PR #68, thanks @lesbass)
- **Exclude globs now match nested directories (issue #64)** — the default `node_modules/**` pattern only excluded top-level `node_modules/`, not nested ones like `projectA/node_modules/`. Changed default to `**/node_modules/**`. Also added `is_excluded_dir()` so bare patterns like `**/dist` correctly prune directories during scanning without requiring a trailing `/**`.
- **VS Code multi-folder workspaces can now start the Copilot MCP server (issue #66)** — the Copilot config used `${workspaceFolder}` which VS Code cannot resolve in multi-folder workspaces. Dropped in favour of the serve command's built-in project discovery, matching every other agent integration.

## [4.3.16] - 2026-05-11

### Fixed
- **Windows CI failure introduced by v4.3.15's zed regression test** — `test_zed_install_preserves_existing_config` seeded `AppData/Roaming/Zed/settings.json` on Windows, but `zed_config_dir` actually uses `.config/zed/settings.json` on every non-macOS platform (Linux *and* Windows). The hand-written `#[cfg(target_os = "windows")]` branch in the test silently diverged from the production helper, so the test wrote the seed to one path and the install wrote to another — backup never appeared at the seeded location and the test failed. The Windows job (Linux passed, since its branch happened to be correct) was the only one to catch the drift.

### Changed
- **`AgentIntegration::primary_config_path(home) -> Option<PathBuf>`** — new trait method that returns the single config file the integration rewrites on install/uninstall. Every agent that goes through `safe_write_json_file` or `write_toml_file` implements it (claude, gemini, cursor, opencode, zed, cline, roo-code, copilot, kilo, antigravity, codex); vibe leaves the default `None` because its TOML config is append-only and has no rewrite path. Regression tests in `tests/agent_test.rs` now call `agent.primary_config_path(home)` instead of duplicating platform-conditional path logic — the production helper is the single source of truth, so a future `zed_config_dir`-style change can't drift between tests and reality. A meta-test (`test_every_tested_agent_advertises_primary_config_path`) walks every integration covered by the install regression suite and asserts the method returns `Some(path)` under the test home, so a new integration added without wiring it up fails fast with a clear message instead of producing a confusing missing-backup panic later.

## [4.3.15] - 2026-05-11

### Fixed
- **Installing the Codex integration no longer wipes `~/.codex/config.toml` (issue #63)** — `load_toml_file` used `contents.parse::<toml::Value>()`, which in the `toml = "1"` crate parses a single TOML *value* rather than a *document*. Any well-formed `config.toml` therefore parsed as an error and silently fell back to an empty table; `install_mcp_server` then serialized that empty-plus-tokensave table back over the file, erasing every other key the user had set (model, approval_policy, other `[mcp_servers.*]` entries, comments). `load_toml_file` now uses `toml::from_str::<toml::Table>` so real documents round-trip, returns `Result` instead of swallowing errors, and refuses to overwrite when an existing file cannot be parsed (so a typo or partial edit leaves the original intact for the user to fix). `doctor_check_config`, `install_mcp_server`, `uninstall_mcp_server`, and `CodexIntegration::has_tokensave` were updated to handle the `Result` shape — the doctor now reports parse errors as a failed check, and `has_tokensave` returns `false` on parse error rather than panicking.

### Changed
- **Every config-file write across all agent integrations now leaves a `.bak` copy first.** Previously only install paths went through `backup_config_file`; uninstall paths and `doctor` auto-repair paths called `std::fs::write` directly, so a corrupted serialization or a bug in the rewrite logic could destroy the user's settings with no recovery. A new shared `backup_and_write_json` helper (in `src/agents/mod.rs`) wraps `backup_config_file` + `safe_write_json_file` with best-effort error handling suited to uninstall flows. Every agent's uninstall path (claude, cursor, copilot, cline, zed, kilo, roo-code, opencode, gemini) now goes through this helper, as do the claude `doctor` auto-repair and local-settings-cleanup paths. The Codex TOML write path (`write_toml_file`) also creates a `.bak` before writing for the same reason. Eight per-agent install-side regression tests plus a cursor uninstall-side regression test were added to `tests/agent_test.rs` to guard the new invariant.

## [4.3.14] - 2026-05-11

### Fixed
- **`tokensave_body` no longer drops the function's outer closing brace (issue #62)** — `handle_body` returned the source spanning `start_line..end_line`, but stored line fields are tree-sitter rows (0-based) while `extract_lines` was written assuming 1-based inclusive inputs. The mismatch meant `lines[start..end_line]` exclusive — one short, lopping off the trailing `}` (or any language's outer block closer sitting on its own line). Inner braces were unaffected because they were never on the boundary. `extract_lines` now treats inputs as 0-based row indices and slices inclusively, so the returned body is byte-exact usable as an `Edit` tool `old_string`. Regression added in `test_body_returns_full_function_source` (`tests/mcp_handler_test.rs`) — verified failing pre-fix with `body: "\nfn format_greeting(name: &str) -> String {\n    format!(\"Hello, {}!\", name)"` (closing `}` missing).

### Changed
- **`tokensave_body` now exposes `start_line` / `end_line` as 1-based file line numbers** — they were previously the raw 0-based tree-sitter row indices, which read as "off by one" against the line numbers any editor or `Edit`-style tool displays. The values now match what users see when they open the file, so the reported `end_line` is the line containing the function's closing brace. The shift is local to `handle_body`; other handlers still expose `node.start_line` as-is.

## [4.3.13] - 2026-05-10

### Changed
- **Switched to `tree-sitter-grammars/tree-sitter-markdown` (block + inline split parsers)** — the previously-vendored `ikatyang/tree-sitter-markdown` (last updated 2023, GLR-heavy without native frontmatter handling) hung the indexer on otherwise-fine markdown files containing YAML frontmatter. Specifically, the old grammar parsed `---\n…\n---` content as ordinary markdown, where 6/8/10-space-indented YAML lines were simultaneously valid as both deeply-nested list-item continuations and as indented code blocks; tree-sitter's GLR explored all alternatives in parallel, with the surviving-versions count growing exponentially per line. A real-world 18 KB resume.md hung the worker indefinitely; a 4.4 KB minimal reproducer was bisected and is now a regression fixture (`tests/fixtures/markdown_yaml_frontmatter_hang.md`). The new grammar emits an opaque `(minus_metadata)` / `(plus_metadata)` node for frontmatter, so the markdown rules never see the YAML — the same 4.4 KB reproducer parses in ~7 ms, the full 18 KB file in ~16 ms. The markdown extractor was rewritten for the new AST (block parser produces `(atx_heading … heading_content: (inline …))`, headings still become `Module` nodes; the inline parser is run over each `(inline)` byte range via `set_included_ranges` to extract `(inline_link)` for `Uses` edges). All 16 existing markdown extraction tests still pass; 3 new regression tests guard the migration.

### Added
- **Per-file extraction timeout** — every extractor round trip is now wrapped in a watchdog (configurable via `extraction_timeout_secs` in `~/.tokensave/config.toml`, default 60 s). A file whose extractor doesn't respond in time has its worker subprocess killed via `Child::kill()` and is recorded in `SyncResult.skipped_paths` with reason `"extractor timed out (>Ns)"`. Worker crashes (the existing failure path) are now also recorded with reason `"extractor crashed (...)"` instead of disappearing silently. This bounds the worst case for any future grammar pathology — `tokensave sync` can no longer hang forever on a single malformed file.

## [4.3.12] - 2026-05-09

### Changed
- **The beta channel is open again** — `tokensave channel beta` was hard-gated to `"the beta channel is not available at this time"` while the prior 4.5.x beta line was being merged into stable. With v5.0.0-beta.1 published on the prerelease channel, the gate is removed: `switch_channel` now resolves `"beta"` through the same path as `"stable"` and downloads the latest GitHub prerelease. The `unknown channel` error message also lists `beta` as a valid target again.
- **Retired the "beta channel has been merged into stable" nudge** in `main.rs`. Beta users (anyone whose binary version contains `-`) used to see the nudge on every invocation; with the channel reopened the nudge is no longer correct. Beta users now stay on beta until they explicitly run `tokensave channel stable`.

### Fixed
- **`tokensave wipe` no longer leaks the global DB into the wipe set when `$HOME` is symlinked** — the home `.tokensave` skip relied on lexical path equality, so a user whose `$HOME` resolves through a symlink (e.g. macOS `/Users/x` vs the canonical `/private/var/...`) could see `~/.tokensave` show up as a wipe target if the descendant walk reached it via the canonical chain. The skip now canonicalizes both the home path and every candidate before comparing.

### Changed (carried forward from the prior unreleased section)
- **Descendant walk for `tokensave wipe` / `tokensave list` is now iterative with cycle protection** — `find_descendant_tokensave` used to recurse, which made deep trees a stack-overflow risk and relied entirely on `file_type()` skipping symlinks for cycle safety. It now uses an explicit worklist plus a canonical-path `visited` set, so the walk is bounded even if a directory cycle slips past the symlink filter (e.g. Windows junctions).
- **`tokensave doctor` purges stale global-DB entries in batched statements** — purging used to issue one `DELETE` per stale row, which meant N serial round-trips against libsql for a stale-store cleanup (the case that prompted this: 216 deletes). A new `GlobalDb::delete_projects(&[String])` issues one `DELETE … WHERE path IN (…)` per chunk of 256, so the same 216-row purge is now one round-trip.
- **`gather_local_projects_from` is now a separately-exported helper** — extracts the pure discovery logic from the cwd-driven `gather_local_projects` wrapper so the ancestor + descendant walk can be unit-tested without mutating the process's working directory. Backed by 7 new tests covering cwd / ancestor-only / descendant-only / ancestor+descendant dedup / `node_modules` skip / canonical home-skip / empty-dir.
- **Cleared `clippy::map_unwrap_or` warning in `display::shuffle_flags`** — the xorshift seed now uses `map_or` instead of `map(...).unwrap_or(...)`. Behavior unchanged.

## [4.3.11] - 2026-05-09

### Added
- **`tokensave doctor` now reports stale entries in the global DB and offers to purge them** — projects registered in `~/.tokensave/global.db` whose `.tokensave/` directory is gone (deleted, moved, or scratch dirs cleaned up by the OS) are listed under the "Global database" section. Up to 10 paths are shown with an "… and N more" tail. When run interactively, the doctor prompts `Purge N stale row(s) from the global DB? [Y/n]`; on confirmation each stale row is deleted via `GlobalDb::delete_project`. When stdin is not a terminal (CI, piped invocation), the stale list is shown as a warning with a hint to re-run interactively.

### Fixed
- **`tokensave reinstall` now refreshes every detected agent, not just the first one ever installed** — `migrate_installed_agents` previously returned early as soon as `installed_agents` was non-empty. A user who installed agent A and later configured agent B (e.g. installed Copilot first, then Claude) would have only A in the list, so `reinstall` silently skipped B and its tool permissions never got refreshed when new tools shipped. The migration now scans every agent on each call and additively appends any whose tokensave config exists on disk but is missing from the tracked list. Side effect: a stale `tokensave install` warning ("N new tokensave tool(s) not yet permitted") could persist across reinstalls — that no longer happens. The detection logic is also extracted into a pure `detect_missing_installed_agents` helper covered by a regression test that reproduces the original "claude missing when copilot is tracked" scenario.
- **`tokensave wipe` warning banner now reaches full width** — the colored title row was 49 visual columns while the `═` rules above and below were 64, producing a short red strip floating between long horizontal lines. The title is now centered and padded with red-background spaces, sandwiched between two blank red rows so the warning reads as a single fixed-width block.

## [4.3.10] - 2026-05-09

### Added
- **`tokensave list` command for inspecting tracked projects** — `list` shows the same projects `wipe` would target (current folder, ancestors, and descendants), with on-disk `.tokensave/` size and tokens-saved per row, sorted by tokens-saved descending. `tokensave list --all` (or `-a`) lists every project tracked in `~/.tokensave/global.db`, marking entries whose `.tokensave/` directory has been removed as `(stale)`.

### Changed
- **Country flags in `tokensave status` are now shuffled on every render** — when more flags are tracked than fit on the line, the row used to always show the same prefix and `…` truncate the rest. Each `status` invocation now applies a Fisher-Yates shuffle (xorshift64 seeded from time + PID) before truncation, so a different sample of contributing countries is shown each time.

### Fixed
- **Tool-permission warning now points at `tokensave reinstall`** — when new tokensave tools are detected that aren't yet permitted in the agent config, the warning previously said "Run `tokensave install` to update", which would re-do the full install. The warning now reads "Run `tokensave reinstall` to update permissions", which is the right command for refreshing permissions on already-installed agents.

## [4.3.9] - 2026-05-09

### Added
- **`tokensave wipe` command for clearing local DBs** — `wipe` finds every `.tokensave/tokensave.db` project in the current folder, all its ancestors, and all its descendants (skipping `node_modules`, `target`, `.git`, `vendor`, `dist`, `build`, `.next`, `.venv`, `__pycache__`, and the user-level `~/.tokensave/`), then prompts for a `go!` confirmation before removing each `.tokensave/` directory and its row in the global DB. `tokensave wipe --all` (or `-a`) instead wipes every project tracked in `~/.tokensave/global.db` and then deletes the global DB itself, leaving it empty. Both flows display a bordered, blinking warning that lists every target before asking for confirmation.

## [4.3.8] - 2026-05-06

### Added
- **`DISABLE_TOKENSAVE=true` environment variable to opt out per-project (#19)** — when set in the MCP server configuration, the `serve` command exits cleanly without initializing. This lets users selectively disable tokensave for large projects that consume too much RAM, without removing it from their global agent config.

## [4.3.7] - 2026-05-06

### Fixed
- **Incremental sync no longer aborts on cross-file edge references (#58)** — `insert_edges` now uses a conditional INSERT that silently skips edges whose source or target node does not yet exist in the database. Additionally, both incremental sync loops now insert all nodes first and queue edges for a second pass, so cross-file edges within the same sync batch always find their targets. Previously, `INSERT OR IGNORE` did not suppress FK violations, causing the sync to abort with `FOREIGN KEY constraint failed`.

## [4.3.6] - 2026-05-06

### Fixed
- **`upgrade` no longer stops the daemon when release assets aren't ready yet** — the preflight asset check now runs before stopping the daemon, so if CI hasn't finished building the release binaries, the command exits cleanly without disrupting the running MCP server.

## [4.3.5] - 2026-05-06

### Changed
- **Copilot MCP server now passes the workspace folder to `serve`** — both the VS Code (`mcp.servers.tokensave`) and the Copilot CLI (`mcpServers.tokensave`) registrations now launch the daemon as `tokensave serve -p ${workspaceFolder}` instead of plain `tokensave serve`. This lets the MCP server scope its index to the active workspace automatically without requiring a manual `-p` flag.
- **Copilot agent args validation tightened** — tests for `CopilotIntegration` now verify that `"serve"` is strictly the first argument and that all remaining args are limited to `-p` / `${workspaceFolder}`. This prevents silent regressions where extra or reordered flags could be injected into the MCP server launch command.

### Fixed
- **`serve` now falls back to the global project database when CWD discovery fails (#55)** — when VS Code Copilot (or another host) launches `tokensave serve` with the working directory set to the user's home folder and `${workspaceFolder}` fails to resolve, the server now checks `~/.tokensave/global.db` for registered projects. If exactly one project is found, it is used automatically; if multiple are found, they are listed on stderr with guidance to pass `-p <path>`.
- **`insert_at` no longer strips the trailing newline from edited files (#57)** — `str::lines()` discards the final `\n`, so the file was silently rewritten without its POSIX-required trailing newline. The join result now re-appends `\n` when the original file ended with one.
- **Clippy CI failures resolved** — fixed 6 `deny`-level clippy errors across extractors (identical `if`/`else` blocks in clojure, redundant `trim()` before `split_whitespace` in haskell, `map_or` → `is_some_and`, `Iterator::last` → `next_back` in SQL, `too_many_arguments` allow in haskell `emit`).
- **Foreign-key violations during incremental sync now point at the recovery path** — when an extractor produces an edge whose source or target is not in the same file's node set, `tokensave sync` would die with `failed to insert edge: SQLite failure: FOREIGN KEY constraint failed` and no guidance. Full re-index masks this because bulk load disables FK enforcement, so the top-level error handler now detects this specific failure and suggests `tokensave sync -f`.
- **Spinner no longer leaks on early exit** — added `Drop` for `Spinner` so when `?` propagates an error mid-sync the worker thread is joined, the line is cleared, and the cursor is restored. Previously the cursor stayed hidden after a failed sync.

## [4.3.4] - 2026-05-02

### Fixed
- **`tokensave sync` no longer hangs on large monorepos with `node_modules` symlinks** — the directory walker now prunes excluded directories (e.g. `node_modules`, `vendor`, `build`) at the `filter_entry` level before descending into them. Previously, exclusions were only checked per-file after the walker had already entered the directory, so monorepo setups where a package manager creates symlinks inside `node_modules` pointing back into source directories (e.g. `../../api`) could cause the scanner to spin indefinitely. Closes #36.

## [4.3.3] - 2026-05-02

### Added
- **`tokensave_body`** — new MCP tool that returns the full source body of a symbol by name (function, struct, const, etc.). Collapses search + node lookup + file read into a single call; returns multiple ranked matches when the name is ambiguous.
- **`tokensave_todos`** — new MCP tool that finds TODO, FIXME, XXX, HACK, WIP, NOTE, and UNIMPLEMENTED markers across the project. Each result includes the marker kind, file, line, the comment text, and the enclosing symbol name. Filterable by marker kind and path prefix.

### Fixed
- **SQL (and 8 other new-language) files no longer panic during sync** — `tokensave-large-treesitters 0.4.0` is now published to crates.io and `Cargo.toml` references the registry version instead of a local path. Users who built 4.3.2 via `cargo install` received the old 0.3.2 grammar bundle (no SQL), causing a panic per `.sql` file. Closes #53.

### Changed
- **`tokensave-large-treesitters` dependency pinned to published 0.4.0** — switched from a local path dependency to `"0.4.0"` so `cargo install tokensave` picks up the full grammar set including SQL, R, Julia, Haskell, OCaml, Clojure, Erlang, Elixir, and F#.

### Internal
- **Grammar completeness test** — `ts_provider::tests::all_extractor_keys_are_registered` verifies every language key an extractor passes to `ts_provider::language()` is present in the bundled grammar table. CI will catch mismatches before a release ships.

## [4.3.2] - 2026-05-01

### Added
- **9 new language extractors — R, SQL, Julia, Haskell, OCaml, Clojure, Erlang, Elixir, F#** — closes the gap between tokensave and sentrux for functional and data-science languages. Each extractor handles the language's primary top-level constructs and is gated behind its own `lang-*` feature flag, all included in `full`:
  - **R** (`.r`, `.R`) — function assignments (`foo <- function(...)`), call sites, roxygen2 docstrings. Requires `tokensave-large-treesitters` ≥ 0.4.0.
  - **SQL** (`.sql`) — `CREATE TABLE`, `CREATE VIEW`, `CREATE FUNCTION`, `CREATE PROCEDURE` via `tree-sitter-sequel`.
  - **Julia** (`.jl`) — `function`, `macro`, `struct`, `abstract_definition`, `module` definitions; import/using nodes.
  - **Haskell** (`.hs`, `.lhs`) — `function`/`bind` declarations, `data_type`/`newtype`, `class`, `instance`, `import` nodes.
  - **OCaml** (`.ml`, `.mli`) — top-level `let_binding` (function vs const), `type_definition`, `module_definition`, `class_definition`, `open` nodes.
  - **Clojure** (`.clj`, `.cljs`, `.cljc`) — `defn`/`defmacro`, `ns`, `def`/`defonce`, `defprotocol`/`defrecord`/`deftype` via `list_lit` dispatch on the first symbol.
  - **Erlang** (`.erl`, `.hrl`) — `fun_decl` with arity-qualified names (`foo/2`), `-module` attribute, `-type`/`-opaque` declarations.
  - **Elixir** (`.ex`, `.exs`) — `def`/`defp`, `defmodule`, `defmacro`/`defmacrop`, `defstruct` via `call`-node dispatch on the function head.
  - **F#** (`.fs`, `.fsi`, `.fsx`) — `function_or_value_defn`, `type_definition`, `module_defn`, `namespace`, `open_decl` nodes.
- **Complexity configs for all 9 new languages** — `R_COMPLEXITY`, `SQL_COMPLEXITY`, `JULIA_COMPLEXITY`, `HASKELL_COMPLEXITY`, `OCAML_COMPLEXITY`, `CLOJURE_COMPLEXITY`, `ERLANG_COMPLEXITY`, `ELIXIR_COMPLEXITY`, `FSHARP_COMPLEXITY` added to `src/extraction/complexity.rs`.
- **`tokensave-large-treesitters` 0.4.0** — bundles the 9 new tree-sitter grammars: `tree-sitter-r`, `tree-sitter-sequel`, `tree-sitter-julia`, `tree-sitter-haskell`, `tree-sitter-ocaml`, `tree-sitter-clojure-orchard`, `tree-sitter-erlang`, `tree-sitter-elixir`, `tree-sitter-fsharp`.

### Fixed
- **`tokensave monitor` displayed temp directories as projects** — MCP clients that create per-request temp directories (names matching `.tmp…`) were appearing as project entries in the monitor. These are now filtered out at render time; the TOTAL line reflects only real projects.

### Changed
- **`tokensave monitor` now supports scrolling** — Up/Down arrows scroll one line at a time; PageUp/PageDown scroll one screen. Scroll offset is clamped to the available content and resets to zero on Ctrl+R. Footer hint updated accordingly.

## [4.3.1] - 2026-05-01

### Fixed
- **`tokensave_str_replace`, `tokensave_multi_str_replace`, and `tokensave_insert_at` silently mutated files for unsupported types (issue #51)** — all three tools write the file to disk and then call `reindex_file` to update the graph. For file types without a registered extractor (e.g. `.css`, `.html`), `reindex_file` returned `Err("unsupported file type: …")`; the `?` propagated that error to the caller, which reported tool failure — but the write had already been committed. The fix changes `reindex_file` to return `Ok(())` early when no extractor is found, so edits to unsupported file types succeed and the graph simply skips reindexing for those files.

### Changed
- **Sync duration is now tracked and displayed** — `GraphStats` gains a `last_sync_duration_ms` field persisted to the metadata store. All three sync paths (full index, `sync_single_files`, `sync_with_progress_verbose`) write this value. The status table's sync row now shows the duration inline: `Last sync 2m ago (1.2s)  Full sync 1d ago`. Duration is omitted when the value is unknown (existing databases before this change).

## [4.3.0] - 2026-04-30

### Added
- **Subprocess-isolated extraction** — every file is now parsed inside a short-lived worker process rather than in the sync process itself. If a tree-sitter grammar segfaults, calls `abort()`, or otherwise terminates by a path Rust cannot intercept, only the worker dies; the pool respawns it, the offending file is logged and skipped, and sync continues. This is a stronger guarantee than the v4.2.1 `catch_unwind` defense, which could only catch Rust panics.
  - The worker is exposed via a hidden subcommand (`tokensave extract-worker`) that authenticates against the parent through a 256-bit per-spawn token: required as both an env var and as the first 32 bytes on stdin. A user invoking the binary directly hits the missing-env check and exits non-zero. The subcommand is also hidden from `--help`.
  - When `current_exe()` does not point at a real `tokensave` binary (e.g. under `cargo test`, where the test harness is the running binary), extraction transparently falls back to the in-process path. Tests therefore continue to exercise extractors directly without needing to spawn subprocesses.
  - Defaults to `available_parallelism()` workers; opt out via `TOKENSAVE_DISABLE_SUBPROCESS=1` if needed.

### Changed
- Single-file extraction (used by the `tokensave_str_replace`, `tokensave_insert_at`, etc. edit tools) still runs in-process — the subprocess overhead is unjustified for one-shot operations and these tools are interactive enough that an extractor crash is immediately visible.

## [4.2.1] - 2026-04-30

### Fixed
- **Sync no longer aborts when a tree-sitter grammar hits an internal assertion (issue #49)** — the vendored `tree-sitter-markdown` C++ scanner contains `assert()` calls that, on certain autolink constructs, called `abort()` and killed the entire `tokensave sync` process (core-dumped on Linux). Two layers of defense:
  - Added `.cargo/config.toml` with `CFLAGS=-DNDEBUG` and `CXXFLAGS=-DNDEBUG`. `cc-rs` reads these env vars when compiling vendored grammars in `tokensave-large-treesitters`'s build script, disabling C/C++ assertions in release builds. A failed assertion now degrades to a malformed parse tree (which the extractor handles gracefully) instead of `SIGABRT`.
  - Added a `safe_extract` helper that wraps every `extractor.extract()` call site with `std::panic::catch_unwind`. A Rust panic from any extractor (malformed input, future bugs) now logs the file path and skips it instead of bringing down the whole sync.
- See issue #50 for the broader follow-up: migrating to pure-Rust generated parsers via the `--rust` fork of tree-sitter to eliminate this class of failure entirely.

## [4.2.0] - 2026-04-30

### Added
- **Health & structural analysis tools** — seven new MCP tools that expose quality insights from the existing code graph:
  - `tokensave_health` — composite quality signal (0–10000) from five independent dimensions: acyclicity, depth, equality, redundancy, and modularity. Uses geometric mean so no single dimension can be gamed. Supports `details: true` for per-dimension breakdown.
  - `tokensave_gini` — Gini inequality coefficient for any metric (complexity, lines, fan_in, fan_out, members) across files or symbols. Identifies god files and uneven complexity distribution with interpretive labels and ranked outliers.
  - `tokensave_dependency_depth` — longest file-level dependency chains (Lakos levelization). Shows transitive fragility that direct coupling metrics miss, with full chain reconstruction after cycle-breaking via Tarjan's SCC.
  - `tokensave_dsm` — Design Structure Matrix in three output formats: `stats` (density, cluster count), `clusters` (per-directory edge analysis), and `matrix` (NxN grid with short filenames). Reveals hidden coupling patterns and layering violations.
  - `tokensave_test_risk` — risk-weighted test gap analysis combining complexity, fan-in, test coverage, and git churn (90-day window) into a single score. Answers "where should the next test go?" with `include_tested` option for finding weak-test candidates.
  - `tokensave_session_start` — saves current health metrics as a JSON baseline for later comparison. Call before starting an AI coding session.
  - `tokensave_session_end` — re-computes health and diffs against the session baseline. Reports per-dimension deltas with improved/degraded/unchanged labels, overall pass/fail, and cleans up the baseline file.
- **Git churn integration** — new `src/graph/git.rs` module shells out to `git log` at runtime to compute per-file commit frequency. Used by `tokensave_test_risk` as a risk multiplier (log2-scaled) without persisting any data to the tokensave DB.
- **File-level DAG builder** — new `build_file_adjacency` method on `GraphQueryManager` constructs a directed file dependency graph from the existing edge data in a single SQL query. Shared foundation for health, depth, DSM, and modularity computations.

## [4.1.8] - 2026-04-30

### Added
- **`include` config glob** — new `include` field in `.tokensave/config.json` lets users whitelist hidden (dot-prefixed) paths for indexing. By default, all dot-directories are skipped during sync; paths matching an `include` glob (e.g. `[".github/**"]`) are now walked and indexed. The `exclude` list still applies after inclusion, so `.git/**` and `.tokensave/**` remain filtered even with broad include patterns.
- **Markdown extraction** — tree-sitter based markdown parser that extracts headers as `Module` nodes with hierarchical `Contains` edges, and code links as `Uses` edges for cross-reference tracking (PR #47)

## [4.1.7] - 2026-04-29

### Fixed
- **Nested `.gitignore` files were silently ignored** — `git_ignore(true)` in the `ignore` crate relies on git repository detection (walking up to find `.git`) to build the gitignore rule stack. When the walk root was outside a git repo — or in a subdirectory that the crate couldn't trace back to a `.git` — rules in nested `.gitignore` files were never applied. Added `add_custom_ignore_filename(".gitignore")` to the `WalkBuilder`, which makes the crate read every `.gitignore` it encounters as a standalone ignore source regardless of git repo presence. Five regression tests cover: subdirectory exclusion, scope isolation, negation overrides, deep descendant exclusion, and a direct `ignore`-crate sanity check.

## [4.1.6] - 2026-04-29

### Fixed
- **`logging/setLevel` returned MethodNotFound on every session start** — the server correctly advertised the `logging` capability in its `initialize` response (required for the `notifications/message` version-warning feature), but had no handler for the `logging/setLevel` request that MCP clients send immediately after. Every session produced a `-32601` error in the client log. The handler now returns an empty success as required by the MCP spec (RFC 5424 log-level filtering is advisory; the server continues to emit notifications at its own discretion).
- **`java_extraction` panic on empty Javadoc** — parsing a Java file containing a docstring with no content caused a panic (fixes #44).

## [4.1.5] - 2026-04-29

### Added
- **Edit primitives for code modification** — four new MCP tools enable Claude and friends to edit files without regex or shell quoting hazards (PR #43 by @pierreaubert):
  - `tokensave_str_replace` — replaces a unique `old_str` with `new_str`; fails if 0 or >1 matches, protecting against multi-edit bugs
  - `tokensave_multi_str_replace` — applies N `(old, new)` replacements atomically; all-or-nothing transaction
  - `tokensave_insert_at` — inserts content before or after a unique anchor string or line number
  - `tokensave_ast_grep_rewrite` — structural code rewrite via ast-grep CLI (`--rewrite` mode)
- **Auto re-indexing** — all four edit tools automatically re-index the modified file in the code graph after writing, keeping the graph in sync without manual steps (PR #43 by @pierreaubert)

### Performance
- **Fixed N+1 query patterns in graph traversal** — `traverse_bfs`, `traverse_dfs`, `get_callers`, `get_callees`, `get_file_dependencies`, `get_file_dependents`, and `find_dead_code` were each making a separate database query per node, causing excessive CPU usage on large codebases. All methods now batch-fetch nodes using a single `WHERE id IN (...)` query, reducing database roundtrips from O(N) to O(1). (PR #40 by @pierreaubert)

### Fixed
- **`find_dead_code` hit SQLite variable limit on large codebases** — the query used `IN (?, ?, …)` binds which SQLite caps at 999 variables; replaced with `NOT EXISTS (SELECT 1 FROM edges WHERE …)` to avoid the limit entirely. (PR #43 by @pierreaubert)
- **`tokensave_test_map` failed to resolve cross-crate qualified calls** — when a reference contained `::` (e.g. `crate_name::func`), a failed qualified-name match returned `None` without falling back to a simple-name lookup, breaking test coverage queries for integration tests that call across crate boundaries. Fixed by removing the early return and adding a simple-name fallback that strips the qualifier before matching. (PR #43 by @pierreaubert)
- **Sync frequency reduced and stale-warning auto-sync added** — sync interval dropped from its previous default to 2 s (configurable); the MCP server now automatically triggers a live sync when an agent receives a stale-graph warning, avoiding a manual `tokensave sync` round-trip. (PR #43 by @pierreaubert)
- **`TOOL_NAMES` and `EXPECTED_TOOL_PERMS` were static** — `doctor` and `install` would not detect or register newly-introduced MCP tools. Both lists are now built dynamically so adding a tool automatically propagates to health checks and permission installation. (PR #43 by @pierreaubert)
- **`tokensave monitor` now groups output per project then per tool** — previously all tool calls were listed in a flat stream; entries are now grouped by project path first, then by tool name, making it easier to see which project is driving activity. (PR #43 by @pierreaubert)

## [4.1.4] - 2026-04-25

### Fixed
- **`tokensave monitor` panicked on macOS/Linux with "Cannot start a runtime from within a runtime" (issue #39)** — the previous fix for the Windows panic kept a Unix-only branch that built a new `tokio::runtime` and called `block_on` from inside `#[tokio::main]`, which panics on every platform, not just Windows. `refresh_cost_cache` now uses `block_in_place + Handle::current().block_on` unconditionally, since `monitor::run()` is always invoked from the existing multi-threaded runtime.

## [4.1.3] - 2026-04-24

### Fixed
- **Backslashed Windows hook paths never self-healed (issue #38)** — the v4.0.2 fix for #20 normalized `which_tokensave()` output but could not rewrite existing settings. `install_single_hook` is idempotent by presence, so when a tokensave hook already existed with a backslashed path, the silent backfill in `check_install_stale` left it untouched. Additionally, the backfill only scanned `~/.claude/settings.json` — project-level `.claude/settings.json` and `.claude/settings.local.json` were never touched, so opening a previously-configured project could still trigger `bash: C:Usersalkamscoopappstokensavecurrenttokensave.exe: command not found`. Fixed with a new `normalize_hook_command_paths` pass that rewrites any backslash-containing tokensave hook command to forward slashes, and by extending the backfill to the current project's `.claude` directory.

## [4.1.2] - 2026-04-22

### Added
- **Mistral Vibe agent integration** — `tokensave install --agent vibe` registers the tokensave MCP server in Vibe's `~/.vibe/config.toml` as a `[[mcp_servers]]` stdio entry, and appends prompt rules to `~/.vibe/prompts/cli.md`. Supports install, uninstall, and healthcheck. Respects the `VIBE_HOME` environment variable. Closes #37.

## [4.1.1] - 2026-04-22

### Added
- **`tokensave sync --verbose` (`-v`)** — prints per-phase diagnostic lines during sync to help diagnose slow or stuck syncs on large repos. Shows file counts, change breakdowns, and timings for each phase (scan, stat-check, hash, content check, index, resolve, DB write). Also works with `--force` full re-index. Addresses #36.

## [4.1.0] - 2026-04-20

### Added
- **Walk-up project discovery** — `tokensave serve`, `tokensave sync`, and `tokensave status` now walk up the directory tree to find the nearest `.tokensave/` database when no `--path` is given. This means you can launch an AI agent from a subdirectory of your project and tokensave will find the index automatically — similar to how git finds `.git/`. `tokensave init` is unchanged and always creates a new project at the target directory.
- **Subdirectory scope filtering** — when the MCP server is started from a subdirectory, listing and discovery tools (`tokensave_files`, `tokensave_search`, `tokensave_context`, `tokensave_dead_code`, `tokensave_rank`, `tokensave_largest`, `tokensave_coupling`, `tokensave_complexity`, `tokensave_doc_coverage`, `tokensave_god_class`, `tokensave_unused_imports`, `tokensave_hotspots`, and others) automatically scope results to that subdirectory. Graph traversal tools (`tokensave_callers`, `tokensave_callees`, `tokensave_impact`, `tokensave_affected`, `tokensave_type_hierarchy`) remain unscoped so cross-directory relationships are preserved. The user can always override the scope by providing an explicit `path` parameter. `tokensave_status` reports the active scope prefix when one is in effect.

## [4.0.7] - 2026-04-18

### Fixed
- **Symlinked source directories were not indexed** — both the plain `walkdir` and `.gitignore`-aware `ignore::WalkBuilder` file discovery paths now follow symlinks (`follow_links(true)`), so projects that expose source code through symlinked directories are fully indexed. (PR #34 by @lesbass)

## [4.0.6] - 2026-04-18

### Added
- **GLSL language support** — new tree-sitter-based extractor for OpenGL shading language files (`.glsl`, `.vert`, `.frag`, `.geom`, `.comp`, `.tesc`, `.tese`). Extracts functions, structs with fields, uniform/in/out/varying declarations, preprocessor defines, call sites, and complexity metrics. Requires `tokensave-large-treesitters` 0.3.0. Feature-gated as `lang-glsl` in the Full tier. Closes #35.

### Fixed
- **`tokensave upgrade` fails on Homebrew installs** — `self_replace` failed with `ENOENT` on Homebrew symlinks because it resolved relative symlink targets from CWD instead of the symlink's parent. Now dispatches to install-method-aware replacement: Homebrew bypasses `self_replace` and atomically replaces the binary at the canonical Cellar path, renames the version directory, and updates the symlink + `INSTALL_RECEIPT.json` so `brew` reports the correct version. Scoop updates the version directory, junction, and `manifest.json`. Other symlinked installs get a canonicalization fallback. Supersedes PR #33.

## [4.0.5] - 2026-04-17

### Changed
- **Separate `tokensave init` from `tokensave sync`** — previously, `tokensave sync` silently created a new database if none existed. This was a problem because the global git post-commit hook runs `tokensave sync` in every repo after each commit, causing phantom `.tokensave/` databases to appear in projects that never opted in. Now `tokensave init` handles first-time project setup (creates DB + full index) and errors if already initialized, while `tokensave sync` only performs incremental updates and errors if the project was never initialized. The git hook (`tokensave sync >/dev/null 2>&1 &`) now safely exits with an error in non-enrolled repos — no database created. All agent setup messages and documentation updated to reference `tokensave init` for first-time use.

## [4.0.4] - 2026-04-17

### Added
- **Google Antigravity support** — new `tokensave install --agent antigravity` registers the MCP server in `~/.gemini/antigravity/mcp_config.json`. Includes install, uninstall, healthcheck, and auto-detection. Closes #24.
- **Kilo CLI support** — new `tokensave install --agent kilo` registers the MCP server in `~/.config/kilo/kilo.jsonc` using Kilo's `mcp` key with `type: "local"` format. Includes install, uninstall, healthcheck, and auto-detection. Closes #31.

### Changed
- **Simpler install prompts** — `tokensave install` now asks a Y/n question per detected agent instead of showing a multi-select dialog box. Prints a +/- summary of changes at the end. Removed `dialoguer` dependency.
- **No-op upgrade is no longer an error** — `tokensave upgrade` when already on the latest version now exits successfully instead of printing a misleading error. Same for `tokensave channel` when already on the requested channel. (PR #30 by @lesbass)

### Fixed
- **Default branch detection wrote `"HEAD"` instead of actual branch name** — `detect_default_branch()` used `reference.name()` on the `refs/remotes/origin/HEAD` symbolic ref, which returns the ref's own name. Now resolves through `reference.follow()` to get the target (e.g. `refs/remotes/origin/master`), then strips the prefix correctly. (PR #26 by @LucioPg)
- **Branch detection in git worktrees** — `current_branch()` read `.git/HEAD` directly as a plain file, which fails in git worktrees where `.git` is a pointer file (not a directory). Fixed with a two-tier approach: `gix::open()` first, then `git symbolic-ref -q HEAD` subprocess fallback. (PR #28 by @LucioPg)
- **Windows monitor nested runtime panic** — `tokensave monitor` cost cache refresh panicked on Windows due to nested tokio runtimes. Now uses `block_in_place` + `Handle::current()` on Windows. (PR #29 by @LucioPg)
- **Clippy clean** — resolved all clippy errors across the codebase; CI clippy step now passes.

## [4.0.3] - 2026-04-16

### Fixed
- **Windows daemon nested runtime panic** — `tokensave daemon` panicked on Windows because `daemon-kit` runs the closure inline (no fork), creating a nested tokio runtime. Now uses `block_in_place` + `Handle::current()` on Windows while keeping `Runtime::new()` on Unix where the forked child genuinely has no runtime.

## [4.0.2] - 2026-04-14

### Added
- **Token cost observability** — new `tokensave cost` command parses Claude Code session transcripts (`~/.claude/projects/**/*.jsonl`), classifies each API turn into 13 task categories (coding, debugging, exploration, ...), and computes dollar cost per model. Supports `--by-model`, `--by-task`, `--export json|csv`, and time ranges (`today`, `7d`, `30d`, `all`). Model pricing is refreshed from LiteLLM every 24 hours and cached at `~/.tokensave/pricing.json`. Cost data is stored in the existing `~/.tokensave/global.db`. The `tokensave status` header now shows today's cost, 7-day cost, and efficiency ratio. The `tokensave monitor` TUI includes a cost panel. The `hook_stop` handler prints a session cost receipt. Task classification adapted from [AgentSeal/codeburn](https://github.com/AgentSeal/codeburn).
- **`tokensave status --details`** — the node-kind breakdown table is now opt-in via the `--details` flag. Default status output is more compact.
- **Per-file diversity caps** — `tokensave_context` now limits how many symbols from a single file appear in results (default: `max_nodes/3`, minimum 3), preventing one large file from dominating context output. Configurable via the new `max_per_file` parameter.
- **Exact name match supplementing** — context search now supplements FTS5 results with exact case-insensitive name lookups, so perfect symbol name matches are never buried by BM25 noise.
- **Stem variant search expansion** — search terms are expanded with suffix-based stem variants (e.g. "authenticate" also finds "authentication", "authenticator") via 13 derivational suffix rules, improving recall for conceptual queries.
- **Co-occurrence boosting** — when a query has multiple terms, symbols where 2+ terms co-locate in name, qualified name, or file path get a multiplicative score boost, improving precision on multi-word searches.
- **Edge recovery after node trimming** — when BFS subgraph expansion trims nodes to fit `max_nodes`, edges are now filtered to retain only those connecting surviving nodes, keeping the returned subgraph consistent.
- **Adaptive SQLite pragmas** — `cache_size` and `mmap_size` now scale to the DB file size instead of using fixed 64 MB / 256 MB values. Small projects (5 MB DB) drop from ~320 MB baseline to ~12 MB; large projects keep the same performance.
- **`tokensave reinstall` command** — re-runs install for all already-configured agents, refreshing MCP server registration, hooks, permissions, and prompt rules without the interactive picker.

### Removed
- **Graph visualizer** — `tokensave visualize` command, `src/visualizer.rs`, and the embedded HTML file have been removed. The upstream CodeGraph project also removed its visualizer in the same period.

### Fixed
- **Windows path separators in hooks and MCP config** — `which_tokensave()` now normalizes backslash paths to forward slashes, fixing broken hook command execution on Windows (e.g. Scoop installs). Existing settings with backslash paths are also normalized when read back.

## [4.0.0] - 2026-04-13

### Added
- **Multi-branch indexing** — opt-in per-branch databases so switching branches never gives stale results. `tokensave branch add` tracks a branch by copying the nearest ancestor DB and syncing only changed files. `tokensave branch list`, `tokensave branch remove`, `tokensave branch removeall`, and `tokensave branch gc` manage tracked branches.
- **`tokensave branch removeall`** — remove all tracked branches except the default in one command, deleting their DB files.
- **`tokensave_branch_search`** MCP tool — search symbols in another branch's code graph without switching your checkout.
- **`tokensave_branch_diff`** MCP tool — compare code graphs between two branches: shows symbols added, removed, and changed (signature differs). Supports file and kind filters.
- **`tokensave_branch_list`** MCP tool and **`tokensave://branches`** MCP resource — list tracked branches with DB sizes, parent branch, sync times.
- **Branch fallback warnings** — when the MCP server serves from an ancestor branch DB (current branch not tracked), every tool response warns to `tokensave branch add`.
- **`keywords` parameter for `tokensave_context`** — agent-driven synonym expansion. Pass extra search terms (e.g. `["login", "session", "token"]` for "authentication") and the context builder searches each keyword independently, bridging conceptual queries to lexically-unrelated symbol names without embedding models.
- **`tokensave monitor` CLI command** — global live TUI showing MCP tool calls from all projects in real time via a shared memory-mapped ring buffer at `~/.tokensave/monitor.mmap`. Entries show `prefix - project - tool_name` so multiple tool suites and projects are distinguishable. Uses `memmap2` with file locking for concurrent writer safety.
- **`path` filter on 7 analytics MCP tools** — `tokensave_god_class`, `tokensave_largest`, `tokensave_complexity`, `tokensave_rank`, `tokensave_coupling`, `tokensave_inheritance_depth`, and `tokensave_recursion` now accept an optional `path` parameter to scope results to a directory (e.g. `"path": "src/main/java"`), preventing large languages from dominating global rankings.
- **Right-click context menu in graph visualizer** — callers, callees, call graph, and impact actions on node right-click.
- **Type annotation references** — TypeScript, Java, and Kotlin type annotation references now tracked as edges in the graph.
- **Graph visualizer** — interactive Cytoscape.js-based code graph visualization served via `tokensave visualize`.
- **Daemon version mismatch detection** — `tokensave daemon --status` warns when the daemon version differs from the CLI with a corrective restart command.
- **Parent branch in status output** — `tokensave status` and `tokensave_status` now show which branch a tracked branch was seeded from.

### Removed
- **Vector/embedding module** — removed `src/vectors/`, `enable_embeddings` config field, and `Vector` error variant. The `keywords` parameter on `tokensave_context` replaces the need for local embedding models. The `vectors` DB table is retained (empty, harmless) to avoid migration issues.

### Changed
- **Monitor is now global** — moved from per-project (`<project>/.tokensave/monitor.mmap`) to machine-level (`~/.tokensave/monitor.mmap`). `tokensave monitor` no longer takes a `--path` flag.
- Quality improvements to resolution, search, and traversal.
- Tool count increased from 34 to 37.

### Dependencies
- Added `memmap2`, `crossterm`, `fs2` for the monitor feature.

## [3.5.1] - 2026-04-13

### Fixed
- **Doctor validates hook subcommands** — `tokensave doctor` now checks that each hook event uses the correct tokensave subcommand (e.g. `hook-prompt-submit` for `UserPromptSubmit`, not an invalid or mismatched command).
- **Doctor auto-repairs broken hooks** — when a hook has a wrong subcommand or is missing entirely, `tokensave doctor` replaces it with the correct command automatically.

### Added
- **18 unit tests for Claude hook lifecycle** — install, uninstall, doctor detection, and doctor auto-repair for all three hook events.

## [3.5.0] - 2026-04-13

### Added
- **Per-call token savings reported inline** — every MCP tool response now appends a `tokensave_metrics: before=N after=M` line showing how many raw-file tokens were avoided.
- **`UserPromptSubmit` and `Stop` hooks** — `tokensave install` now registers three hooks (PreToolUse, UserPromptSubmit, Stop) instead of just PreToolUse. Existing installs are silently backfilled on startup.
- **`tokensave current-counter` / `reset-counter` commands** — expose and reset a per-project local token counter, separate from the lifetime total.
- **Respect global gitignore** for `.tokensave` warning.

### Changed
- **Hook install/uninstall generalized** — `install_hook` and `uninstall_hook` now iterate over all three hook events.
- **Sync uses mtime/size pre-filter** — skips hashing unchanged files, only reads files whose mtime or size changed since last sync.
- **Dependency upgrades** — dialoguer 0.11→0.12, notify 7→8, sha2 0.10→0.11, zip 6→8, windows-sys 0.59→0.61.

## [3.4.6] - 2026-04-07

### Fixed
- **SQLite FTS corruption from interrupted sync** — handle UTF-16 encoded files, report unreadable files during sync.

## [3.4.5] - 2026-04-07

### Added
- **`--version` / `-V` flag** to CLI.

### Fixed
- Replace `self_update` crate with direct `ureq`+`tar`+`self_replace` implementation for more reliable upgrades.

## [3.4.4] - 2026-04-07

### Fixed
- Fix `tokensave upgrade` ENOENT error on Homebrew symlink installs.

## [3.4.3] - 2026-04-07

### Fixed
- Handle UTF-16 encoded files and report unreadable files during sync.

## [3.4.2] - 2026-04-07

### Added
- **`tokensave channel` command** — show or switch the update channel (stable/beta).

### Fixed
- Cross-workflow Homebrew/Scoop failures on wrong release type.
- Better upgrade error messages when CI is still building.

## [3.4.1] - 2026-04-07

### Fixed
- Beta Homebrew bottle 404 — fix bottle archive naming.
- Update notices now suggest `tokensave upgrade` instead of platform-specific commands.

## [3.4.0] - 2026-04-07

### Added
- **`tokensave upgrade` command** — self-update the binary directly from GitHub releases. Detects the current channel, downloads the correct platform-specific archive, and replaces the running binary.
- **Annotation/attribute extraction for 7 languages** — Rust, Swift, Dart, Scala, PHP, C++, and VB.NET. All create `AnnotationUsage` nodes with `Annotates` edges. Brings annotation support to 12 of 31 languages.
- **McpTransport trait** — zero-cost abstraction for MCP server I/O, enabling in-memory test transports.
- **370+ new tests** — line coverage 71% → 84%.

## [3.3.3] - 2026-04-05

### Added
- `tokensave sync --doctor` lists added/modified/removed files.

## [3.3.2] - 2026-04-05

### Fixed
- **Windows build failure blocking Homebrew/Scoop updates** — `SHELLEXECUTEINFOW` in `windows-sys` 0.59 requires the `Win32_System_Registry` feature flag, which was missing. This caused Windows CI builds to fail since v3.2.0, and because the release workflow used `fail-fast: true`, the failure cascaded to skip the Homebrew tap and Scoop bucket update jobs entirely. Users on Homebrew were stuck on v3.1.0. ([#12](https://github.com/aovestdipaperino/tokensave/issues/12))
- **`HANDLE` type mismatch on Windows** — `windows-sys` 0.59 changed `HANDLE` from `usize` to `*mut c_void`. The UAC elevation code now uses `std::ptr::null_mut()` and `.is_null()` instead of literal `0`.
- **Release workflow resilience** — changed build matrix to `fail-fast: false` and downstream jobs (`update-homebrew`, `update-scoop`) to `if: !cancelled()`, so a single platform build failure no longer blocks formula/manifest updates for platforms that succeeded.

## [3.3.1] - 2026-04-05

### Fixed
- **Windows `is_installed()` always returned `false`** — the daemon autostart check via `daemon-kit` used a file-path probe that returns `None` on Windows, so `is_service_installed()` never detected an existing service. This caused `tokensave install` to re-offer autostart every time. Now dispatches to the Windows SCM query that was already implemented but never wired up. (daemon-kit 0.1.4)
- **Windows `--enable-autostart` failed on reinstall** — running `tokensave daemon --enable-autostart` twice would error with "service already exists". The installer now stops and removes the old service before re-creating, making the operation idempotent. (daemon-kit 0.1.4)

### Added
- **Upgrade-aware daemon restart** — the background daemon now snapshots its own binary's mtime and size at startup and checks every 60 seconds. When an upgrade is detected (via `brew upgrade`, `cargo install`, `scoop update`, or any package manager), the daemon flushes pending syncs, logs the event, and exits. The service manager (launchd `KeepAlive`, systemd `Restart=on-failure`, Windows SCM failure actions) automatically relaunches with the new binary. Previously the old version ran until the next reboot or manual restart.
- **Windows SCM failure recovery** — the Windows service is now configured with `ServiceFailureActions` (restart after 5s, then 10s) so the SCM relaunches the daemon after upgrade-triggered exits.
- **Daemon version logging** — the daemon startup log now includes the version (`v3.3.1 started, watching N projects`) so log readers can confirm which version is running after an upgrade restart.

### Changed
- Bumped `daemon-kit` dependency from 0.1.3 to 0.1.4.

## [3.3.0] - 2026-04-05

### Changed
- **Sync progress now matches full-index display** — `tokensave sync` now shows `[current/total] syncing file (ETA: Ns)` with the braille spinner and path truncation, matching the progress display used during initial indexing. Previously sync only showed phase names without file counters or ETA.

### Added
- **MCP tool annotations** — all 34 tools now include `readOnlyHint: true` and a human-friendly `title` in their MCP annotations. Clients that support annotations can run all tokensave tools concurrently without permission prompts and display cleaner tool names.
- **`_meta["anthropic/alwaysLoad"]`** on core tools — `tokensave_context`, `tokensave_search`, and `tokensave_status` are marked for immediate loading, bypassing the client's tool-search round-trip on first use.
- **Server instructions** — the MCP `initialize` response now includes an `instructions` field guiding the model to start with `tokensave_context` and noting all tools are read-only and safe to call in parallel.
- **MCP resources** — three resources exposed via `resources/list` and `resources/read`:
  - `tokensave://status` — graph statistics as JSON
  - `tokensave://files` — indexed file tree grouped by directory
  - `tokensave://overview` — project summary with language distribution and symbol kinds
- **`tokensave_commit_context`** — semantic summary of uncommitted changes for commit message drafting. Returns changed symbols grouped by file role (source/test/config/docs), a suggested commit category, and recent commit subjects for style matching.
- **`tokensave_pr_context`** — semantic diff between two git refs for pull request descriptions. Returns commit log, symbols added/modified, affected tests, and impacted modules.
- **`tokensave_simplify_scan`** — quality analysis of changed files: detects symbol duplications, dead code introductions, complexity hotspots, and high-coupling files.
- **`tokensave_test_map`** — source-to-test mapping at the symbol level. Shows which test functions call which source functions and identifies uncovered symbols.
- **`tokensave_type_hierarchy`** — recursive type hierarchy tree for traits, interfaces, and classes showing all implementors and extenders with file locations.
- **`tokensave_context` extended** — new `include_code` parameter includes source code snippets for key symbols (wires through to the existing context builder). New `mode: "plan"` parameter appends extension points (public traits/interfaces with implementor counts) and test coverage for related modules.

### Changed
- Tool count increased from 29 to 34.
- Trimmed verbose tool descriptions for lower token overhead in deferred tool lists (`tokensave_rank`, `tokensave_coupling`, `tokensave_port_status`, `tokensave_port_order`, `tokensave_affected`, `tokensave_complexity`, `tokensave_doc_coverage`, `tokensave_god_class`, `tokensave_recursion`, `tokensave_inheritance_depth`, `tokensave_distribution`).

## [3.2.2] - 2026-04-05

### Fixed
- **MCP tools no longer warn on patch-only updates** — the `tokensave_status` MCP tool now uses `is_newer_minor_version` instead of `is_newer_version`, so patch-level releases (e.g. 3.2.0 → 3.2.1) no longer trigger update warnings in MCP tool output. The CLI status command continues to show all available updates.
- **Separate beta/stable update channels** — `is_newer_version` now returns `false` for cross-channel comparisons (beta vs stable). Previously a beta user could be told to upgrade to a stable release, or vice versa. Each channel now only sees updates from its own channel.

## [3.1.1] - 2026-04-02

### Fixed
- **Windows daemon service installation** — `tokensave install` and `tokensave daemon --enable-autostart` no longer fail on non-elevated Windows terminals. When administrator privileges are required to register the Windows Service, the process now automatically requests UAC elevation for just the service installation step; everything else continues non-elevated. ([#7](https://github.com/aovestdipaperino/tokensave/issues/7))
- **Quieter version update warnings** — the CLI no longer warns about patch-only releases (e.g. 3.2.0 → 3.2.1); warnings now appear only for minor or major version bumps. The status page (`tokensave_status` MCP tool) continues to show all available updates.

## [3.1.0] - 2026-04-01

### Fixed
- **Edge duplication during incremental sync** — reference resolution was re-resolving ALL unresolved refs on every sync (not just from changed files) and inserting duplicate edges with no deduplication. Over many syncs this caused unbounded DB growth (e.g. 5.1 GB for a 108 MB codebase). A unique index on edges and `INSERT OR IGNORE` now prevent duplicates entirely. A V5 migration automatically deduplicates existing databases on upgrade. ([#5](https://github.com/aovestdipaperino/tokensave/issues/5))

### Added
- **Concurrent sync prevention** — a PID-based lockfile (`.tokensave/sync.lock`) prevents the CLI and the background daemon from running sync simultaneously. If a sync is already in progress, the second attempt fails immediately with a clear error message. Stale locks from crashed processes are reclaimed automatically.
- **`doctor` database compaction** — `tokensave doctor` now opens the project database, reports its size, and runs `VACUUM + ANALYZE` to reclaim space. Particularly useful after upgrading from versions affected by edge duplication.
- **Index design documentation** — new `docs/INDEX-DESIGN.md` describes the full indexing pipeline, database schema, extraction process, reference resolution, incremental sync, and how `diff_context` uses the graph.

## [3.0.1] - 2026-04-01

### Fixed
- **Safe JSON config editing** — `tokensave install` no longer silently destroys agent config files (e.g. `opencode.json`, `settings.json`) when they contain invalid or unparseable JSON. Previously, a parse failure caused the file to be silently replaced with an empty object plus the tokensave entry, wiping all existing configuration.

### Added
- **Atomic backup before config writes** — a `.bak` copy of the original file is created (via atomic staging) before any modification. If the install fails at any point, the original file is untouched and the backup is preserved.
- **Strict JSON/JSONC loading for edits** — new `load_json_file_strict` and `load_jsonc_file_strict` functions return an error (with a helpful hint) when an existing file cannot be parsed, instead of silently returning `{}`.
- **Atomic config writes** — new content is written to a `.new` sibling file first, then atomically renamed into place via `rename(2)`. The original file is never opened for writing, so a crash or interruption cannot leave it half-written.
- **20 regression tests** covering backup creation, strict loading, atomic writes, round-trip validation, and the end-to-end install cycle for both valid and corrupt config files.

## [3.0.0] - 2026-03-28

### Changed
- **Bundled tree-sitter grammars** — all 31 language grammars now come from the `tokensave-large-treesitters` crate (which includes `tokensave-medium-treesitters` and `tokensave-lite-treesitters`). Zero individual `tree-sitter-*` crate dependencies remain in tokensave itself. The grammar provider (`ts_provider`) is a single `LazyLock<HashMap>` lookup, replacing 100+ lines of per-crate match arms.
- **Removed vendored C grammars** — the Protobuf and COBOL grammars previously compiled from C source via `build.rs` are now vendored inside the bundled crate. tokensave no longer needs `cc` as a build dependency.
- **Simplified feature flags** — the `lang-*` feature flags still control which extractors are compiled, but no longer pull in individual grammar crate dependencies (all grammars are always present via the bundle). The `ts-ffi`/`ts-rust`/`ts-both` grammar source selection flags have been removed.

### Added
- **Daemon install prompt** — `tokensave install` now offers to install the background daemon as an autostart service (launchd on macOS, systemd on Linux) after agent configuration. Skips silently in non-interactive mode or when the service is already installed.
- **Last sync / Full sync in status** — the status table header now shows a third row with relative timestamps for the most recent incremental sync and the most recent full reindex, stored in the metadata table.

## [2.4.0] - 2026-03-27

### Added
- **Daemon mode** — `tokensave daemon` watches all tracked projects for file changes and runs incremental syncs automatically; debounce configurable via `daemon_debounce` in `~/.tokensave/config.toml` (default `"15s"`)
- **Daemon management** — `--stop`, `--status`, `--foreground` flags for process control; PID file at `~/.tokensave/daemon.pid`
- **Autostart service** — `--enable-autostart` / `--disable-autostart` generates and manages a launchd plist (macOS) or systemd user unit (Linux); cross-platform via `daemon-kit` crate
- **Doctor daemon checks** — `tokensave doctor` now reports daemon running status and autostart configuration
- **`daemon-kit` crate** — new standalone cross-platform daemon/service toolkit published to crates.io, using `daemonize2` on Unix and `windows-service` on Windows

## [2.3.2] - 2026-03-27

### Added
- **5 new agent integrations** — Copilot (VS Code), Cursor, Zed, Cline, and Roo Code now supported via `tokensave install --agent <id>`; each registers the MCP server in the agent's native config format (VS Code `settings.json`, `~/.cursor/mcp.json`, Zed `settings.json`, Cline/Roo Code `cline_mcp_settings.json`)
- **Auto-detect agents** — running `tokensave install` without `--agent` detects which agents are installed by checking their config directories; if one is found it installs directly, if multiple are found an interactive checkbox selector is shown
- **Installed-agent tracking** — `installed_agents` list in `~/.tokensave/config.toml` tracks which integrations are active; on upgrade from older versions the list is backfilled by scanning existing configs
- **Uninstall-all** — `tokensave uninstall` without `--agent` silently removes all tracked integrations
- **JSONC parser** — VS Code and Zed settings files (JSON with comments and trailing commas) are now parsed correctly

### Changed
- **Renamed `Agent` trait to `AgentIntegration`** and all struct names from `XxxAgent` to `XxxIntegration` for consistency; functions renamed accordingly (`get_integration`, `all_integrations`, etc.)

## [2.3.1] - 2026-03-27

### Changed
- **Version-update warning suppressed for 15 minutes** — the "Update available" notice shown after `sync` and in MCP tool responses is now suppressed for 15 minutes after it was last displayed, reducing noise for frequent users; `tokensave status` always shows the warning regardless of suppression

## [2.3.0] - 2026-03-27

### Added
- **`--skip-folder` flag for sync** — accepts one or more folder names to exclude during indexing (e.g. `tokensave sync --skip-folder tests benches`); each folder is converted to a `folder/**` glob pattern at runtime
- **ETA during full index** — the progress spinner now shows `[current/total]` file counts and an estimated time remaining (e.g. `[12/150] indexing src/main.rs (ETA: 8s)`)

### Changed
- `index_all_with_progress` callback signature now provides `(current, total, path)` for richer progress reporting
- Schema migration re-index also shows `[current/total]` progress

## [2.2.0] - 2026-03-27

### Changed
- **Status table title split into two rows** — top row shows version (left) and country flags (right); bottom row shows token counts right-aligned in green
- **Country flags always shown** — removed `--show-flags` option; flags are now fetched automatically and cached for 30 minutes
- **Fixed table width** — cell width capped at 32 columns (max table width 100), with a derived maximum of 25 display flags
- **Upgraded gix to v0.81.0** — from v0.72.1; added explicit `sha1` feature flag and adapted to new `ControlFlow`-based tree diff API

## [2.1.0] - 2026-03-26

### Added
- **QuickBASIC 4.5 language support** — new `QuickBasicExtractor` handles `.bi` (include) and `.bm` (module) files, sharing the QBasic grammar under the existing `lang-qbasic` feature flag (31 languages total)
- **`gix` for native git operations** — replaced `Command::new("git")` shell-outs with the `gix` crate (minimal features: `revision` + `blob-diff`), removing the runtime dependency on a `git` binary for commit counting and tree diffing
- **Test coverage improvements** — 77 new tests across 6 files:
  - `complexity_test.rs` (18 tests) — direct tests for the complexity counting algorithm: branches, loops, nesting, unsafe blocks, unwrap/expect detection, assertion counting
  - `rust_extraction_test.rs` (17 tests) — Rust extractor: functions, structs, enums, traits, impls, modules, async, visibility, derive macros, call sites
  - `display_test.rs` (10 tests) — formatting functions with boundary values
  - `php_extraction_test.rs` (11 tests) — classes, interfaces, traits, namespaces, enums, visibility, inheritance
  - `ruby_extraction_test.rs` (9 tests) — classes, modules, methods, inheritance, constants, nested classes
  - `quickbasic_extraction_test.rs` (12 tests) — QB4.5-specific parsing (REDIM, SLEEP, ERASE), SUBs, FUNCTIONs, TYPEs, call sites

### Changed
- **Legacy BASIC grammars updated to 0.2.0** — `tree-sitter-qbasic`, `tree-sitter-msbasic2`, and `tree-sitter-gwbasic` bumped from 0.1 to 0.2, adding 27 new AST node types for QuickBasic 4.5 constructs (REDIM, SLEEP, ERASE, SHELL, metacommands, and more)
- `git_commits_since` now uses `gix` revision walk with `ByCommitTimeCutoff` sorting, which is more efficient than the previous `git log` approach as gix stops walking once all queued commits are older than the cutoff
- `handle_changelog` tree diff now uses `gix` tree-to-tree comparison with rename tracking, replacing `git diff --name-only`

## [2.0.3] - 2026-03-26

### Fixed
- **Windows: sync re-adding files** — normalize all relative file paths to forward slashes in the scanner, preventing path mismatch between index and sync on Windows
- **Windows: wrong upgrade command** — detect Scoop installations (`\scoop\` in binary path) and suggest `scoop update tokensave` instead of `cargo install tokensave`
- **Windows: git hook backslashes** — write forward slashes in `core.hooksPath` and the post-commit hook snippet, since Git's shell expects `/` separators
- **Scoop bucket structure** — moved manifest to `bucket/` subdirectory for better compatibility with `scoop update`
- **Double-counted token savings** — "Global" total no longer includes the current project's count; display now shows "Project" and "All projects" labels

## [2.0.2] - 2026-03-26

### Fixed
- COBOL tree-sitter scanner uses fixed-size arrays instead of C99 variable-length arrays, fixing MSVC compilation failure on Windows that blocked the v2.0.0 Scoop manifest update

## [2.0.0] - 2026-03-26

### Added

#### 16 new language extractors (15 → 30 languages)
- **Swift** — classes, structs, protocols, enums, extensions, init constructors, async methods, visibility modifiers, inheritance
- **Bash** — functions, `readonly` constants, `source` imports, command call sites, comment docstrings
- **Lua** — functions, colon-methods (OOP via metatables), `require()` imports, LDoc comments, `local` constants
- **Zig** — structs, enums, unions, pub/private visibility, `@import` resolution, `test` blocks as functions, doc comments
- **Protobuf** — `message` → `ProtoMessage`, `service` → `ProtoService`, `rpc` → `ProtoRpc` (new node kinds), enums, fields with type signatures, nested messages, `oneof`, package, imports
- **Nix** — functions, modules (attrsets), constants, `inherit` as imports, `apply_expression` call sites, `#` comments
- **VB.NET** — classes, structures, interfaces, modules, enums, `Sub`/`Function`, `Sub New` constructors, properties, `Inherits`/`Implements`, XML doc comments
- **PowerShell** — functions, typed constants, `Import-Module` / dot-source imports, command call sites, `<# ... #>` block comments
- **Batch/CMD** — labels as functions, `SET` as constants, `CALL :label` as call sites, `REM` docstrings (no complexity counting — too flat)
- **Perl** — `sub` functions/methods, `package` as modules, `use`/`require` imports, `our` constants, method invocations (`->`), `#` comments
- **Objective-C** — `@interface`/`@implementation`/`@protocol`, instance (`-`) and class (`+`) methods, `@property`, `NS_ENUM`, `#import`, message expression call sites, inheritance and protocol conformance
- **Fortran** — `module`, `program`, `subroutine`, `function`, derived `type` with fields, `type extends()` inheritance, `interface`, `parameter` constants, `use` imports, `!` comments
- **COBOL** — `PROGRAM-ID` as module, paragraph labels as functions, `WORKING-STORAGE` data items as fields/constants, `PERFORM` as call sites, `REM` comments (vendored grammar)
- **MS BASIC 2.0** — subroutine synthesis from `REM...RETURN` blocks, `LET` constants, `GOSUB`/`GOTO` call sites
- **GW-BASIC** — `DEF FN` functions, `WHILE/WEND` loops, subroutine synthesis, typed constants
- **QBasic** — `SUB`/`FUNCTION` blocks, `TYPE...END TYPE` as structs with fields, `CONST`, `DIM SHARED`, `CALL` sites, `SELECT CASE`

#### Enhanced Nix extraction
- **Derivation field extraction** — `mkDerivation`, `mkShell`, `buildPythonPackage`, `buildGoModule`, `buildRustPackage`, `buildNpmPackage` calls have their attrset arguments extracted as `Field` nodes (`pname`, `version`, `buildInputs`, `nativeBuildInputs`, `src`, `meta`, etc.)
- **Import path resolution** — `import ./path.nix` creates a `Use` node with a `Uses` unresolved ref, enabling cross-file dependency tracking via `tokensave_callers` and `tokensave_impact`
- **Flake output schema awareness** — in `flake.nix` files, standard output attributes (`packages`, `devShells`, `apps`, `nixosModules`, `nixosConfigurations`, `overlays`, `lib`, `checks`, `formatter`) are force-classified as `Module` nodes with recursive child extraction

#### Feature flag tiers
- Three compilation tiers via Cargo feature flags to control binary size:
  - **`lite`** (11 languages, always compiled): Rust, Go, Java, Scala, TypeScript/JS, Python, C, C++, Kotlin, C#, Swift
  - **`medium`** (20 languages): lite + Dart, Pascal, PHP, Ruby, Bash, Protobuf, PowerShell, Nix, VB.NET
  - **`full`** (30 languages, default): medium + Lua, Zig, Objective-C, Perl, Batch/CMD, Fortran, COBOL, MS BASIC 2.0, GW-BASIC, QBasic
- Individual `lang-*` feature flags for cherry-picking languages (e.g., `--no-default-features --features lang-nix,lang-bash`)
- `default = ["full"]` — existing users get all 30 languages with no config changes

#### New node kinds
- `ProtoMessage` — Protobuf message definitions
- `ProtoService` — Protobuf service definitions
- `ProtoRpc` — Protobuf RPC method definitions

#### Porting assessment tools
- **`tokensave_port_status`** — compare symbols between source and target directories within the same project to track porting progress; matches by name with cross-language kind compatibility (`class` ↔ `struct`, `interface` ↔ `trait`); reports matched/unmatched/target-only counts and coverage percentage
- **`tokensave_port_order`** — topological sort of source symbols for porting; uses Kahn's algorithm on the internal dependency graph to produce levels (port leaves first, then dependents); detects and reports dependency cycles

#### Agent prompt improvements
- **SQLite fallback instruction** — agents are told to query `.tokensave/tokensave.db` directly via SQL when MCP tools can't answer a code analysis question
- **Improvement feedback loop** — agents propose opening a GitHub issue when they discover an extractor/schema/tool gap, reminding the user to strip sensitive data

### Changed
- Cargo.toml `description` now lists lite-tier languages with "and many more" instead of all 30
- Vendored tree-sitter grammars for Protobuf and COBOL (no compatible crates for tree-sitter 0.26)

### Breaking
- Tree-sitter grammar dependencies for medium/full tier languages are now **optional** behind feature flags. Downstream crates depending on specific extractors must enable the corresponding `lang-*` feature.
- `cargo install tokensave --no-default-features` now builds a **lite** binary (11 languages) instead of the previous 15. To get the old behavior, use `cargo install tokensave` (default = full, 30 languages).
- Three new `NodeKind` variants (`ProtoMessage`, `ProtoService`, `ProtoRpc`) added — code matching exhaustively on `NodeKind` will need updating.

### Upgrade guide
```bash
cargo install tokensave          # or: brew upgrade tokensave
tokensave install                # re-run to get updated prompt rules
tokensave sync --force           # re-index to pick up new language extractors
```

## [1.10.0] - 2026-03-26

### Added
- **Version update notifications** — the MCP server checks GitHub releases (with a 5-minute cache) and warns users when a newer version is available, via both a `notifications/message` logging notification and a text block prepended to tool responses
- **Global git post-commit hook** — `tokensave install` now offers to install a global `post-commit` hook that auto-runs `tokensave sync` after each commit, keeping the index up to date without manual intervention
- MCP `logging` capability advertised in `initialize` response
- Minimal gitconfig parser for reading `core.hooksPath` from `~/.gitconfig` and `~/.config/git/config` without shelling out to `git`
- 12 unit tests for gitconfig parsing, insertion, and tilde expansion

## [1.8.3] - 2026-03-26

### Fixed
- OpenCode MCP config uses `mcp` key (not `mcpServers`) with `"type": "local"` and `"command": [bin, "serve"]` array format, matching the current OpenCode schema
- Removed legacy `~/.opencode.json` fallback — config always writes to `~/.config/opencode/opencode.json` (or `$XDG_CONFIG_HOME`)
- Healthcheck validates the `command` array contains `"serve"` instead of checking `args`

## [1.8.2] - 2026-03-26

### Fixed
- OpenCode config path resolution now checks `~/.config/opencode/opencode.json` (modern location) before `$XDG_CONFIG_HOME` and `~/.opencode.json` (legacy)
- OpenCode prompt path prefers `~/.config/opencode/OPENCODE.md` when the modern config directory exists

## [1.8.1] - 2026-03-26

### Added
- **OpenCode agent** (`tokensave install --agent opencode`) — registers MCP server in `.opencode.json`, appends prompt rules to `OPENCODE.md`; healthcheck validates config and prompt file
- **Codex CLI agent** (`tokensave install --agent codex`) — registers MCP server in `~/.codex/config.toml` with auto-approval for all 27 tools, appends prompt rules to `~/.codex/AGENTS.md`; healthcheck validates config, tool approval counts, and prompt file
- TOML helpers (`load_toml_file`, `write_toml_file`) in agents module for Codex config support
- `TOOL_NAMES` constant with bare tool names (without agent-specific prefix) for cross-agent use

### New files
- `src/agents/opencode.rs` — `OpenCodeAgent` implementing `Agent`
- `src/agents/codex.rs` — `CodexAgent` implementing `Agent`

## [1.8.0] - 2026-03-26

### Added
- **Multi-agent architecture** with a trait-based `Agent` abstraction (`install`, `uninstall`, `healthcheck`) to support CLI agents beyond Claude Code
- `tokensave install [--agent NAME]` replaces `claude-install` — defaults to `claude` when no agent is specified
- `tokensave uninstall [--agent NAME]` replaces `claude-uninstall` — defaults to `claude`
- `tokensave doctor [--agent NAME]` now checks all registered agents by default; use `--agent` to narrow to one
- Agent registry with `get_agent()`, `all_agents()`, and `available_agents()` for programmatic access
- `tokensave install --agent unknown` returns a clear error listing available agents

### Changed
- Extracted ~600 lines of Claude-specific install/uninstall/doctor logic from `main.rs` into `src/agents/claude.rs`
- Shared helpers (`load_json_file`, `write_json_file`, `which_tokensave`, `home_dir`, `DoctorCounters`, `EXPECTED_TOOL_PERMS`) moved to `src/agents/mod.rs`
- Error messages updated from `tokensave claude-install` to `tokensave install`
- Backward compatibility preserved: `tokensave claude-install` and `tokensave claude-uninstall` still work as aliases

### New files
- `src/agents/mod.rs` — `Agent` trait, `InstallContext`, `HealthcheckContext`, `DoctorCounters`, agent registry, shared helpers
- `src/agents/claude.rs` — `ClaudeAgent` implementing `Agent`

## [1.7.1] - 2026-03-25

### Fixed
- Database schema migrations now trigger an automatic full re-index instead of printing a warning asking users to run `tokensave sync --full` manually

### Changed
- Decomposed 6 oversized functions into small orchestrators + helpers for NASA Power of 10 Rule 4 compliance (no function exceeds 47 lines):
  - `run_doctor` (389 → 31 lines + 14 helpers)
  - `claude_install` (265 → 35 lines + 8 helpers)
  - `claude_uninstall` (160 → 16 lines + 6 helpers)
  - `print_status_table` (179 → 22 lines + 6 helpers)
  - `extract_symbols_from_query` (147 → 13 lines + helper)
  - `get_tool_definitions` (445 → 30 lines + 27 per-tool `def_*()` helpers)
- Added 84 `debug_assert!` preconditions and postconditions across 10 source files for NASA Power of 10 Rule 5 compliance (zero overhead in release builds)

## [1.7.0] - 2026-03-25

### Added
- **3 new safety metrics on every function/method node** extracted from the AST during indexing, enabling NASA Power of 10 compliance audits without grep:
  - `unsafe_blocks` — counts unsafe blocks/statements (Rust `unsafe {}`, C# `unsafe {}`)
  - `unchecked_calls` — counts force-unwrap and unchecked operations (Rust `.unwrap()`/`.expect()`, TypeScript `!`, Kotlin `!!`, Java `.get()` on Optional, Scala `.get()`, Ruby `.fetch()`)
  - `assertions` — counts assertion calls per function (Rust `assert!`/`debug_assert!`, Java `assertEquals`, Python `assertEqual`, Go `require`, C++ `EXPECT_EQ`/`ASSERT_TRUE`, and framework-specific variants for all 15 languages)
- Extended `ComplexityConfig` with 6 new fields (`unsafe_types`, `unchecked_types`, `unchecked_methods`, `call_expression_types`, `call_method_field`, `assertion_names`, `macro_invocation_types`) to support cross-language detection
- `count_complexity` now accepts source bytes for method-name and macro-name matching in call expressions
- DB migration V4 adds `unsafe_blocks`, `unchecked_calls`, and `assertions` columns to the nodes table
- `tokensave_node` and `tokensave_complexity` MCP tools now include the 3 new fields in their responses
- Migration log message advises users to run `tokensave sync --full` to populate new columns for existing data

## [1.6.2] - 2026-03-25

### Fixed
- Suppressed the "new tokensave tool(s) not yet permitted" warning when running `tokensave claude-install`, since that command is about to fix the permissions anyway

## [1.6.1] - 2026-03-25

### Fixed
- `claude-install` now registers all 27 tool permissions — 9 tools added in v1.6.0 (`complexity`, `coupling`, `distribution`, `doc_coverage`, `god_class`, `inheritance_depth`, `largest`, `rank`, `recursion`) were missing from `EXPECTED_TOOL_PERMS`, so `claude-install` didn't grant them and `doctor` didn't flag them
- README permissions example updated to show all 27 tools (was showing only 9)
- README: fixed MCP server location reference (`~/.claude.json`, not `~/.claude/settings.json`)

## [1.6.0] - 2026-03-25

### Added
- 9 new MCP tools (27 total) for codebase analytics, code quality, and guideline compliance:
  - `tokensave_rank` — rank nodes by relationship count with direction support (incoming/outgoing); answers "most implemented interface", "class that implements the most interfaces", etc.
  - `tokensave_largest` — rank nodes by line count; find largest classes, longest methods
  - `tokensave_coupling` — rank files by fan-in (most depended-on) or fan-out (most dependencies)
  - `tokensave_inheritance_depth` — find deepest class hierarchies via recursive CTE on extends chains
  - `tokensave_distribution` — node kind breakdown per file/directory with summary mode
  - `tokensave_recursion` — detect recursive/mutually-recursive call cycles (NASA Power of 10, Rule 1)
  - `tokensave_complexity` — rank functions by composite complexity score with real cyclomatic complexity from AST
  - `tokensave_doc_coverage` — find public symbols missing documentation (Rust guidelines M-CANONICAL-DOCS)
  - `tokensave_god_class` — find classes with the most members (methods + fields)
- **Complexity metrics on every function/method node** — 4 new columns extracted from the AST during indexing:
  - `branches` — branching statements (if, match/switch arms, ternary, catch). CC = branches + 1.
  - `loops` — loop constructs (for, while, loop, do). Enables NASA Rule 2 audits.
  - `returns` — early exits (return, break, continue, throw).
  - `max_nesting` — deepest brace nesting level. Enables NASA Rule 1 (≤4 levels) audits.
- Generic `count_complexity()` helper with per-language configs for all 15 supported languages
- DB migration V3 adds the 4 complexity columns to the nodes table
- All new tools use efficient SQL queries (JOINs, GROUP BY, recursive CTEs) instead of loading all edges into memory

## [1.5.4] - 2026-03-25

### Fixed
- Token counter inflation: `tokensave_files` no longer accumulates tokens saved (listing file names is metadata, not a file-read substitute)
- Worldwide counter staleness: periodic flush every 30 seconds during MCP sessions instead of only on shutdown
- Shutdown flush was effectively a no-op (delta always 0 because `accumulate_tokens_saved` already upserted the current value to global DB); now uses `last_flushed_tokens` to correctly track remaining delta

## [1.5.1] - 2026-03-25

### Added
- `tokensave doctor` command — comprehensive health check of binary, project index, global DB, user config, Claude Code integration (MCP server, hook, permissions, CLAUDE.md), and network connectivity
- Stale install warning: automatically detects when `claude-install` needs re-running due to new tool permissions and warns on every CLI command

### Added
- 9 new MCP tools (18 total):
  - `tokensave_dead_code` — find unreachable symbols with no incoming edges
  - `tokensave_diff_context` — semantic context for changed files (modified symbols, dependencies, affected tests)
  - `tokensave_module_api` — public API surface of a file or directory
  - `tokensave_circular` — detect circular file dependencies
  - `tokensave_hotspots` — most connected symbols by edge count
  - `tokensave_similar` — find symbols with similar names
  - `tokensave_rename_preview` — all references to a symbol
  - `tokensave_unused_imports` — import statements never referenced
  - `tokensave_changelog` — semantic diff between two git refs
- `get_all_edges()`, `get_nodes_by_file()`, `get_all_nodes()`, `get_incoming_edges()`, `get_outgoing_edges()` delegation methods on `TokenSave`
- `find_circular_dependencies()` graph query for file-level cycle detection
- `tokensave status` prompts to create index if none exists (Y/n)
- Country flags in status output via `--show-flags`

## [1.4.3] - 2026-03-25

### Added
- Country flags row in `tokensave status` — shows emoji flags of countries where tokensave is used, centered below the token counters
- `fetch_country_flags()` in cloud module (500ms timeout, best-effort)
- Flags truncated with ellipsis if they exceed the available table width

## [1.4.2] - 2026-03-25

### Added
- PHP language support (`.php`) — functions, classes, methods, traits, interfaces, enums, constants, properties, namespaces, imports, and call sites
- Ruby language support (`.rb`) — methods, classes, modules, constants, inheritance, and call sites

## [1.4.1] - 2026-03-25

### Added
- Cross-platform release workflow — GitHub Actions builds prebuilt binaries for macOS (ARM), Linux (x86_64, ARM64), and Windows (x86_64) on every release
- Scoop package manager support for Windows (`scoop install tokensave`)
- Automated Scoop bucket updates on release
- Automated Homebrew formula + bottle updates on release

### Changed
- README updated with all install methods (brew, scoop, cargo, prebuilt binaries)

## [1.4.0] - 2026-03-25

### Added
- Worldwide token-saved counter — aggregates anonymous token counts across all tokensave users via Cloudflare Worker + Upstash Redis
- `tokensave status` shows three tiers: Local, Global, and Worldwide token counts
- `tokensave disable-upload-counter` / `tokensave enable-upload-counter` commands to opt out of uploading
- All upload state stored transparently in `~/.tokensave/config.toml`
- Version check on `status` (5-min cache) and `sync` (parallel, no added latency) with auto-detected upgrade command (cargo/brew)
- First-run notice informing users about the worldwide counter and how to opt out
- Flush cooldown (60s) after failed uploads to prevent sluggish CLI during outages
- Network Calls & Privacy section in README documenting all outbound requests

### Changed
- `update_global_db()` now computes token-saved deltas for accurate pending upload accumulation
- Moved Cloudflare Worker source to separate `tokensave-cloud` repository

## [1.3.0] - 2026-03-24

### Added
- User-level global database (`~/.tokensave/global.db`) that tracks all TokenSave projects and their cumulative saved tokens
- `tokensave_status` and CLI `tokensave status` now report both local (project) and global (all projects) tokens saved when the global DB is available
- All CLI entry points (`sync`, `status`, `claude-install` init) register the project in the global DB on every run
- MCP server updates the global DB on every token accumulation and on shutdown (best-effort, no locking)

### Changed
- `print_status_table` title row shows `Local ~X  Global ~Y` when global data is available, falls back to `Tokens saved ~X` otherwise

## [1.2.1] - 2026-03-24

### Fixed
- Renamed all remaining `codegraph` references in release workflow, Homebrew formula, setup script, and hook to `tokensave`
- Release workflow now produces `tokensave` binary, bottles, and source tarballs (was still using `codegraph` names)
- Homebrew formula class renamed from `Codegraph` to `Tokensave` with updated URLs
- Setup script variable `CODEGRAPH_BIN` renamed to `TOKENSAVE_BIN`
- CLAUDE.md marker in setup script updated to use `Tokensave` name

## [1.2.0] - 2026-03-24

### Added
- `claude-install` CLI command — configures Claude Code integration (MCP server, permissions, hook, CLAUDE.md rules) in a single step, replacing the bash `setup.sh` script
- `hook-pre-tool-use` hidden CLI command — cross-platform PreToolUse hook handler written in pure Rust (no bash/jq dependency), blocks Explore agents and exploration-style prompts

### Removed
- Embedded bash hook script — the hook is now a native Rust subcommand

## [1.1.0] - 2026-03-24

### Added
- `tokensave files` CLI command — list indexed files with `--filter` (directory prefix), `--pattern` (glob), and `--json` output
- `tokensave affected` CLI command — BFS through file dependency graph to find test files impacted by source changes; supports `--stdin` (pipe from `git diff --name-only`), `--depth`, `--filter`, `--json`, `--quiet`
- `tokensave_files` MCP tool — file listing with path/pattern filtering, flat or grouped-by-directory output
- `tokensave_affected` MCP tool — find affected test files via file-level dependency traversal
- Graceful shutdown handler for MCP server — persists tokens-saved counter, checkpoints SQLite WAL, and logs session summary on SIGINT/SIGTERM
- `Database::checkpoint()` method for WAL cleanup on shutdown

## [1.0.1] - 2026-03-24

### Changed
- Increased ANSI logo size by 25%

## [1.0.0] - 2026-03-24

### Changed
- **Renamed project from `token-codegraph` to `tokensave`**
- Crate name: `tokensave` (was `token-codegraph`)
- Binary name: `tokensave` (was `codegraph`)
- Data directory: `.tokensave/` (was `.codegraph/`)
- MCP tool prefix: `tokensave_*` (was `codegraph_*`)
- Version bump to 1.0.0

### Added
- TypeScript/JavaScript language support (.ts, .tsx, .js, .jsx)
- Python language support (.py)
- C language support (.c, .h)
- C++ language support (.cpp, .hpp, .cc, .cxx, .hh)
- Kotlin language support (.kt, .kts)
- Dart language support (.dart)
- C# language support (.cs)
- Pascal language support (.pas, .pp, .dpr)
- Legacy `.codegraph/` directory detection with migration warning
- CHANGELOG.md for tracking version history

## [0.6.0]

### Added
- Scala language support (.scala, .sc)

### Fixed
- Self-animating spinner with cursor hiding and path truncation
- Show each language as its own cell in status table

### Changed
- Show indexed languages in status, fix multi-language file discovery

## [0.5.2]

### Changed
- Update repo URLs after GitHub rename to tokensave
- Rename crate to tokensave for crates.io

## [0.5.1]

### Added
- Compact bordered table for status output

## [0.5.0]

### Added
- Java language support (.java)
- Go language support (.go)
- ANSI logo and crates.io readiness

### Changed
- NASA rules compliance improvements

## [0.4.2]

### Added
- Versioned DB migration system with exclusive locking

### Fixed
- Create metadata table on open for existing databases

## [0.4.1]

### Added
- Show version number in tokensave status
- Persist tokens-saved counter to database
- Show indexed token count in tokensave status

### Changed
- Update dependencies

## [0.4.0]

### Added
- Initial Rust language support (.rs)
- Replace rusqlite with native libsql (Turso) crate
- Sync progress spinner and post-commit hook
- Prompt to create index when invoked with no command
- Install section with setup script and hooks

### Changed
- Replace `index` command with `sync --force`

## [0.3.0]

### Added
- MCP tool call logging to stderr
- Merge init and index into a single command

### Fixed
- Harden MCP inputs and prevent path traversal

## [0.2.0]

### Added
- Go extractor with deep extraction support
- Java extractor with deep extraction support
- LanguageExtractor trait and LanguageRegistry for multi-language dispatch
- Runtime stats tracking to MCP server
- Homebrew release workflow

### Fixed
- Sanitize FTS5 search queries to handle special characters
- Address code review findings (UTF-8 safety, FK violations, stats accuracy)

## [0.1.0]

### Added
- MCP server (JSON-RPC 2.0 over stdio)
- CLI interface and TokenSave orchestrator
- Vector embeddings for semantic search
- Context builder for AI-ready code graph context
- Incremental sync for detecting file changes
- Graph traversal and query operations
- Reference resolution module
- Tree-sitter Rust extraction module
- libsql database layer with full CRUD operations
- Configuration module with glob-based file filtering
- Core types and error handling scaffold
