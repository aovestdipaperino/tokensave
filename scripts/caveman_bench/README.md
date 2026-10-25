# Paired TokenSave/Caveman performance trials

No performance results are published by this change. The Python standard-library
runner measures real agent trials; its unit tests use synthetic telemetry only.
A paid or authenticated agent and a telemetry adapter are prerequisites. This
runner does not install either product, change routing, or invoke a model itself.
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
driver when needed. Disk traces scale with the trial's tool activity and are
released with its temporary directory; normalized output is limited to 16 MiB.

Report per-task results and failure reasons, completion rate, latency distribution,
and cost per success. Do not report only successful trials or equate estimated
payload tokens with total model input. Run voice-only B first with proxy
compression off. Treat catalog shrinking, proxy compression, Cavecrew, and model
routing as separate named experiments: they change additional variables. Lossy
proxy experiments must verify exact TokenSave result payloads at model ingress;
without that evidence, byte-preservation compatibility remains unverified.
