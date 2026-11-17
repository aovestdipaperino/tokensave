"""TokenSave-internal timings from #754's profiler, per trial.

The harness does not instrument TokenSave itself. It starts the MCP server
with `TOKENSAVE_PROFILE=light` and reads the aggregate files the profiler
writes: one file per process and hour, named `<hour>-<pid>-<nonce>.json`.
Files that did not exist before a trial belong to processes started during
it, which is the agent's MCP server unless another TokenSave process was
started at the same time (see README, "Limitations").
"""

from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

SUM_FIELDS = (
    "count",
    "errors",
    "sum_us",
    "exclusive_sum_us",
    "db_us",
    "db_ops",
    "response_bytes",
    "slow",
)


def default_dir() -> Path:
    return Path.home() / ".tokensave" / "profile"


def snapshot(directory: Path) -> set[str]:
    try:
        return {p.name for p in directory.iterdir() if p.suffix == ".json"}
    except OSError:
        return set()


def collect(directory: Path, before: set[str], settle_s: float = 5.0) -> dict[str, Any] | None:
    """Merges the files written since `before`. Waits up to `settle_s` for
    the server's exit flush. Returns `None` when no file appeared."""
    deadline = time.monotonic() + settle_s
    names: set[str] = set()
    while True:
        names = snapshot(directory) - before
        if names or time.monotonic() >= deadline:
            break
        time.sleep(0.25)
    if not names:
        return None
    # Give a just-created file's final rewrite a moment to land.
    time.sleep(0.5)
    merged: dict[str, dict[str, Any]] = {}
    processes = set()
    for name in sorted(names):
        try:
            data = json.loads((directory / name).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if data.get("schema") != 1:
            continue
        processes.add(name.split("-", 1)[1] if "-" in name else name)
        for series, snap in (data.get("series") or {}).items():
            into = merged.setdefault(series, {"repeated_executions": 0})
            for f in SUM_FIELDS:
                into[f] = into.get(f, 0) + int(snap.get(f, 0) or 0)
            into["max_us"] = max(into.get("max_us", 0), int(snap.get("max_us", 0) or 0))
            for rep in (snap.get("repeated") or {}).values():
                into["repeated_executions"] += int(rep.get("executions", 0) or 0)
    return summarize(merged, len(processes))


def _ms(us: int) -> float:
    return round(us / 1000.0, 3)


def summarize(series: dict[str, dict[str, Any]], processes: int) -> dict[str, Any]:
    """Derives per-trial figures. Only static series names are kept."""

    def s(name: str) -> dict[str, Any]:
        return series.get(name) or {}

    tools = {
        name[len("tool.") :]: {
            "calls": v.get("count", 0) + v.get("errors", 0),
            "errors": v.get("errors", 0),
            "ms": _ms(v.get("sum_us", 0)),
            "db_ms": _ms(v.get("db_us", 0)),
            "db_ops": v.get("db_ops", 0),
            "response_bytes": v.get("response_bytes", 0),
            "repeated_db_executions": v.get("repeated_executions", 0),
        }
        for name, v in series.items()
        if name.startswith("tool.")
    }
    total = s("mcp.total")
    return {
        "processes": processes,
        "mcp_calls": total.get("count", 0) + total.get("errors", 0),
        "mcp_total_ms": _ms(total.get("sum_us", 0)),
        "mcp_dispatch_ms": _ms(s("mcp.dispatch").get("sum_us", 0)),
        "mcp_project_resolution_ms": _ms(s("mcp.project_resolution").get("sum_us", 0)),
        "mcp_freshness_ms": _ms(s("mcp.freshness").get("sum_us", 0)),
        "mcp_handler_ms": _ms(s("mcp.handler").get("sum_us", 0)),
        "db_ms_in_tools": round(sum(t["db_ms"] for t in tools.values()), 3),
        "db_ops_in_tools": sum(t["db_ops"] for t in tools.values()),
        "repeated_db_executions": sum(t["repeated_db_executions"] for t in tools.values()),
        "sync_ms": _ms(s("sync.total").get("sum_us", 0)),
        "index_ms": _ms(s("index.total").get("sum_us", 0)),
        "slow_ops": sum(v.get("slow", 0) for k, v in series.items() if k.startswith("db.")),
        "tools": tools,
    }
