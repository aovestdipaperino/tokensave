# Agent workflow benchmark report

Trials: 63 (runs: 0bf0d35219f3); models: claude-haiku-5-5.

Times in seconds. `n/a` = unavailable, never zero. P90 needs at least 10 trials. Wall-time shares are an exact partition; TokenSave and DB shares are subsets of tool wait.

## By arm

| Arm | n | Success | Wall p50 | Wall p90 | Model calls | Tool calls | TS calls | In tok (incl. cache) | Out tok | Cost p50 $ | Startup | Model wait | Tool wait | TS internal | DB | Exact dup | Near dup | Reval |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| baseline | 21 | 100% | 27.4 | 47.0 | 3.0 | 3.0 | 0.0 | 61471 | 978 | 0.002 | 25% | 62% | 9% | n/a | n/a | 0.0 | 2.0 | 0.0 |
| tokensave | 21 | 100% | 30.8 | 75.4 | 3.0 | 3.0 | 1.0 | 70833 | 778 | 0.002 | 29% | 51% | 15% | 6% | 0% | 0.0 | 0.0 | 0.0 |
| tokensave-guided | 21 | 100% | 33.9 | 61.1 | 3.0 | 3.0 | 2.0 | 74214 | 582 | 0.002 | 28% | 47% | 22% | 8% | 1% | 0.0 | 4.0 | 0.0 |

## TokenSave internals (#754 profiler)

Totals over profiled trials, in ms. Project resolution, freshness and handler are phases of MCP time; DB time is inside the handler.

| Arm | Profiled trials | MCP calls | MCP ms | ms/call | Project resolution | Freshness | Handler | DB | Catch-up sync |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| tokensave | 16 | 19 | 35198 | 1853 | 6295 | 1943 | 4990 | 571 | 0.0 |
| tokensave-guided | 19 | 47 | 72687 | 1547 | 26196 | 6570 | 12190 | 6462 | 0.0 |

## By task and arm

| Task | Arm | n | Success | Outcomes | Wall p50 | Wall min–max | Model calls | Tool calls | TS calls | In tok (incl. cache) | Cost p50 $ | TS resp tok | MCP ms p50 | DB ms p50 | DB ops p50 |
|---|---|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| bugfix | baseline | 3 | 100% | pass 3 | 42.9 | 34.8–80.6 | 4.0 | 4.0 | 0.0 | 76716 | 0.002 | 0.0 | n/a | n/a | n/a |
| bugfix | tokensave | 3 | 100% | pass 3 | 42.9 | 25.6–76.8 | 4.0 | 4.0 | 0.0 | 95334 | 0.003 | 0.0 | 0.0 | 0.0 | 0.0 |
| bugfix | tokensave-guided | 3 | 100% | pass 3 | 59.5 | 59.2–113.2 | 4.0 | 4.0 | 0.0 | 103182 | 0.004 | 0.0 | 0.0 | 0.0 | 0.0 |
| callers | baseline | 3 | 100% | pass 3 | 27.9 | 22.1–38.5 | 4.0 | 5.0 | 0.0 | 77563 | 0.002 | 0.0 | n/a | n/a | n/a |
| callers | tokensave | 3 | 100% | pass 3 | 23.3 | 20.3–32.4 | 3.0 | 3.0 | 1.0 | 71940 | 0.002 | 550 | 1329 | 3.4 | 6.0 |
| callers | tokensave-guided | 3 | 100% | pass 3 | 55.5 | 13.3–59.4 | 2.0 | 2.0 | 2.0 | 49681 | 0.001 | 617 | 7114 | 10 | 8.0 |
| definition | baseline | 3 | 100% | pass 3 | 19.8 | 15.6–30.6 | 4.0 | 3.0 | 0.0 | 74473 | 0.001 | 0.0 | n/a | n/a | n/a |
| definition | tokensave | 3 | 100% | pass 3 | 13.6 | 13.4–15.7 | 3.0 | 3.0 | 1.0 | 70833 | 0.002 | 377 | 1375 | 26 | 4.0 |
| definition | tokensave-guided | 3 | 100% | pass 3 | 29.1 | 19.3–33.9 | 3.0 | 3.0 | 3.0 | 76361 | 0.001 | 804 | 3786 | 169 | 38 |
| impact | baseline | 3 | 100% | pass 3 | 33.2 | 15.2–47.0 | 2.0 | 2.0 | 0.0 | 43278 | 0.003 | 0.0 | n/a | n/a | n/a |
| impact | tokensave | 3 | 100% | pass 3 | 37.1 | 23.7–53.8 | 2.0 | 2.0 | 0.0 | 52694 | 0.003 | 0.0 | n/a | n/a | n/a |
| impact | tokensave-guided | 3 | 100% | pass 3 | 26.9 | 25.5–37.9 | 2.0 | 2.0 | 1.0 | 56454 | 0.003 | 204 | 2553 | 24 | 1.0 |
| missing-symbol | baseline | 3 | 100% | pass 3 | 12.0 | 9.2–14.1 | 2.0 | 1.0 | 0.0 | 35666 | 0.001 | 0.0 | n/a | n/a | n/a |
| missing-symbol | tokensave | 3 | 100% | pass 3 | 29.3 | 28.9–75.4 | 2.0 | 2.0 | 1.0 | 45431 | 0.001 | 156 | 2797 | 0.0 | 0.0 |
| missing-symbol | tokensave-guided | 3 | 100% | pass 3 | 31.4 | 19.9–35.7 | 3.0 | 3.0 | 2.0 | 74170 | 0.001 | 158 | 2792 | 33 | 3.0 |
| multi-hop | baseline | 3 | 100% | pass 3 | 26.1 | 25.3–58.7 | 5.0 | 9.0 | 0.0 | 133387 | 0.006 | 0.0 | n/a | n/a | n/a |
| multi-hop | tokensave | 3 | 100% | pass 3 | 40.8 | 39.2–77.4 | 6.0 | 10 | 3.0 | 168476 | 0.006 | 2160 | 4292 | 166 | 49 |
| multi-hop | tokensave-guided | 3 | 100% | pass 3 | 61.1 | 28.9–120.2 | 7.0 | 15 | 9.0 | 207858 | 0.007 | 2678 | 5687 | 186 | 51 |
| unindexed-file | baseline | 3 | 100% | pass 3 | 27.4 | 25.8–35.5 | 3.0 | 2.0 | 0.0 | 54009 | 0.001 | 0.0 | n/a | n/a | n/a |
| unindexed-file | tokensave | 3 | 100% | pass 3 | 30.8 | 26.8–52.8 | 3.0 | 3.0 | 1.0 | 68607 | 0.002 | 49 | 2219 | 2.7 | 2.0 |
| unindexed-file | tokensave-guided | 3 | 100% | pass 3 | 21.0 | 16.9–39.8 | 3.0 | 3.0 | 2.0 | 73754 | 0.002 | 99 | 2436 | 5.9 | 4.0 |

