# Database workload benchmark harness

Status: measurement infrastructure for #741 and #742. It times tokensave's
current storage paths through the public `TokenSave` API. Comparing backends
and recording baselines are separate follow-up PRs.

## Run

```sh
cargo bench --bench db_workloads -- \
    [--project DIR] [--out FILE] [--iterations 1000] [--warmup 100] \
    [--index-runs 3] [--inputs 64]
```

- `--project`: the source tree to index. Defaults to this crate's `src/`.
  The tree is **copied into a temporary directory** first (skipping `.git`,
  `.tokensave`, `target`, `node_modules`), so the original is never modified.
- `--out`: the JSON result path. Defaults to
  `target/db-bench/db_workloads-<unix-time>.json`.
- `--iterations` / `--warmup`: timed and untimed operations per lookup,
  search and traversal workload. Sync and open workloads use a tenth of each
  (at least 10 timed and 1 warm-up), because each of their operations is
  10–100× slower.
- `--inputs`: how many sampled symbols to use. Symbols are functions and
  methods, sorted by id and evenly spaced, so the same index always produces
  the same inputs.

A human-readable table goes to stderr. The exit code is 0 on success, 1 if
any operation failed or changed its result, and 2 for setup errors.

## Workloads

| Name | Inventory workload | Operation |
| --- | --- | --- |
| `index.full` | W1 | Remove `.tokensave/`, `TokenSave::init`, `index_all`. |
| `lookup.node_by_id` | W3 | `get_node` |
| `lookup.by_name` | W3 | `get_nodes_by_name` |
| `lookup.by_qualified_name` | W3 | `get_nodes_by_qualified_name` |
| `search.fts_ranked` | W4 | `search(name, 20)` (FTS5 + BM25 + ranking) |
| `graph.callers_d3` / `graph.callees_d3` | W5 | `get_callers` / `get_callees`, depth 3 |
| `graph.impact_d2` | W5 | `get_impact_radius`, depth 2 |
| `sync.noop` | W2 | `sync()` on an unchanged tree |
| `open.existing` | W6 | `TokenSave::open` on the existing index |
| `open.first_search` | W6 | First `search` on that freshly opened handle |

Inputs are used round-robin.

## Correctness gates

- Before timing, every workload runs once per input and stores a
  **fingerprint** of the result: result ids in order (so ranking changes are
  detected), plus the edge count for subgraphs.
- Every timed operation must reproduce its input's fingerprint. Errors and
  mismatches are **counted as failures, never as latency samples**, and any
  failure makes the run exit 1. The existing `large_repos` bench, by
  contrast, maps errors to `Value::Null` and times them.
- Lookups and searches for sampled symbols must return at least one row.
  No-op sync must change zero files.
- `index.full` must produce identical file, node and edge counts on every run.

## Output

`schema: "tokensave-db-workloads/1"` JSON with:

- `environment`: tokensave version, git commit and dirty flag, rustc, engine,
  `sqlite_version()`, OS, kernel, CPU model, logical CPUs, total RAM, build
  profile.
- `dataset`: source path, file, node and edge counts, database size.
- `config`: iterations, warm-up, index runs, inputs, `cache: "warm"`,
  `clients: 1`.
- `workloads[]`: samples, errors (with the first five messages), min, mean,
  standard deviation, p50, p95, p99, max (all in µs), `ops_per_sec`, and
  `p99_reliable` (true when there are at least 100 samples).
- `process_rss_bytes_after_queries`.

Percentiles use the nearest-rank method on successful samples
(`benches/db_workloads/stats.rs`). The integration test binary covers that
math via `tests/db_bench_stats_test.rs`.

## What it does not measure (yet)

- **Cold cache.** All numbers are warm: same process, OS page cache populated.
  Cold-cache runs need privileged cache dropping or a reboot between runs.
- **Concurrency.** There is one client only. Roadmap item 11 covers
  concurrency.
- **Extraction in `index.full`.** Extraction runs **in-process** here: the
  subprocess worker pool only starts from the `tokensave` binary. So
  `index.full` is not the CLI's wall time, and it includes parsing as well as
  storage.
