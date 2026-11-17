# Comparison MCP lifecycle

`TokensaveMcp(root, initialization_timeout=30, request_timeout=120,
shutdown_timeout=3)` accepts finite positive seconds. A single monotonic
initialization deadline covers the initialize request and initialized notification,
including blocked writes and partial reads. Each subsequent request or standalone
notification has its own deadline. Timeout raises `TimeoutError`; failed entry
cleans up immediately, while a request error cleans up when its context exits.

The comparison harness already requires Unix (`resource`). Its MCP transport uses
nonblocking binary pipes and `select`, with no helper threads, tasks or queues.
Each client owns one new session/process group, one direct child and two pipes.
Writes advance a memoryview of the encoded request; storage scales with the caller's
request. Reads use 64 KiB chunks and retain at most 16 MiB plus one chunk before
rejecting an unterminated response. Complete lines are parsed separately; decoded
JSON scales with the capped line. Cleanup clears the buffer and closes both pipes.

Exit closes stdin, waits up to the shutdown grace period, then sends SIGKILL to
the owned process group (also when its leader exited), and waits up to another
grace period for the direct child. KeyboardInterrupt during cleanup is deferred;
it is re-raised on normal exit, while an existing body/entry exception takes
precedence. Wait retries share the same absolute deadlines. Cleanup errors are
attached to an existing exception as notes; otherwise they propagate. An unreaped
child handle remains owned by the client so cleanup can be retried.

SIGKILL of the benchmark itself cannot run Python cleanup. Descendants that call
setsid or otherwise leave the owned group are outside containment. Group members
are killed but only the direct child can be reaped by this parent; the OS must reap
orphan descendants. Uninterruptible kernel sleep can exceed reaping deadlines;
this is reported as a cleanup failure, not a guarantee of resource release.
Continuous signals, OS scheduling stalls and process creation are not hard real-time
bounded. Windows is unsupported by this Unix comparison harness. No unrelated
process groups are targeted. `select` also inherits the platform descriptor limit.

Run real-child regressions with an external hang guard:

```sh
PYTHONDONTWRITEBYTECODE=1 timeout 60 python3 -m unittest discover -s benchmarks/tests -v
```

Tests repair children even after assertion failures. They cover normal/error exit,
entry interruption, two interruptions during shutdown, partial/stalled initialization,
blocked notification writes, later request deadlines, oversized output, spawn failure,
independent clients and descendants retaining stdout (including observed EOF after
cleanup). They use no database, network or TokenSave build. This is bounded synthetic
coverage, not a claim of general leak freedom or Windows validation.

## Related work

- #768 / PR #771: merged into master as `14dfe65`; failed-entry cleanup and stdout
  closure, with seven regressions and 20-trial resource measurements recorded there.
- #847: this follow-up depends on #771 and adds deadlines, bounded transport storage,
  process-group termination and interruption handling. Implementation is proposed
  for master, not merged or released. The issue tracks the PR and current validation.

## Linux resource check

`PYTHONDONTWRITEBYTECODE=1 timeout 90 python3 benchmarks/resource-management/measure_deadlines.py`
repeats the lifecycle suite 20 times. Raw observations are in
`resource-management/deadlines-linux.json`. In this run the first twelve tests
were repeated (the UTF-8 success case was added afterwards). Every iteration
passed; direct children were observed reaped and pipes closed, and the descendant
case observed pipe EOF. Open parent descriptors stayed at four. Suite elapsed
time was 1.911–2.866 s; parent CPU was 0.192–0.314 s per iteration, including the
17 MiB oversized-output test. Peak parent RSS rose from 41,768 to 43,096 KiB
in the second iteration, then to 43,124 KiB at iteration 18. High-water marks
are not live retained-memory samples and do not establish long-running stability.
No baseline throughput or RSS improvement is claimed. This fixed workload does
not test escaped descendants, kernel stalls, signal storms or parent SIGKILL.
