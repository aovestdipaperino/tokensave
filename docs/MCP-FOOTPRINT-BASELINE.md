# MCP footprint and latency baseline

A baseline of what a default tokensave MCP server costs an agent: the bytes and
tokens of the `initialize` result and of `tools/list`, cold start, full index
time and size, and the latency of a fixed query set (#917, part of #916). Every
number below was produced by the command in [Reproducing](#reproducing) and is
copied from its raw output in [`benchmarks/mcp-footprint/`](../benchmarks/mcp-footprint/).
Nothing is estimated from other sources.

## Setup

| Item | Value |
| --- | --- |
| Binary | `tokensave 7.17.0`, `cargo build --release --locked --bin tokensave`, default `full` features |
| Source of the binary | branch `test/mcp-footprint-baseline` on top of `cb57475` (this change adds tests and docs only) |
| Indexed repository | tokensave itself at `cb57475c201fcbac1688b24c8db023247a392b5d`, 798 tracked files, cloned without the `codegraph` submodule |
| Host | Windows 11 Pro 10.0.26300, Intel64 Family 6 Model 140 (8 logical CPUs), AMD64 |
| Python | 3.14.7, with `tiktoken` 0.14.0 in a virtual environment |
| `ast-grep` on `PATH` | No |
| Environment | isolated `HOME`/`USERPROFILE`, `TOKENSAVE_UPDATE_CHECK=off`, `TOKENSAVE_SKIP_AGENT_MAINTENANCE=1`, `TOKENSAVE_AUTO_INIT=0`, `TOKENSAVE_TOOLS` and `TOKENSAVE_REPORT_SAVINGS` unset except where noted |

The probe clones the repository into a fresh temporary directory, so the
server finds no sibling projects and the `initialize` instructions carry no
project paths. The machine was a developer laptop with other programs running,
and OS file caches were not flushed, so the timings are indicative, not
controlled. They come from one host; on WSL2 or shared CI runners expect more
noise.

## Tokenizers

| Name | Version | Limits |
| --- | --- | --- |
| chars/4 | tokensave's own estimate, `src/context/read_modes.rs` | Characters divided by four, integer division. Not a model tokenizer; it is what tokensave uses for its savings figures. |
| tiktoken `o200k_base` | `tiktoken` 0.14.0 | OpenAI's encoding for GPT-4o-class models. Not Claude's tokenizer. Counts the text of the payload only, without any request framing a client adds. |
| tiktoken `cl100k_base` | `tiktoken` 0.14.0 | OpenAI's GPT-4-class encoding. Same limits as `o200k_base`. |

No Claude token count was measured: the Anthropic `count_tokens` API needs an
API key, which this run did not have, and its count depends on the model and on
how the client frames tool schemas in the request. Adding it is follow-up work;
treat the tiktoken figures as an order of magnitude for Claude models, not as a
Claude count.

## Payload size

Bytes are UTF-8 bytes of the compact JSON of each `result` object, as written
to [`initialize-core.json`](../benchmarks/mcp-footprint/initialize-core.json),
[`tools-list-core.json`](../benchmarks/mcp-footprint/tools-list-core.json),
[`initialize-full.json`](../benchmarks/mcp-footprint/initialize-full.json) and
[`tools-list-full.json`](../benchmarks/mcp-footprint/tools-list-full.json).
"Instructions" is the `instructions` string alone.

| Toolset | Listed tools | `initialize` result | instructions | `tools/list` result | `tools/list` + instructions |
| --- | ---: | ---: | ---: | ---: | ---: |
| core (default) | 12 | 2,068 B | 1,891 B | 22,165 B | **24,056 B** |
| full (`TOKENSAVE_TOOLS=full`) | 86 | 1,236 B | 1,077 B | 96,884 B | 97,961 B |

The core list is the 11 core tools plus `tokensave_more`. The core
instructions are longer than the full ones because they name the core tools and
the areas `tokensave_more` can list. With `ast-grep` on `PATH` the full list
has 87 tools, adding `tokensave_ast_grep_rewrite`; the core list does not
change, because that tool is in the `edit` area.

| Toolset | Payload | chars/4 | `o200k_base` | `cl100k_base` |
| --- | --- | ---: | ---: | ---: |
| core | `initialize` result | 516 | 440 | 437 |
| core | instructions | 472 | 390 | 389 |
| core | `tools/list` result | 5,533 | 5,072 | 5,000 |
| full | `initialize` result | 308 | 259 | 255 |
| full | instructions | 268 | 212 | 210 |
| full | `tools/list` result | 24,198 | 21,834 | 21,439 |

A client sends the listed schemas on every turn and the instructions once per
session, so with the default toolset roughly 5,000 tiktoken tokens of schema
are a fixed per-turn cost, against roughly 21,500 with the full toolset. A
client that defers tool schemas pays less: only the five `anthropic/alwaysLoad`
tools are loaded up front.

### Size guard

`default_footprint_tests` in `src/mcp/server.rs` builds the same quantity in
process: the default `tools/list` result (core toolset, no area listed) plus
the `initialize` instructions with savings reporting off and no sibling
projects. It fails when that exceeds `DEFAULT_FOOTPRINT_BYTES`, recorded as
24,056 bytes, the value the live server returned above. The `serverInfo`
version and project-dependent text are left out so the count is the same on
every host.

The count does not depend on the language features (`lite`, `medium`, `full`),
because no tool definition is feature-gated, or on `ast-grep`, because the core
list never includes the one tool it gates. The test was run under `lite` and
`full`; see the pull request for the results. Raising the constant requires a
reason stated in the pull request and an update to this report.

## Cold start

Wall time from spawning `tokensave serve --path <project>` to the `initialize`
response, and to the response of a `tools/list` sent right after it. Ten runs
each, a new process per run.

| State | To `initialize`, median | p95 | min–max | To first `tools/list`, median | p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| No index | 84.1 ms | 192.6 ms | 62.7–192.6 ms | 87.6 ms | 197.2 ms |
| Indexed | 299.2 ms | 439.0 ms | 233.5–439.0 ms | 302.1 ms | 442.9 ms |

With an index the server opens the database and checks for stale files before
answering, which accounts for most of the difference. With 10 runs, p95 is the
largest value.

## Index time and size

| Measurement | Value |
| --- | ---: |
| `tokensave init <project> --no-git-hook --cpu-usage balanced`, wall time | 35.1 s |
| `.tokensave` directory after `init` | 115,672,568 B (110.3 MiB) |

One run. `--cpu-usage balanced` pins the concurrency profile so the run does
not start the one-time hardware benchmark that an unpinned `init` would run in
a fresh home.

## Query latency

One server process on the indexed project. Each query is sent once to warm up,
then five timed times; latency is from writing the request to reading its
response. The queries are `tokensave_context` for each task in
[`benchmarks/queries/default.toml`](../benchmarks/queries/default.toml) and
`tokensave_search` for five fixed names. Result bytes are the compact JSON of
the last response.

| Tool | Argument | Result | Median | p95 (max) |
| --- | --- | ---: | ---: | ---: |
| `tokensave_context` | How is configuration loaded at startup? | 2,045 B | 184.3 ms | 210.4 ms |
| `tokensave_context` | Where are command-line arguments parsed and dispatched? | 1,619 B | 159.8 ms | 249.7 ms |
| `tokensave_context` | How is the main entry point organized? | 1,476 B | 151.3 ms | 181.4 ms |
| `tokensave_context` | How are errors defined, wrapped, and propagated? | 1,873 B | 141.5 ms | 155.4 ms |
| `tokensave_context` | Where is logging or diagnostic output emitted? | 1,725 B | 164.9 ms | 234.5 ms |
| `tokensave_context` | How are tests organized and what test harness is used? | 1,837 B | 174.5 ms | 209.4 ms |
| `tokensave_context` | How is data persisted to disk or to a database? | 1,517 B | 116.0 ms | 128.5 ms |
| `tokensave_context` | How are async tasks or background work spawned? | 1,729 B | 183.6 ms | 228.2 ms |
| `tokensave_context` | How does the build wire up dependencies and initialize state? | 1,673 B | 135.1 ms | 166.1 ms |
| `tokensave_context` | How are public API surfaces exposed (HTTP endpoints, library exports, or CLI commands)? | 1,357 B | 169.4 ms | 193.8 ms |
| `tokensave_search` | `McpServer` | 937 B | 162.3 ms | 239.7 ms |
| `tokensave_search` | `handle_tools_list` | 249 B | 206.1 ms | 244.2 ms |
| `tokensave_search` | `sync` | 1,130 B | 154.4 ms | 202.0 ms |
| `tokensave_search` | `Toolset` | 1,471 B | 167.6 ms | 490.1 ms |
| `tokensave_search` | `resolve` | 1,343 B | 267.6 ms | 313.0 ms |

Median of the per-query medians: 164.9 ms. `tokensave_search` is not faster
than `tokensave_context` here, which suggests a per-call cost shared by every
tool rather than query work; finding where that time goes is follow-up work and
outside this baseline.

## Not covered

- A Claude token count (see [Tokenizers](#tokenizers)).
- Idle CPU, RSS, and disk growth over time. `benchmarks/mcp_startup.py`
  measures RSS and idle CPU on Linux; this probe does not.
- More than one indexed repository, and Linux or macOS hosts.
- Incremental sync time.

## Reproducing

```sh
cargo build --release --locked --bin tokensave
python -m venv .venv && .venv/bin/python -m pip install tiktoken==0.14.0   # optional
.venv/bin/python benchmarks/mcp_footprint.py \
  --binary target/release/tokensave \
  --repo . --rev cb57475c201fcbac1688b24c8db023247a392b5d \
  --output-dir benchmarks/mcp-footprint --runs 10 --query-runs 5
```

On Windows the binary is `target/release/tokensave.exe` and the interpreter
`.venv/Scripts/python`. Without `tiktoken` the probe reports chars/4 only. It
writes `results.json` (every number in this report, plus the command, binary
version, commit and host) and the four payload files to `--output-dir`, and
removes its temporary clone when it finishes.

The size-guard test:

```sh
cargo test --lib default_footprint
cargo test --lib --no-default-features --features lite default_footprint
```