## Against baseline

| Task / arm | Δ wall p50 (s) | 95% CI (bootstrap) | Δ success | Dominant actionable bottleneck |
|---|---:|---|---:|---|
| bugfix/tokensave | -0.0 | -55.0 … +42.0 | +0% | no single dominant bottleneck |
| bugfix/tokensave-guided | +16.6 | -21.4 … +78.4 | +0% | no single dominant bottleneck |
| callers/tokensave | -4.6 | -18.2 … +10.3 | +0% | no single dominant bottleneck |
| callers/tokensave-guided | +27.6 | -25.2 … +37.3 | +0% | no single dominant bottleneck |
| definition/tokensave | -6.2 | -17.2 … +0.1 | +0% | no single dominant bottleneck |
| definition/tokensave-guided | +9.3 | -11.3 … +18.3 | +0% | agent/model overhead (63% of wall time; TokenSave internals 16%) |
| impact/tokensave | +4.0 | -23.3 … +38.6 | +0% | agent/model overhead (55% of wall time) |
| impact/tokensave-guided | -6.3 | -21.5 … +22.7 | +0% | agent/model overhead (67% of wall time; TokenSave internals 8%) |
| missing-symbol/tokensave | +17.4 | +14.8 … +66.2 | +0% | no single dominant bottleneck |
| missing-symbol/tokensave-guided | +19.4 | +5.7 … +26.5 | +0% | number of calls (3 of 6 TokenSave calls repeated) |
| multi-hop/tokensave | +14.6 | -19.5 … +52.2 | +0% | agent/model overhead (67% of wall time; TokenSave internals 7%) |
| multi-hop/tokensave-guided | +34.9 | -29.8 … +94.9 | +0% | agent/model overhead (51% of wall time; TokenSave internals 11%) |
| unindexed-file/tokensave | +3.4 | -8.7 … +27.1 | +0% | no single dominant bottleneck |
| unindexed-file/tokensave-guided | -6.4 | -18.6 … +14.0 | +0% | agent/model overhead (64% of wall time; TokenSave internals 14%) |

## Verdict by arm

- **tokensave**: agent/model overhead (51% of wall time; TokenSave internals 6%)
- **tokensave-guided**: no single dominant bottleneck

## Environment

```json
[
  {
    "agent_version": "2.1.302 (Claude Code)",
    "cpu_count": 8,
    "harness_commit": "251a0550af0fb89c4fdb3a873bbd002871333eea",
    "instruction_isolation": "verified",
    "machine": "AMD64",
    "os": "Windows",
    "os_release": "11",
    "python": "3.14.7",
    "tokensave_sha256": "3837635c7615920d6a82f9b8bad1d2fecbeeca7bb861769094ac23755eb5e43c",
    "tokensave_version": "tokensave 7.17.0"
  }
]
```
