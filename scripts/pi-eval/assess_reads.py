#!/usr/bin/env python3
"""Refs #834: assess reviewed evidence; never intercept or disable a Pi tool."""
import argparse
import json
from pathlib import Path

from report import records, summarize

SCENARIOS = {"navigation", "docs", "unindexed", "verification", "stale", "missing", "disconnected", "long"}
FALLBACKS = {"documentation", "configuration", "logs", "unindexed", "unsupported",
             "stale-index", "missing-index", "incomplete-index", "unavailable-mcp",
             "retrieval-failure", "verification", "unrepresentable"}
ASSERTIONS = ("source_indexed", "index_fresh", "range_sufficient", "alternative_equivalent",
              "tokensave_interface_verified")


def assess(evidence, review):
    """A conservative design-review gate, not an enforcement authorization."""
    reasons = []
    if review.get("schema") != 1 or not review.get("reviewer") or review.get("independent") is not True:
        reasons.append("Independent named review is required")
    if review.get("real_model_evaluation") is not True:
        reasons.append("Reviewer must attest that the corpus contains real-model evaluation, not faux-provider tests")
    runs = {}
    for directory in sorted(evidence.iterdir()):
        if directory.is_dir() and (directory / "run.json").exists():
            if len(runs) >= 160:
                raise ValueError("Evidence corpus exceeds 160 sessions")
            summary = summarize(directory)
            runs[directory.name] = {"metadata": summary["metadata"],
                                    "guidance_verified": summary["guidance_verified"]}
    judgments = {entry["run"]: entry for entry in review.get("runs", [])}
    identities = set()
    coverage = set()
    for name, summary in runs.items():
        run = summary["metadata"]
        identities.add((run.get("provider"), run.get("model"), run.get("pi_version"),
                        run.get("pi_source_commit"), run.get("tokensave_commit"),
                        run.get("tokensave_binary_sha256"), run.get("interface")))
        judgment = judgments.get(name, {})
        if run.get("status") != "completed" or not summary["guidance_verified"]:
            reasons.append(f"{name}: incomplete run or unverified prompt state")
        if judgment.get("correct") is not True or judgment.get("fallback_ok") is not True:
            reasons.append(f"{name}: correctness and native fallback need independent review")
        stats = run.get("session_stats", {})
        if not isinstance(stats.get("tokens", {}).get("input"), (float, int)) or stats["tokens"]["input"] <= 0:
            reasons.append(f"{name}: no positive host-reported model input usage")
        if run.get("scenario") == "long" and run.get("compacted") is not True:
            reasons.append(f"{name}: compaction not completed")
        coverage.add((run.get("repeat"), run.get("scenario"), run.get("arm")))
    if len(identities) != 1:
        reasons.append("Comparable runs must pin the same provider/model/source/binary/interface")
    elif any(value is None or value == "" for value in next(iter(identities))):
        reasons.append("Source, model, binary and interface provenance is incomplete")
    repeats = {repeat for repeat, _scenario, _arm in coverage}
    if len(repeats) < 2 or any((repeat, scenario, arm) not in coverage
                             for repeat in repeats for scenario in SCENARIOS for arm in ["baseline", "enabled"]):
        reasons.append("At least two complete matched repeats across all eight scenarios are required")

    candidates = []
    excluded = []
    seen = set()
    current_run = None
    current_contexts = []
    first_error_time = float("inf")
    starts = {}
    ends = {}
    for entry in sorted(review.get("reads", []), key=lambda row: row.get("run", "")):
        key = (entry.get("run"), entry.get("call_id"))
        if key in seen:
            continue
        seen.add(key)
        why = []
        summary = runs.get(entry.get("run"))
        if summary is None:
            excluded.append({"run": key[0], "call_id": key[1], "reasons": ["Unknown run"]})
            continue
        run = summary["metadata"]
        if current_run != entry["run"]:
            current_run = entry["run"]
            current_contexts = summarize(evidence / current_run)["contexts"]
            starts = {}
            ends = {}
            first_error_time = float("inf")
            for row in records(evidence / current_run / "events.jsonl"):
                event = row["event"]
                if event.get("type") == "tool_execution_start":
                    starts[event.get("toolCallId")] = row
                elif event.get("type") == "tool_execution_end":
                    ends[event.get("toolCallId")] = (event.get("isError"), row.get("wall_time_ms"),
                                                     bool(event.get("result", {}).get("content")))
                if event.get("isError") is True and isinstance(row.get("wall_time_ms"), (int, float)):
                    first_error_time = min(first_error_time, row["wall_time_ms"])
        native = starts.get(entry.get("call_id"), {})
        alternative = starts.get(entry.get("alternative_call_id"), {})
        native_event = native.get("event", {})
        alternative_event = alternative.get("event", {})
        if native_event.get("toolName") != "read":
            why.append("No observed native read with this call ID")
        if run.get("arm") != "enabled" or run.get("scenario") not in {"navigation", "long"}:
            why.append("Only enabled code-exploration scenarios can justify additional routing")
        classification = entry.get("classification")
        if classification in FALLBACKS:
            why.append(f"Preserve legitimate native fallback: {classification}")
        elif classification != "unnecessary-exploration":
            why.append("Unclassified reads are never enforcement evidence")
        if not entry.get("reason") or any(entry.get(field) is not True for field in ASSERTIONS):
            why.append("Indexed/fresh/representable/equivalent alternative needs explicit evidence and rationale")
        if entry.get("verification") is not False or entry.get("recovery") is not False:
            why.append("Verification or recovery cannot be ruled out")
        if entry.get("path") != native_event.get("args", {}).get("path") or not entry.get("path"):
            why.append("Review does not identify the actual native-read path")
        time = native.get("wall_time_ms")
        contexts = [context for context in current_contexts
                    if isinstance(context.get("wall_time_ms"), (int, float))
                    and isinstance(time, (int, float)) and context["wall_time_ms"] <= time]
        declared = {tool["name"] for tool in contexts[-1]["tools"]} if contexts else set()
        tool = alternative_event.get("toolName")
        if not tool or tool == "read" or tool not in declared or tool != entry.get("alternative_tool"):
            why.append("Alternative must use an actually declared tool/interface; naming is not inferred")
        for identifier in [entry.get("call_id"), entry.get("alternative_call_id")]:
            result = ends.get(identifier, (None, None, False))
            if result[0] is not False or not result[2]:
                why.append("Native read and alternative both need observed successful results")
        alternative_time = ends.get(entry.get("alternative_call_id"), (None, None))[1]
        if not isinstance(alternative_time, (int, float)) or not isinstance(time, (int, float)) or alternative_time > time:
            why.append("Working retrieval must be observed before the native read")
        if isinstance(time, (int, float)) and first_error_time <= time:
            why.append("Earlier tool failure requires native recovery to remain available")
        row = {"run": key[0], "call_id": key[1], "path": entry.get("path"),
               "alternative_tool": tool, "repeat": run.get("repeat")}
        if why:
            excluded.append({**row, "reasons": why})
        else:
            candidates.append(row)
    if len({candidate["repeat"] for candidate in candidates}) < 2:
        reasons.append("Repeated independently reviewed unnecessary reads across matched repeats are absent")
    return {"schema": 1, "issue": 834,
            "decision": "defer" if reasons else "eligible-for-design-review",
            "reasons": reasons, "candidates": candidates, "excluded": excluded,
            "enforcement_enabled": False,
            "remaining": "Even eligible evidence requires a scoped implementation and real-session correctness/tool-impact comparison; this report never enables interception."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--review", type=Path, required=True)
    args = parser.parse_args()
    if args.review.stat().st_size > 4 * 1024 * 1024:
        parser.error("Review exceeds 4 MiB")
    print(json.dumps(assess(args.evidence, json.loads(args.review.read_text())), indent=2))


if __name__ == "__main__":
    main()
