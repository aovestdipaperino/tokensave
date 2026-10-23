# Profiling overhead and probe attribution — 2026-10-23

This run **does not establish the <2% light-mode target** from #754. Several
observed median differences exceed it, and variation (including negative
differences and changes in an uninstrumented workload) prevents treating these
small measurements as a universal or causal bound. This PR supplies the
measurement path and evidence; the issue's overhead acceptance remains open.

## Method and provenance

- Code: `304f4a25d5621000d3245b58fa13b7df4955066c`; runtime manifests report a
  clean tree. Optimized lite-feature executable, Rust 1.98.1, libSQL 0.9.30 /
  SQLite 3.45.1. Windows/macOS performance was not measured.
- WSL2 Linux on an Intel i7-1165G7, 8 logical CPUs, about 7.6 GiB RAM.
- Five fresh processes per mode/dataset, seeded randomized order (seed 754),
  one sequential client, 1,000 timed operations after 100 warm-ups for queries
  and traversals. Sync/open use 100 timed after 10 warm-ups. Each process runs
  three fresh full indexes. All correctness fingerprints match across modes.
- Dataset 1: the committed five-function fixture (1 file, 6 nodes, 7 edges).
  Dataset 2: actual `src/profile` sources (10 files, 607 nodes, 626 edges).
  This is a subsystem, not a large-repository or real-user MCP reference run.
- No concurrent builds or test runs of this task were active during the final
  repetitions. The shared desktop and OS cache were not controlled. Processes
  start fresh, but this is **not cold OS-cache** testing. Full indexing always
  starts with an empty database and includes parsing and resolution.
- Detailed currently executes light's recording path: no sampled traces or
  slow-event ring are implemented by this benchmark. Off is a recording-disabled
  instrumented build, not a separate uninstrumented baseline.
- Raw per-run data and summaries are in [profile-overhead-results](profile-overhead-results/).
  Only `dataset.source` was normalized to a repository-relative path for
  publication; numerical values, histograms and fingerprints are unchanged.
  Each raw file includes environment, configuration, sample counts, per-workload
  standard deviation and p95/p99 reliability. Index p99 has only 3 samples/run
  and is explicitly unreliable. Other workloads have at least 100/run.

## Observed medians

Each row below uses the median of five per-process p50 values. The sigma column
is the sample standard deviation of the light-mode per-process p50s, not a
confidence interval or the standard deviation of individual operations.

### Actual profiling subsystem

| Workload | Off p50 µs | Light p50 µs | Light difference | Detailed difference | Light sigma µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| `index.full` | 58017.20 | 55614.98 | -4.14% | +1.31% | 15688.71 |
| `lookup.node_by_id` | 18.67 | 19.57 | +4.82% | +2.73% | 1.15 |
| `lookup.by_name` | 21.72 | 22.54 | +3.79% | +1.73% | 2.05 |
| `lookup.by_qualified_name` | 21.89 | 21.78 | -0.51% | -0.56% | 3.56 |
| `search.fts_ranked` | 96.59 | 100.25 | +3.78% | +10.19% | 6.93 |
| `graph.callers_d3` | 54.53 | 54.25 | -0.53% | +5.61% | 3.39 |
| `graph.callees_d3` | 127.70 | 131.12 | +2.68% | +3.13% | 3.88 |
| `graph.impact_d2` | 78.20 | 76.18 | -2.58% | +3.47% | 6.00 |
| `sync.noop` | 1577.73 | 1712.17 | +8.52% | +0.65% | 290.97 |
| `open.existing` | 1573.22 | 1804.55 | +14.70% | +11.50% | 309.28 |
| `open.first_search` | 196.48 | 227.18 | +15.62% | +11.16% | 40.08 |

### Small connected fixture

| Workload | Light difference | Detailed difference |
| --- | ---: | ---: |
| `index.full` | +1.20% | +11.73% |
| `lookup.node_by_id` | +6.59% | +7.13% |
| `lookup.by_name` | +0.41% | +5.01% |
| `lookup.by_qualified_name` | +2.60% | +0.82% |
| `search.fts_ranked` | +4.14% | +3.70% |
| `graph.callers_d3` | +3.37% | +4.64% |
| `graph.callees_d3` | +0.25% | +0.38% |
| `graph.impact_d2` | +4.68% | +4.73% |
| `sync.noop` | +2.72% | +4.82% |
| `open.existing` | +8.99% | +7.98% |
| `open.first_search` | +10.72% | +16.54% |

