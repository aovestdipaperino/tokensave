# Pi model-routing evaluation (Refs #832)

This runner uses the real Pi RPC host and a real selected provider/model. It
does not supply a faux provider or treat offline tests as behavioral evidence.
It currently supports POSIX process groups and Pi **1.4.0**, whose API was
inspected at commit `d5b3de22d0e24a3e9c38a9410d3329b099717283`.

## Prepare and run

Use a disposable account/container with an installed TokenSave binary and
provider credentials. The runner invokes normal TokenSave init, MCP and hooks;
their normal global accounting/registry side effects apply to that account.
It does not install integrations in your existing Pi profile. Fixture directories,
temporary agent profiles and optional credential copies are removed after each
run, including failures. Never put credentials in command arguments or output.

For the pinned source checkout, clone it into an isolated directory and hydrate
dependencies with lifecycle scripts disabled:

```sh
git clone --no-hardlinks /home/andrea-tontini/repository/pi /tmp/pi-eval-source
git -C /tmp/pi-eval-source checkout d5b3de22d0e24a3e9c38a9410d3329b099717283
cd /tmp/pi-eval-source
npm ci --ignore-scripts --no-audit --no-fund
```

Use a Node runtime with native TypeScript support. The system Node distribution
in this environment lacks it; a temporary `node-linux-x64@22.22.1` runtime was
used successfully. Pi also requires matching generated provider data: follow
its model-data instructions or copy `dist/providers/data` from published
`@earendil-works/pi-ai@1.4.0` into the isolated source's
`packages/ai/src/providers/data`. Do not modify the original Pi checkout.

Save the source runtime's argv prefix as a JSON array in a private command file:

```json
["/path/to/node", "--import", "/tmp/pi-eval-source/packages/coding-agent/src/experimental/source-resolver.ts", "/tmp/pi-eval-source/packages/coding-agent/src/cli.ts"]
```

Pass `--pi-command-file /path/to/command.json --pi-source /tmp/pi-eval-source`
instead of `--pi ...` below. The runner verifies the source commit and clean
tracked files. The source runtime and published runtime both reported 1.4.0;
source model discovery was checked separately.

Alternatively, prepare the pinned published host without lifecycle scripts:

```sh
npm install --prefix /tmp/pi-eval-runtime --ignore-scripts --no-audit --no-fund --save-exact @earendil-works/pi-coding-agent@1.4.0
```

Commit the TokenSave checkout first: the runner records its commit and rejects
tracked changes. Record the runtime's package-lock.json with the evaluation to
pin transitive dependencies. Run with the provider's usual environment-based
authentication, or use `--auth-file /private/path/auth.json` for a Pi auth file.
That file is copied to private temporary profiles, never evaluation artifacts.
Model catalog refresh is disabled; provider calls are still real and billable.

```sh
python3 scripts/pi-eval/run.py \
  --pi node /tmp/pi-eval-runtime/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js \
  --tokensave /absolute/path/to/tokensave \
  --provider YOUR_PROVIDER --model YOUR_EXACT_MODEL_ID \
  --output /private/path/pi-routing-results --repeats 3 --long-turns 12
python3 scripts/pi-eval/report.py /private/path/pi-routing-results > /private/path/summary.jsonl
```

The output path must not exist. Artifacts can contain source, prompts, tool
arguments and model answers. Review/redact them before publication; keep hashes
and document redactions. No environment values or provider headers are recorded.
Automatic Pi retries/compaction are disabled so failures and manual compaction
are explicit. Three failures stop the run; override with `--max-failures`.

## Comparison contract

Each repeat creates independent identical fixture/index setups for baseline and
enabled sessions, alternating order across repeats. Both arms use built-in direct
MCP exposure and the production managed extension, including existing grep/find/
bash guardrails. Only the `tokensave_retrieval` section assignment is omitted in
the baseline. Context-file discovery, unrelated extensions, skills and templates
are disabled. This measures the structured guidance contribution, **not** the
total effect of installing TokenSave or an adapter comparison.

