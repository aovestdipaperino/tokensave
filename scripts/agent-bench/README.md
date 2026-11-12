# Agent workflow benchmark (#948)

Measures what TokenSave changes in **end-to-end agent tasks**: correctness,
wall time, model turns, tool calls, tokens and cost. Fast local queries matter
only when they change these numbers. The harness drives a real agent headless,
runs each task with and without TokenSave, and checks every answer itself.

TokenSave-internal time comes from the existing opt-in profiler (#754,
[docs/PROFILING.md](../../docs/PROFILING.md)). The harness adds no
instrumentation to TokenSave. Methodology, results and the decision report
are in [docs/AGENT-WORKFLOW-BENCHMARK.md](../../docs/AGENT-WORKFLOW-BENCHMARK.md).

## Prerequisites

- Python 3.9+, git, and a `tokensave` binary (the one under test).
- [Claude Code](https://docs.claude.com/en/docs/claude-code) (`claude`),
  authenticated. Every trial is a **paid, real-model run**. `--budget` caps
  each trial's spend (default 2 USD), as enforced by the agent.
- Network access to fetch the pinned repository once (or a local clone passed
  with `--repo-source`).

## Commands

```sh
# 1. Pinned, disposable checkouts: <work>/ws/<repo>-plain and <repo>-indexed.
python3 scripts/agent-bench/agent_bench.py prepare --work /private/bench \
    --tokensave target/release/tokensave \
    --repo-source tokensave="$PWD"          # optional: fetch the pinned commit locally

# 2. Trials. Appends one record per trial to <work>/results.jsonl.
python3 scripts/agent-bench/agent_bench.py run --work /private/bench \
    --tokensave target/release/tokensave --model claude-haiku-5-5 \
    --repetitions 3 --seed 948 --warmup

# 3. Report.
python3 scripts/agent-bench/agent_bench.py report --work /private/bench --out report.md
```

Useful `run` options: `--only-tasks definition,bugfix`, `--only-arms
baseline,tokensave`, `--effort low`, `--timeout 900`, `--no-profile`,
`--keep-raw` (see Privacy). `report --json` prints the aggregates.
`report --run <id>` limits it to some runs.

Unit tests (synthetic traces, no model calls):

```sh
python3 -m unittest discover -s scripts/agent-bench -v
```

## What is compared

[`arms.json`](arms.json) defines agent configurations. All arms share the
model, CLI arguments and permission allowlist; they differ only in the
declared fields:

| Arm | MCP servers | Checkout | Extra prompt |
|---|---|---|---|
| `baseline` | none | unindexed | none |
| `tokensave` | `tokensave serve` (libSQL) | indexed | none (server instructions only) |
| `tokensave-guided` | `tokensave serve` (libSQL) | indexed | `~/.claude/rules/tokensave.md`, as `tokensave install` writes it |

`tokensave-guided` is skipped and reported as unsupported when the rules file
does not exist.

### Caveman arms (#797)

Three more arms compare TokenSave with and without
[Caveman](https://github.com/JuliusBrussee/caveman). They run only when `run`
gets `--caveman-skill PATH`, the `SKILL.md` of a Caveman checkout you pinned
yourself. The harness never fetches or installs Caveman. Without the option
these arms are skipped and reported as unsupported.

| Arm | MCP servers | Extra prompt | Cooperation | Compared with |
|---|---|---|---|---|
| `caveman` | none | the Caveman skill | n/a | `baseline` |
| `tokensave-caveman-nocoop` | `tokensave serve` | the Caveman skill | off | `caveman` |
| `tokensave-caveman` | `tokensave serve` | the Caveman skill | on | `tokensave-caveman-nocoop` |

Each comparison changes one thing. `tokensave` against `baseline` and
`tokensave-caveman-nocoop` against `caveman` show TokenSave's effect
without and with Caveman; `tokensave-caveman` against
`tokensave-caveman-nocoop` shows the effect of the cooperation paragraph
alone (#971).

- The skill must be UTF-8, at most 64 KiB and contain a `name: caveman`
  line, the same checks TokenSave's discovery applies. It is appended
  verbatim to the system prompt with `--append-system-prompt`. This is
  not stock plugin activation: Claude Code's plugin, its hooks and its
  mode switching are not loaded (`--setting-sources ""` drops them), and
  the intensity is whatever the skill text defaults to.
- For both TokenSave+Caveman arms the server gets `CLAUDE_CONFIG_DIR` set to
  `<work>/caveman-agent-home`, which holds only the pinned skill, so
  discovery finds that skill and not your own install. The agent keeps its
  own configuration directory. The arms differ only in
  `TOKENSAVE_CAVEMAN_COMPATIBILITY` (`1` or `0`). Every other TokenSave arm
  gets `0`, so a `caveman_compatibility = true` in your config cannot leak
  into them.
- Before the first trial, `run` starts the server of each TokenSave+Caveman
  arm, sends MCP `initialize`, and stops unless the cooperation paragraph
  is present exactly in `tokensave-caveman`. The `--tokensave` binary must
  include #971.
- Each trial records `conditions.caveman` and
  `conditions.caveman_cooperation`. The run environment records the
  skill's SHA-256, the verified cooperation state per arm, and the SHA-256
  of each arm's appended prompt text.

Isolation keeps the arms comparable:

- `--strict-mcp-config` drops the user's MCP servers.
- `--setting-sources ""` drops user and project settings, hooks and
  instruction files. `--setting-sources project` is not enough: it still
  loads `~/.claude/rules/`, so a user with TokenSave installed would hand
  its rules to every arm, including the baseline. `--safe-mode` is not
  usable: it also drops the `--mcp-config` server.
- Agent instruction files (`AGENTS.md`, `CLAUDE.md`, `.claude/`) are removed
  from the checkouts.
- The agent gets the harness's environment minus every `CLAUDE*` and
  `MCP_CONNECTION*` variable (except `CLAUDE_CONFIG_DIR` and
  `CLAUDE_CODE_GIT_BASH_PATH`), so running the harness from inside an agent
  session does not pass on that session's id, effort or messaging socket.
- Before the first trial, `run` asks the agent, with the arms' arguments and
  no MCP server, to list the instruction files it loaded. It stops unless
  the answer is `NONE` (one cheap model call; `--skip-preflight` skips it
  and records `instruction_isolation: not-checked`).

A Grafeo arm (#904) is not defined: Grafeo is beta-only, and an equivalent
workload needs its build and locking model on the binary under test. Add an
arm with a beta `--tokensave` binary when that holds.

## Tasks

[`tasks.json`](tasks.json) holds the suite. Repositories are pinned:
`tokensave` at a fixed commit (expected answers hold only at that commit) and
`fixtures/inventory`, a small Python project with one seeded bug.

| Task | Category | Check |
|---|---|---|
| `definition` | symbol definition | answer names `src/profile/storage.rs` and `file_path`; no file changed |
| `callers` | references | answer names `load` and `flush` in `storage.rs`; no file changed |
| `impact` | impact analysis | answer names `load` and `flush` and says old files are skipped |
| `multi-hop` | multi-hop retrieval | answer names `handle_profile_action`, `load`, `sanitized` |
| `missing-symbol` | missing information | answer says `NOT FOUND` |
| `unindexed-file` | stale or unindexed | a file written after indexing is found |
| `bugfix` | patch | `python -m unittest discover -s tests` passes, `tests/` unchanged |

Checks are independent of the agent: regular expressions over the final
answer, a command run in the checkout after the trial, and `git status`.
Every trial starts from `git reset --hard` to the snapshot plus the task's
`setup_files`. Outcomes are `pass`, `wrong_answer`, `check_failed`,
`modified_workspace`, `modified_protected_files`, `timeout`,
`agent_<subtype>` (for example `agent_error_max_budget_usd`) or
`agent_crash`, or `harness_error` when a check itself could not run.

## What is measured

Times are harness receipt times of each stream-json line.

- **Wall time** is partitioned into startup (launch to the agent's `init`
  event), model wait (no tool call pending), tool wait (at least one tool
  call pending) and tail (final result to exit). The four add up to the wall
  time, so nothing is double counted when tools run in parallel.
- **Model wait** is the observable API round trip, including streaming and
  network. It is not provider inference time, which no client can observe.
  The agent's own `duration_api_ms` is recorded beside it.
- **Summed per-tool wait** is recorded separately. It can exceed tool wait
  when calls overlap.
- **Tokens and cost** are as reported by the agent (`usage`,
  `total_cost_usd`, the agent's own estimate at its built-in rates).
  Missing values are `null` and shown as `n/a`, never zero.
- **Model calls** are distinct top-level assistant messages; **turns** are
  the agent's `num_turns`. Subagent tool calls are counted apart.
- **Duplicates:** an *exact duplicate* is a retrieval call with the same
  tool and arguments as an earlier one, with no file-changing call
  (`Edit`, `Write`, shell, TokenSave edit tools) in between. After such a
  change it is a *revalidation*, which is legitimate. A *near duplicate*
  repeats the same normalized query argument (`query`, `task`, `pattern`,
  `symbol`, `path`, ...) with other arguments changed.
- **TokenSave response size** is the byte size of TokenSave tool results;
  `tokens_est` divides it by 4.
- **TokenSave internals** (#754): the MCP server runs with
  `TOKENSAVE_PROFILE=light`. After each trial the harness merges the profile
  files created during it: MCP calls and time (`mcp.total`), freshness
  checks, database time and operations attributed to tools, repeated
  per-call database operations (N+1 candidates), and catch-up sync/index time.
  These are subsets of tool wait (sync on connect may also overlap startup).

Each arm is compared with its reference arm: `reference` in `arms.json`,
`baseline` by default and for records written before the field existed.
The comparison table shows the differences in median wall time (with a
bootstrap 95% interval), median cost and median output tokens, and in
success rate.

The report's verdict per task and arm is a documented heuristic, applied in
this order: lower success than the reference arm means **response/context quality**;
database time of at least 10% of wall time means **database latency**;
exact plus near duplicates of at least 20% of TokenSave calls means **number
of calls**; model wait of at least 50% of wall time means **agent/model
overhead**. Otherwise no single bottleneck dominates.

## Privacy

By default `results.jsonl` holds numbers, tool names, outcome labels and
check booleans only. It contains no prompts, answers, tool arguments, file
paths, source code or tool output. Duplicate detection uses HMAC-SHA-256
with a random key that exists only for one run and is never written.
Profile data is the profiler's own sanitized aggregates. Nothing is
uploaded.

`--keep-raw` is the explicit opt-in for full traces under
`<work>/raw/<run>/`. They contain prompts, arguments, paths, source and
answers. Review and scrub them before sharing.

The agent itself sends prompts and code to its model provider, as in normal
use.

## Limitations

- Models are not deterministic. Report repetitions, ranges and confidence
  intervals, not single runs. The order of trials is shuffled with a seed;
  `--warmup` runs one discarded trial per task and arm first.
- Every trial starts a new agent and MCP server process (cold process). The
  index is prebuilt (warm index). OS file caches are warm after `--warmup`
  and unknown otherwise.
- Profile files are attributed by creation time. Another TokenSave process
  started during a trial (a git hook, another agent) is merged into it. Run
  the benchmark on an otherwise idle machine. If the server is killed rather
  than exiting, its last minute of data may be missing.
- On Windows, `~` for the profile directory is taken from `USERPROFILE`;
  pass `--profile-dir` when TokenSave resolves the home folder elsewhere.
- Keep `--work` outside the system temp directory. Claude Code protects its
  own temp folder, so edits there are denied and the `bugfix` task fails
  with `check_failed`.
- Only Claude Code is supported. Another agent needs its own runner and
  trace parser.
