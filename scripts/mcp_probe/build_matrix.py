#!/usr/bin/env python3
"""Read /tmp/tokensave_matrix.log (TSV from probe.py) and render a markdown
status matrix on stdout. Optionally merge an isolated-probe log so
post-cascade rows get replaced with their fresh-server results.

Status legend:
  ✓ N/N     all passed (or some allowed EMPTY)
  ∅ E/N     all empty (no content)
  🐢 ok/slow some calls took >10s
  🐛 e/N    some errors (caller-side or tool-side)
  ⏱ T/N    some timeouts (likely real perf bug if first in a series)
  🐛 all fail every call errored or timed out

Usage:
  python3 build_matrix.py
  python3 build_matrix.py --log /tmp/tokensave_matrix.log \
                          --isolated /tmp/tokensave_isolated.log
"""
from __future__ import annotations

import argparse
import os
from collections import defaultdict
from pathlib import Path

DEFAULT_LOG = os.environ.get("TOKENSAVE_PROBE_LOG", "/tmp/tokensave_matrix.log")
DEFAULT_ISO = os.environ.get("TOKENSAVE_PROBE_LOG_ISO", "/tmp/tokensave_isolated.log")


def load(path: Path):
    """Yield (repo, tool, query, status, detail) tuples."""
    if not path.exists():
        return
    with open(path) as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if len(parts) >= 5:
                yield parts[0], parts[1], parts[2], parts[3], parts[4]


def summarize(statuses: list[tuple[str, str]]) -> str:
    if not statuses:
        return "—"
    n = len(statuses)
    c: dict[str, int] = defaultdict(int)
    for s, _ in statuses:
        c[s] += 1
    err = c["ERROR"] + c["BAD"]
    if err + c["TIMEOUT"] == n:
        return "🐛 all fail"
    if err and c["TIMEOUT"]:
        return f"🐛 {err}err/{c['TIMEOUT']}to"
    if c["TIMEOUT"]:
        return f"⏱ {c['TIMEOUT']}/{n}"
    if err:
        return f"🐛 {err}/{n} err"
    if c["OK"] == 0:
        return f"∅ {c['EMPTY']}/{n}"
    if c["SLOW"]:
        return f"🐢 {c['OK']}ok/{c['SLOW']}slow"
    if c["EMPTY"]:
        return f"✓ {c['OK']} +{c['EMPTY']}∅"
    return f"✓ {c['OK']}/{n}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", default=DEFAULT_LOG)
    ap.add_argument("--isolated", default=DEFAULT_ISO)
    args = ap.parse_args()

    # tool -> repo -> [(status, detail), ...]
    matrix: dict[str, dict[str, list[tuple[str, str]]]] = defaultdict(lambda: defaultdict(list))
    repos: list[str] = []

    for repo, tool, _q, st, det in load(Path(args.log)):
        if repo not in repos:
            repos.append(repo)
        matrix[tool][repo].append((st, det))

    # Replace cells that have isolated re-run data.
    iso_replacements: dict[tuple[str, str], list[tuple[str, str]]] = defaultdict(list)
    for repo, tool, _q, st, det in load(Path(args.isolated)):
        iso_replacements[(tool, repo)].append((st, det))
    for (tool, repo), entries in iso_replacements.items():
        matrix[tool][repo] = entries
        if repo not in repos:
            repos.append(repo)

    # Header
    print(f"| tool | {' | '.join(repos)} | debug |")
    print("|---|" + "---|" * (len(repos) + 1))

    fail_details: list[str] = []
    for tool in sorted(matrix):
        cells: list[str] = []
        debug = False
        for repo in repos:
            sts = matrix[tool].get(repo, [])
            cells.append(summarize(sts))
            if any(s in ("ERROR", "BAD", "TIMEOUT") for s, _ in sts):
                debug = True
                for s, det in sts:
                    if s in ("ERROR", "BAD", "TIMEOUT"):
                        fail_details.append(f"- `{tool}` @ **{repo}** ({s}): {det}")
                        break
        flag = "🚩" if debug else ""
        print(f"| `{tool}` | " + " | ".join(cells) + f" | {flag} |")

    if fail_details:
        print()
        print("## Failure details (first per cell)")
        print()
        for line in fail_details:
            print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
