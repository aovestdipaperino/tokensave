# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in tokensave, please report it responsibly:

- **Email:** enzinol@gmail.com
- **GitHub:** Open a [private security advisory](https://github.com/aovestdipaperino/tokensave/security/advisories/new)

Please do **not** open a public issue for security vulnerabilities. We aim to acknowledge reports within 48 hours and provide a fix or mitigation plan within 7 days.

## Supported Versions

Only the current major release line is supported. All minor and patch versions within it receive security fixes.

| Version | Supported |
|---------|-----------|
| 7.x (current) | Yes — all minor and patch releases |
| < 7 | No |

**No vulnerabilities have been reported or discovered to date.**

When a vulnerability is found, the fix is shipped as a new release — there are no backports to older major versions. Fixes are not applied in place to existing binaries. **If you run tokensave in production automation (CI pipelines, scheduled agents, server-side MCP deployments), keep it updated to the latest release** so any future fix reaches you immediately via `tokensave upgrade`.

## Security Model

### What tokensave stores

tokensave builds a **local** code graph stored in a SQLite (libSQL) database (`.tokensave/tokensave.db`) inside your project directory. The database contains:

- Symbol names, signatures, and docstrings
- File paths, sizes, and content hashes
- Call relationships and dependency edges
- FTS5 search index (`executable_body_fts`): the `body` column retains the complete text of every indexed function body
- Cross-session memory (recorded decisions and code-area notes)
- A legacy response cache for `tokensave_read` (`read_cache` table): the rendered output served to the agent, stored as a BLOB keyed by file path, mode, and arguments. For full/line-range reads this rendered output contains source text. Since #650 the table is no longer read or written. Since #866 the first writable open of a database deletes the rows earlier releases left there, with SQLite's `secure_delete` on so the freed pages are overwritten; a database that is only ever opened read-only keeps them until a writable open or a rebuild.

The database stores structural metadata, plus source text in the `executable_body_fts` full-text index, whose `body` column retains complete function bodies (and, until it is cleared, the legacy `read_cache` table). Treat `.tokensave/tokensave.db` as containing your source code, and keep it out of version control. The database is local-only — there is no cloud sync, remote database, or server-side storage.

A second database (`~/.tokensave/global.db`) tracks which projects have been indexed, aggregate token-saved counts, and cost accounting data parsed from Claude Code session transcripts. It contains directory paths, counters, per-turn cost/token/category records, and JSONL parse offsets. No source code or conversation content is stored.

### Network access

tokensave makes **no inbound network connections**. It never binds a port or listens for traffic. The MCP server communicates exclusively over stdio.

Outbound connections are limited to:

| Destination | Purpose | Auth | Failure mode |
|-------------|---------|------|-------------|
| `api.github.com` | Check for new releases | None (public API) | Silently ignored |
| `github.com` | Download binary during `tokensave upgrade` | None (public releases) | Error shown to user |
| `tokensave-counter.enzinol.workers.dev` | Aggregate token-saved counter | None | Silently ignored |
| `raw.githubusercontent.com` | Fetch model pricing from [LiteLLM](https://github.com/BerriAI/litellm) | None (public file) | Falls back to embedded pricing |

All best-effort network calls use short timeouts (1-5 seconds) and never block the CLI or MCP server. The pricing fetch (5s timeout) only runs during `tokensave cost` and is cached for 24 hours at `~/.tokensave/pricing.json`.

### No credentials or secrets

tokensave does not require, store, or transmit any credentials, API keys, tokens, or passwords. All external API calls target public, unauthenticated endpoints.

### MCP server tools

The MCP server exposes **more than 80 tools** (one fewer when the optional `ast-grep` binary is not on `PATH`). The large majority are **read-only** analysis and query operations marked `readOnlyHint: true`. A small set mutate local state and are marked `readOnlyHint: false`:

**File-editing tools** (modify source files in your project):

- `tokensave_str_replace`, `tokensave_multi_str_replace` — anchored string replacement
- `tokensave_insert_at`, `tokensave_insert_at_symbol` — anchored insertion
- `tokensave_replace_symbol` — replace a symbol's body
- `tokensave_ast_grep_rewrite` — structural rewrite via the external `ast-grep` binary

**Local-state tools** (write only inside `.tokensave/`, never your source):

- `tokensave_session_start`, `tokensave_session_end` — health-metric baselines
- `tokensave_record_decision`, `tokensave_record_code_area` — cross-session memory

**Test execution:**

- `tokensave_run_affected_tests` — compiles and runs the project's own test suite via a `cargo` subprocess (bounded by a configurable wall-clock timeout, default 300 s, and a per-invocation test cap)

**Path containment.** The edit tools only write inside the indexed project root or another git worktree of the same repository. They resolve symlinks and `..` before checking, and they refuse paths inside `.git` and tokensave's state directory (#866). The indexer follows a symlink inside the root into a directory outside it, so a file the index tracks through such a link stays editable. A repository can contain such links, so review symlinks in a repository you do not trust before indexing it. `tokensave_create_file` is stricter: it accepts only the project root. `tokensave_read` reads only files inside the root of the selected graph (#636), and refuses paths inside `.git`, whose `config` can hold credentials in a remote URL (#866). The check and the following read or write are not atomic, so a symlink swapped in between them by another local process is followed.

The edit tools target a single file with a unique anchor and re-index in place. They never run shell commands you didn't supply, and the server still **cannot** access the network on behalf of the AI agent. Every editing and state-mutating tool is single-file or single-record scoped — there is no bulk-delete or recursive-write primitive.

> Note: file edits are applied by the agent on your behalf through your agent's own tool-approval flow. Treat tokensave's edit tools with the same caution as your agent's built-in file-write tools.

### Ignored and excluded files

Files matched by `.gitignore` or by the `exclude` list (which includes `vendor/` and other dependency directories by default) are never indexed, so no index-backed tool returns them: search, context, symbol, graph and file-listing tools only see what the index holds. `tokensave_read` is different by design: it is a file reader confined to the project, like the agent's own read tool, and it reads an ignored or excluded file inside the project when asked for it by path. It does not list such files, and it refuses `.git`. `tests/security_boundaries_test.rs` pins both behaviors (#866).

### Indexed content is data, not instructions

Repository content (source, comments, documentation) reaches the agent through tool results, and anyone who can commit to the repository controls it. tokensave never turns it into instructions: the MCP server's `initialize` instructions and the rules file `tokensave install` writes both tell the agent to treat that content as data and never follow instructions that appear inside it (#866). That guidance lowers the risk but cannot remove it, because the agent decides what to do with what it reads. Keep your agent's approval prompts on for commands and edits when you work in a repository you do not trust.

### Diagnostics and logs

tokensave's own output while indexing and syncing, and `tokensave status` (including the `--runtime` telemetry snapshot and `--json`), report counts, sizes, timings and file paths, not source text. `tests/security_boundaries_test.rs` indexes a fixture with synthetic secrets in a function body, an ignored file, an excluded file, an invalid UTF-8 file and `.git/config`, and checks that none of them appears in that output (#866); it runs on Linux and macOS. The profile export (#754) keeps only static operation names and aggregates, enforced by its own allowlist and tests. File paths do appear in status and log output, so a report pasted into an issue reveals your project's layout.

### Self-update integrity

`tokensave upgrade` downloads pre-built binaries from [GitHub Releases](https://github.com/aovestdipaperino/tokensave/releases). The upgrade process:

- Downloads from the same release channel (stable/beta) currently installed
- Replaces the running binary in place via `self-replace`
- Re-registers agent integrations on the next launch when the version bump is minor or major

**macOS / Linux:** Release artifacts are not cryptographically signed. The integrity guarantee relies on HTTPS transport security and GitHub's release infrastructure.

**Windows:** Authenticode code signing via the [SignPath.io Foundation](https://signpath.io/foundation) program is being rolled out so Windows binaries are signed as part of the release workflow (addresses the Smart App Control block reported in #79). Until that lands in a published release, Windows binaries remain unsigned.

### No background daemon

tokensave runs **no background daemon, system service, or autostart process**. The standalone `tokensave daemon` command and its launchd/systemd/Windows-Service autostart were removed in 6.0.0. Index freshness is maintained entirely on demand: an on-demand staleness check on each MCP tool call (30-second cooldown) plus a catch-up sync when the MCP server connects. The server lives only for the lifetime of the attached agent and runs with **standard user privileges** — it never requests elevation.

### Subprocess-isolated extraction

Tree-sitter grammars are compiled C/C++ and can crash the process in ways Rust cannot catch. Each file is parsed inside a short-lived worker subprocess (the hidden `extract-worker` subcommand). The worker authenticates against its parent with a 256-bit per-spawn token supplied via the `TOKENSAVE_WORKER_TOKEN` environment variable; a user invoking `tokensave extract-worker` directly fails immediately. Opt out with `TOKENSAVE_DISABLE_SUBPROCESS=1`.

### Unsafe code

The codebase contains minimal `unsafe`, used in two cross-platform places:

- **Memory-mapped monitor ring buffer** (`src/monitor.rs`) — `memmap2` maps `~/.tokensave/monitor.mmap`, the shared buffer the `tokensave monitor` TUI reads
- **Tree-sitter FFI** (`src/extraction/ts_provider.rs`) — constructing the bundled WGSL grammar from its raw C entry point

The Windows-elevation `unsafe` documented in earlier versions was removed alongside the daemon in 6.0.0.

## Best Practices

- Add `.tokensave/` to your `.gitignore` to avoid committing the local database.
- If your project contains sensitive code, be aware that the database stores symbol names and signatures, that the `executable_body_fts` index retains complete function bodies, and that a database last opened for writing by a release before #866 can still hold rendered source text in the legacy `read_cache` table. Adding `.tokensave/` to `.gitignore` keeps them out of version control.
- Keep tokensave updated (`tokensave upgrade`) to receive security fixes.
- Review the [CHANGELOG](CHANGELOG.md) before upgrading to understand what changed.

## Scope

The following are **not** security issues:

- The aggregate token counter sending a count to the public Cloudflare Worker endpoint (this is documented behavior and contains no identifying information beyond an approximate country derived from IP by Cloudflare)
- The database containing symbol names or file paths from your project (this is core functionality)
- The MCP edit tools modifying files (this is opt-in functionality your AI agent invokes through its own tool-approval flow)
- `tokensave_run_affected_tests` compiling and running your project's own test suite (this is the tool's documented purpose)
