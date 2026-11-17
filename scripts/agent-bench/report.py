"""Aggregates agent-bench results into a decision report (#948).

Wall time is partitioned into startup, model wait, tool wait and tail, so
the parts add up to the wall clock. Summed per-tool waits and TokenSave
internal timings are reported beside it, never added to it: parallel tool
calls overlap, and TokenSave time is a subset of tool wait.
"""

from __future__ import annotations

import json
import random
import statistics
from collections import defaultdict
from pathlib import Path
from typing import Any, Callable, Iterable

MIN_TAIL_SAMPLES = 10
BOOTSTRAP_ROUNDS = 2000
DB_SHARE_ACTIONABLE = 0.10
REDUNDANT_SHARE_ACTIONABLE = 0.20


def load(path: Path, include_warmup: bool = False) -> list[dict[str, Any]]:
    records = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return records
    for line in lines:
        if not line.strip():
            continue
        rec = json.loads(line)
        if rec.get("warmup") and not include_warmup:
            continue
        records.append(rec)
    return records


def _vals(records: Iterable[dict[str, Any]], fn: Callable[[dict[str, Any]], Any]) -> list[float]:
    out = []
    for r in records:
        try:
            v = fn(r)
        except (KeyError, TypeError):
            v = None
        if v is not None:
            out.append(float(v))
    return out


def _median(xs: list[float]) -> float | None:
    return statistics.median(xs) if xs else None


def _p90(xs: list[float]) -> float | None:
    if len(xs) < MIN_TAIL_SAMPLES:
        return None
    s = sorted(xs)
    return s[min(len(s) - 1, int(round(0.9 * (len(s) - 1))))]


def _bootstrap_median_diff(a: list[float], b: list[float], seed: int = 948):
    """95% percentile-bootstrap interval of median(b) - median(a)."""
    if len(a) < 2 or len(b) < 2:
        return None
    rng = random.Random(seed)
    diffs = []
    for _ in range(BOOTSTRAP_ROUNDS):
        ra = [rng.choice(a) for _ in a]
        rb = [rng.choice(b) for _ in b]
        diffs.append(statistics.median(rb) - statistics.median(ra))
    diffs.sort()
    return diffs[int(0.025 * len(diffs))], diffs[int(0.975 * len(diffs)) - 1]


def _input_total(r: dict[str, Any]) -> int | None:
    """Uncached plus cache-read plus cache-write input tokens; unavailable
    when the uncached count itself is missing."""
    met = r["metrics"]
    if met.get("input_tokens") is None:
        return None
    return met["input_tokens"] + (met.get("cache_read_tokens") or 0) + (met.get("cache_creation_tokens") or 0)


