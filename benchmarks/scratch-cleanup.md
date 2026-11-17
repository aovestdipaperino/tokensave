# Large-repository benchmark scratch ownership

The previous cleanup removed the fixed scratch directory, then stashed and
dropped **all** tracked/untracked changes in the cached repository. A disposable
reproduction confirmed that unrelated tracked edits and untracked files were
removed. Cleanup ran only at the end of the successful benchmark function.

`ScratchDir` now reserves `.tokensave-bench-scratch` with exclusive `create_dir`.
An existing directory, file or symlink is refused with an inspection/separate
cache hint. `RepoBench` owns the guard after its graph field, so the graph drops
first, then the owned scratch directory is removed. Early preparation errors
and ordinary panic unwinding drop the same guard. The repository-wide Git
stash/drop operations are removed. Clones, graph caches, markers and unrelated
files are retained. Failed removal emits the exact scratch path and error.

Run the integration module with:

```bash
cargo test --locked --test integration bench_repo_cleanup_test::
```

Six tests cover success, early errors, panic unwinding, pre-existing scratch
preservation, concurrent exclusion followed by successful reuse, and pre-existing
symlink preservation (Unix). The source helper was also compiled directly into
a test binary with the existing tempfile dependency; all six passed. The panic
message in the unwinding test is expected and caught.

## Measured behavior

The `resource-management/scratch-driver.rs.in` template accepts `REPOS_PATH`,
which is substituted with an absolute old/new `benches/repos.rs` path. Compile
with `rustc --cfg before` or `rustc --cfg after`. The driver creates a 1 MiB
scratch payload, then performs the corresponding actual cleanup: old code
removes scratch before invoking `restore_repo`, new code drops the guard.
`resource-management/measure_scratch_cleanup.py` accepts the compiled driver,
creates five separate disposable Git repositories on the current filesystem,
and records the driver's internal elapsed time and retained block allocation.
Each repository has one unrelated tracked edit and a 64 KiB untracked file.
Raw JSON is adjacent. Git identity and hooks are isolated with command options.
All measurement fixtures are removed after observation.

| Metric (5 repositories) | Before | After |
| --- | ---: | ---: |
| Median scratch write + cleanup | 75.90 ms | 11.14 ms |
| Unrelated changes preserved | 0 / 5 | 5 / 5 |
| Scratch directories remaining | 0 / 5 | 0 / 5 |
| Retained allocated bytes per repository | 245,760 | 249,856 |
| Peak scratch payload | 1,048,576 bytes | 1,048,576 bytes |
| Parent peak RSS | 15,512 KiB | 15,352 KiB |
| Maximum child RSS (`RUSAGE_CHILDREN`) | 15,512 KiB | 15,352 KiB |
| Redundant repository copy calls | 0 | 0 |

This isolated workload shows lower cleanup latency from eliminating Git stash
work and, critically, preservation of user changes. The retained allocations
are not equivalent data sets: the new code deliberately retains the user's
64 KiB file; the old code discards it but retains unreachable stash objects in
`.git`. No general benchmark throughput, total indexing storage or memory
improvement is claimed. The measurements do not sample instantaneous whole-tree
peak blocks or attribute child RSS solely to cleanup. No Cargo build is part
of this measurement, and fixtures are on disk, not default `/tmp`.

## Resource review and limits

The owner retains one path per prepared repository; there is no queue, polling,
background task, repository copy or cleanup subprocess. Cleanup work scales
with files written by the existing fixed query catalog. The guard's lifetime
covers setup and the benchmark and is released after the graph. Concurrent
runs sharing this scratch directory fail reservation instead of sharing files.

SIGKILL, panic-abort and termination without unwinding can leave scratch data;
the next run refuses it for manual inspection. No automatic removal of stale
paths is attempted. This does not prevent deliberate external replacement of
an owned directory while the benchmark runs. Use dedicated benchmark clones.
The existing pinned-ref refresh path can still force-checkout a cache when its
marker does not match; this change addresses end-of-run cleanup, not clone
refresh semantics. No real large repository or Windows run was measured.