- **MCP transport.** The harness calls the library directly.
- **Datasets.** Only the input tree's own size. Deterministic synthetic tiers
  are roadmap item 3; the large pinned repositories live in
  `benches/large_repos.rs`.
- **Per-workload memory, CPU time and I/O.** Only one process-RSS reading is
  taken.
- **Backend comparisons.** This harness targets the production libsql path.
  Other engines are compared at the SQL level (`benchmarks/db-compat/`) or in
  later adapter PRs.

## Example run

Filled in from an actual run in the PR that introduces the harness; see below.

## Profiling comparison (#754, PR 6)

The harness accepts `--profile off|light|detailed` (default `off`). It sets the
in-process recorder explicitly without reading profiling preferences, starting
persistence, or starting sharing. Each measured API call has an invocation
context for exclusive-time accounting. `workloads[].profile` is a production
`Snapshot`, using the existing static operations and histograms. Warm-up and
correctness-setup samples are drained before timing; draining happens outside
the latency measurement. Opening and first-search snapshots are separate.
`workloads[].fingerprints` lets the comparison reject changed results across
modes and processes, as well as within each workload.

Build the optimized harness, then pass its executable path (Cargo's
`--message-format=json` reports it) to the runner:

```sh
cargo build --release --bench db_workloads --locked --message-format=json
python3 scripts/profile-overhead.py --binary target/release/deps/db_workloads-HASH \
    --project benchmarks/profile-fixture --out target/profile-overhead \
    --repetitions 5 --iterations 1000
python3 scripts/test_profile_overhead.py
```

Each repetition/mode uses a fresh child process, waited on before the next one.
Run order is randomized with a recorded seed. Inputs, configuration and result
fingerprints must match. The summary reports each run's median, median across
runs, sample standard deviation of those medians, and the incremental percentage
relative to `off`. The driver aborts on correctness failures. Failed, unsupported
and mismatching work never become fast timing samples. Raw run JSON retains
counts, p95/p99 reliability, hardware/toolchain and profiler breakdowns.

These are warm-cache, sequential library measurements. They do not cover MCP
transport, cold OS caches, concurrent clients, flush I/O, sampled tracing or
long-running production workloads. Detailed currently uses light's recording
path, so equal costs do not establish the cost of future detailed traces.
`off` is an instrumented build with recording disabled; comparing it with an
uninstrumented build requires a separate matched build. Neither a noisy median
nor a small fixture establishes the issue's universal <2% target.

## Existing engine compatibility probes

With the #746 harness present:

```sh
cargo run --release --locked --manifest-path benchmarks/db-compat/Cargo.toml \
    --bin tokensave-db-compat -- --profile --json target/probes-profile.json
```

`--profile` adds per-probe, per-actual-engine `profile` snapshots, using the
production recorder, static operation registry and histogram implementation
through source modules, rather than another benchmark framework or a production
Turso dependency. It separately measures connection opening (`db.probe_open`),
batch execution including setup (`db.probe_batch`), time to query return
(`db.probe_query_execute`), and row iteration/rendering
(`db.probe_materialize`, with rows returned). These spans do not nest; their
exclusive and inclusive times match. Rows affected and internal engine prepare,
plan or pool costs are unavailable. Error counters stay outside successful
latency histograms. A probe failure still retains successful earlier-stage
measurements, so examine `outcome` before comparing. `profile` is null when off.

Actual engine labels matter: file-compatibility probes may create a database
with libSQL and read it with Turso (or conversely). They are not attributed only
to the tested arm. Unsupported SQL, FTS5 failures and incorrect results remain
explicit compatibility outcomes, never equivalent-workload latency comparisons.
No LadybugDB arm exists. Native Tantivy FTS benchmarks remain separate; this
change does not equate them with FTS5 or replace the #746 relevance methodology.

The runner owns each probe through an `Arc` instead of leaking its catalogue.
On a timeout it aborts **and awaits** its task before removing the temporary
directory. Cancellation is cooperative: synchronous engine work that does not
yield cannot be forcibly interrupted by Tokio's timeout. Engine handles, the
fixed four-engine recorder array, snapshots and temporary directories are
released after each sequential probe. No background recorder or queue is added.
