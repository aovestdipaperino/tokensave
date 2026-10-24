# Benchmark resource investigation (2026-10-24)

## Incident attribution

The reported October 22 WSL2 session placed two approximately 1.6 GB release
builds and approximately 689 MB of working data on a 3.9 GB `/tmp` tmpfs.
Their combined footprint explains the reported ENOSPC; moving them to disk
allowed completion. These are supplied incident observations, not measurements
reproduced by this investigation. The original incident harness was not supplied.
No tracked `perf.py` was found. It is therefore not possible to attribute those
allocations to a particular script or assert that the incident script was
external. In particular, there is no evidence that `tokensave config benchmark`
caused the incident.

The inspected checkout is 5357fe1, predating merged PR
[#753](https://github.com/aovestdipaperino/tokensave/pull/753). The comparison
script and `benches/repos.rs` were verified unchanged against upstream on the
investigation date. Uncommitted database benchmark/build work in the original
checkout was preserved; it is not treated as released behavior.

## Ownership and allocations

| Path | Storage and lifecycle | Findings |
| --- | --- | --- |
| `src/hardware.rs`: `measure`, `probe_resources`, `run_benchmark` | Fixed integer kernel; at most 16 scoped CPU threads, joined before return; no workload files or repository copies | Uses available parallelism and sysinfo memory/cgroup limits. No large `/tmp` allocation. |
| Merged #753: `src/commands.rs::run_config_benchmark`, `src/global_benchmark.rs::{ensure,ensure_in,write}`, `src/user_config.rs::write_atomic` | Small machine cache `~/.tokensave/benchmark.toml`, advisory lock, same-directory temporary file and rename | Reuses valid results; cache invalidation checks versions, CPUs and total memory. Available-memory changes do not invalidate it. `[benchmark] enabled/auto_run` configure this hardware policy, not repository comparison storage. No reason to add a workload directory here. |
| `src/bench.rs::{run_bench,run_bench_with_toml}` | Queries an existing project graph and estimates retrieval token savings; query results scale with supplied query count | Does not build baseline/candidate binaries or copy repositories. |
| `benchmarks/run_benchmarks.py` | Shared shallow clones cached across runs; before this change always `/tmp/tokensave-bench`; `.tokensave` rebuilt for a cold comparison; reports beside the script | Confirmed directory-selection limitation. MCP initialization also lacked cleanup on failed entry. Cached clones are intentional retained data, not disposable scratch. |
| `benches/{large_repos,repos,queries}.rs` | Explicit `TOKENSAVE_BENCH_REPOS_DIR`; shallow pinned clones, marker reuse, full reindex; retains prepared graphs for selected repos | Already supports choosing disk-backed storage. Write queries use scratch files. Original cleanup uses repository-wide stash/drop and only runs after normal completion; this is unsafe for unrelated user changes and merits a separate fix. |
| `benches/language_detection.rs` | 500 or 10,000 generated source files and graph in Rust `tempfile::tempdir`; graph dropped before explicit directory close | Standard temp selection can choose tmpfs. RAII covers ordinary error unwinding; process termination can leave artifacts. No two-build allocation. |
| `benchmarks/cpu_profiles.py` | One generated/copied source workload in `TemporaryDirectory` beside `--output`; excludes `.git`, `.tokensave`, `target`, vendor and other large directories when copying | Existing output placement selects workload filesystem; binaries supplied by caller, not built/copied by harness. Context manager removes workload on ordinary exit/errors/KeyboardInterrupt. Temporary stderr logs use standard temp storage. |
| `benchmarks/mcp_startup.py` | Generated fixtures, homes, graph data, one reusable outdated-index copy and results beneath explicit `--output` | Deliberately retained for diagnosis; not an implicit temp cache. Runs paired measurements sequentially, then eight servers for an explicit idle-concurrency experiment. Includes server/thread cleanup paths. |
| `scripts/mcp_probe/{probe,isolated}.py` | Opens configured repos, writes diagnostics; subprocess timeout paths | Not a two-build performance comparator. `McpClient.close` kills on exceptional shutdown without an explicit post-kill wait; not validated/fixed by the storage change. |
| `benchmarks/tsbench/bench_tokensave.patch` | Adapter for an externally cloned agent benchmark; result/session files in that checkout | Distinct from core TokenSave; README's `/tmp/tsbench` is an example chosen by the caller. |
| Cargo | Target directories controlled by caller (`--target-dir` or `CARGO_TARGET_DIR`), normally `target` in the workspace; separate targets duplicate compilation outputs | No TokenSave runtime change can reclaim external baseline/candidate builds. Choose disk-backed build directories, and limit build jobs explicitly on constrained machines. |

## Changes and usage

The Python comparison harness now selects its persistent clone cache using:

1. `TOKENSAVE_BENCH_REPOS_DIR`, the existing Criterion setting, when nonempty;
2. Python's standard temporary-directory selection (`TMPDIR`, `TEMP`, `TMP`,
   platform defaults), with a `tokensave-bench` child directory.