The eight scenarios cover definition/callers/cross-file navigation, docs/config,
excluded source, exact-source verification, a source edit after indexing, no
index, a missing MCP executable, and repeated conversation followed by **real
model-backed compaction**. Twelve warm-up turns are a reproducible workload,
not a claim to cover every long-session length. Production MCP startup may sync
the stale fixture; assess actual tool results rather than assuming it stayed
stale. Missing/unavailable retrieval may expose existing guardrail limitations.

Every provider-bound `context_with_system` event records normalized system
message deltas, prompt sections and complete tool declarations without mutation.
The report replays deltas, including tool removals, and checks section presence
in every captured request. It does not assume adapter names. RPC events retain
ordered tool arguments/results, authoritative assistant message ends, command
responses and compaction completion. Streaming deltas are omitted to avoid double
counting. The report never assigns correctness automatically.

## Independent review and measurements

Review each answer and trace against this rubric, preferably with a reviewer who
did not implement the guidance. Record reviewer, run identifier, outcome and exact
event/call references. Failed runs and unverified prompt/tool state stay visible.

| Scenario | Expected evidence / legitimate native fallback |
| --- | --- |
| navigation | `keys.py:normalize_key` called by `users.py:lookup_user`; correct cross-file explanation |
| docs | README marker `amber-lantern`, settings retries `3`; native reads legitimate |
| unindexed | `excluded/recovery.py` contains `violet-anchor`; native fallback required |
| verification | Current return expression `value.strip().lower()`; native source verification legitimate |
| stale | Current expression `value.strip().casefold()`; native verification or successful sync legitimate |
| missing / disconnected | Correct definition/callers despite unavailable retrieval; native fallback legitimate |
| long | Correct navigation before and after recorded compaction; inspect prompt/tool snapshots on both sides |

Separate native exploration bypasses from docs/config, unsupported/unindexed
content, exact verification, failed retrieval recovery and stale index recovery.
Native read counts alone are never evidence for interception.

Use `run.json.session_stats.tokens` for Pi's input/output/cache-read/cache-write
totals and `session_stats.cost` for Pi's estimated pricing, including reported
compaction usage. These are host-reported measurements/estimates, not billing
invoices or TokenSave's estimated tokens saved. Preserve the raw assistant usage
events and stats response for audit. Unknown/missing usage is unknown, not zero.
Compare equivalent model IDs, task correctness and scenarios; report per-pair
differences and all repeats rather than pooling long and short workloads.

`elapsed_s` is wall time for RPC startup, turns, compaction and cleanup after
fixture preparation, not pure model latency. RPC tool events include host tool
duration when supplied. Pair order alternates but provider caching, latency and
model nondeterminism remain confounders. Record Pi/package lock, model/provider,
TokenSave source commit and binary hash, host and review methodology with results.

## Validation and current limits

Offline checks:

```sh
python3 -m unittest discover -s tests/pi-eval -p 'test_*.py' -v
node --test tests/pi-eval/observer_test.mjs
node scripts/pi-tests/run.mjs /path/to/prepared/pi
```

These verify RPC framing, response/settlement order, provider errors, timeout
cleanup, tool-delta replay and non-mutating observation. They are not real-model
results. The published Pi 1.4.0 runtime was installed in a temporary directory
with scripts disabled and its version checked. Real-model runs remain pending
an available provider. Discovery against the isolated pinned source returned
an empty model list; conventional local model endpoints were unavailable.
No credential file was inspected. Adapter direct/proxy/discovery and
independently reviewed real-model correctness are not yet validated. Do not
claim #832 complete or introduce #834 enforcement from these offline tests.

One Pi process group runs at a time. Each command has a deadline, each session
has a 128-assistant-turn bound, RPC data has a 4 MiB record / 256 MiB session
bound, and prompt observations have a 32 MiB bound. Shutdown closes stdin,
terminates/kills the owned process group and reaps Pi. No reader thread, unbounded
queue, detached server or retained credential copy is intended. No long-running
RSS measurement has been performed.
