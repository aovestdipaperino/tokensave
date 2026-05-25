# Tokensave vs Token-Savior Benchmark

Generated: 2026-05-25 12:08:32 UTC

Side-by-side comparison on real-world Python repositories. Both tools index
the same clone, and the same random sample of symbols (seed=42) is used for
query timing so the find_symbol column is directly comparable.

**Memory notes.** token-savior's peak memory is measured with `tracemalloc`
(Python heap only). tokensave runs as a subprocess, so its peak is the
`ru_maxrss` delta from `getrusage(RUSAGE_CHILDREN)` (resident set size).
These are *not* identical units — treat them as order-of-magnitude.

**Query timing.** token-savior is called in-process (pure Python dict
lookups). tokensave is driven over MCP via `tokensave serve --timings`
and the per-query column reports the handler's `_meta.duration_us` —
i.e. the time spent inside the Rust handler, with JSON-RPC / stdio /
Python-parse overhead stripped out. A warm-up call is issued before
each timed loop. `get_change_impact` for tokensave sums the handler
times of `search → impact`, mirroring how an agent must resolve the
symbol to a node_id before querying.

## fastapi

Naive `.py` source size: 3.6 MiB

| Metric | token-savior | tokensave |
|--------|--------------|-----------|
| Cold index time | 6.212s | 2.147s |
| Warm reindex time | 1.575s | 1.363s |
| Peak memory (cold) | 65.4 MiB | 146.5 MiB |
| Cache / DB size | 5.2 MiB | 28.6 MiB |
| Files indexed | 2,715 | 2,677 |
| Symbols / nodes | 2,740 | 29,019 |
| find_symbol avg | 0.004 ms | 0.136 ms |
| get_function_source avg | 0.133 ms | 0.117 ms |
| get_change_impact avg | 24.374 ms | 0.569 ms |

_tokensave indexed languages:_ Bash=6, JavaScript=4, Other=1548, Python=1118, TOML=1
