# Paired TokenSave/Caveman performance trials

No performance results are published by this change. The Python standard-library
runner measures real agent trials; its unit tests use synthetic telemetry only.
Paid trials require a metered provider. The optional direct Anthropic benchmark
adapter below is implemented; external stock-agent adapters remain operator-owned.
The runner itself does not install either product, change routing, or invoke a model.
It currently requires POSIX for process-group cleanup.

1. Copy `example.json` outside the checkout. Supply a disposable task repository,
   a full commit SHA, concrete tasks, and an independent verification command for
   each task. Verification must check correctness, not just whether the agent
   exited successfully. Include code understanding, caller/impact analysis, and
   edits with regression tests. Keep verification scripts outside editable task
   files or check their hashes so agents cannot weaken the success criterion.
2. Supply absolute driver command arrays. Each driver reads one JSON request on
   stdin: `arm`, `prompt`, `repository`, `home`, and `metadata`. It runs your coding
   agent in the supplied repository. Configure TokenSave and its index before the
   task in BOTH arms. For A, Caveman must be absent. For B, independently install
   and activate the pinned Caveman version in the temporary home. Check actual
   session activation; do not use the existence of a skill as proof. Never use
   your ordinary agent home, routing config, or proxy listener for this benchmark.
3. Pin the same agent, TokenSave binary, model, provider, model settings, pricing,
   toolset, prompt, and initial index in both arms. Record them in `metadata`.
   Scope `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_CONFIG_HOME`, agent credentials,
   caches, and any other agent-specific override to the supplied temporary home.
   Drivers must manage credentials without printing them. Keep native traces in
   private storage and record their hashes in the normalized telemetry. Drivers
   must bound logs and shut down owned MCP servers/proxies before returning.
4. Return ONLY this JSON document on stdout, containing **all** actual provider
   requests and observed tools. Missing usage/cost or incomplete traces invalidate
   a run; never fill gaps with token estimates or zeroes:

   ```json
   {"complete": true, "trace_sha256": "HASH_OF_PRIVATE_NATIVE_TRACE", "events": [
     {"type": "usage", "input_tokens": 100, "output_tokens": 20, "cost_usd": 0.001},
     {"type": "mcp_call", "name": "tokensave_search"},
     {"type": "file_read", "path": "/repo/src/lib.rs", "range": "1:20", "sha256": "CONTENT_HASH"}
   ]}
   ```

   Values above illustrate the schema, not a measurement. Emit one usage event
   per billed request, including retries and delegated model requests. Input
   tokens must include cache-read/write tokens exactly once according to the
   provider's usage definition. Use billed cost, or actual usage multiplied by a
   pinned applicable pricing snapshot, recording that method in metadata.
   Subscription cost without attributable task billing is unknown: this runner's
   cost comparison requires a metered provider. MCP calls include all servers;
   retain names for auditing. Emit file-read events for native reads AND
   TokenSave reads, with canonical paths, exact ranges, and content digests.
   A redundant read means a repeated identical path/range/content within a task;
   it does not infer whether that repetition was semantically unnecessary.
5. Run at least three paired repeats (more for useful statistical power):

   ```bash
   python3 scripts/caveman_bench/run.py /path/to/manifest.json /path/to/new-results
   python3 -m unittest discover -s scripts/caveman_bench -p 'test_*.py'
   ```

Each trial starts from a fresh local clone at the pinned commit and a fresh home.
Order alternates A/B then B/A. `trials.jsonl` records token totals, MCP calls,
redundant reads, independently verified completion, wall latency, and cost.
`summary.json` aggregates each arm and divides **all** trial costs, including
failed tasks, by successful tasks. Zero successes produces null, not free work.
Normalized events and the hashed manifest accompany results. Driver/setup time
is included in latency: use identical setup and record it separately if reporting
agent-only latency. Timeout or missing telemetry aborts the experiment and leaves
partial records; do not publish these as a complete comparison. A driver which
returns full telemetry after an unsuccessful task can still be independently
scored as a failure. Git submodules/LFS assets must be prepared identically by the
driver when needed. Disk traces scale with the trial's tool activity. The built-in adapter retains
them in the results directory for audit; external drivers own their retention
policy. Normalized output is limited to 16 MiB.

Report per-task results and failure reasons, completion rate, latency distribution,
and cost per success. Do not report only successful trials or equate estimated
payload tokens with total model input. Run voice-only B first with proxy
compression off. Treat catalog shrinking, proxy compression, Cavecrew, and model
routing as separate named experiments: they change additional variables. Lossy
proxy experiments must verify exact TokenSave result payloads at model ingress;
without that evidence, byte-preservation compatibility remains unverified.

## Concrete task suite

Prepare an invoice-code repository and manifest outside this checkout:

```bash
python3 -B scripts/caveman_bench/tasks.py prepare /tmp/caveman-invoice-suite
```

The output directory must not exist. The command creates a local Git repository,
pins its full commit, and supplies three tasks: explain the documented invoice
behavior, enumerate direct/transitive callers, and fix aggregate discount
rounding. Answers use exact JSON contracts. The edit verifier checks 251 inputs
through five public call paths, including empty input and discount boundaries;
the original implementation fails the fractional-cent cases.

Scoring runs outside the editable task repository. Source hashes prevent
unrelated changes; unexpected task files and symlinked answers/source fail
verification. The runner verifies `verification_artifacts` SHA-256 hashes before
trials and before/after each scoring command. These include the scorer and its
baseline sources. This catches accidental scoring changes; it is not a security
sandbox against an agent with arbitrary host filesystem access. Keep verifier
artifacts read-only for real trials. Model output and source are not published
by the verifier. Child execution has a ten-second timeout, bounded captured
output, and process-group cleanup using the same POSIX helper as the runner.