def summarize_group(records: list[dict[str, Any]]) -> dict[str, Any]:
    m = lambda key: (lambda r: r["metrics"][key])  # noqa: E731
    p = lambda key: (lambda r: (r.get("tokensave_profile") or {})[key])  # noqa: E731
    wall = _vals(records, m("wall_ms"))
    n = len(records)
    successes = sum(1 for r in records if r.get("success"))
    outcomes: dict[str, int] = defaultdict(int)
    for r in records:
        outcomes[r["outcome"]] += 1
    ts_calls = _vals(records, lambda r: r["metrics"]["tool_calls_by_family"].get("tokensave", 0))
    dupes = _vals(records, m("exact_duplicates"))
    near = _vals(records, m("near_duplicates"))
    reval = _vals(records, m("revalidations"))
    profiled = [r for r in records if r.get("tokensave_profile")]

    def total(xs: list[float]) -> float:
        return sum(xs)

    def share(num: list[float], den: list[float]) -> float | None:
        d = sum(den)
        return (sum(num) / d) if d > 0 and num else None

    out = {
        "trials": n,
        "success_rate": successes / n if n else None,
        "outcomes": dict(outcomes),
        "wall_ms_median": _median(wall),
        "wall_ms_p90": _p90(wall),
        "wall_ms_min": min(wall) if wall else None,
        "wall_ms_max": max(wall) if wall else None,
        "model_calls_median": _median(_vals(records, m("model_calls"))),
        "tool_calls_median": _median(_vals(records, m("tool_calls"))),
        "tokensave_calls_median": _median(ts_calls),
        "input_tokens_median": _median(_vals(records, m("input_tokens"))),
        "input_total_tokens_median": _median(_vals(records, _input_total)),
        "output_tokens_median": _median(_vals(records, m("output_tokens"))),
        "cache_read_tokens_median": _median(_vals(records, m("cache_read_tokens"))),
        "cost_usd_median": _median(_vals(records, m("cost_usd"))),
        "cost_usd_total": total(_vals(records, m("cost_usd"))) if _vals(records, m("cost_usd")) else None,
        "tokensave_response_tokens_est_median": _median(_vals(records, m("tokensave_response_tokens_est"))),
        "share_startup": share(_vals(records, m("startup_ms")), wall),
        "share_model_wait": share(_vals(records, m("model_wait_ms")), wall),
        "share_tool_wait": share(_vals(records, m("tool_wait_ms")), wall),
        "share_tail": share(_vals(records, m("tail_ms")), wall),
        "exact_duplicates_total": total(dupes),
        "near_duplicates_total": total(near),
        "revalidations_total": total(reval),
        "tokensave_calls_total": total(ts_calls),
        "profiled_trials": len(profiled),
    }
    if profiled:
        pwall = _vals(profiled, m("wall_ms"))
        out.update(
            {
                "share_tokensave_internal": share(_vals(profiled, p("mcp_total_ms")), pwall),
                "share_db": share(_vals(profiled, p("db_ms_in_tools")), pwall),
                "share_sync": share(_vals(profiled, p("sync_ms")), pwall),
                "db_share_of_mcp": share(
                    _vals(profiled, p("db_ms_in_tools")), _vals(profiled, p("mcp_total_ms"))
                ),
                "mcp_ms_median": _median(_vals(profiled, p("mcp_total_ms"))),
                "mcp_calls_total": total(_vals(profiled, p("mcp_calls"))),
                "mcp_ms_total": total(_vals(profiled, p("mcp_total_ms"))),
                "handler_ms_total": total(_vals(profiled, p("mcp_handler_ms"))),
                "project_resolution_ms_total": total(_vals(profiled, p("mcp_project_resolution_ms"))),
                "freshness_ms_total": total(_vals(profiled, p("mcp_freshness_ms"))),
                "db_ms_total": total(_vals(profiled, p("db_ms_in_tools"))),
                "sync_ms_total": total(_vals(profiled, p("sync_ms"))),
                "db_ms_median": _median(_vals(profiled, p("db_ms_in_tools"))),
                "db_ops_median": _median(_vals(profiled, p("db_ops_in_tools"))),
                "repeated_db_executions_total": total(_vals(profiled, p("repeated_db_executions"))),
            }
        )
    return out


def verdict(arm: dict[str, Any], baseline: dict[str, Any] | None) -> str:
    """Rule-based answer to "what is the dominant actionable bottleneck".
    The thresholds are documented in README.md; they are heuristics."""
    if baseline and arm.get("success_rate") is not None and baseline.get("success_rate") is not None:
        if arm["success_rate"] < baseline["success_rate"]:
            return "response/context quality (lower success than the reference arm)"
    db = arm.get("share_db")
    if db is not None and db >= DB_SHARE_ACTIONABLE:
        return f"database latency ({db:.0%} of wall time)"
    calls = arm.get("tokensave_calls_total") or 0
    redundant = (arm.get("exact_duplicates_total") or 0) + (arm.get("near_duplicates_total") or 0)
    if calls and redundant / calls >= REDUNDANT_SHARE_ACTIONABLE:
        return f"number of calls ({redundant:.0f} of {calls:.0f} TokenSave calls repeated)"
    model = arm.get("share_model_wait")
    if model is not None and model >= 0.5:
        internal = arm.get("share_tokensave_internal")
        tail = f"; TokenSave internals {internal:.0%}" if internal is not None else ""
        return f"agent/model overhead ({model:.0%} of wall time{tail})"
    return "no single dominant bottleneck"


