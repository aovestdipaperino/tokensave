# Conditional native-read routing assessment (Refs #834)

**Current decision: defer enforcement.** The pinned real-model evaluation runner
exists in [PR #838](https://github.com/aovestdipaperino/tokensave/pull/838), but no
working model was discoverable in this environment. There are no real-model
traces demonstrating repeated unnecessary indexed-source reads. Pi's native
`read` remains untouched. This assessment does not enable interception, alter
tool activation, infer an adapter namespace or modify Pi core.
The [recorded decision](READ-ROUTING-DECISION.json) comes from empty evidence
and the unfilled review template; it explicitly records the missing prerequisites.

The issue explicitly requires real-model evidence before a blocking handler is
implemented. A code change that blocks reads now would bypass that prerequisite.
This PR supplies reproducible assessment and deterministic fallback checks while
leaving the enforcement decision pending. It does not claim #834 complete.

## Collect and independently review evidence

First run [the matched evaluation](README.md). Have an independent reviewer
assess task correctness and legitimate native fallback from every run's exact
answers, tool arguments/results and provider-bound prompt/tool state. Retain the
original artifacts and record their hashes with any published results. Missing
results or provider errors are not successful outcomes.

Copy `review-template.json` outside the repository. Fill in `reviewer`,
`independent` and `real_model_evaluation` truthfully; the last field is a reviewer
attestation, not proof the software can derive from a JSON file. Synthetic
fixtures only test this evaluator; they are never production evidence.

`runs` must contain one judgment for each run identifier:

```json
{"run":"0-navigation-enabled","correct":true,"fallback_ok":true}
```

Annotate native read call IDs individually in `reads`. A candidate exploration
bypass needs this complete record, including the **actual** declared tool name
and a previously successful matching retrieval call in the same run:

```json
{
  "run": "0-navigation-enabled",
  "call_id": "ACTUAL_NATIVE_CALL_ID",
  "path": "ACTUAL_PATH_FROM_READ_ARGUMENTS",
  "alternative_call_id": "ACTUAL_SUCCESSFUL_RETRIEVAL_CALL_ID",
  "alternative_tool": "ACTUAL_DECLARED_TOOL_OR_PROXY_NAME",
  "classification": "unnecessary-exploration",
  "reason": "Explain why the observed indexed result already satisfied this requested source range; cite exact event content and freshness evidence.",
  "source_indexed": true,
  "index_fresh": true,
  "range_sufficient": true,
  "alternative_equivalent": true,
  "tokensave_interface_verified": true,
  "verification": false,
  "recovery": false
}
```

Coverage, freshness, representability and content equivalence require independent
source/result inspection. Boolean fields are attestations, not automatic static
analysis. Do not assert them merely because the file has a source extension or
because a tool appears in MCP configuration. For proxies, inspect the actual
proxy arguments/discovery and underlying TokenSave operation as part of
`tokensave_interface_verified`; the evaluator does not fabricate proxy names.

For legitimate native reads, use a fallback classification: `documentation`,
`configuration`, `logs`, `unindexed`, `unsupported`, `stale-index`, `missing-index`,
`incomplete-index`, `unavailable-mcp`, `retrieval-failure`, `verification` or
`unrepresentable`. Unclassified/uncertain reads are excluded from enforcement
evidence. Retain those annotations rather than deleting inconvenient cases.

```sh
python3 scripts/pi-eval/assess_reads.py \
  --evidence /private/path/pi-routing-results \
  --review /private/path/review.json > /private/path/read-routing-decision.json
```

The assessor checks complete paired repeats across all eight scenarios,
consistent host/model/source/binary/interface metadata, positive host-reported
usage, exact observed model identity, prompt section observations before/after
completed compaction and independent
correctness/fallback judgments. A read candidate must be from an enabled
navigation/long-session run, have a successful native result and an earlier
successful alternative, and identify a tool declared before that read. Earlier
tool failures preserve native recovery. Duplicate annotations of one call do
not count as repeated evidence; qualifying reads must span at least two repeats.

The result is `defer` or `eligible-for-design-review`, always with
`enforcement_enabled: false`. It is conservative: later retrieval success does
not prove the interface was usable at the earlier read. The program cannot
authenticate reviewer identity, establish remote-model provenance from arbitrary
files, or independently infer semantic intent. Keep raw provenance and review
attestations auditable.

## Required implementation if real evidence later warrants it

An eligible report starts design review, not automatic blocking. Any subsequent
handler must establish all of these at the time of each call:

| Condition | Required fallback |
| --- | --- |
| Appropriate active index, indexed source, current enough data and representable range | Otherwise allow native read |
| Actually declared and reachable direct/proxy retrieval interface | Otherwise allow; no invented tool names |
| Exploration intent established | Exact-source verification before edits always allowed |
| No pending retrieval failure or unavailable result | Recovery allowed; no retry/block loops |
| Docs/config/logs, unsupported/unindexed content or incomplete index | Native read always allowed |

The present Pi `read` payload does not reliably encode verification intent.
Design must resolve that uncertainty conservatively rather than guess from a
file extension. After implementation, run deterministic handler tests and
matched real sessions against guidance-only behavior, reporting correctness,
tool counts, tokens, cost estimates and latency. Only then assess impact.

## Validation and resources

Run `python3 -m unittest discover -s tests/pi-eval -p 'test_*.py' -v`.
Synthetic fixtures cover every fallback category, unknown index/range/interface,
invented names, unavailable/failed retrieval, missing comparison/compaction/usage,
insufficient review, duplicated calls and the positive design-review transition.
No test enables enforcement or counts as real-model evidence.

The assessor is read-only, uses the existing bounded trace reader, caps the
corpus at 160 sessions and the review at 4 MiB, and processes read annotations
grouped by run. It retains tool-call metadata and prompt state for one annotated
run at a time, not tool result bodies. It starts no subprocess, polling or
background work. Memory scales with bounded input/call metadata; no RSS/CPU
benchmark or long-session real-model evaluation was performed.
