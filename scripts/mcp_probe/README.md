# MCP probe matrix

Drive a fresh `tokensave serve` MCP server over stdio and exercise every
read-only tool against a set of real repos. Produces a per-tool / per-repo
status matrix that flags tools needing investigation (errors, timeouts,
empty results, performance regressions).

Used as both:
- **Bug probe.** New language support, new tool, or refactor — re-run the
  matrix and any regression shows up as a flagged cell.
- **Benchmark.** Timings per call are logged; the same repos serve as a
  fixed corpus for cross-version perf comparison.

## Layout

```
scripts/mcp_probe/
├── README.md              ← this file
├── repos.toml             ← repo paths + which probe sets apply
├── probe.py               ← main driver (spawn MCP, run tools, log)
├── isolated.py            ← per-tool fresh-server retry (escapes cascades)
├── build_matrix.py        ← read logs → markdown table
└── tools/
    └── rust.py            ← Rust-flavored probe inputs (5 queries per tool)
```

`tools/` is the place to add language-specific probe inputs (Python, Go, …).
Each module exposes a `probes_for(discovered)` function that returns
`{tool_name: [args_dict, …]}`.

## Quick run

```sh
# 1. Build the release binary (driver shells out to ../../target/release/tokensave)
cargo build --release --bin tokensave

# 2. Run the matrix
python3 scripts/mcp_probe/probe.py
python3 scripts/mcp_probe/build_matrix.py > /tmp/matrix.md
```

The driver writes per-call status to `/tmp/tokensave_matrix.log` in TSV:
`repo  tool  query_json  status  detail`, where status ∈
`OK | EMPTY | ERROR | TIMEOUT | BAD | SLOW`.

## Cascade caveat

The MCP server is single-threaded over stdio. If one tool times out, its
response arrives late and poisons every subsequent call's id-match (clients
that don't strictly match by id will read the late response as the new one
and appear to time out). `probe.py` matches by id when possible; for tools
that already showed a real timeout, re-run them via `isolated.py` which
spawns a fresh server per tool.

## Adding a new repo

Append to `repos.toml`:

```toml
[[repos]]
name = "polkadot"
path = "/Volumes/home_ext1/Src/0parity/polkadot-sdk"
languages = ["rust"]
```

`languages` selects which probe modules (`tools/<lang>.py`) contribute their
query set for that repo.

## Adding a new language

Drop `tools/<lang>.py` exposing `probes_for(discovered)`. Then list
`<lang>` in any repo's `languages` array.
