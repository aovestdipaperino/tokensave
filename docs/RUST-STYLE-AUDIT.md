# Rust style audit progress

Tracking issue: [#912](https://github.com/aovestdipaperino/tokensave/issues/912).
This record covers `master` only. It does not establish repository-wide
compliance. Reviews use CONTRIBUTING.md's manual checklist and the project
exceptions in AGENTS.md. The three audit branches start at `7796fddf`; their
baseline module sources match the locally fetched `origin/master` at `6a9a8e54`.

## Related implementations

GitHub merge state verified on 2026-11-16. All three follow-ups below merged
into `master`; merge status does not establish release publication.

| Scope | PR | Merge commit | Validation recorded in PR; limitations |
| --- | --- | --- | --- |
| Feature gates for lite-only extraction code | [#974](https://github.com/aovestdipaperino/tokensave/pull/974) | [5f8d06ee](https://github.com/aovestdipaperino/tokensave/commit/5f8d06ee149750cd96b8342ee3403391aa8f3d37) | Lite tests and feature checks passed; doctests passed on retry; full suite not run locally. |
| Standard string parsing for node/edge kinds and visibility | [#976](https://github.com/aovestdipaperino/tokensave/pull/976) | [40b44015](https://github.com/aovestdipaperino/tokensave/commit/40b44015069a6457d0819b88885801bd77e7aacd) | Formatting/checks passed; local full tests blocked by a Windows file lock. Library API change. |
| Remove unused resolver database field | [#977](https://github.com/aovestdipaperino/tokensave/pull/977) | [7656f098](https://github.com/aovestdipaperino/tokensave/commit/7656f098fe3d93f4dee18f35c6e777f123fa2d50) | Formatting/checks passed; unrelated Clippy findings; local tests not run. Library API change. |
| Context manual-review cleanup | [#1027](https://github.com/aovestdipaperino/tokensave/pull/1027) (`refactor/context-style-audit`), draft | Unmerged | Validation below. Depends only on the existing context APIs, not the other audit draft changes. |

## Context review, 2026-11-16

Files inspected: `src/context/{mod,builder,formatter,ranking,read_cache,read_modes}.rs`,
including inline tests. This first pass addresses imports, avoidable copies,
iteration readability, module documentation, and public parser documentation:

- Qualify imported ranking/text functions through their modules; import types
  rather than repeating their full paths. Scope the digest trait to its user.
- Borrow the exclusion set and formatter symbol list. Build owner scores
  directly without cloning whole search candidates. Borrow code-block labels.
- Name asynchronous query results before iterating. Keep ranking calculations,
  stable sorting, filters, query arguments, and error handling unchanged.
- Replace string `+` concatenation with `format!`, retaining the same text.
- Add module/API docs, correct the character-based signature-limit description,
  and remove outdated blanket compliance comments. Explain test lint allowances.

`mod.rs` already contains only declarations and re-exports. There are no new
boolean parameters, unsafe sites, shared owners, background tasks, SQL statements,
or persistent caches. Removed temporary collections were request-scoped and
input-proportional. The executable-body loop keeps input and output buffers
within the existing query bound (at most `max_nodes.saturating_mul(4)` matches);
explicit iteration may change allocator reuse compared with `collect`.
Temporary buffers still drop within the request. No claim about leak freedom
or measured memory improvement is made.

The follow-up review replaces the private entry-point/diagnostics tuple with
`EntryPointSearch`, uses descriptive lifetimes, and simplifies query-token and
executable-body pipelines into explicit loops. Query order, deduplication,
short-circuit filters, scoring, and caps remain unchanged.

The public `context::builder` path and optional read-mode/range parsers are
retained with nearby compatibility rationales for PR review. Adding parser error
types has no application caller that needs them. Function-local tuple keys and
formatting records remain local; no unclear boolean API parameters were found.
The parent module contains declarations/re-exports only. No unsafe sites or
new macros were introduced; existing formatting/JSON macros express output.

## Parallel module reviews

Each implementation is published from the user fork in a separate draft PR
targeting upstream `master` and is unmerged. Their
reports describe their own source only; context validation does not validate
the other branches. Independent static review found no concrete behavior
regression in the graph or resolution record/enum migrations; tests below are
reported separately from that static review.

| Scope | Local branch | Status and dependencies |
| --- | --- | --- |
| Context | [#1027](https://github.com/aovestdipaperino/tokensave/pull/1027), `refactor/context-style-audit` | Manual follow-up implemented; current validation below. |
| Graph | [#1028](https://github.com/aovestdipaperino/tokensave/pull/1028), `refactor/graph-style-audit` | Commit `dcffca46` in `/tmp/tokensave-graph-style`; 12 library tests and 59 integration tests passed (one manual benchmark ignored); formatting, Clippy and wiki checks passed. Independent of context. |
| Resolution | [#1029](https://github.com/aovestdipaperino/tokensave/pull/1029), `refactor/resolution-style-audit` | Commit `7a171c0e` in `/tmp/tokensave-resolution-style`; 6 library, 90 scoped lite integration, and 87 Ruby/GDScript-enabled integration tests passed; formatting, Clippy and wiki checks passed. Independent of context. |

The graph review found a separate functional defect:
[#1026](https://github.com/aovestdipaperino/tokensave/issues/1026). A one-way edge
between two distinct cyclic components is incorrectly included in the cyclic
edge count. A standalone harness compiled the unmodified repository health/SCC
modules with pinned Rust 1.100.0 and reproduced `(0.0, 5)` for four cyclic edges
plus one bridge. Expected cyclic-edge count is four (score approximately 0.2).
This finding is tracked separately and is not fixed by the style cleanup.

## Validation

Linux, pinned Rust 1.100.0, lite features:

- `cargo test --lib --locked --no-default-features --features lite context::`:
  passed (91 passed, one manual benchmark ignored; includes matching
  profile context tests).
- `cargo test --test integration --locked --no-default-features --features lite context`:
  passed (46 tests, including context-related integration regressions).
- `cargo clippy --lib --locked --no-default-features --features lite`: passed;
  no context warnings. Reports an unrelated `single_match_else` warning at
  `src/mcp/tools/handlers/graph.rs:1329` and the redundant Cargo homepage warning.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `python3 scripts/wiki/sync_wiki.py check`: passed.
- `python3 -m unittest discover -s scripts/wiki`: 11 tests passed.

Other branch validation was performed in each branch's own worktree, not on a
combined implementation:

- Graph: library `graph::` (12 passed, one manual benchmark ignored), integration
  `graph_test::`, `path_search_test::`, `module_imports_test::` (59 passed).
  Details: `docs/RUST-STYLE-GRAPH-AUDIT.md` on `refactor/graph-style-audit`.
- Resolution: library `resolution::` (6 passed); explicitly scoped resolution,
  variant, TypeScript specifier, C# receiver, incremental and Go integration
  modules (90 passed); GDScript/Ruby-enabled integration (87 passed).
  Details: `docs/RUST-STYLE-RESOLUTION-AUDIT.md` on
  `refactor/resolution-style-audit`.
- Both branches passed lite library Clippy, formatting, wiki link checks and
  the 11 wiki tests. Clippy reported only the same unrelated MCP style and
  redundant-homepage warnings. Source diffs contain no SQL changes.

Initial resolution lite runs reused shared build artifacts and were discarded.
After `cargo clean -p tokensave` against the shared target, fresh logs explicitly
show compilation from the resolution worktree. A broad `go_` filter also matched
an unrelated install test, which failed once with Linux `ExecutableFileBusy`
(error 26): 97 passed, one failed. The isolated retry passed, the broad retry
passed 98 tests, and the final explicit module run passed 90, without source
changes. Logs and exit records remain in `/tmp/*-style-validation-*.log` and
`/tmp/module-style-validation-exit-codes.jsonl`.

The upstream `master` AGENTS.md blob matched this branch's file
(`1712682f4e5222e70fd7a6798095ddf11feddb1f`); no guidance update was needed.
No beta files changed. Full-feature/workspace tests, Windows checks, sanitizers,
and memory benchmarks were not run for this cleanup.

## Remaining work

The module cleanups are unmerged and do not complete the repository-wide audit.
Continue module-scoped review of database, extraction, MCP, agents, tokensave,
CLI, standalone tests, and benchmarks. Graph and resolution reports retain
module-specific follow-ups (including naming/imports and private tuple/clone
cleanup); obtain review of the documented compatibility choices. The earlier
broad draft remains in `/tmp/tokensave-style-audit`; it has uncommitted edits
and historical compiler failures and must not be described as a validated
implementation. Split and validate those edits before publication.

Keep #912 open. Update the repository-wide audit wording in AGENTS.md and
CONTRIBUTING.md only after all manual reviews and follow-ups are complete.
`beta` remains outside the style adoption scope.
