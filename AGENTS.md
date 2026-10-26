# AGENTS.md

This file provides guidance to the agent when working with code in this repository.

## Keeping agent guidance current

Treat `AGENTS.md` on the original repository's `master` branch
(`aovestdipaperino/tokensave`) as the baseline for shared repository guidance.
Before changing this file or preparing a branch for merge, compare it with the
latest upstream `master` version and incorporate missing shared instructions.
Preserve branch-specific instructions and intentional differences; do not
blindly replace the file. If upstream cannot be checked, disclose that limit.

Update this file in the same change whenever build/test commands, architecture,
tooling, contributor policies, or release workflows make its guidance outdated.
Keep shared rules synchronized between `master` and `beta`, and verify the
instructions against the affected branch's files before committing. Report any
remaining differences and why they are needed in the handoff.

## Keep beta executable identity on beta

The executable-identity changes in [PR #802](https://github.com/aovestdipaperino/tokensave/pull/802)
(Refs #801) MUST remain exclusively on `beta`. Merge that PR only into `beta`;
do not backport, cherry-pick, or otherwise propagate these changes to `master`
or any other branch.

This restriction includes the beta executable names, icons, application and
version metadata, operating-system identity, build/packaging configuration,
associated CI changes, and their agent/build guidance. When merging or
synchronizing branches later, explicitly exclude these changes from every
destination other than `beta`.

This policy and the beta-identity build guidance are intentional beta-only
exceptions to the shared-guidance synchronization rule above. Do not copy them
to other branches while synchronizing shared instructions.

## Graphs and diagrams

Every graph and diagram created or updated for repository documentation MUST
use Mermaid. Embed Mermaid source in fenced `mermaid` code blocks in Markdown
so GitHub can render it and contributors can edit it directly.

## README consistency and installation

Keep `README.md` synchronized between `master` and `beta`. Installation
instructions are the only permitted branch-specific difference in README
content; keep all other content the same, including the beta announcement.
Record this rule identically in both branches' `AGENTS.md` files.

- On `master`, preserve the stable installation guide: Homebrew
  (`brew install aovestdipaperino/tap/tokensave`), crates.io
  (`cargo install tokensave`), `cargo binstall tokensave`, Windows Scoop
  (add `https://github.com/aovestdipaperino/scoop-bucket` as the `tokensave`
  bucket, then `scoop install tokensave`), and prebuilt binaries from the
  latest release added to `PATH`. Retain the Cargo medium/lite variants and
  the documented release archive table.
- On `beta`, preserve installation directly from the beta branch:
  `cargo install --git https://github.com/aovestdipaperino/tokensave.git --branch beta --locked --force`.
  Retain the alternative of cloning `beta` and running
  `cargo build --release --locked` with the pinned toolchain, then adding
  the resulting binary to `PATH`, plus the source-based medium/lite variants.
  Keep the explanation that Homebrew, Scoop, crates.io, cargo-binstall, and
  published release binaries install stable releases; no prebuilt beta
  archives are published. Preserve the schema-migration/full-reindex notice,
  disposable-checkout advice, and link to the master guide for returning
  to stable.
- Preserve these installation differences when merging or synchronizing the
  branches. The beta announcement on both branches may continue to show the
  explicit beta installation command.

## README beta announcement

Keep the beta announcement near the top of `README.md` current with the new
release features available on `beta`. When adding or changing beta features,
update the announcement's highlights and links accordingly on both `master`
and `beta`; describe only features already available on the beta branch.
When those features reach a stable release, revise or remove the announcement
so it continues to describe what is available in beta beyond stable. Keep the
installation instructions accurate and link to the beta changelog for details.

## Contributing

Follow [CONTRIBUTING.md](CONTRIBUTING.md) when changing this repository, including
its mandatory [memory and resource management requirements](CONTRIBUTING.md#memory-and-resource-management).
Use the resource review checklist for resource-sensitive changes and report the
checks actually performed, their results, and any limitations in the pull request.

Follow the [GitHub issue lifecycle rules](CONTRIBUTING.md#github-issue-lifecycle):
create issues for identified bugs and enhancements, link PRs with "Refs #N" (never
"Fixes #N" / "Closes #N"), and never close an issue (including via `gh issue close`)
without explicit permission — issue closure is left to the repository maintainer.

## Summaries for related implementations

When a topic spans multiple implementations or related pull requests, the
agent MUST write and maintain a consolidated summary. Include each change's
scope, linked issues/PRs, implementation and merge status, dependencies,
validation results and limitations, and any remaining work. Update the summary
as related changes are added or their status changes, and link it in the final
handoff. Use an existing parent/tracking issue when available; a repository
summary document may hold the detailed record. Do not describe published but
unmerged work as merged, or benchmark candidates as completed implementations.

## SQL query inventory

Keep [docs/QUERY-REPORT.md](docs/QUERY-REPORT.md) synchronized with every SQL
query added, changed, moved, or removed in the report's documented scope, in the
same change as the source modification. This includes SQL templates, shared
fragments, dynamic builders, parameter bindings, migrations, triggers, and
database maintenance statements.

Update the affected SQL entries and construction/binding context, source links,
and inventory counts. Add entries for new queries and remove obsolete entries;
keep existing query IDs stable and assign unused IDs to new entries. If query
coverage changes, update the report's scope and limitations. Before finishing,
review the source diff for SQL changes and verify that each is reflected in the
report. Git history records the changes; the report describes the current source.

## Keep beta query debugging available

All beta builds MUST keep the supported query debugging and testing methods
available on demand: CLI tool replay (`tokensave tool`), SQL tracing and
`EXPLAIN QUERY PLAN` through the global `debug_sql` / `debug_sql_explain`
settings (`tokensave config debug-sql`), MCP handler timings
(`tokensave serve --timings`), and sync diagnostics (`tokensave sync --doctor`
and `--verbose`). Do not remove these methods, compile them out of beta builds,
or require a custom binary to use them. Keep SQL debugging off by default and
preserve runtime activation through global settings; restart existing MCP
servers after changing those settings.

Keep the regression tests and documented reproduction commands usable as beta
evolves. Changes to these paths MUST preserve parameter redaction, bounded
diagnostic output, unchanged CLI/MCP stdout, and application results when
diagnostics fail. Run the relevant SQL wrapper, settings, and CLI/MCP tests
(`cargo test --lib db::debug_sql::tests`,
`cargo test --lib user_config::tests::sql_debug_`, and
`cargo test --test integration sql_debug_cli_test::`). For real-project
init/sync checks, use a disposable snapshot and private home so the user's
index and global settings are preserved. Maintain the coverage and timing
limitations in [the SQL debugging guide](docs/USER-GUIDE.md#sql-debugging-beta).
Refs #789; the initial implementation is PR #790, targeting beta only.

## Build, lint, and test

Follow the mandatory [filesystem path construction rule](CONTRIBUTING.md#filesystem-path-construction):
use one component per `Path`/`PathBuf` `join` or `push` call for hard-coded
relative filesystem paths, as in PR #738. Preserve external path inputs,
portable string formats, and documented path-syntax test fixtures.

- Toolchain is pinned by `rust-toolchain.toml` (Rust 1.98.1, clippy + rustfmt).
- `cargo build` builds the default `full` feature set (50+ languages). For fast dev cycles use `cargo build --no-default-features --features lite` (or `medium`).
- `cargo test --workspace` is the CI test command. On Windows set `RUST_MIN_STACK=8388608` (CI does this; async-heavy tests overflow the 1 MB MSVC default).
- `cargo clippy --workspace --all-targets` (CI runs it with `continue-on-error: true`).
- `cargo fmt --all -- --check`.
- Executable identity is centralized in `build/identity.json`. Stable Cargo builds
  retain their identity; `--features beta` or a prerelease package version selects
  beta metadata. Package independently named binaries with
  `python3 scripts/build-app.py --channel beta` (or `stable`); beta releases pass
  `--version <base>-beta.N`. See `docs/BUILD-IDENTITY.md` for output paths,
  compatibility, and platform validation. The beta release workflow remains disabled.
- `src/lib.rs` denies `clippy::all`, `clippy::unwrap_used`, and `clippy::expect_used`, and warns on `clippy::pedantic` — avoid `unwrap()`/`expect()` in library code.
- Language-specific tests: `cargo test --test integration rust_extraction_test::`; single test: `cargo test test_find_stale_files`; filter: `cargo test sync`. Lite-only local loop: `cargo test-lite`.
- Integration tests live in `tests/` (one file per module/language) but compile into a single binary, `tests/integration.rs` (`autotests = false`): a new test file must be added there as a `mod`. Files that mutate env vars or the `cancel` flag are separate `[[test]]` targets in `Cargo.toml` instead. Helpers are shared via `crate::common` (`tests/common/mod.rs`); fixtures are in `tests/fixtures/`. See CONTRIBUTING.md "Test layout".

### Non-standard test/bench harnesses

- Criterion bench against large real repos: `TOKENSAVE_BENCH_REPOS_DIR=~/tokensave-bench-cache cargo bench --bench large_repos`. Without `TOKENSAVE_BENCH_REPOS_DIR` it registers zero benchmarks. Filter with `cargo bench --bench large_repos -- 'scipy/tokensave_search'`. Repos and pinned refs are in `benches/repos.rs`; delete the `<repo>/.bench-ref` marker to force a re-fetch.
- MCP test-matrix probe: `cargo build --release --bin tokensave && python3 scripts/mcp_probe/probe.py && python3 scripts/mcp_probe/build_matrix.py > matrix.md`. `scripts/mcp_probe/repos.toml` lists target repos (override with `TOKENSAVE_PROBE_REPOS`).
- `tokensave bench` is a separate CLI benchmark (fixed query set, retrieval-savings report).

## Architecture

Pipeline: source files → tree-sitter parse → language extractors → nodes/edges/unresolved refs → reference resolver → SQLite (libSQL) graph → graph queries → CLI + MCP server.

- `src/main.rs` is the CLI + MCP entry point; `src/lib.rs` is the crate root (module declarations + lint config); `src/tokensave/mod.rs` is the `TokenSave` facade that orchestrates indexing/querying/syncing.
- `src/extraction/` — one extractor per language, registered behind feature flags (`lite` always compiled; `medium`; `full` default; `lang-*` flags).
- `src/db/` — SQLite persistence: connection, sequential migrations (`PRAGMA user_version`), query methods.
- `src/resolution/` — cross-file reference resolution (resolver, touched-set tracking, variants).
- `src/graph/` — higher-level algorithms: traversal, queries, SCC/cycles, imports, health.
- `src/context/` — AI-ready context assembly (builder, formatter, ranking, read modes).
- `src/mcp/` — stdio JSON-RPC server, tool definitions, handlers, transport.
- `src/agents/` — agent integrations (Claude, Codex, Cursor, etc.) and git hooks.
- `src/sync.rs` — incremental sync (content hashing, stale/new/removed detection).
- `src/bench.rs` — `tokensave bench` harness.
- `build.rs` compiles vendored grammars (WGSL, ActionScript, GDScript) only when their feature is enabled.

## Setup and environment

- Rust 1.98.1 pinned via `rust-toolchain.toml`.
- `.cargo/config.toml` sets `CFLAGS`/`CXXFLAGS=-DNDEBUG` (disable C/C++ assertions in vendored grammars), isolates `core.hooksPath` for tests that create real git repos, and reserves an 8 MB main-thread stack on Windows MSVC.
- Env-var policy: tokensave-owned variables must start with `TOKENSAVE_`; enforced by `tests/env_var_namespace_test.rs`. External (OS/Cargo/git/agent) vars need an explicit allowlist rationale. Never print env-var values (some carry auth material).
- `TOKENSAVE_WORKER_TOKEN` — auth for the hidden `extract-worker` subcommand (256-bit token, also required as first 32 bytes on stdin). Direct invocation fails. `TOKENSAVE_DISABLE_SUBPROCESS=1` opts out of the worker pool.
- `TOKENSAVE_DISABLE_GREP_HOOK=1` — opt out of the Claude PreToolUse grep guardrail (useful for headless/subagent dispatch).
- `TOKENSAVE_AUTO_TRACK` — per-run override of the `auto_track` config field (truthy values enable; `0`/`false`/`no`/`off`/empty disable).
- Bench vars: `TOKENSAVE_BENCH_REPOS_DIR` (required), `TOKENSAVE_BENCH_REPOS`, `TOKENSAVE_BENCH_SKIP_CLONE`; probe: `TOKENSAVE_PROBE_REPOS`.

## Gotchas and workflow quirks

- **Vendored libsql patch (#367).** `vendor/libsql` is a copy of libsql 0.9.30 with a one-line fix for a double-close use-after-free (intermittent `STATUS_ACCESS_VIOLATION` on Windows). `tests/vendored_libsql_test.rs` fails if the patch and vendored copy drift. `cargo package` strips `[patch.crates-io]` and excludes `vendor/libsql`, so those tests pass trivially in a published crate. Delete the patch + vendor copy together once upstream ships the fix.
- **tree-sitter-dart is forced to 0.2** via a direct dependency because 0.1 fails to wrap top-level declarations, breaking the Dart extractor.
- **Versioning is not SemVer.** Patch/minor/major encode required maintenance: patch = no action, minor = silent reinstall of agent integrations, major = reinstall + forced full reindex. The DB schema has its own version (`LATEST_VERSION` in `src/db/migrations.rs`, stored in `PRAGMA user_version`); reindex detection is schema-driven, not release-version-driven. See `TOKENSAVE-VERSIONING.md`.
- **CHANGELOG.md is huge (~500 KB).** The release workflow extracts the `## [<version>]` block; run `scripts/prepare-release.py` before a release to promote `[Unreleased]` content into it (idempotent).
- **CI disk-space flake.** The Linux runner ships ~10 GB free; the ~713 MB cache restore plus the cargo target dir (34 tree-sitter parsers) can exceed it, so CI frees disk space first.
- **No daemon / file watcher.** Since 6.1.1 the design is on-demand staleness: every MCP tool call checks for stale files (30 s cooldown), and the server runs a catch-up sync on connect. CLI-only workflows rely on git hooks (`scripts/post-commit`, `scripts/post-checkout`) to keep the index fresh.
- **`init` vs `sync`.** `init` is one-time opt-in per project; `sync` only updates already-initialized projects. Global git hooks never create DBs in uninitialized repos.
- **Multi-agent work assumes git worktrees.** tokensave detects nested worktrees and serves the correct branch graph; changes accumulate independently and reconcile via merge/rebase.
- **`codegraph` is a git submodule** (`codegraph/`), not part of the crate.
- **`test-transport` feature** adds test-only JSON-RPC transport hooks in `src/mcp/`.
- **Release workflow** builds per-platform binaries and skips beta/prerelease tags (handled by `release-beta.yml`).
