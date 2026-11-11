# Database recovery

Tracking: [issue #864](https://github.com/aovestdipaperino/tokensave/issues/864).
This document describes the implementation candidate in PR #861 and its limits;
it does not describe a published release.

## Durability and interrupted indexing

The libSQL backend uses SQLite WAL mode and `synchronous = NORMAL`.
Transactions distinguish committed writes from incomplete writes after a
process exits without closing its connection. This is not a guarantee that
the latest commits survive power loss or storage failure. Indexing consists
of multiple database operations, rather than one atomic graph transaction.
Bulk loading drops secondary indexes and FTS triggers temporarily; a valid
SQLite B-tree alone does not prove that the source graph is complete.

Indexing and synchronization write a project dirty sentinel before changing
the graph and remove it only after success. On reopen, an abandoned sentinel
triggers an integrity check, finalization of any interrupted bulk load, and a
full source reindex. Finalization restores external-content FTS before it can
be cleared again without error. The reindex recreates
the graph, secondary indexes and source search data while retaining memory
decisions and sessions. A live sync lock defers this recovery. Recovery itself
acquires the normal sync lock before integrity checking or bulk-load finalization,
rechecks the sentinel under that lock, and retains the same lock through full
indexing. Failure leaves the sentinel behind and releases the lock through RAII.
Recovery refuses to rebuild a borrowed ancestor database from a different
checkout, including detached HEAD; check out the owning branch to recover it.
The sentinel remains project-wide: it does not identify the interrupted branch,
so switching between tracked branches after a crash remains a limitation.
An active writer can still expose transient state to concurrent readers;
this candidate does not implement atomic snapshot publication.

Corruption detected while opening or checking a dirty database causes an
error with operation `database_corruption`. It does not delete the database
or deliberately remove its WAL/SHM sidecars. Normal SQLite open can replay
WAL and run supported migrations, so this is not a byte-preserving forensic
open. Lock and I/O failures propagate instead of being treated as corruption.
Compatibility refusals still bypass recovery. The separately requested
failed-migration rebuild path retains its existing behavior.

## Manual remediation

1. Stop all TokenSave processes using the project (`tokensave servers` can
   identify servers).
2. Copy the selected database and its `-wal` and `-shm` sidecars together to
   a separate backup location. Preserve branch metadata and configuration as
   well; a project can have multiple branch databases. Do not copy a database
   while a writer is active.
3. Run `tokensave verify --deep` to inspect the state. Verification does not
   migrate or repair databases; `--repair` is unsupported.
4. Restore a known-good backup, or move the damaged database and its sidecars
   aside and run `tokensave init` to recreate source-derived data. Keep the
   damaged copy for possible salvage. Reindexing cannot reconstruct sessions,
   decisions or other user-authored memory that was lost to corruption.

## Consolidated implementation status

| Work | Status and dependencies | Validation and remaining work |
| --- | --- | --- |
| [PR #861](https://github.com/aovestdipaperino/tokensave/pull/861), refs #864 | Open, unmerged contract and runtime recovery candidate on upstream `test/crash-recovery-contract`, targeting `master`; checked 2026-10-29 | Linux lite recovery suite: 23 passed; default `cargo test --workspace --locked` passed (including 3,143 integration tests); broader contract coverage remains outstanding below |

`tests/corruption_test.rs` includes a subprocess that commits a row, leaves a
second row uncommitted and exits without running destructors. Its parent
requires a surviving WAL and checks commit retention and rollback on reopen.
The child has a 30-second deadline and is killed and reaped on timeout. This
is abrupt process exit, not an externally delivered force-kill or power cut.
Additional fixtures corrupt a database header and simulate an incomplete
bulk load; reopening must refuse destructive corruption recovery or rebuild
the source graph while retaining a recorded decision, respectively.
Clean and dirty header-damage fixtures check the stable error operation and
unchanged damaged bytes. A live-lock fixture checks that a reader preserves
the writer's sentinel, then recovery runs after that lock is released.

Reproduce the focused checks with:

```bash
cargo test --locked --test integration corruption_test::
cargo test --locked --test integration database_channel_test::
cargo test --locked --test cancel_shutdown_test
```

Validation on 2026-10-29: the focused lite run used
`cargo test --locked --test integration corruption_test:: --no-default-features --features lite`.
The first run exposed a repeated FTS-clear failure during interrupted-bulk-load
recovery. Finalization before reindex fixed it; the focused rerun and default
workspace suite passed. Formatting and `git diff --check` also passed.
`cargo clippy --workspace --all-targets --locked --message-format short` passed
with warnings in unchanged files (`branch/gc.rs`, `config.rs`, `mcp/server.rs`
and `tests/branch_auto_gc_test.rs`); none referred to the changed Rust files.

The shared integration binary is part of the existing workspace CI command.
Platform results must be recorded before claiming cross-platform guarantees.
Outstanding contract work includes deterministic force-kill points during
initial index, incremental sync and migration, repeated crash/restart cycles,
branch-metadata corruption, and read-only verification of truncated copies.
No automatic backup or general repair mechanism is implemented here.

Resource review: recovery performs one full reindex for an abandoned sentinel,
using existing index batching and concurrency. Its CPU and memory costs scale
with the project as normal full indexing does; it is more expensive than only
finalizing missing indexes, because finalization cannot replace missing source
rows. The new subprocess has bounded polling and a timeout with explicit
reaping. Long-running RSS/CPU measurements and multi-process race testing
remain outstanding; no leak-free claim is made.

## Follow-up review changes

The follow-up holds one writer lock through finalization and reindexing and
refuses rebuilding a borrowed branch database. Regression fixtures cover an
untracked checkout retaining the ancestor graph and dirty sentinel, and a live
writer retaining its unfinished bulk load until lock release. These do not
constitute deterministic force-kill or power-loss coverage. Lock ownership is
bounded to recovery and is released on failure; no additional scans, buffers,
workers, or polling are introduced. Long-running resource measurements remain
outstanding.

Follow-up validation (2026-10-29): the focused lite corruption suite passed
all 25 tests. The full workspace run completed with 3,144 integration tests
passing and one failure in `serve_without_project_test`; library tests had
1,125 passes and two discovery-test failures. The library failures report
`/tmp` as a Git root in this sandbox, whose `/tmp/.git` is read-only. A rerun
with `GIT_CEILING_DIRECTORIES=/tmp` did not remove the failures. The full suite
is therefore not green in this environment; none of the recovery regressions
failed. Formatting and `git diff --check` passed. Changes are local and have
not been pushed to PR #861; direct Git network access was unavailable.
Clippy (`--workspace --all-targets --locked`) passed with warnings in unchanged
files only. No long-running CPU/RSS or cross-platform validation was performed.