Fill in both drivers and the model/provider/pricing/TokenSave metadata in the
generated manifest before running `run.py`. The suite pins candidate code and
scoring artifacts; it does not select a model or invent a pricing snapshot.
The tiny fixture tests correctness and harness behavior, not large-repository
retrieval performance. Each task receives a fresh checkout. Drivers must keep
native traces outside it and must initialize TokenSave identically in both arms.


## Direct Anthropic benchmark adapter

`anthropic_driver.py` is a small benchmark agent with a sequential tool loop,
not a stock Claude Code/Codex session. It requires Python 3.11+ and POSIX.
It calls Anthropic's direct Messages API with an explicitly chosen model,
standard service tier, no prompt caching, no retries, and no inherited proxy.
It exposes seven TokenSave tools plus native `read_file` and `write_file`.
`tokensave_context` source snippets, cross-project selection, shell commands,
other MCP servers, delegation, shrinking, and proxy compression are unsupported.
Use `tokensave_read` for source. Tool descriptions and input schemas come from
the installed server; TokenSave's production schemas/results are unchanged.

Both arms initialize the same disposable graph. Arm A has no Caveman skill.
Arm B copies the independently supplied skill from a pinned Caveman Git commit
into the disposable home and explicitly includes it in the benchmark system
prompt. Its activation record says `explicit_benchmark_system_prompt` and records
the skill hash. This proves benchmark prompt assembly, not activation in Claude
Code or another stock client. No user installation/configuration is changed.

Prepare the concrete suite above, and create a pricing JSON file outside the
checkout with these fields (replace every placeholder using the applicable
provider price sheet; no prices or model are selected automatically):

```json
{
  "model": "EXACT_MODEL_SNAPSHOT",
  "source": "OFFICIAL_PRICE_SHEET_URL",
  "retrieved_at": "YYYY-MM-DD",
  "input_tokens_usd_per_million": 0,
  "output_tokens_usd_per_million": 0
}
```

Zero prices are placeholders and are rejected. Use standard, global inference
prices applicable to the selected model and context length. The adapter rejects
nonzero cached usage, server-tool charges, regional/nonstandard-tier usage,
contexts above 200,000 input tokens, missing usage, and a returned model that
differs from the pricing snapshot. Cost is actual provider usage times pinned
rates, not invoice reconciliation or subscription billing. Pricing reference:
[Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing).
Usage and tier behavior follow the provider's
[prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching),
[service tiers](https://platform.claude.com/docs/en/api/service-tiers), and
[tool definitions](https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools)
documentation. Revalidate these interfaces before expanding support.

Pin the prepared suite to your local candidates without invoking a model:

```bash
python3 -B scripts/caveman_bench/configure_anthropic.py \
  /tmp/caveman-invoice-suite/manifest.json /tmp/caveman-invoice-suite/metered.json \
  --tokensave /absolute/path/to/tokensave \
  --tokensave-commit FULL_TOKENSAVE_COMMIT_SHA \
  --caveman-source /absolute/path/to/independent/caveman-checkout \
  --caveman-commit FULL_CAVEMAN_COMMIT_SHA \
  --pricing /absolute/path/to/pricing.json
```

The builder pins the binary hash, full candidate commits, model settings,
toolset, skill hash, external scorer/price artifacts, and driver bundle hash
(`anthropic_driver.py`, `transport.py`, `run.py`, in that order, each filename
followed by a NUL and its bytes). It does not establish build provenance: build
the binary from the specified commit and retain that evidence. Reconfigure after
changing pinned sources. The output manifest must not already exist.

For a separately authorized paid run, configure `ANTHROPIC_API_KEY` privately
in the launching environment and run the ordinary runner with `metered.json`.
The adapter never prints credentials or records HTTP authorization headers;
it withholds the model API key from TokenSave subprocesses. Missing credentials
fail the run. No paid requests are part of the offline test suite.

Each driver request includes an absolute `trace_directory`. Native JSONL traces
are retained under `RESULTS/private-traces/NNNN/agent.jsonl`, with directory mode
0700 and file mode 0600. They contain source, model prompts/responses and tool
results: keep them private. The runner verifies the returned file digest before
accepting metrics. A mismatch, timeout, incomplete provider response or unsupported
read format invalidates the experiment; partial traces remain for diagnosis.
Failed independently scored tasks retain their complete usage and contribute
to cost per success. The traces include the actual outgoing tool-result content;
this adapter has no intervening compressor. This is not a proxy preservation test.

File-read events describe source delivered to the model, not every backend OS
read during indexing, automatic sync, or database access. Native and TokenSave
full/line reads share canonical paths, inclusive ranges, and SHA-256 digests of
delivered source with line-number prefixes removed. Their renderer normalizes
line endings and omits the final newline. Map/signature results and unchanged
digest stubs are graph data, not delivered source reads. Truncated, ambiguous or
unrecognized source payloads fail closed. Source tool results themselves are
forwarded intact; auditing never rewrites them.

Bounds: 32 model turns, 64 content blocks per response, 1 MiB per MCP message,
128 consecutive MCP notifications, a 7 MiB accumulated context budget, 8 MiB
HTTP request, 1 MiB HTTP response, 64 MiB private trace per trial, and 30-second
MCP/60-second HTTP operation timeouts. Native file reads/writes are capped at
1 MiB. Trials are sequential. MCP stdin is closed on exit, followed by bounded
termination/kill and reaping if necessary. The runner also kills/reaps the driver
process group on completion, cancellation or timeout; subprocess extraction is
disabled for the benchmark server. No persistent cache or background worker is
added. Fixed-workload long-running CPU/RSS stabilization remains unmeasured.
