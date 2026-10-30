# Periodic database integrity checks

Implementation for [the periodic-check comment on #864](https://github.com/aovestdipaperino/tokensave/issues/864#issuecomment-6507699473).

The MCP server checks its selected project database after at least 60 seconds
without incoming requests, at most once per day by default. Set
`integrity_interval_secs` in the project's channel-specific `.tokensave/config.json`
(or `.tokensave/beta/config.json`) to another interval in seconds; `0` disables
periodic checks. Restart the server after changing this setting. Due checks wait
while startup indexing, lazy sync, project sync or requests are in flight.
This is an idle maintenance check, not a daemon or an exact wall-clock schedule.
CLI-only projects and projects accessed only through `graph_root` do not get
periodic checks; use the manual command for those databases.

```sh
tokensave integrity-check .tokensave/tokensave.db
tokensave integrity-check .tokensave/tokensave.db --full
tokensave integrity-check .tokensave/tokensave.db --status
```

Supply the database file named by the relevant branch entry in
`.tokensave/branch-meta.json` when using branch databases. The explicit file
argument also permits checking the local global database. The only implemented
backend is local SQLite through libsql; remote databases are not supported.

Periodic and default manual checks use `PRAGMA quick_check(100)` to inspect
physical structure. `--full` uses `PRAGMA integrity_check(100)`, which additionally
cross-checks SQLite indexes against tables. Neither certifies graph semantics,
source freshness, foreign-key relationships or completeness of an interrupted
index build. Use `tokensave verify [project] [--deep]` for logical checks.
These checkers do not call normal startup, migrate, sync, repair, delete, or
rebuild a database. SQLite can use/create its normal WAL shared-memory sidecar
when reading a live WAL database; the checker does not checkpoint it.

Results are JSON, also saved atomically to `<database>.integrity.json`, separately
from the database so an unreadable database can still have diagnostics. The
record contains start/completion Unix timestamps, mode, outcome and up to 100
messages of 1,024 bytes each. `--status` reads this record without checking the
file. Exit codes are 0 for healthy, 1 for completed checks reporting corruption,
and 2 for incomplete/error/interrupted checks. Lock, permission, incompatible
creation channel and SQL errors are incomplete checks, not proof of corruption.
A leftover `running` record after an abrupt crash is **not** a successful check.
All attempts count toward the configured interval, including interruptions, so
continuous activity or repeated failures cannot cause a retry storm. A manual
check bypasses that interval, but respects the cross-process checker lock.

Failures are logged to server stderr. Preserve the database **and its WAL**
before recovery; inspect the saved diagnostics and run the manual command.
A healthy physical check alone does not justify deleting or rebuilding an index.
Existing crash-recovery behavior outside this checker is unchanged. Optional
unclean-shutdown startup checks are not added by this change.

## Resource ownership and limits

Each database has a nonblocking OS lock (`<database>.integrity.lock`) permitting
one checker at a time. The lockfile is retained; the OS releases the lock when
its owner exits. A checker uses its own read-only connection with no busy wait,
a 2 MiB page cache and no mmap. WAL writers can proceed against the read snapshot;
on other journal modes readers can briefly delay writer commits. There are no
write transactions or shared serving-connection locks.

The server owns at most one child, polls it once per second while active, and
kills/reaps it on incoming activity or shutdown. Every check has a five-second
budget, also enforced by a native watchdog in the child so synchronous SQLite
work and loss of the parent cannot leave an indefinitely running scan. Large
checks may therefore remain incomplete; the tool never reports those as healthy.
The watchdog's error exit leaves an incomplete record if the parent also died.
An idle scheduler wakes once a minute and does not open a database while busy,
disabled, or within the persisted interval. There is no accumulated task queue.

## Validation and status

This work is an unmerged implementation on `periodic-database-integrity`,
referencing #864. It builds on the existing `verify` logical checker and does
not implement all crash-recovery work proposed in the issue. No release or
cross-platform validation is implied by this document.

Focused regressions: `cargo test --test integration integrity_test::`.
The fixtures cover healthy quick/full checks, malformed and damaged files,
concurrent WAL writes and rollback, preservation of database bytes, persisted
interval/disable behavior, checker exclusion, repeated cancellation and handle
release, and logical inconsistencies distinct from physical corruption.
Performance validation covers the no-work busy/disabled paths and bounded child
lifetime; it is not a production throughput or long-running RSS benchmark.
Representative large-repository latency/RSS measurements and Windows/macOS runs
remain necessary before claiming no significant performance degradation.

Validation recorded on Linux, 2026-10-30:

- `TMPDIR=/var/tmp cargo test --workspace --locked --no-fail-fast`: passed,
  including all 11 integrity regressions. The initial sandbox run hit two
  existing discovery tests because its read-only `/tmp/.git` marker makes
  non-repository fixtures appear nested in a repository; clean fixtures passed.
- `TMPDIR=/var/tmp cargo test --locked --features test-transport --test integration serve_idle_timeout_test::`: all three idle/shutdown regressions passed.
- `cargo clippy --workspace --all-targets --locked`: passed with existing warnings.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- 100 persisted interval gates took approximately 2.2 ms on the small fixture;
  a nonexistent worker executable confirmed that skipped checks spawn no process.
- A real CLI server on a disposable indexed project ran its idle check and
  reported healthy, then exited after 65.09 seconds with a 65-second idle timeout.
  The manual `integrity-check --status` command returned the saved healthy report.

These timings are fixture observations, not portable performance thresholds.

## Before merging

Investigate verification overhead before merging this implementation. Measure
actual quick/full check duration, CPU use, disk I/O, peak and sustained RSS,
WAL/checkpoint effects, and query/indexing latency with checking enabled versus
disabled on representative small and large databases. Include concurrent writes,
cold and warm caches, and repeated cancellation. The five-second budget is a
limit, not evidence that verification is cheap; the 65.09-second observation
above measures server idle exit, not database verification duration.

Also review the scheduling policy: this implementation counts interrupted and
failed attempts toward the full interval and replaces the last result. Separate
attempt/completion timestamps, preserving the last completed result, and bounded
shorter retries are proposed follow-ups, not implemented behavior.