def aggregate(records: list[dict[str, Any]]) -> dict[str, Any]:
    by_task_arm: dict[tuple[str, str], list] = defaultdict(list)
    by_arm: dict[str, list] = defaultdict(list)
    for r in records:
        by_task_arm[(r["task"], r["arm"])].append(r)
        by_arm[r["arm"]].append(r)
    groups = {f"{t}/{a}": summarize_group(rs) for (t, a), rs in sorted(by_task_arm.items())}
    arms = {a: summarize_group(rs) for a, rs in sorted(by_arm.items())}
    # Each arm is compared with its reference arm (records older than the
    # field compare with the baseline).
    refs: dict[str, str] = {}
    for r in records:
        refs.setdefault(r["arm"], r.get("reference_arm") or "baseline")
    comparisons = {}
    for (task, arm), rs in sorted(by_task_arm.items()):
        ref = refs[arm]
        base = by_task_arm.get((task, ref))
        if arm in ("baseline", ref) or not base:
            continue
        a = _vals(base, lambda r: r["metrics"]["wall_ms"])
        b = _vals(rs, lambda r: r["metrics"]["wall_ms"])
        a_cost = _vals(base, lambda r: r["metrics"].get("cost_usd"))
        b_cost = _vals(rs, lambda r: r["metrics"].get("cost_usd"))
        a_out = _vals(base, lambda r: r["metrics"].get("output_tokens"))
        b_out = _vals(rs, lambda r: r["metrics"].get("output_tokens"))
        comparisons[f"{task}/{arm}"] = {
            "reference": ref,
            "wall_ms_median_diff": (_median(b) - _median(a)) if a and b else None,
            "wall_ms_median_diff_ci95": _bootstrap_median_diff(a, b),
            "cost_usd_median_diff": (_median(b_cost) - _median(a_cost)) if a_cost and b_cost else None,
            "output_tokens_median_diff": (_median(b_out) - _median(a_out)) if a_out and b_out else None,
            "success_rate_diff": groups[f"{task}/{arm}"]["success_rate"]
            - groups[f"{task}/{ref}"]["success_rate"],
            "verdict": verdict(groups[f"{task}/{arm}"], groups[f"{task}/{ref}"]),
        }
    envs = {json.dumps(r.get("environment"), sort_keys=True) for r in records}
    models = sorted({str(r["metrics"].get("model") or r.get("requested_model")) for r in records})
    return {
        "trials": len(records),
        "runs": sorted({r["run_id"] for r in records}),
        "models": models,
        "environments": [json.loads(e) for e in envs],
        "arms": arms,
        "groups": groups,
        "comparisons": comparisons,
        "references": {a: refs[a] for a in arms if a != "baseline"},
        "arm_verdicts": {a: verdict(s, arms.get(refs[a])) for a, s in arms.items() if a != "baseline"},
    }


def _fmt(v: Any, kind: str = "") -> str:
    if v is None:
        return "n/a"
    if kind == "s":
        return f"{v / 1000:.1f}"
    if kind == "%":
        return f"{v:.0%}"
    if kind == "$":
        return f"{v:.3f}"
    if isinstance(v, float):
        return f"{v:.0f}" if abs(v) >= 10 else f"{v:.1f}"
    return str(v)


