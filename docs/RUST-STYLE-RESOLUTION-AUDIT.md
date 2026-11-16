# Resolution style audit

Refs [#912](https://github.com/aovestdipaperino/tokensave/issues/912).
This manual-review pass is published as draft
[PR #1029](https://github.com/aovestdipaperino/tokensave/pull/1029), targeting
`master` from `atontini:refactor/resolution-style-audit`, and remains unmerged.
`beta` is excluded. The
[consolidated audit record](https://github.com/atontini/tokensave/blob/refactor/context-style-audit/docs/RUST-STYLE-AUDIT.md)
tracks related PRs, validation and remaining work.

## Scope and changes

Reviewed `src/resolution/{mod,resolver,touched,variants,js_specifier}.rs`
against CONTRIBUTING.md's manual checklist and AGENTS.md's project exceptions.
The earlier draft supplied the private reachability enum, imports and several
documentation corrections. This pass adds:

- Private `Reachability` and `ResultUse` enums make the reachability gate and
  awaited-task handling explicit without changing public signatures or branches.
- Named, borrowed `CallSite` fields replace five-element tuple keys consistently
  in Go sibling suppression, typed-receiver suppression and ambiguity cleanup.
  Caller, file, line, column and bare-name equality remain identical.
- Rayon trait imports live in the sole parallel-resolution method. `Path` and
  `BuildHasher` are imported as types. The reverse-byte iterator is named before
  its loop instead of embedding a combinator chain in the header.
- Directory comparison borrows slices instead of allocating two strings.
  `None` (no slash) and `Some("")` (leading slash) remain distinct.
- Module docs replace misplaced outer docs and blanket compliance comments;
  public touched-key fields/accessors explain their indexing role, and the
  broken `Resolver::resolve_all` link names `ReferenceResolver` correctly.

## Checklist findings and application choices

| Area | Review result |
| --- | --- |
| Imports | No unsupported glob imports; one module per use. Local same-module helpers stay unqualified; external free functions remain module-qualified. |
| Module structure | `mod.rs` contains declarations and re-exports. Helpers and the resolver implementation stay in leaf modules. The private `resolver.rs` name still differs from `ReferenceResolver`; a filename-only rename and updates to cross-module prose remain a follow-up. |
| Ownership | Indexes borrow preloaded nodes. New call-site keys borrow existing strings; no new shared ownership, retained cache or deep copies. `find_best_matches` still clones winning nodes and could return borrowed winners in a separately validated cleanup. |
| API design | No public signature changes. Public `resolve_batch` retains its documented two-result tuple for streaming caller compatibility. Variant input tuples preserve the documented SQL-row contract, avoiding a caller migration/conversion collection. Private bool parameters are now enums. Predicate callbacks still return bool, as predicates should. |
| Naming | Constructors/conversions/accessors reviewed; no extension traits. The main resolver's existing short lifetime names and the small local tuple parser results remain cleanup candidates. |
| Implementation | No check-then-unwrap or panic path introduced; matching, confidence values, scoring, filters, ordering and suppression keys remain unchanged. Longer pre-existing combinator chains and private batch-result tuples remain follow-up candidates. |
| Docs and unsafe | No unsafe sites exist in this module. Public field/accessor and module docs improved. Existing parser docs/tests retain their error behavior. |
| Macros | No new macros; existing match predicates are suitable standard `matches!` uses. |

The tuple API decisions are application-context recommendations, not a claim
that every tuple in the module is fully compliant or exempt from later review.
Do not mark the repository-wide audit complete from this pass.

## Resource and soundness review

No SQL, database opening, filesystem access, FFI, shared process state, threads,
or task lifecycle code changes. Parsing path strings remains portable graph
path syntax, not construction of host filesystem paths. Existing Rayon work
and its request-scoped input-proportional vectors remain unchanged. New enum
values and call-site records add no heap allocation; directory comparisons
remove temporary allocation. Borrow lifetimes statically prevent keys from
outliving source records; suppression retains its existing two-phase keep mask
before mutating references. No memory/performance measurement or leak-free
claim is made.

## Validation

Linux, pinned Rust 1.100.0:

- Lite library `resolution::`: 6 passed.
- Lite integration filters `resolution_`, `variant_edges_bounded_test::`,
  `ts_js_specifier_647_test::`, `csharp_receiver_types_test::`,
  `incremental_resolution_test::`, and the five explicitly named Go modules:
  90 passed.
- Integration with `--no-default-features --features lite,lang-gdscript,lang-ruby`
  and filters `gdscript_extraction_test::`, `ruby_template_extraction_test::`,
  `resolution_test::`: 87 passed.
- `cargo clippy --lib --locked --no-default-features --features lite`: passed;
  only the existing MCP `single_match_else` warning and redundant Cargo homepage
  warning were reported. No resolution warning was reported.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `python3 scripts/wiki/sync_wiki.py check`: passed.
- `python3 -m unittest discover -s scripts/wiki`: 11 passed.

Cargo used the shared target directory under the main checkout. Initial lite
runs reused another worktree's artifacts and are **not validation evidence**.
The coordinating agent ran `cargo clean -p tokensave` for that target, preserving
the dependency cache, then reran lite tests/Clippy. The fresh logs explicitly show
compilation from `/tmp/tokensave-resolution-style`. The Ruby/GDScript run also
explicitly compiled this worktree's source.

The broad `go_` filter unintentionally matched `cargo_` in an unrelated install
test. That run had 97 passes and one `ExecutableFileBusy` (Linux error 26) failure
while spawning a copied fixture binary. The test passed alone, the broad retry
passed 98 tests, and the final explicit module filters passed 90 tests, all
without source changes. This transient failure is retained in the logs rather
than omitted. Logs: `/tmp/resolution-style-validation-*.log`.

Independent static review found no concrete behavioral defect in the call-site
keys, enum mappings, or borrowed directory comparison. This is not an exhaustive
correctness or soundness proof. Full-feature/workspace tests, sanitizers, memory
benchmarks, and Windows checks were not run. Upstream `master` AGENTS.md matched
the local file (`1712682f4e5222e70fd7a6798095ddf11feddb1f`); no shared guidance
change was needed. No beta files changed.
