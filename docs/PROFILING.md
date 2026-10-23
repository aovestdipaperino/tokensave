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
tokensave profile analyze [--since 7d] [--top 10] [--json]
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

Every row is ranked by cumulative time, not by its single slowest call. `analyze` turns the same data into ranked findings:

```text
Recorded MCP calls: 8420 (window: 7d, engine: libsql)

1. [excessive query count] db.get_outgoing_edges in tokensave_context
   Impact: 18.4 s (estimated: executions x mean latency)
   Measured: 42180 executions in 1530 of 8420 calls (at least 20 per affected call, 28 on average); median 5 DB ops per call (n=8420)
   Hypothesis: per-item lookups in a loop (N+1); consider batching them into one query
```

Findings use exclusive time for phases, so nothing is counted twice, and the evidence is what was measured. The hypothesis is only a suggestion. Percentiles carry their sample count; P95/P99 below 100 samples are marked with `*`.

## Privacy

- Nothing is sent anywhere. Profiling code has no network access, and a test enforces this.
- Never recorded: source code, file paths, project or folder names, repository URLs, prompts, tool arguments, SQL text or parameters, search terms.
- `profile export` prints the **sanitized export**: tokensave version, OS family, CPU architecture and count, engine name, and aggregates keyed by static operation and registered tool names. Any other series name is dropped. Review it before attaching it to an issue.

## Storage and overhead

Each process keeps fixed-size in-memory aggregates (about 1 MiB, plus about 21 KiB per registered tool in an MCP server). A background thread writes one small JSON file per process per hour under `~/.tokensave/profile/`, every 60 s and at exit, enforcing `retention_days` and `max_bytes` as it writes. Nothing is written while nothing has been recorded. `profile status` shows the profiler's own flush cost. With profiling off, each instrumented site costs one atomic load.

## Performance sharing (draft; uploads disabled)

This build has **no configured sharing destination**. The maintainer must approve
and publish the HTTPS endpoint, who can access uploads, the server retention
period, the deletion process, and whether aggregates will be published before
sharing can ship. `profile share enable` fails without changing consent until
that policy is present. There is no environment or configuration setting that
can substitute a destination. No real upload was used to validate this draft.

```sh
tokensave profile share status
tokensave profile share preview
tokensave profile share enable
tokensave profile share disable
```

`preview` prints exactly the bytes `profile export` prints with its default
seven-day window, including the trailing newline. The uploader uses the same
serializer. Fields are the versioned export schema, tokensave version, OS,
architecture, logical CPU count, window length, approved engine names, static
operation/tool names, counters and histograms. No machine/user identifier,
repository name, paths, symbols, SQL, parameters or prompts are sent. The
existing export schema identifies the payload; uploads add no envelope or IDs.
Payloads over 1 MiB are skipped.

The separate `profiling_share_enabled` preference defaults to false. It is
independent of `upload_enabled` (token counting) and of the profiling mode in
both directions. Sharing requires profiling; granting sharing consent does not
enable it. Once a destination is approved, `profile enable` offers sharing once
only on an interactive terminal, displaying the destination, policies, fields
and exact payload; the default answer is **No**. Hooks, MCP servers, installation,
upgrades and redirected input never prompt. `share enable` is the explicit
non-interactive consent action. `TOKENSAVE_PROFILE_SHARE=0` (also `off`, `false`,
`no` or empty) blocks uploads for a run. It cannot grant consent.

Uploads run on the owned profiler flush thread after local persistence, with a
two-second total request timeout, no redirects, and the existing TLS trust-store
configuration. No request runs on the recording or MCP request path. A separate
cross-process lock and attempt timestamp permit at most one attempt per 24 hours,
including failures; failures are discarded without immediate retries. Shutdown
joins the flush thread and may wait up to the request timeout for an in-flight
upload. There is no detached task or upload queue. Local aggregates and retention
are unchanged. `share disable` is read again before each send, including by
already-running servers; a request already sent cannot be recalled. The draft
cannot promise deletion of historical server data: the deletion process remains
a maintainer decision and is a release prerequisite.
