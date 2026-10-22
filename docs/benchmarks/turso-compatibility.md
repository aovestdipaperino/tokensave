# Turso compatibility probes

Status: executable compatibility check for #739. No production code changes and
no migration. Performance is not measured here.

> **Follow-up:** this page answers whether turso runs tokensave's *existing* SQL
> (it doesn't run FTS5). Whether turso's *native* Tantivy full-text search can
> replace FTS5 with adapted DDL and queries, and how fast it is, is measured
> separately in [turso-fts-report.md](turso-fts-report.md) (methodology:
> [turso-fts-methodology.md](turso-fts-methodology.md)). The probe results
> below are unchanged. They were re-run after the crate switched to the system
> allocator (turso's `mimalloc` default feature off; see the methodology), and
> all 220 outcomes were identical.

## Summary

Turso 0.8.3 runs **39 of 55** probes with results identical to SQLite. The
only blocker for tokensave is **FTS5**: Turso doesn't support it, and its own
full-text index can't reproduce tokensave's search behavior. Everything else
tokensave's SQL depends on works, including recursive CTEs, which upstream
documentation lists as unsupported.

| Finding | Severity for tokensave |
| --- | --- |
| No FTS5 module: every `nodes_fts`, `executable_body_fts` and `memory_decisions_fts` probe fails with `no such module: fts5`. | **Blocker.** Search (W4), bulk load (W1) and incremental sync (W2) all depend on it. |
| Turso's native FTS (`CREATE INDEX … USING fts`, Tantivy-based) offers `default`, `raw`, `simple`, `whitespace` and `ngram` tokenizers and per-field weights. There is no stemming tokenizer. | Not a drop-in replacement. `porter unicode61` stemming (for example *running* matching *runs*) can't be reproduced, so ranking parity can't be claimed. Measured in [turso-fts-report.md](turso-fts-report.md). |
| 0.8.3 opens an existing tokensave v18 database and reads its ordinary tables correctly, but `nodes_fts` does not exist for it. | Read-only access to the graph works; search does not. |
| 0.8.3 `PRAGMA integrity_check` on a valid tokensave database (libsql reports `ok`) returns `Page N: never used` for many pages. | **Blocker for `verify.rs`.** A healthy index would be reported as corrupt. The cause was not determined; FTS5 shadow-table pages are a plausible explanation. |
| **0.9.0-pre.1 refuses to open** any existing tokensave database: `Virtual table module not found: fts5`. | The upgrade path gets worse, not better, on the current prerelease. |
| `PRAGMA mmap_size = N` is accepted, but reading it back returns no row. | Accepted but has no effect. Memory-mapping assumptions don't carry over. |
| `execute_batch` rejects batches that include a row-returning pragma (`unexpected row during execution`). The same pragma works through `query()`. | Rust API difference. `Database::apply_pragmas` and `begin_bulk_load` would need restructuring. |
| `VACUUM` requires `experimental_vacuum`. With `experimental_multiprocess_wal` enabled, `VACUUM is incompatible with experimental multiprocess WAL`. `VACUUM INTO` works without either flag. | `tokensave doctor` compaction conflicts with multi-process WAL. Branch snapshots via `VACUUM INTO` are fine. |

Recursive CTEs (`UNION ALL` with the `instr` cycle guard, and `UNION`), named
windows with `FIRST_VALUE`/`ROW_NUMBER`, `WHEN` triggers with joins (the
trait-dispatch triggers, verbatim), FK cascades and runtime toggling,
expression and unique expression indexes, conflict clauses, `AUTOINCREMENT`,
`BEGIN EXCLUSIVE`/`ROLLBACK`, `query_only`, `wal_checkpoint(TRUNCATE)`,
`quick_check`/`integrity_check` on Turso-created files, reopen persistence, a
second handle reading committed data, and plain-table file compatibility in
both directions all pass.

**Consequence for #739:** benchmarking Turso as a tokensave backend (roadmap
item 8) would only cover non-search workloads, unless an FTS replacement is
designed and accepted separately. The probes don't justify building a full
Turso adapter now. Run them again when a Turso release claims FTS5 support.

## What was tested

`benchmarks/db-compat/` is a standalone crate. It is not a member of the
tokensave package and is not built by `cargo build` or CI. It runs each probe
in its own temporary directory on an on-disk database against four
configurations:

| Label | Engine |
| --- | --- |
| `libsql-0.9.30` | Control: the vendored libsql that production uses (bundled SQLite 3.45.1, FTS5 enabled). |
| `turso-0.8.3` | Turso, default builder options (default crate features: `fts`, `mimalloc`). |
| `turso-0.8.3-experimental` | Plus `experimental_vacuum` and `experimental_index_method`. |
| `turso-0.8.3-mp-wal` | Plus `experimental_multiprocess_wal`. |

Each probe expects a literal SQLite result. **The control must pass every
probe** (the runner exits 1 otherwise), which validates the probe before any
Turso outcome is read. Expected-failure probes (constraint violations,
`query_only`) also check that the table is unchanged, so a failure for an
unrelated reason isn't counted as a pass. The SQL follows production
statements: the FTS schema and triggers come from `create_schema`, the CTEs
from `get_inheritance_depth` and `gdscript_live_overrides`, the window from
`rebuild_ruby_reopenings`, and the triggers from `TRAIT_DISPATCH_TRIGGERS_SQL`.

`--existing-db` copies a real, checkpointed tokensave database and replays
production read queries on it. For those probes the expected rows are what
libsql returns on its own copy.

Outcomes: `pass`, `mismatch` (ran, wrong result), `error`, `panic`, `timeout`
(30 s). No probe panicked or timed out.

## Results

Full per-probe tables and JSON, including error messages:

- [`results/turso-0.8.3.md`](../../benchmarks/db-compat/results/turso-0.8.3.md),
  [`.json`](../../benchmarks/db-compat/results/turso-0.8.3.json)
- [`results/turso-0.9.0-pre.1.md`](../../benchmarks/db-compat/results/turso-0.9.0-pre.1.md),
  [`.json`](../../benchmarks/db-compat/results/turso-0.9.0-pre.1.json)

| Gate (probes) | libsql | turso 0.8.3 | turso 0.9.0-pre.1 |
| --- | --- | --- | --- |
| FTS5 (9) | 9 | 0 | 0 |
| Recursive CTE (2) | 2 | 2 | 2 |
| Window (1) | 1 | 1 | 1 |
| Triggers (2) | 2 | 2 | 2 |
| Foreign keys (3) | 3 | 3 | 3 |
| Expression index (2) | 2 | 2 | 2 |
| Conflict clauses (4) | 4 | 4 | 4 |
| Transactions (2) | 2 | 2 | 2 |
| Pragmas (11) | 11 | 10 (`mmap_size`) | 10 (`mmap_size`) |
| Rust API (1) | 1 | 0 | 0 |
| Integrity (2) | 2 | 2 | 2 |
| Maintenance (2) | 2 | 1 (2 with `experimental_vacuum`) | 1 (2 with `experimental_vacuum`) |
| Functions (1) | 1 | 1 | 1 |
| Persistence (1) | 1 | 1 | 1 |
| Concurrency (1) | 1 | 1 | 1 |
| File compatibility (4) | 4 | 2 (plain tables only) | 2 (plain tables only) |
| Existing tokensave DB (7) | 7 | 5 (no FTS, false integrity failure) | 0 (open fails) |
| **Total (55)** | **55** | **39** | **34** |

## Not verified

- **Multi-process access.** The concurrency probe uses two handles in one
  process. CLI, git-hook and MCP processes sharing one file were not tested,
  and SQLite and Turso processes on the same file are documented upstream as
  unsupported.
- **Busy-timeout behavior under real contention, crash recovery and
  power-loss durability.**
- **Pragma effects** beyond read-back: `cache_size`, `synchronous` and
  `temp_store` return the set value, which doesn't prove they change
  behavior.
- **Query plans.** The probes check results, not index use.
- **Historical schemas v1–v17** and migration replay.
- **Turso native FTS relevance.** It was inspected in source (`turso_core`
  0.8.3 `index_method/fts`), not run. Now run in
  [turso-fts-report.md](turso-fts-report.md).
- **macOS and Windows.** All runs were on Linux.
- **Performance.** Not in scope here; see [turso-fts-report.md](turso-fts-report.md)
  for full-text workloads.

## Environment

- Linux 6.18.33.2-microsoft-standard-WSL2, Ubuntu 26.04.1 LTS, 11th Gen Intel
  Core i7-1165G7 (8 threads), 7 GiB RAM.
- rustc 1.98.1. Versions are pinned in `benchmarks/db-compat/Cargo.toml` and
  `Cargo.lock` (`turso = "=0.8.3"`, `turso_core 0.8.3`; libsql from
  `vendor/libsql`).
- Existing database: produced by `tokensave 7.15.0 init` (schema v18) on a
  copy of this repository's `src/db` and `src/graph` (21 files, 558 nodes,
  1,273 edges).
- Base commit: `5357fe1`.

## Reproduce

```sh
cd benchmarks/db-compat
cargo build --release
# Optional real database: index any small project with tokensave, then
cp /path/to/project/.tokensave/tokensave.db target/real-v18.db
./target/release/tokensave-db-compat --existing-db target/real-v18.db --json results/turso-0.8.3.json
```

To test another Turso release, change the `turso` pin in `Cargo.toml`. The
0.9.0-pre.1 run used that change from a scratch copy, with labels renamed
afterwards because engine labels are fixed strings in `src/engine.rs`.
