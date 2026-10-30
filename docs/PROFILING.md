# Performance profiling

tokensave can record where its own time goes (database operations, MCP tool phases, indexing and sync phases) from real usage. Profiling is **off by default**, and the data **stays on your machine**.

## Enabling it

```bash
tokensave profile enable                 # light: counters and latency histograms
tokensave profile enable --detailed --for 2h
tokensave profile disable                # stop recording; data is kept
TOKENSAVE_PROFILE=light tokensave sync   # one run only (off|light|detailed)
```

`enable` and `disable` change the `[profiling]` table in `~/.tokensave/config.toml`:

```toml
[profiling]
mode = "light"            # off | light | detailed
slow_query_ms = 50        # operations at least this slow are counted as slow
retention_days = 14       # older data is deleted
max_bytes = 33554432      # size cap for ~/.tokensave/profile/ (32 MiB)
```

New tokensave processes pick the change up. Restart running MCP servers to include them. Detailed mode falls back to light when `--for` runs out (default 24h).

## What is recorded

| Series | Meaning |
|---|---|
| `db.*` | One per database query method: latency (execution plus reading rows), rows, errors, slow count. Also write-lock wait (`db.write_lock_wait`) and write transactions (`db.transaction`). |
| `mcp.*` | Phases of a `tools/call`: `total`, `project_resolution`, `freshness`, `handler`, `graph_traversal`. |
| `tool.<name>` | Per tool: end-to-end latency, database time and operations per call, response size, errors. |
| `index.*`, `sync.*` | Phases of a full index and of an incremental sync. |
| `run.*` | Whole runs bucketed by project file count, so latency can be compared across repository sizes. |

Failed operations are counted as errors and kept out of the latency histograms. Execution and row decoding are reported together because libSQL performs them lazily inside the same call.

## Reports

```bash
tokensave profile status
tokensave profile report [--mcp|--database|--indexing] [--since 24h] [--engine libsql] [--json]
tokensave profile slow-queries [--since 24h]
tokensave profile export [--since 7d] [-o profile.json]
tokensave profile reset
```

Example `report --mcp` (illustrative numbers):

```text
Engine: libsql (window: 24h)
OPERATION                                        COUNT    ERR    TOTAL ms      MEAN       P50        P95        P99       MAX
tool.tokensave_context                             412      0     18233.0     44.25     38.91      97.28     151.55    402.10
mcp.handler                                        961      3     31002.6     32.26     21.50      88.06     140.29    402.10
tool.tokensave_search                              233      0      2410.7     10.35      8.19      22.14*     40.95*    61.20

Database share of measured MCP time: 41% (outside DB: 59%)

Times in ms. Percentiles are bucket upper bounds (<= 1.6% high). * = fewer than 100 samples, P95/P99 unreliable.
```

Every row is ranked by cumulative time, not by its single slowest call. Percentiles carry their sample count; P95/P99 below 100 samples are marked with `*`.

## Privacy

- Nothing is sent anywhere. Profiling code has no network access, and a test enforces this.
- Never recorded: source code, file paths, project or folder names, repository URLs, prompts, tool arguments, SQL text or parameters, search terms.
- `profile export` prints the **sanitized export**: tokensave version, OS family, CPU architecture and count, engine name, and aggregates keyed by static operation and registered tool names. Any other series name is dropped. Review it before attaching it to an issue.

## Storage and overhead

Each process keeps fixed-size in-memory aggregates (about 1 MiB, plus about 21 KiB per registered tool in an MCP server). A background thread writes one small JSON file per process per hour under `~/.tokensave/profile/`, every 60 s and at exit, enforcing `retention_days` and `max_bytes` as it writes. Nothing is written while nothing has been recorded. `profile status` shows the profiler's own flush cost. With profiling off, each instrumented site costs one atomic load.
