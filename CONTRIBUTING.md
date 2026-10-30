# Contributing to tokensave

Thanks for your interest in contributing! This guide covers everything you need to get started.

## Getting Started

```bash
git clone https://github.com/aovestdipaperino/tokensave.git
cd tokensave
cargo build --locked
cargo test --workspace --locked
```

Requires **Rust 1.95.0+** (edition 2021). The CI and release toolchain is pinned to **1.98.1**.

## Build and Artifact Lifecycle

Standard Cargo commands define the supported workflow:

```bash
cargo build --locked                     # debug development build
cargo test --workspace --locked          # locked workspace tests
cargo build --release --locked           # optimized release build
cargo install --path . --locked          # optimized locked install into ~/.cargo/bin
```

Development commands use the debug profile; installation uses only the
optimized release output. `target/debug` is never an installation input.

`.cargo/config.toml` sets `TOKENSAVE_SKIP_AGENT_MAINTENANCE=1` for every cargo
process, which keeps the test suite from touching your own agent configuration
(#575). Tests spawn the freshly built binary, whose version is ahead of
whatever you last installed, and that is the signal the silent agent resync
watches for. Unset it only if you are deliberately exercising that path, and
not against your real home directory.

## Project Structure

```
src/
  extraction/    Language-specific extractors (tree-sitter based)
  db/            Database layer (libSQL)
  graph/         Knowledge graph queries and traversal
  mcp/           MCP server (tools + handlers)
  context/       Context builder for AI-ready output
  resolution/    Cross-file reference resolution
  sync.rs        Incremental sync engine
  main.rs        CLI entry point
tests/           Integration tests (one per module/language)
tests/fixtures/  Sample source files for extraction tests
vendor/          Vendored tree-sitter grammars
docs/            Design docs and guides
```

## Feature Flags

tokensave supports more than 50 languages via feature flags (see the README for the full table):

| Feature | Languages |
|---------|-----------|
| `lite` (default subset) | Rust, Go, Java, Scala, TypeScript/JS, Python, C, C++, Kotlin, C#, Swift, Svelte, Astro |
| `medium` | +Dart, Pascal, PHP, Ruby (including ERB and Slim templates), Bash, Protobuf, PowerShell, Nix, VB.NET |
| `full` (default) | +ActionScript, Lua, Zig, Obj-C, Perl, Batch, Fortran, COBOL, the BASIC family, Dockerfile, shader languages (GLSL/WGSL/HLSL/Metal), CUDA/HIP, Markdown, R, SQL, Julia, Haskell, OCaml, Clojure, Erlang, Elixir, F#, F*, Quint, TOML, Lean |

Build with fewer languages for faster compile times during development:

```bash
cargo build --locked --no-default-features --features lite
cargo test --locked --no-default-features --features lite
```

## Making Changes

1. **Fork and branch** from `master` for stable changes, `beta` for experimental features.
2. **Write tests.** Every extraction change should have a corresponding test in `tests/`. Follow the existing pattern: create a fixture in `tests/fixtures/` and assert on extracted nodes/edges.
3. **Run the full test suite** before submitting:
   ```bash
   cargo test --workspace --locked
   ```
4. **Format your code** with the standard Rust toolchain:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --locked
   ```

## Filesystem Path Construction

Contributors and AI coding agents MUST follow the convention established in
[PR #738](https://github.com/aovestdipaperino/tokensave/pull/738) when constructing
filesystem paths in production code and tests:

- Use `Path`/`PathBuf` APIs and one component per `join` or `push` call for
  hard-coded relative paths: `home.join(".pi").join("agent")`, rather than
  `home.join(".pi/agent")`. Do not embed `/` or `\` separators in these components
  or assemble filesystem paths with string concatenation or formatting.
- Keep externally supplied paths intact and preserve their existing resolution
  semantics. This rule does not require splitting user input or changing roots,
  prefixes, `.` or `..` components.
- String joins such as `segments.join("/")`, URLs, import specifiers, and formats
  that explicitly require portable `/` separators are outside this rule.
- A test fixture may retain embedded separators when the path syntax itself is
  what it tests (for example, `a/b/c/../c/tokensave`). Explain the exception in a
  nearby comment so it is distinguishable from ordinary path construction.

Separate component calls let Rust insert the OS-native separator, avoiding mixed
separators in Windows paths and displayed output. Review new and modified path
construction against this rule; do not mechanically rewrite string operations
or intentional path-syntax fixtures.

## Memory and Resource Management

These requirements apply to contributors and AI coding agents. **MUST** and
**MUST NOT** are mandatory; **SHOULD** indicates a recommendation whose omission
should be explained in the PR. Apply them to indexing, synchronization, repeated
MCP requests, and long-running server workloads, including error paths.

### Memory management

- Changes MUST NOT introduce known memory leaks or unbounded memory growth.
- Caches, queues, buffers, and collections MUST have growth limits appropriate
  to their workload and a defined eviction, draining, or release lifecycle.
  Document input-proportional storage and its scope rather than imposing an
  arbitrary fixed cap on all workloads.
- Code MUST avoid unnecessary allocations, cloning, and prolonged retention of
  large objects. Release processed batches and temporary graph data when their
  consumers finish.
- Shared ownership through `Arc` and `Rc` MUST avoid strong-reference cycles and
  unintended retention; use `Weak` or explicit ownership boundaries where needed.
- Resources MUST be released during normal operation, errors, cancellation, and
  shutdown. Intentionally retained process-lifetime data MUST have a documented
  bound and rationale; this does not permit new known leaks.

### CPU efficiency

- Code MUST NOT introduce busy-wait loops or uncontrolled polling. Necessary
  polling MUST have a justified interval or backoff and a termination condition.
- Changes MUST avoid unnecessary filesystem scans, database queries, and repeated
  computations. Preserve existing scope filters and automatic-sync bounds.
- Parallel work MUST use bounded concurrency and appropriate backpressure;
  limiting workers alone is insufficient if queued work can grow without bound.
- Background operations MUST terminate when no longer required, including when
  their owning request or server ends.
- Code SHOULD favor incremental processing and bounded batches, and MUST avoid
  unnecessary lock contention, including holding locks across unrelated I/O.

### Resource lifecycle

- File handles, database connections and transactions, subprocesses, threads,
  and asynchronous tasks MUST have explicit owners and cleanup paths. Use Rust
  ownership and RAII where appropriate.
- Code MUST prevent orphaned background tasks and unintended resource retention.
  Dropping a task or subprocess handle alone MUST NOT be assumed to stop its work;
  arrange cancellation and joining or reaping as appropriate.
- Error, cancellation, and shutdown paths MUST release resources correctly.
  Cleanup that needs asynchronous work MUST have an explicit completion path
  rather than relying solely on `Drop`.

### Regression prevention

- Resource-sensitive changes MUST evaluate potential memory and CPU regressions
  and record the workload, checks, results, and limitations in the PR.
- Validation SHOULD exercise repeated execution and representative long-running
  workloads, including idle periods and cancellation or shutdown where relevant.
- Memory checks SHOULD confirm stabilization after warm-up for a fixed workload,
  allowing for bounded caches and allocator retention. Compare equivalent inputs
  and concurrency; a single RSS sample cannot establish the absence of a leak.
- Intentional, significant increases in resource consumption MUST be documented
  with their rationale and expected scaling.
- Contributors and agents MUST NOT claim code is leak-free without supporting
  evidence, and MUST qualify conclusions by the paths and workloads checked.

Use the existing [memory diagnostics](docs/guide/TOKEN-TRACKING.md#memory-diagnostics) for sampled
RSS and phase attribution, the [automatic-sync and server lifecycle guide](docs/USER-GUIDE.md#how-the-mcp-server-refreshes-the-index)
for operational constraints, and [query optimization notes](docs/QUERY-OPTIMIZATIONS.md)
for existing database strategies. These describe specific mechanisms; the policy
above governs new development. Existing test artifact and process-global-state
requirements remain in [Test layout](#test-layout).

### Resource review checklist

For resource-sensitive changes, contributors and AI coding agents MUST review:

- [ ] Growth bounds, scaling with input, and release or eviction points are defined.
- [ ] Ownership, shared references, and cleanup cover success, errors, cancellation,
      and shutdown, including background work and subprocesses.
- [ ] Scans, queries, polling, concurrency, queues, and lock scope avoid unnecessary
      work and provide backpressure where needed.
- [ ] Memory and CPU regression risks, validation evidence, and untested paths are
      recorded; repeated and long-running checks are considered.
- [ ] Significant resource increases are justified and claims match the evidence.

## Adding a New Language Extractor

1. Add a tree-sitter grammar dependency (or vendor it under `vendor/`).
2. Create `src/extraction/{lang}_extractor.rs` implementing the `Extractor` trait.
3. Register it in the `LanguageRegistry` with a feature flag (e.g., `lang-{name}`).
4. Add a fixture file `tests/fixtures/sample.{ext}` and a test file `tests/{lang}_extraction_test.rs`, then declare it as `mod {lang}_extraction_test;` in `tests/integration.rs` (test files are not auto-discovered; see [Test layout](#test-layout)).
5. Update the feature flag tables in `Cargo.toml` and this document.

## Running Specific Tests

```bash
# All tests for a specific language (a module of the shared integration binary)
cargo test --locked --test integration rust_extraction_test::

# A single test by name
cargo test --locked test_find_stale_files

# Only sync-related tests
cargo test --locked sync

# Faster local loop: only the 11 lite-tier languages (language tests for the
# other tiers are compiled out). CI still runs the full feature set.
cargo test-lite
```

### Test layout

All `tests/*.rs` files are modules of one test binary, `tests/integration.rs`, because every test binary statically links all tree-sitter grammars and SQLite: 180 separate binaries took hundreds of gigabytes of `target/`. `Cargo.toml` sets `autotests = false`, so a new test file must be declared as a `mod` in `tests/integration.rs`, and helpers are shared through `crate::common`.

Tests in the shared binary run concurrently with every other test file, so a test must not mutate process-global state: environment variables (`std::env::set_var`), the current directory, or the `tokensave::cancel` flag. Such a file gets its own `[[test]]` target in `Cargo.toml` instead, next to the existing isolated ones. A test that re-runs its own binary with `--exact` must pass the module-qualified name from `common::qualified_test_name(module_path!(), "...")`.

### Analysis-result stability tests

Run the small metamorphic fixtures with:

```bash
cargo test --locked --test integration analysis_stability_test::
```

`tests/common/stability.rs` captures production `TokenSave` queries, rather than
reimplementing analyzers. The Rust and Python fixtures exercise blank lines,
consistent indentation changes, ordinary comment insertion/removal, CRLF, and
moving independent function declarations within a file. Rust also covers a
controlled rename (including updated call references), independent function
insertion, repeated full indexing, no-op syncs, and bounded concurrent readers.
Every transformed fixture is compared with a fresh full index, then restored
where applicable. Existing `incremental_resolution_test` retains the broader
incremental-versus-full-resolution edit-sequence coverage.

There are four distinct comparison contracts:

- **Exact output:** repeated node reads without writes retain every field;
  sorting by ID only accounts for unspecified query ordering.
- **Normalized findings:** compare sorted vectors of dead-code entities,
  file-cycle components, coupling counts, and structural complexity metrics.
  Vectors retain duplicates. Temporary roots, timestamps, source coordinates,
  and raw IDs are outside this projection. Function signatures retain their
  contents with whitespace runs collapsed; rename cases explicitly map the
  renamed declaration signatures to their original signatures.
- **Semantic graph:** compare every stored relationship by semantic endpoints,
  edge kind, and resolver provenance. Source line numbers are excluded;
  relationship multiplicity is preserved.
- **Finding correspondence:** `(relative path, kind, qualified name)` identifies
  an entity. A controlled rename supplies an explicit mapping back to the old
  entity. Unchanged inputs additionally require identical raw IDs; line edits
  may change line-derived IDs without introducing new semantic findings.

Structural complexity includes all stored branch, loop, early-exit, nesting,
unsafe, unchecked-call, assertion, cognitive and Halstead counts. The composite
complexity ranking includes physical line span, so its score and order are
intentionally outside formatting equivalence. Coupling preserves both fan-in
and fan-out counts. External compiler diagnostics and their severity are not
captured by these fixtures; no diagnostic severity or confidence field is
silently discarded. Resolver provenance is retained.

To add coverage, create a small valid fixture with nonempty baseline assertions,
apply a language-safe transformation, call `sync`, and compare the snapshot with
both its baseline and a new `Project` containing the transformed source. Include
a negative control for any newly captured analyzer. `assert_equivalent` reports
an analyzer, fixture/transformation, and complete before/after rows on failure;
`assert_same_identities` reports raw identity correspondence separately. Avoid
fuzzy matching, set/subset comparisons that hide duplicates, process-global
configuration, and timing-based synchronization. `Project` owns its temporary
directory and closes the database before cleanup, including on test failure.

The cases are deterministic and bounded; no property-testing dependency or
random seed is needed. This first version does not promise arbitrary file-move,
overload, class/member, suppression-directive, documentation-comment, or
cross-language equivalence. Python indentation changes preserve nesting; only
independent definitions are reordered. Concurrent reads do not test simultaneous
writers or establish a public output-order guarantee. Repeated small full
indexes exercise the production parallel extraction path, without timing
assertions or a large repository stress test.

## Documentation

User documentation lives in [`docs/guide/`](docs/guide/README.md) and is published
to the [GitHub Wiki](https://github.com/aovestdipaperino/tokensave/wiki). The
guide is the single source: the wiki is generated from it by
`.github/workflows/wiki.yml` on every push to `master` that changes it, and any
edit made directly in the wiki is overwritten.

- **Keep `README.md` short.** It holds what tokensave is, the installation guide,
  the quick start, and links into the guide. Put details in the guide page for
  that topic, or add a page if no existing page fits.
- **Update the docs with the code.** A change that adds, removes, or changes a
  command, flag, MCP tool, configuration field, environment variable, supported
  agent, or language updates the affected guide pages (and `README.md`, if it is
  mentioned there) in the same pull request.
- **One place per fact.** Explain each topic on one guide page and link to it
  from other pages instead of copying it. The installation commands are the one
  intended duplicate: keep `README.md` and
  [`docs/guide/INSTALLATION.md`](docs/guide/INSTALLATION.md) identical.
- **Linking.** Link to other guide pages with relative links such as
  `[MCP Tools](MCP-TOOLS.md#tools-by-category)`, and to other repository files
  with relative paths. Keep each page's single `# Title`: the wiki names the
  page after it.
- **New pages.** Use an uppercase, hyphenated file name (`NEW-TOPIC.md`) and add
  it to the `wiki-sidebar` list in [`docs/guide/README.md`](docs/guide/README.md)
  and to the table in `README.md`.
- **Check before submitting.** Run `python3 scripts/wiki/sync_wiki.py check` to
  validate links, anchors, and the sidebar, and
  `python3 scripts/wiki/sync_wiki.py build --out /tmp/tokensave-wiki` to
  preview the generated wiki pages.

Design notes, evaluations, and reports stay in `docs/` outside the guide and are
not published to the wiki.

## Environment Variables

Tokensave-owned environment variables must start with `TOKENSAVE_`. Runtime Rust sources are checked by `tests/env_var_namespace_test.rs`; operating-system, Cargo, Git, and agent-owned variables require an explicit allowlist rationale. Use qualified environment APIs such as `std::env::var` so the policy check can identify accesses. Keep variable names as string literals at their access site unless a shared constant is required, and never print environment-variable values because some carry authentication material.

## Commit Messages

Follow conventional commit style:

```
fix: handle UTF-16 encoded files in sync
feat: add Dart annotation extraction
refactor: simplify reference resolver lookup
```

Keep the first line under 72 characters. Add a body explaining *why* if the change isn't obvious.

## Pull Requests

- Target `master` for bug fixes and stable features.
- Target `beta` for experimental or breaking changes.
- Keep PRs focused — one logical change per PR.
- Include test coverage for new behavior.
- Update `CHANGELOG.md` under an `[Unreleased]` section.

## GitHub Issue Lifecycle

These rules apply to every contributor, human or AI agent:

- Create issues for identified bugs and enhancements.
- Never close an issue immediately after creating it.
- Never close an issue simply because an implementation is complete.
- Do not execute `gh issue close` without explicit permission.
- Do not include "Fixes #N" or "Closes #N" in PR descriptions.
- Use "Refs #N" to link pull requests to issues without closing them.
- Leave issue closure to the repository maintainer.

## Reporting Issues

Open an issue at https://github.com/aovestdipaperino/tokensave/issues with:

- tokensave version (`tokensave --version`)
- OS and architecture
- Steps to reproduce
- Expected vs. actual behavior

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). Be respectful and constructive.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