def render_markdown(records: list[dict[str, Any]]) -> str:
    agg = aggregate(records)
    lines = ["# Agent workflow benchmark report", ""]
    lines.append(f"Trials: {agg['trials']} (runs: {', '.join(agg['runs']) or 'none'}); models: {', '.join(agg['models'])}.")
    if len(agg["environments"]) > 1:
        lines.append("")
        lines.append("**Warning:** results come from more than one environment (agent, TokenSave or machine version); compare with care.")
    lines += ["", "Times in seconds. `n/a` = unavailable, never zero. P90 needs at least "
              f"{MIN_TAIL_SAMPLES} trials. Wall-time shares are an exact partition; TokenSave and DB shares are subsets of tool wait.", ""]

    lines += ["## By arm", ""]
    lines.append("| Arm | n | Success | Wall p50 | Wall p90 | Model calls | Tool calls | TS calls | In tok (incl. cache) | Out tok | Cost p50 $ | Startup | Model wait | Tool wait | TS internal | DB | Exact dup | Near dup | Reval |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for arm, s in agg["arms"].items():
        lines.append(
            f"| {arm} | {s['trials']} | {_fmt(s['success_rate'], '%')} | {_fmt(s['wall_ms_median'], 's')} | "
            f"{_fmt(s['wall_ms_p90'], 's')} | {_fmt(s['model_calls_median'])} | {_fmt(s['tool_calls_median'])} | "
            f"{_fmt(s['tokensave_calls_median'])} | {_fmt(s['input_total_tokens_median'])} | {_fmt(s['output_tokens_median'])} | "
            f"{_fmt(s['cost_usd_median'], '$')} | {_fmt(s['share_startup'], '%')} | {_fmt(s['share_model_wait'], '%')} | {_fmt(s['share_tool_wait'], '%')} | "
            f"{_fmt(s.get('share_tokensave_internal'), '%')} | {_fmt(s.get('share_db'), '%')} | "
            f"{_fmt(s['exact_duplicates_total'])} | {_fmt(s['near_duplicates_total'])} | {_fmt(s['revalidations_total'])} |"
        )

    internals = [(a, s) for a, s in agg["arms"].items() if s.get("profiled_trials")]
    if internals:
        lines += ["", "## TokenSave internals (#754 profiler)", ""]
        lines.append("Totals over profiled trials, in ms. Project resolution, freshness and handler are phases of MCP time; DB time is inside the handler.")
        lines.append("")
        lines.append("| Arm | Profiled trials | MCP calls | MCP ms | ms/call | Project resolution | Freshness | Handler | DB | Catch-up sync |")
        lines.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
        for arm, s in internals:
            calls = s["mcp_calls_total"]
            per = s["mcp_ms_total"] / calls if calls else None
            lines.append(
                f"| {arm} | {s['profiled_trials']} | {_fmt(calls)} | {_fmt(s['mcp_ms_total'])} | {_fmt(per)} | "
                f"{_fmt(s['project_resolution_ms_total'])} | {_fmt(s['freshness_ms_total'])} | "
                f"{_fmt(s['handler_ms_total'])} | {_fmt(s['db_ms_total'])} | {_fmt(s['sync_ms_total'])} |"
            )

    lines += ["", "## By task and arm", ""]
    lines.append("| Task | Arm | n | Success | Outcomes | Wall p50 | Wall min–max | Model calls | Tool calls | TS calls | In tok (incl. cache) | Cost p50 $ | TS resp tok | MCP ms p50 | DB ms p50 | DB ops p50 |")
    lines.append("|---|---|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for key, s in agg["groups"].items():
        task, arm = key.split("/", 1)
        outcomes = ", ".join(f"{k} {v}" for k, v in sorted(s["outcomes"].items()))
        lines.append(
            f"| {task} | {arm} | {s['trials']} | {_fmt(s['success_rate'], '%')} | {outcomes} | "
            f"{_fmt(s['wall_ms_median'], 's')} | {_fmt(s['wall_ms_min'], 's')}–{_fmt(s['wall_ms_max'], 's')} | "
            f"{_fmt(s['model_calls_median'])} | {_fmt(s['tool_calls_median'])} | {_fmt(s['tokensave_calls_median'])} | "
            f"{_fmt(s['input_total_tokens_median'])} | {_fmt(s['cost_usd_median'], '$')} | "
            f"{_fmt(s['tokensave_response_tokens_est_median'])} | {_fmt(s.get('mcp_ms_median'))} | "
            f"{_fmt(s.get('db_ms_median'))} | {_fmt(s.get('db_ops_median'))} |"
        )

    if agg["comparisons"]:
        lines += ["", "## Against reference arm", ""]
        lines.append("Each arm is compared with its reference arm (`reference` in arms.json; default `baseline`).")
        lines.append("")
        lines.append("| Task / arm | Reference | Δ wall p50 (s) | 95% CI (bootstrap) | Δ cost p50 $ | Δ out tok p50 | Δ success | Dominant actionable bottleneck |")
        lines.append("|---|---|---:|---|---:|---:|---:|---|")
        for key, c in agg["comparisons"].items():
            ci = c["wall_ms_median_diff_ci95"]
            ci_text = f"{ci[0] / 1000:+.1f} … {ci[1] / 1000:+.1f}" if ci else "n/a (n<2)"
            diff = c["wall_ms_median_diff"]
            cost = c["cost_usd_median_diff"]
            out = c["output_tokens_median_diff"]
            lines.append(
                f"| {key} | {c['reference']} | {f'{diff / 1000:+.1f}' if diff is not None else 'n/a'} | {ci_text} | "
                f"{f'{cost:+.3f}' if cost is not None else 'n/a'} | {f'{out:+.0f}' if out is not None else 'n/a'} | "
                f"{c['success_rate_diff']:+.0%} | {c['verdict']} |"
            )
    if agg["arm_verdicts"]:
        lines += ["", "## Verdict by arm", ""]
        for arm, v in agg["arm_verdicts"].items():
            lines.append(f"- **{arm}** (vs {agg['references'][arm]}): {v}")
    lines += ["", "## Environment", "", "```json", json.dumps(agg["environments"], indent=2), "```", ""]
    return "\n".join(lines)
