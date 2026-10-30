# Database crash recovery and corruption regression contract

Status: **test plan; not an implementation of automatic repair**.

## Current behavior

`tokensave verify` performs read-only verification. `--deep` includes SQLite integrity checks, while `--repair` currently reports `REPAIR_UNSUPPORTED`. Opening a database may also run compatibility checks/migrations. This proposal must preserve the non-mutating verification contract.

## Invariants

1. A failed or interrupted index operation must never silently expose a partially updated graph as complete.
2. Last committed state must remain readable after a killed writer; rollback/WAL recovery must not depend on a successful CLI shutdown.
3. A corrupt or incompatible database must produce a deterministic actionable error, not be reset or silently overwritten.
4. Recovery operations must not overwrite existing state before making a bounded, explicit backup.
5. Read-only `verify` must never run migrations or repairs.

## Required test matrix

| Case | Setup | Expected result |
|---|---|---|
| Force-kill during initial indexing | Index generated repository, terminate worker repeatedly at different phases | Reopen succeeds or reports a recoverable error; no silent partial success |
| Force-kill during incremental sync | Edit/rename/delete symbols, interrupt sync | Committed snapshot remains internally consistent |
| Kill during migration | Use older supported schema and terminate updater | Compatible clean restart or explicit failure without destructive reset |
| Truncated database | Copy database, truncate copy | `verify --deep` reports failure and does not alter file |
| Damaged branch metadata | Malformed `branch-meta.json` | Diagnostic instead of fallback to wrong branch |
| Lock contention | Hold writer transaction open | Bounded busy handling without deleting/recreating DB |
| Crash-restart cycle | Repeat indexing, kill, restart, verify | Same correctness properties over repeated runs |

## Implementation sequence

1. Create a deterministic subprocess fault-injection integration harness, using disposable repositories and copied fixture databases.
2. Capture pre/post database hashes and WAL state; assert `verify` has no write side effects.
3. Document the interruption points and provide seed/replay controls.
4. Fix any integrity violation discovered; keep recovery behind a separate reviewed PR with a clear data-loss policy.
5. Run on Linux, macOS and Windows (graceful process termination and force-kill must be distinguished).

## Acceptance criteria

- Automated crash tests run against both clean initialization and incremental sync.
- Corrupted fixtures fail safely, with stable diagnostic codes.
- No test changes real user databases or invokes destructive recovery.
- CI publishes failure logs and deterministic reproduction commands.
