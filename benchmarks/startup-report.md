# MCP startup allocation review

Base: upstream master `bbc0920`. Focused issue: [#735](https://github.com/aovestdipaperino/tokensave/issues/735).

## Scope and existing lifecycle

Reviewed #253/#305 (resolver allocation peaks), #320 (version-triggered rebuilding), #421 (server registry), #436 (opt-in idle timeout and reapability), and recent lifecycle/concurrency work including #628, #694, #708, #712, #733 and #734. This change does not replace those mechanisms.

Startup creates the Tokio runtime, parses CLI arguments and loads user/project configuration. `serve` discovers an index or starts without a default project. `TokenSave::open` resolves the branch database, handles applicable migrations/interrupted operations, and initializes one selected graph connection. The graph database applies its existing adaptive cache/mmap pragmas and trait-dispatch cache. The graph and global databases have different persistent scopes; this change does not combine their connections or add statement caches. Required file-token/accounting data remain eager. The language registry stores stateless extractors; unchanged indexed projects do not require source parsing merely to construct the MCP server.

`McpServer::new_inner` builds routing metadata and schema accounting, opens the global database, snapshots sibling projects and starts the existing bounded catch-up sync. Tool requests retain their freshness cooldown and version-rebuild behavior. Unix retains its existing 30-second parent-process check; there is no new recurring maintenance. The opt-in idle deadline, SIGTERM handling, shutdown persistence, checkpoints, explicit exit after shutdown and registry cleanup are unchanged.

Previously routing metadata and accounting each rebuilt every tool schema: three full constructions. The change constructs one temporary registry, derives both name sets, retains the always-loaded definitions in their original order, computes the same accounting value, and drops every schema before the next database await. Only the existing name sets and scalar survive. There is no process-global schema cache or work shifted to the first request.

## Method

`benchmarks/mcp_startup.py` runs equivalent full-feature release binaries in separate directories, both named **tokensave**. The basename is required: production extraction workers are disabled for differently named binaries. Each build gets separate initially equivalent user databases; fixture initialization uses a third user directory. Network update checks, counter uploads and agent maintenance are disabled in these isolated directories.

The probe alternates build order on each pair. Each process completes a protocol handshake, its first search, ten repeated searches and normal EOF shutdown. Without an index the measured requests are `tools/list`. Fixture graphs contain independent Rust functions: 300 files / 6,300 nodes, and 3,000 files / 63,000 nodes; neither contains call edges. Pair zero is reported separately and excluded from warm medians. OS page caches are not flushed, so this is not a storage-cold benchmark.

Each of ten version-bump trials per build restores an independent initialized fixture before setting its version marker to `6.0.0`. The probe observes the existing rebuild, checks the resulting graph size and classifies explicit failure/timeout. Four servers per build then idle concurrently for ten minutes, sampled each minute. The probe checks clean stdout, successful EOF exit and that sampled child processes do not survive shutdown.

Parent, extraction-worker and other-child RSS peaks are sampled every 50 ms; short peaks or unsampled detached children can be missed. The tree peak is sampled jointly; independent role maxima must not be added together. Idle RSS and CPU are per-process Linux `/proc` measurements. RSS sums double-count shared pages; CPU-time resolution is one kernel accounting tick. The probe's own resource use is outside the measured server tree. These fixed workloads do not establish absence of all leaks, nor cover dense resolver graphs or actual host-specific reconnection after idle exit.

## Reproduce

Build the base and candidate with `cargo build --release --locked`, preserving their executables as `/path/baseline/tokensave` and `/path/candidate/tokensave`. Run with a new output directory outside indexed source trees:

```sh
python3 benchmarks/mcp_startup.py \
  --before /path/baseline/tokensave --after /path/candidate/tokensave \
  --output /path/results --runs 30 --idle-seconds 600
```

The output is append-only JSONL and includes executable SHA-256 values. It refuses an existing measurements file. Preserve the raw output before comparing medians. macOS/Windows runtime validation depends on CI; the profiling script itself is Linux-only.

## Linux results

Measured on WSL2 Linux 6.18.33.2, x86_64, Intel i7-1165G7 (8 logical CPUs), using the pinned Rust 1.98.1 toolchain and default full-feature release builds. Raw paired and idle samples are in `startup-results.jsonl`; `python3 benchmarks/summarize_startup.py benchmarks/startup-results.jsonl` reproduces medians and paired bootstrap intervals (10,000 resamples, seed 20261020).

### Startup and requests

Milliseconds, median of 29 pairs after excluding pair zero. The first request follows the handshake; repeated latency is each process's median of ten requests.

| Fixture | Startup before / after | First request before / after | Repeated before / after |
| --- | ---: | ---: | ---: |
| 6,300-node index | 123.32 / 132.37 | 41.58 / 39.20 | 22.11 / 21.59 |
| 63,000-node index | 123.75 / 128.88 | 174.52 / 184.08 | 76.43 / 84.22 |
| No index (`tools/list`) | 83.17 / 88.70 | 1.46 / 1.54 | 1.32 / 1.50 |

The main run does **not** establish a wall-clock startup improvement. All paired median-difference 95% bootstrap intervals include zero: startup intervals are [-4.53, 29.63], [-4.72, 19.79] and [-8.16, 8.03] ms respectively. First-request intervals are [-8.45, 6.27], [-15.97, 20.91] and [-0.18, 0.18] ms. This noisy host run neither demonstrates a first-request regression nor rules out a material one on indexed workloads. Pair-zero data remain in the raw output, but are not an OS-cold measurement. The very first indexed launches took 1,242.85 / 171.38 ms; one ordered pair with uncontrolled page caches is insufficient evidence of a cold-start improvement.

### Single-CPU sensitivity run

A separate run used `taskset -c 0` (recorded affinity `[0]`) with otherwise identical binaries, new fixtures/homes and 30 alternating pairs. This changes scheduler/runtime parallelism and is a sensitivity check, not a replacement for the default run. Raw samples are in `startup-constrained-results.jsonl`. Reproduce by prefixing the probe command with `taskset -c <allowed-CPU>` and using `--idle-seconds 1`; this run does not provide ten-minute idle evidence.

| Fixture | Startup before / after | First request before / after | Repeated before / after |
| --- | ---: | ---: | ---: |
| 6,300-node index | 148.13 / 149.97 | 35.88 / 35.31 | 18.73 / 18.68 |
| 63,000-node index | 184.24 / 176.03 | 91.18 / 88.96 | 64.32 / 63.10 |
| No index | 93.08 / 80.09 | 1.10 / 1.15 | 1.03 / 0.98 |

Startup paired-difference intervals again include zero. The no-index first request had a small positive paired median difference of 0.06 ms, interval [0.02, 0.28] ms; indexed first-request intervals include zero. This still does not establish a consistent end-to-end startup gain. Differences between aggregate medians and paired differences illustrate the host variability.

### Allocation attribution

A separate warmed no-project handshake + EOF Memcheck run reports:

| Parent process | Before | After |
| --- | ---: | ---: |
| Total allocation calls | 18,178 | 7,547 |
| Cumulative allocated bytes | 2,374,753 | 1,356,045 |
| Reported memory-access errors | 0 | 0 |

This removes 58.5% of allocation calls and 42.9% of cumulative allocated bytes in that path. These are **not** live bytes or RSS savings. Leak checking was disabled; this does not establish leak freedom. The registry construction is the only production behavior changed, and every definition is released before the next database await. No initialization moves to the first request.

To reproduce allocation attribution, warm each binary once with an isolated HOME, then run `valgrind --tool=memcheck --leak-check=no --log-file=allocations.log /path/to/tokensave serve --path /path/to/no-index` with this line on stdin followed by EOF:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"allocation-probe","version":"1"}}}
```

Use the same isolated environment as the timing probe, and compare the final parent-process heap summary (an optional failed child exec can produce its own summary).

### Idle and multiple instances

Four servers per build idle concurrently after one query. Per-server values below are medians across those four instances; totals are sums of parent RSS, which double-count shared pages.

| Metric | Before | After |
| --- | ---: | ---: |
| Parent RSS immediately after query | 22.63 MiB | 22.92 MiB |
| Parent RSS at 1 minute | 22.76 MiB | 23.04 MiB |
| Parent RSS at 5 minutes | 22.76 MiB | 23.04 MiB |
| Parent RSS at 10 minutes | 22.76 MiB | 23.04 MiB |
| Four parents at 10 minutes | 91.18 MiB | 91.95 MiB |
| Highest instance's average idle CPU, percent of one core | 0.0017% | 0.0033% |

The highest-CPU instances accumulated one / two 10 ms accounting ticks respectively during the ten minutes. Other instances accumulated zero measured ticks. No descendants remained in any idle sample. Memory stabilized by one minute, but this run shows no idle-memory reduction: the candidate's four-process sum was 0.76 MiB higher. Normal EOF exit succeeded throughout the probe; sampled descendants did not survive shutdown and both isolated registries were empty afterward.

### Version-triggered indexing

All ten trials per build completed with the expected 6,300 nodes. Median completion after issuing the first query was 1,058.75 / 920.44 ms. Maximum sampled parent RSS was 42.98 / 43.07 MiB, extraction-worker RSS 28.57 / 28.53 MiB, other-child RSS 17.55 / 17.49 MiB, and jointly sampled tree RSS 63.28 / 63.39 MiB. These independent maxima occur at different times and must not be summed. No indexing optimization is claimed from this small comparison.

## Correctness and resource review

- `cargo build --release --locked`: both revisions, default full features.
- `cargo test --workspace --locked`: 4,341 passed, 4 ignored.
- `cargo test --workspace --locked --features test-transport`: 4,454 passed, 4 ignored.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked --features test-transport`: passed, existing warnings remain.
- New startup test compares both routing sets and schema accounting against the public registry behavior. Existing transport tests cover protocol negotiation, version gates, freshness, cancellation, SIGTERM, idle-window resets and deferral during indexing. Existing persistence tests cover interrupted bulk loading and checkpoint concurrency; registry tests cover live/dead process handling.

Growth/release: the temporary registry scales with the statically registered tools, is created once per server and drops before subsequent database I/O. Only the existing owned name sets and scalar remain. No new cache, shared ownership, task, connection, queue, lock or timer is introduced in production. Error/cancellation cleanup follows normal Rust ownership. Existing background task/process ownership and shutdown paths are unchanged.

Probe resource ownership: one process, a response reader, a sampler and a temporary log per server; the response queue is bounded to 1,024 messages, sampled descendant IDs are bounded by the finite run's child launches, and all are released on close. Sampling terminates through an event and join; EOF is followed by a bounded wait with kill/reap fallback. A mock server that exits unsuccessfully after spawning a child verified the profiling tool's failure path: the child was stopped and both threads joined. Samples are appended to disk rather than retained indefinitely. The explicit 50 ms sampling and 60-second idle observations exist only in this standalone profiling tool.

Limitations: Linux runtime evidence only; macOS/Windows require upstream CI. Real agent-host reconnection, storage-cold page caches, dense cross-language resolver graphs and long-duration leak detection were not measured. The ten-minute fixed-workload check cannot establish absence of all leaks. No broad startup-speed or idle-memory improvement is claimed from allocation counts alone.
