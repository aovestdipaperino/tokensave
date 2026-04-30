# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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
