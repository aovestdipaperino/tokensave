# Standalone assessment validation — 2026-10-27

Refs #834 / PR #839. The proposed runner/tests in #832 / #838 were closed as not
planned without merging and removed from this PR before targeting master.
The existing offline Pi lifecycle harness is unchanged.

| Check | Result |
| --- | --- |
| Assessment regressions | Ten synthetic tests: fallback categories, direct/proxy names, uncertain coverage/freshness/range, failed/late/empty retrieval, model/compaction provenance, incomplete comparisons, duplicate calls and independent review |
| Empty-evidence decision | `READ-ROUTING-DECISION.json`: defer, no candidates, enforcement disabled |
| Runtime interception | No production extension or tool interception changed |
| Real-model evidence | None; previously inspected isolated Pi profiles discovered no available model; credential files were not inspected |
| Resource scope | Read-only, bounded trace readers, at most 160 runs and 4 MiB review input; temporary state for one annotated run, no subprocess/background work |
| Limits | No actual model-routing, adapter, paid-workload or long-running RSS/CPU measurements; synthetic results are assessor tests only |

Commands for the final scope:

```sh
python3 -m unittest discover -s tests/pi-eval -p 'test_*.py' -v
python3 scripts/pi-eval/assess_reads.py --help
git diff --check
```

Rust and SQL are unchanged by the assessment. No Cargo checks were repeated for
these standalone scripts. Prior diagnostics validation and workspace/environment
failures are recorded in docs/PI-INTEGRATION.md. Upstream master AGENTS.md was
compared before merge preparation; it gains only the retained assessment command.
Beta command guidance must follow when this source is synchronized there.
