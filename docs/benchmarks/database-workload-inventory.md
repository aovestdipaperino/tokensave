# Database workload and compatibility inventory

Status: reference inventory for the database backend investigation
(#739, #741, #742). It describes the storage layer as it exists; it does not
propose or measure anything.

Source snapshot: `master` at `5357fe1` (schema version 18). Line references
drift; function names are the stable anchors.

## 1. Storage architecture

### Databases and files

| Database | Path | Owner | Notes |
| --- | --- | --- | --- |
| Project graph | `<project>/.tokensave/tokensave.db` (+ `-wal`, `-shm`) | `src/db/` via `Database` | Everything in sections 2–5 below. |
| Branch graphs | `<project>/.tokensave/branches/<branch>.db` | `src/branch.rs` | Created by `VACUUM INTO` snapshot of the parent DB, with a `wal_checkpoint(TRUNCATE)` + file-copy fallback. Same schema as the project graph. |
| Global registry | `~/.tokensave/global.db` | `src/global_db.rs` | Per-project token-savings counters and turn records. Best-effort: failures are ignored. Own pragmas (WAL, `busy_timeout=5000`, `synchronous=NORMAL`). |

All three are opened through `libsql::Builder::new_local`. No remote,
replication or embedded-replica libSQL feature is used.

### Engine and dependency

- `libsql = "0.9.30"` (`Cargo.toml`), patched through `[patch.crates-io]` to
  `vendor/libsql` for the double-close fix (#367, PR #370).
- `libsql-ffi 0.9.30` bundles **SQLite 3.45.1** (libSQL 0.2.3), compiled with
  `SQLITE_ENABLE_FTS5` and `SQLITE_ENABLE_FTS3`.
- API is async (Tokio). Rows, values, parameters and errors are libsql types.

### Connection model

`Database` (`src/db/connection.rs`) owns **one** `libsql::Connection` plus the
retained `libsql::Database` handle. There is no connection pool.

- Writes are serialized by `Database::write_lock` (a `tokio::sync::Mutex`, #563)
  in `edges.rs`, `nodes.rs` and `routes.rs` write paths.
- The MCP server answers read-only tool calls concurrently, bounded by a
  `MAX_CONCURRENT_READS` semaphore (`src/mcp/server.rs`, PR #734). Those calls
  share the same connection.
- `Database::open_read_only` opens a separate read-only connection
  (`query_only = 1`) for cross-branch work (`quality_diff.rs`,
  `mcp/graph_scope.rs`). `verify.rs` opens its own read-only connection with
  `cache_size=-8192`, `mmap_size=0`.
- Opening retries up to five times with 20–160 ms backoff on transient setup
  errors (`SQLITE_MISUSE` under parallel opens on macOS, PR #240).
- Cross-process access (CLI, git hooks, MCP servers) relies on WAL and the
  busy timeout. There is no daemon.

### Pragmas

Applied by `Database::apply_pragmas` on every writable open:

| Pragma | Value | Notes |
| --- | --- | --- |
| `page_size` | 8192 | Only effective on a new database. |
| `journal_mode` | WAL | |
| `foreign_keys` | ON | Turned OFF during bulk load. |
| `busy_timeout` | 120000 ms | `TOKENSAVE_BUSY_TIMEOUT_MS` override. |
| `synchronous` | NORMAL | Not changed during bulk load, despite the `begin_bulk_load` doc comment. |
| `cache_size` | −(25 % of DB size, clamped 2–64 MiB) | `adaptive_cache_sizes`; `initialize` (new DB) uses size 0, `open` uses the file size. |
| `temp_store` | MEMORY | |
| `mmap_size` | 2 × DB size, clamped 0–256 MiB | |

Also used: `user_version` (schema version), `table_info` (migration probes),
`wal_checkpoint(TRUNCATE)` (close, branch fallback, global DB), `quick_check`,
`integrity_check` (`verify.rs`), `query_only`.

## 2. Schema (v18)

Created by `migrations::create_schema` for a fresh DB; older DBs replay
`migrate_v1` … `migrate_v18` inside a `BEGIN EXCLUSIVE` transaction.

| Table | Key | Role |
| --- | --- | --- |
| `nodes` | `id TEXT PK` (+ implicit `rowid`) | Symbols: location, signature, docstring, metrics, `parent_id`, `search_terms`. |
| `edges` | `id INTEGER PK AUTOINCREMENT` | `source`, `target`, `kind`, `line`, `resolved_by`. FK → `nodes` `ON DELETE CASCADE` on both ends. |
| `files` | `path TEXT PK` | Hash, size, mtime, node count, kind. |
| `unresolved_refs` | `id INTEGER PK AUTOINCREMENT` | Pending references. FK → `nodes` cascade. |
| `ambiguous_calls` | `(from_node_id, reference_name, line)` | Ambiguity records (#412). |
| `trait_dispatch_callers` | 4-column PK | Maintained by triggers on `edges`. |
| `node_fingerprints` | `node_id PK` | Duplicate-detection hashes. FK cascade. |
| `vectors` | `node_id PK` | Embedding BLOBs. FK cascade. |
| `metadata` | `key PK` | Key/value settings. |
| `memory_decisions`, `memory_code_areas` | `INTEGER PK AUTOINCREMENT` | Agent memory. |
| `read_cache` | 5-column PK | Per-session read cache with BLOB bodies. |

### Indexes

- `nodes`: `kind`, `name`, `qualified_name`, `file_path`,
  `(file_path, start_line)`, `parent_id`, and the expression index
  `lower(name)`.
- `edges`: `(source, kind)`, `(target, kind)`, `kind`, and the unique
  expression index `idx_edges_unique (source, target, kind, COALESCE(line, -1))`.
- `unresolved_refs`: `from_node_id`, `reference_name`, `file_path`.
- Others: `ambiguous_calls(file_path)`, `trait_dispatch_callers(concrete_method_id)`,
  `node_fingerprints(ast_hash)`, `node_fingerprints(body_tokens)`,
  `memory_code_areas(path)` unique, `memory_decisions(created_at)`,
  `read_cache(session_id, created_at)`.

### Full-text search

| Virtual table | Content | Tokenizer | Maintained by |
| --- | --- | --- | --- |
| `nodes_fts` | external content `nodes` (`content_rowid='rowid'`): `name`, `qualified_name`, `docstring`, `signature`, `search_terms` | `porter unicode61` | `nodes_fts_insert/delete/update` triggers using the FTS5 `'delete'` command; `'rebuild'` in `Database::rebuild_fts`. |
| `executable_body_fts` | regular FTS5 table storing its own `body`, with `node_id`, `file_path` `UNINDEXED` | `unicode61` | Explicit inserts/deletes in `search.rs`. |
| `memory_decisions_fts` | external content `memory_decisions`: `text`, `reason` | default (`unicode61`) | Triggers. |

### Triggers

- Three FTS sync triggers on `nodes` and three on `memory_decisions`.
- Four trait-dispatch triggers on `edges` (`TRAIT_DISPATCH_TRIGGERS_SQL`):
  `AFTER INSERT … WHEN NEW.kind = 'calls' | 'implements'` run a three-way join
  and `INSERT OR IGNORE` into `trait_dispatch_callers`; the matching
  `AFTER DELETE` triggers remove rows. Edge-insert cost therefore depends on
  graph shape, not only on row count.

## 3. Workloads

Each workload names its entry point so a benchmark can call the same code.

### W1. Full index (`tokensave init`, forced reindex)

`src/tokensave/indexing.rs`. Extraction runs in worker subprocesses; storage
work is:

1. `Database::begin_bulk_load`: `foreign_keys = OFF`; drop 14 secondary
   indexes (including `idx_edges_unique`); drop the 3 node FTS triggers and
   4 trait-dispatch triggers; `DELETE FROM nodes_fts`.
2. Batched inserts of nodes, edges, unresolved refs, files, fingerprints and
   executable-body documents in explicit `BEGIN … COMMIT` transactions with
   prepared statements.
3. `resolve_all_streamed`: pages `unresolved_refs` in batches of 25,000
   (`RESOLVE_BATCH`), resolves in memory against name indexes built from
   `get_all_nodes_for_resolution`, writes edges.
4. `Database::end_bulk_load`: delete duplicate edges
   (`rowid NOT IN (SELECT MIN(rowid) … GROUP BY source, target, kind, COALESCE(line, -1))`,
   #318); recreate indexes and triggers; repopulate `nodes_fts` with
   `INSERT … SELECT` from `nodes`; `foreign_keys = ON`.

`Database::needs_bulk_load_finalization` detects an interrupted load (edges
table present, unique index missing).

### W2. Incremental sync

`src/sync.rs` + `indexing.rs`. Runs from git hooks, `tokensave sync`, and the
MCP staleness check (30 s cooldown) and catch-up sync on connect.

- Load `files` rows, stat the tree, hash changed candidates.
- For changed/removed files, `delete_nodes_by_file` runs in one transaction:
  delete the file's `executable_body_fts` rows, select the file's node ids,
  then for **each** node delete its edges (`source = ? OR target = ?`),
  unresolved refs and vectors, and finally `DELETE FROM nodes WHERE file_path = ?`
  (FTS via triggers, fingerprints via `ON DELETE CASCADE`, trait dispatch via
  edge delete triggers). New rows are then inserted with indexes and triggers
  live.
- Re-resolve the touched set of unresolved references.
- No-op sync (nothing changed) is the common case on every MCP call.

### W3. Symbol lookup

Point and filtered reads in `src/db/queries/nodes.rs`: `get_node_by_id`,
`get_nodes_by_ids` (batched `IN (…)`), exact name / qualified name,
case-insensitive name via `lower(name)`, nodes by file, children by
`parent_id`, `search_nodes_by_exact_name`.

### W4. Full-text search

`src/db/queries/search.rs`:

- `to_fts_match_query` turns each whitespace token into a quoted prefix term
  `"tok"*` joined with `OR` (#218).
- `to_fts_phrase_query` builds a separate phrase channel for multi-word
  queries (#362).
- `search_nodes` / `search_nodes_bounded` rank with
  `bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0)` and `LIMIT`.
- `search_executable_bodies` queries `executable_body_fts` with bounded
  concept-pair expressions.
- `batch_incoming_call_counts` feeds ranking.
- Agent memory recall: `memory_decisions_fts MATCH ?` in
  `src/tokensave/memory.rs`.

Literal source scanning (`tokensave_search` literal mode, PRs #733/#734) reads
files, not the database, and must be measured separately.

### W5. Graph traversal and references

`src/graph/traversal.rs` traverses in application code:

- `traverse_bfs` / `traverse_dfs` issue one `get_outgoing_edges` /
  `get_incoming_edges` query per visited node, then one batched
  `get_nodes_by_ids` per expanded node. Round trips grow with the number of
  visited nodes, bounded by `max_depth` and `limit`.
- Built on this: `get_callers*`, `get_callees`, `get_impact_radius*`,
  `get_call_graph`, `get_type_hierarchy`, `find_path*`.
- `get_callers_with_dispatch_depth` also reads the in-memory
  `trait_dispatch_callers` cache.

SQL-side graph work:

- `Database::get_inheritance_depth` (`edges.rs`): `WITH RECURSIVE` over
  `extends` edges with a depth bound of 50 and string-path cycle detection
  (`instr`).
- `gdscript_live_overrides` (`src/graph/queries.rs`): `WITH RECURSIVE … UNION`
  ancestry over `extends`.
- `Database::rebuild_ruby_reopenings` (`edges.rs`): named `WINDOW` with
  `FIRST_VALUE` and `ROW_NUMBER()` inside a transaction.

Whole-graph loads: `get_all_edges` (health/cycle analysis in
`mcp/tools/handlers/health.rs`), `get_all_nodes_for_resolution` (resolver).

### W6. Startup and lifecycle

- New project: `Database::initialize` builds, connects, applies pragmas
  sized for an empty file, runs `create_schema` and loads the trait-dispatch
  cache (`refresh_trait_dispatch_callers`).
- Existing project: `Database::open` sizes `cache_size`/`mmap_size` from the
  file's current size, applies pragmas, runs pending migrations and loads the
  trait-dispatch cache.
- Corruption handling: `quick_check`, `rebuild_fts`, rebuild of failed
  migrations (`open_rebuilding_failed_migration`).
- Shutdown: `Database::checkpoint` (`wal_checkpoint(TRUNCATE)`), then
  `Database::close` drops the connection (native-handle cleanup, PR #704).
- Maintenance: `Database::optimize` runs `VACUUM; ANALYZE;` (`tokensave doctor`).
- Branch snapshot: `branch.rs` `VACUUM INTO ?1`.
- Verification: `verify.rs` `quick_check` / `integrity_check` on a read-only
  connection.

### W7. Concurrency

- Readers: bounded concurrent MCP read calls on one shared connection.
- Writers: one process-local write mutex; across processes, SQLite WAL locking
  with a 120 s busy timeout.
- Mixed: staleness-triggered sync inside an MCP server while reads continue;
  git-hook `sync` in another process.

## 4. SQL feature inventory

Features a replacement engine must support, or for which an equivalent must
be designed and verified.

| Feature | Where | Criticality |
| --- | --- | --- |
| FTS5 virtual tables, external content, `'delete'`/`'rebuild'` commands | `migrations.rs`, `connection.rs` | Required by W4 and W1. |
| FTS5 tokenizers `porter unicode61`, `unicode61` | `nodes_fts`, `executable_body_fts` | Search results depend on stemming. |
| FTS5 query syntax: quoted prefix `"x"*`, `OR`, phrases | `search.rs`, `memory.rs` | Required. |
| `bm25()` with per-column weights | `search.rs` | Required for ranking parity. |
| `WITH RECURSIVE` (`UNION ALL` and `UNION`) | `get_inheritance_depth`, `gdscript_live_overrides` | Required; no application-side fallback exists. |
| Window functions: named `WINDOW`, `FIRST_VALUE`, `ROW_NUMBER` | `rebuild_ruby_reopenings` | Required for Ruby. |
| Triggers with `WHEN`, multi-statement bodies, joins | trait dispatch, FTS sync | Required for W1/W2 correctness. |
| Foreign keys with `ON DELETE CASCADE`, toggled at runtime | schema, bulk load | Required for W2 deletes. |
| Expression indexes (`lower(name)`, `COALESCE(line, -1)` unique) | schema | Required for lookups and edge dedup. |
| `INSERT OR IGNORE`, `INSERT OR REPLACE`, `ON CONFLICT` | about 10 source files | Required. |
| `AUTOINCREMENT`, implicit `rowid`, `content_rowid` | `edges`, FTS | Required. |
| Explicit `BEGIN` / `BEGIN EXCLUSIVE` / `COMMIT` / `ROLLBACK` | `db/queries/*`, migrations, `quality_diff.rs`, `rename.rs` | Required. |
| `PRAGMA user_version`, `table_info`, `foreign_keys`, `query_only` | migrations, open | Required. |
| `PRAGMA wal_checkpoint(TRUNCATE)`, `quick_check`, `integrity_check` | close, branch, verify | Required for lifecycle features. |
| `VACUUM`, `VACUUM INTO`, `ANALYZE` | `optimize`, `branch.rs` | Branch snapshots depend on `VACUUM INTO`. |
| `mmap_size`, `cache_size`, `temp_store`, `page_size` | `apply_pragmas` | Performance only; effect must be verified, not assumed. |
| Scalar functions: `lower`, `substr`, `ltrim`, `instr`, `length`, `COALESCE`, `LIKE` | various | Required. |
| JSON SQL functions | none found | Not required. JSON arrays in `memory_decisions` are parsed in Rust. |

## 5. libSQL API surface

The engine is not behind an abstraction. `libsql::` paths appear in 19
files under `src/`, most heavily `db/queries/edges.rs`, `db/queries/mod.rs` and
`graph/queries.rs`. `Database::conn()` is called directly outside `src/db/`
in `graph/queries.rs`, `graph/imports.rs`, `tokensave/mod.rs`,
`tokensave/memory.rs`, `quality_diff.rs`, `edge_audit.rs` and
`runtime_telemetry.rs`. `branch.rs` and `global_db.rs` build their own
libsql databases.

Consequence for the evaluation: alternative engines can be compared in an
isolated benchmark crate or module that replays the same SQL and data, without
touching production code. A production swap would need either a port of every
call site or a storage trait introduced first.

## 6. Existing measurement coverage

| Tool | Measures | Gaps for this investigation |
| --- | --- | --- |
| `benches/large_repos.rs` (Criterion) | MCP `handle_tool_call` latency for 25 tools on 4 pinned repos (polkadot-sdk, emacs, scipy, node), after a full index. `sample_size=10`, `measurement_time=30s`. | Needs `TOKENSAVE_BENCH_REPOS_DIR` and large clones. Failed calls become `Value::Null` and are still timed. No indexing, sync, startup or concurrency timing. Single engine only. No machine-readable export beyond Criterion's own output. |
| `benches/language_detection.rs` | Extension/language detection. | Not database-related. |
| `tokensave bench` (`src/bench.rs`) | Token savings of context retrieval. | Not a latency benchmark. |
| `src/memstats.rs`, `runtime_telemetry.rs` | RSS, phase attribution, DB/WAL/SHM sizes. | Useful as instrumentation; no comparison harness. |
| `scripts/mcp_probe/` | Functional MCP tool matrix. | Correctness coverage, not performance. |
| `docs/QUERY-OPTIMIZATIONS.md` | Historical optimization notes. | Hypotheses to re-verify. |

## 7. Candidate bottlenecks to profile

These are hypotheses from reading the code, not measured findings.

1. Traversal round trips: two queries per expanded node in BFS/DFS (W5).
2. Trait-dispatch triggers: a three-way join on every `calls`/`implements`
   edge insert during incremental sync (W2).
3. `end_bulk_load`: duplicate-edge `GROUP BY` over all edges, 12 index builds
   and a full `nodes_fts` repopulation in one batch (W1).
4. `delete_nodes_by_file`: three delete statements per node of a changed
   file, plus cascade and trigger work (W2).
5. Shared single connection under concurrent MCP reads (W7).
6. Whole-graph loads for health analysis and resolution (W5, W1).

## 8. Compatibility gates for alternative engines

A candidate cannot be benchmarked on a workload until it passes the gates that
workload needs. Unsupported features must be reported as unsupported, not
replaced silently.

| Gate | Needed by |
| --- | --- |
| FTS5 with external content, porter stemming, prefix/phrase queries, weighted `bm25` | W1, W2, W4 |
| Recursive CTEs | W5 (inheritance depth, GDScript overrides) |
| Named windows with `FIRST_VALUE` / `ROW_NUMBER` | W2 (Ruby) |
| Triggers with `WHEN` and joins; FK cascades | W1, W2 |
| Expression indexes, including unique | W1, W2, W3 |
| WAL-style concurrent readers with one writer across processes | W2, W7 |
| `VACUUM INTO` or an equivalent consistent snapshot | Branch support |
| Opening existing v1–v18 SQLite files | Upgrade path |