The `open.existing` snapshot is empty even when enabled: that path has no
recorded spans. Its observed median difference cannot be attributed to measured
DB profiling. An empty snapshot means **unavailable**, never zero database time.
The negative differences and the light/detailed discrepancies also show why
these runs must not be presented as a precise profiler-only slowdown estimate.

## Where measured database time went

The following sums combine the five light-mode subsystem runs, across the
explicit benchmark workload mix (not production MCP traffic). Inclusive and
exclusive DB totals happen to match for these listed spans; parent phase totals
must not be added to them. Warm-ups and input-validation calls are excluded.

| Operation | Successful spans | Exclusive time ms |
| --- | ---: | ---: |
| `db.get_nodes_by_ids` | 44,535 | 1028.05 |
| `db.search_nodes_fts` | 5,500 | 509.33 |
| `db.get_incoming_edges` | 35,275 | 299.38 |
| `db.get_outgoing_edges` | 30,820 | 274.69 |
| `db.get_node_by_id` | 10,000 | 165.10 |
| `db.search_nodes_by_exact_name` | 5,500 | 148.13 |
| `db.end_bulk_load` | 15 | 119.49 |

Measured finding: repeated node-batch retrieval consumes the largest cumulative
DB time in this selected mix. Hypothesis: reducing repeated neighbor/node loads
in traversal may help more than optimizing one unusually slow query. This is
not a production N+1 finding: the harness calls the library directly and does
not generate per-tool MCP invocation aggregates. The source stack's operation
coverage is partial, and this run cannot rank uninstrumented work.

## Engine probes and correctness

All **192 outcomes match** with profiling off and on (48 probes × four actual
engine configurations); libSQL passes **48/48**. The three Turso arms retain
expected FTS5/SQL errors and a correctness mismatch. They are not scored as
latency-equivalent successes. Seven optional existing-database probes were not
run; native Tantivy relevance/performance is still the separate #746 suite.

[Recorded probe snapshots](profile-overhead-results/probes.json) separate opening,
batch execution (including setup), query execution and materialization, labelled
by the engine actually used. Error counters do not enter successful histograms.
The 18 standalone tests include actual-engine labels, successful rows, failed
queries, histogram/recorder invariants and off-mode null output. Query errors
may surface during execute or materialization depending on the engine.

## Resources, checks and limits

- Fixed recorder allocation: **841,824 bytes** per enabled process, zero recorder
  bytes before enablement in off-mode processes. The compatibility harness owns
  four such recorders per profiled probe (about 3.21 MiB), released at its end.
  Snapshots and workload samples are input/iteration proportional and scoped to
  the sequential run. No profiler persistence or sharing thread is started.
- One process-RSS sample is recorded per run after queries. It is not evidence
  of leak freedom or long-running stabilization. Repeated fixed workloads cover
  aggregate bounds; idle periods, real cancellation under non-yielding engine
  work, simultaneous clients and long-lived MCP memory behavior remain untested.
- Probe cancellation now aborts and awaits the owned task before temporary
  directory cleanup (Refs #761). Cooperative cancellation cannot impose a hard
  timeout on non-yielding synchronous engine work.
- `cargo test --workspace --locked -j 1`: **4,467 passed, 4 ignored**.
- `cargo clippy --workspace --all-targets --locked -j 1`: passed with existing warnings.
- `cargo fmt --all -- --check`: passed.
- Standalone probe tests: **18 passed**; standalone all-target release Clippy and
  formatting passed. Python comparison checks: **2 passed**.
- A separate whole-program release binary link was stopped to avoid concurrent
  build memory pressure after the optimized benchmark executable was already
  built. The recorded measurements ran that completed release executable. Full
  debug workspace builds/tests and Clippy completed independently.

Reproduce the performance runs using the commands in the
[harness guide](database-benchmark-harness.md), with `--repetitions 5`,
`--iterations 1000`, and each recorded source directory. This report is bounded
observational evidence; large matched repositories, full-feature release builds,
MCP transport, cold caches, representative concurrent traffic and an uninstrumented
baseline are needed before claiming the issue's overhead target is satisfied.