On a usual Unix installation this preserves `/tmp/tokensave-bench`. Cached
clones are reused without copying. Existing cache content is never relocated or
deleted automatically. Clone and directory-creation errors name the destination
and suggest checking space/permissions or relocating it. No hard-coded space
threshold is introduced: repository size, graph size and Cargo artifacts are
independent and variable, and a preflight free-space check cannot reserve space.

For example, choose a directory on a filesystem verified to be disk-backed:

```bash
mkdir -p "$HOME/tokensave-bench-cache" "$HOME/tokensave-bench-build"
df -h "$HOME/tokensave-bench-cache" "$HOME/tokensave-bench-build"
TOKENSAVE_BENCH_REPOS_DIR="$HOME/tokensave-bench-cache" \
  python3 benchmarks/run_benchmarks.py --repos fastapi --skip-token-savior
# For separate baseline/candidate builds, choose separate disk-backed targets:
cargo build --release --target-dir "$HOME/tokensave-bench-build/baseline"
```

Neither available RAM nor the label WSL/container establishes available storage.
Use `df` on each selected path. The harness does not move Cargo artifacts or
change hardware profiles.

## Validation and measurements

Run storage regression tests with:

```bash
python3 -m unittest discover -s benchmarks/tests -p 'test_clone_storage.py' -v
```

Six deterministic tests cover directory override precedence, standard temporary
selection, actual fresh-process `TMPDIR`, repeated cache reuse/preservation,
new-clone destination, and injected ENOSPC from both Python and Git. No real
filesystem was filled. Success retains the cache by design, and the change adds
no deletion or concurrent clone protocol. Concurrent comparisons against the
same mutable clone remain unsupported; use a distinct cache per run.

`resource-management/measure_clone_storage.py` measures the same local
1,100,000-byte source fixture with each version, five cold clones and 100 cache
hits per clone. The old hard-coded root is overridden **only in this measurement**
to isolate equivalent work on the same disk-backed filesystem. Run from a
disk-backed working directory, passing a saved old/new module path. No network,
TokenSave indexing, token-savior, or Cargo builds are involved. The measurement
fixture is removed afterwards. Raw results are adjacent JSON files.

| Metric | Before | After |
| --- | ---: | ---: |
| Median cold clone wall time (5 runs) | 115.28 ms | 118.12 ms |
| Median 100 cache hits (5 batches) | 2.208 ms | 2.324 ms |
| Parent peak RSS (one process per version) | 24,448 KiB | 24,340 KiB |
| Maximum child RSS (`RUSAGE_CHILDREN`) | 24,448 KiB | 24,212 KiB |
| Allocated blocks per clone cache | 1,302,528 bytes | 1,302,528 bytes |
| Total allocated fixture blocks (source + five caches) | 7,802,880 bytes | 7,802,880 bytes |
| Redundant repository copy calls / build artifacts | 0 / 0 bytes | 0 / 0 bytes |
| New workload bytes in default `/tmp` | 0 (root overridden) | 0 (explicit cache) |

These small differences do not establish a throughput or RSS improvement. The
change enables choosing storage; it does not reduce the amount of cloned data.
RSS is a high-water mark, not evidence of long-running memory stabilization.
No original 3.9 GB exhaustion or full production comparison was reproduced.

Formatting (`cargo fmt --all -- --check`) and Python tests passed on Linux.
Rust runtime code is unchanged by the storage fix; no Rust compilation or full
workspace test claim is made. Core restricted-resource recommendations, hardware
cache concurrency/invalidation and full indexing performance are outside these
Python tests; existing hardware/global-cache tests remain unchanged.

## Related work and remaining limits

Searched upstream issues/PRs for benchmark, resource, `/tmp`, initialization and
stash cleanup. Relevant work includes
[#731](https://github.com/aovestdipaperino/tokensave/issues/731)/
[#732](https://github.com/aovestdipaperino/tokensave/pull/732) (hardware selection),
[#752](https://github.com/aovestdipaperino/tokensave/issues/752)/
[#753](https://github.com/aovestdipaperino/tokensave/pull/753) (machine cache),
[#741](https://github.com/aovestdipaperino/tokensave/issues/741)/
[#745](https://github.com/aovestdipaperino/tokensave/pull/745) (database workload inventory),
[#746](https://github.com/aovestdipaperino/tokensave/pull/746) (compatibility probes),
[#761](https://github.com/aovestdipaperino/tokensave/issues/761) (probe cancellation),
and [#697](https://github.com/aovestdipaperino/tokensave/issues/697) (resource lifecycle).
No duplicate comparison storage/MCP-entry fix was found in those searches.

This investigation does not establish complete interruption safety: SIGKILL,
process crashes, signal termination without unwinding, and abandoned Cargo
builds can leave data. The machine cache's shared atomic writer can leave a
small partial temporary file if its initial write fails or the process dies;
that is a general configuration-writer issue, not a large benchmark workload.
No destructive scavenger or automatic cleanup of user-owned caches is added.
