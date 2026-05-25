# tsbench — tokensave run

Adapts [`Mibayy/tsbench`](https://github.com/Mibayy/tsbench) (the 96-task
agent benchmark token-savior uses to publish its 97.9% score) to drive
tokensave instead of token-savior, then reports the result.

Headline: **184 / 192 (95.8%)** on the first untuned run. Full breakdown in
[`SUMMARY.md`](SUMMARY.md). See also
[`docs/TOKENSAVE-VS-TOKENSAVIOR.md`](../../docs/TOKENSAVE-VS-TOKENSAVIOR.md) §4.

## Reproduce

```bash
# 1. Clone tsbench
git clone --depth=1 https://github.com/Mibayy/tsbench /tmp/tsbench
cd /tmp/tsbench

# 2. Apply the tokensave fork patch
patch -p0 < /path/to/tokensave/benchmarks/tsbench/bench_tokensave.patch
#    -> produces bench_tokensave.py alongside the original bench.py

# 3. Index the synthetic project with tokensave
tokensave init .

# 4. Run all 96 tasks
TOKENSAVE_BIN=$(which tokensave) TSBENCH_BARE=0 \
  python3 bench_tokensave.py --tasks all --run B

# 5. Per-task JSON appears in ./results-tokensave/raw/
#    Aggregate stats with:
python3 - <<'PY'
import json, pathlib
files = sorted(pathlib.Path("results-tokensave/raw").glob("TASK-*-run-B.json"))
score = sum(json.loads(f.read_text())["score"] for f in files)
print(f"{score}/{2*len(files)} = {score/(2*len(files))*100:.1f}%")
PY
```

## What the patch changes (vs. upstream `bench.py`)

- **MCP config** — launches `tokensave serve -p <root> --timings` instead of
  `token_savior.server` over Python stdio.
- **System prompt** — rewrites `SYSTEM_PROMPT_TS` to map each token-savior
  tool to its tokensave equivalent (`find_symbol` →
  `tokensave_find_exact_symbol`, `get_function_source` → `tokensave_body`,
  `get_full_context` → `tokensave_context`, etc.). Where no tokensave
  equivalent exists (`add_field_to_model`, `move_symbol`,
  `analyze_config`, `analyze_docker`), the prompt explicitly allows
  `Read` / `Edit` fallback.
- **`--disallowedTools`** — relaxed from
  `["Read","Grep","Glob","Agent"]` to `["Agent"]` only, since the four
  fallback task categories need text-level tools.
- **Tool-prefix matcher** — `ts_prefixes = ("mcp__tokensave__",)`.
- **Results path** — `results-tokensave/raw/` (so a tokensave run doesn't
  overwrite token-savior's `results/raw/`).
- **Seed-session filename** — `.bench-tokensave-session-id`.
- **`CLAUDE_PROJECT_ROOT`** env var — set to `ROOT` (the local repo) instead
  of the hard-coded `/root/projects/tsbench`.

## Environment

- `TOKENSAVE_BIN` — path to the tokensave binary. Defaults to the release
  build in the canonical checkout location.
- `TSBENCH_BARE` — set to `0` on macOS / Max OAuth (default is `1`, but
  `--bare` mode broke OAuth in our environment). On Linux + API key, leave
  default.
- `TSBENCH_MODEL` — defaults to `claude-opus-4-7`.

## License

The original `bench.py` is MIT (`Mibayy/tsbench`). The patch in this
directory is contributed back under the same terms.
