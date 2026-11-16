# Graph Rust style audit

Module-scoped continuation of [#912](https://github.com/aovestdipaperino/tokensave/issues/912),
reviewed on 2026-11-16. Scope: the seven files under `src/graph/`, including
inline tests. This is a local, unmerged candidate on `refactor/graph-style-audit`;
it does not establish repository-wide compliance.

## Manual checklist

| Category | Review and disposition |
| --- | --- |
| Imports | `health` calls SCC functions through `scc`; `traversal` calls profiling functions through `profile` and `mem::take` through its module. Types used in changed algorithms and tests are imported. Existing qualified libSQL types in `queries` and `imports`, and `tokio::process::Command` in `git`, are deferred so this patch does not churn SQL inventory links or process code. No new glob imports; inline `super::*` imports are permitted. |
| Modules | `mod.rs` contains declarations and re-exports only and now has module docs. The public `graph::traversal` name remains compatible; a nearby comment records why it is not renamed after `GraphTraverser`. `queries` has the same unresolved naming decision for `GraphQueryManager`. The other leaves group related algorithms rather than a single primary type. |
| Ownership | Move owned edges into traversal results instead of cloning them. Keep node clones where the lookup map remains borrowed while producing owned results. SCC node clones retain ownership across maps, stacks and the borrowed input graph. No new shared ownership. |
| APIs | Private SCC visits, queued traversal visits, and predecessor links now have named fields. Public `GraphPath`, caller result tuples, and health score/count pairs preserve their documented field order and existing callers; compatibility comments record that choice. `find_dead_code` keeps its two public boolean parameters; an options/enum API remains a separate compatibility decision. Its parameter names already state which candidates are included. |
| Naming | Existing public constructors use `new`; no extension traits or conversion methods require changes. Existing graph abbreviations (SCC, BFS and DFS) name standard algorithms. The generic database lifetimes in `GraphTraverser` and `GraphQueryManager` remain an open naming improvement. |
| Implementation | Name the combined incoming/outgoing edge iterator before its `for` loop. Keep straightforward map/filter/collect operations and local node/depth and node/edge pairs where destructuring makes meaning immediate. The private hierarchy boolean is always computed by a named predicate, not supplied as an ambiguous literal; that choice is recorded beside its helper. No string `+`, check-then-unwrap pattern, or new macro was introduced. |
| Documentation and unsafe | Add graph/traversal module docs, document health result fields, correct the documented coverage penalty from 10% to the implemented 2%, and give test lint allowances reasons. No unsafe code or native boundary was added or changed. Stale compliance wording in the untouched SQL-bearing `queries.rs` remains deferred. |
| Macros | Existing formatting/assertion macros are standard uses; there are no custom macros to replace. |

The candidate addresses concrete findings in `health.rs`, `scc.rs`,
`traversal.rs`, and `mod.rs`. Deferred items above remain visible; the module
must not be described as fully compliant while they remain unresolved.

## Resource and behavior review

The named private records contain the same fields as the replaced tuples.
All collections remain owned by an invocation and scale with visited nodes or
edges. The SCC work stack remains iterative; changing fields does not add
recursion, neighbor copies, tasks or retained state. Moving result edges removes
copies without changing their order. Traversal depth/expansion limits, filters,
query calls, SQL text, bindings, error handling, stable sorts and connection
ownership are unchanged. No persistent cache, process lifecycle or filesystem
behavior changed. No new resource bound or memory-improvement measurement is
claimed.

## Separate functional finding

[Issue #1026](https://github.com/aovestdipaperino/tokensave/issues/1026) tracks
`acyclicity_score` counting a one-way bridge between two cyclic SCCs as a cyclic
edge. The parent audit reproduced this with a direct pinned-Rust harness.
That issue is pending and intentionally not fixed by this style candidate.

## Validation

Validated independently on Linux with pinned Rust 1.100.0 and lite features:

- `cargo test --lib --locked --no-default-features --features lite graph::`:
  12 passed, one manual benchmark ignored.
- `cargo test --test integration --locked --no-default-features --features lite -- graph_test:: path_search_test:: module_imports_test::`:
  59 passed.
- `cargo clippy --lib --locked --no-default-features --features lite`: passed;
  only the existing MCP `single_match_else` warning and redundant Cargo homepage
  warning were reported.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `python3 scripts/wiki/sync_wiki.py check`: passed.
- `python3 -m unittest discover -s scripts/wiki`: 11 passed.

Cargo test/Clippy logs explicitly show compilation from
`/tmp/tokensave-graph-style`; context or resolution artifacts are not accepted
as graph validation. Independent static review of the named records and moved
edges found no concrete behavioral regression. This is not an exhaustive
correctness or soundness proof.

No full-feature/workspace suite, sanitizer, Miri, long-running resource benchmark
or Windows run was performed. Upstream `master` AGENTS.md was checked and its
blob matched the local file (`1712682f4e5222e70fd7a6798095ddf11feddb1f`);
no shared guidance change was needed. No beta files changed.
