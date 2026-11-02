# Comparison MCP lifecycle validation

`TokensaveMcp.__enter__` previously spawned a server before initialization, but
an initialization error or `KeyboardInterrupt` bypassed `__exit__`. Normal
cleanup also left stdout open. The fix runs the existing close/wait/kill path
on failed entry and explicitly closes stdout. No timeout or concurrency limit
changes, filesystem cleanup, dependency, or runtime Rust changes are added.

Run regression coverage with:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s benchmarks/tests -v
```

Seven tests cover successful/repeated cleanup, body errors, request and notify
failures/interruption, spawn failure, forced kill/wait, overlapping independent
clients and a real Python child whose stdout closes during initialization.
Against the original implementation these yielded nine failed assertions;
all seven tests pass with the change. Tests repair the old implementation's
leaks even when assertions fail.

The measurement script `resource-management/measure_mcp_cleanup.py` accepts an
old/new `run_benchmarks.py` path. It runs 20 successful sessions and 20 failed
entries per version using real Python children, observes cleanup and repairs
any baseline leaks immediately afterwards. Each version runs in a separate
process. The fake server reads stdin until EOF; in the failure case it closes
stdout after reading initialization. It does not run TokenSave, build binaries,
open databases or write workload files. Raw JSON is beside the script.

| Metric | Before | After |
| --- | ---: | ---: |
| Successful-session median (20) | 43.825 ms | 43.813 ms |
| Failed-entry median (20) | 28.756 ms | 35.765 ms |
| Unreaped children after failed entry (total observed) | 20 | 0 |
| Open pipes after failed entry (total observed) | 40 | 0 |
| Open pipes after successful exit (total observed) | 20 | 0 |
| Parent peak RSS | 25,208 KiB | 26,164 KiB |
| Maximum child RSS (`RUSAGE_CHILDREN`) | 25,208 KiB | 26,164 KiB |
| Workload temporary / allocated bytes | 0 / 0 | 0 / 0 |
| Repository copies / build artifact bytes | 0 / 0 | 0 / 0 |

Successful-session timing is effectively unchanged in this small experiment.
Failed entry now waits for cleanup; the old timing returned while leaving a
child waiting on stdin. No speed or RSS improvement is claimed. RSS high-water
marks from single processes do not establish absence of long-running leaks.

Resource ownership is one child and two pipes per client; no collection grows
across requests and independent clients release their own resources. Tests
exercise ordinary failure, Python interruption and timeout escalation. SIGKILL,
repeated interruption during shutdown, descendants retaining pipes and an
unresponsive initialization with no EOF are outside this fix. The blocking
handshake still has no deadline. No link to the reported tmpfs ENOSPC is claimed.

Python regression tests, `cargo fmt --all -- --check` and `git diff --check`
passed. Full Rust workspace test results are recorded in the investigation
handoff; they do not change the scope of the Python lifecycle measurement.
