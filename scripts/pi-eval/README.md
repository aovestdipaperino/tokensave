# Pi read-routing evidence assessment

This standalone read-only tool assesses independently collected traces and a
human review. It does not collect model responses, run Pi, enable interception or
change user configuration. The real-model runner proposed in #838 was closed as
not planned and is not part of this implementation.

See [READ-ROUTING.md](READ-ROUTING.md) for review rules and legitimate native
fallback categories. The current [decision](READ-ROUTING-DECISION.json) is
`defer`: no real-model evidence was available. Synthetic tests do not count as
such evidence.

## Input format

Use Python 3.9+. Each run is a subdirectory of the evidence directory, containing
`run.json`, `events.jsonl` and `contexts.jsonl`. Keep raw provenance, source
versions and artifact hashes with any review. No automatic collector is shipped.

`run.json` records:

- `status`: `completed` only for a successful run.
- `repeat`, `scenario`, `arm`: matched repeat identifier, scenario and
  `baseline`/`enabled` arm. Required scenarios are `navigation`, `docs`,
  `unindexed`, `verification`, `stale`, `missing`, `disconnected`, `long`.
- `provider`, `model`, `pi_version`, `pi_source_commit`, `tokensave_commit`,
  `tokensave_binary_sha256`, `interface`: consistent provenance across runs.
- `observed_model`: `{ "provider": "actual-provider", "id": "actual-model" }`.
- `session_stats.tokens.input`: positive host-reported input usage.
- `compacted`: true for completed long-session compaction.

Each `events.jsonl` line is an object with `wall_time_ms` and an `event` field
containing the original Pi RPC event. Preserve tool execution starts (call ID,
tool name and arguments), execution ends (call ID, error state and result), and
successful compaction responses. Use strict LF framing, not Unicode line/paragraph
separators. Missing usage or results remain unknown, never invented zeros.

Each `contexts.jsonl` line contains `wall_time_ms` and `systemMessages`, the
normalized provider-bound system messages, including named `sections` and
`toolsAdded`/`toolsRemoved` deltas. Include complete state at each observation.
The report replays those deltas without assuming tool namespaces. Observe the
actual prompt/tool state before and after compaction; static MCP configuration
alone does not establish availability.

Copy [review-template.json](review-template.json), fill it truthfully using the
[review protocol](READ-ROUTING.md), then run:

```sh
python3 scripts/pi-eval/report.py /private/path/evidence
python3 scripts/pi-eval/assess_reads.py --evidence /private/path/evidence --review /private/path/review.json
```

Both commands read files only. The assessor reports `defer` or
`eligible-for-design-review`, always with `enforcement_enabled: false`.
An independent reviewer must verify correctness, fallback, index freshness,
range sufficiency and retrieval equivalence. The software cannot authenticate
reviewer identity or prove remote-model provenance from arbitrary files.

## Validation

```sh
python3 -m unittest discover -s tests/pi-eval -p 'test_*.py' -v
```

Ten synthetic assessment tests cover retained behavior. No model call or paid
workload is required. See [VALIDATION.md](VALIDATION.md) for measured checks and
limitations. Trace files may include private source, prompts and tool output;
review artifacts before publishing them. No credential material is needed by
this assessment.
