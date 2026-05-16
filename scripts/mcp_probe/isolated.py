#!/usr/bin/env python3
"""Re-run probes against a single repo with a fresh MCP server per tool.

Use when probe.py reports cascading TIMEOUTs and you need to know which
tools genuinely hang vs. which only appear to hang because they're queued
behind an earlier slow call.

  python3 isolated.py <repo-name>           # uses repos.toml
  python3 isolated.py <repo-name> --tools tokensave_inheritance_depth

Output goes to /tmp/tokensave_isolated.log (override with
$TOKENSAVE_PROBE_LOG_ISO). Per-tool: spawn fresh server, init handshake,
run the 5 queries from the language probe set, abort on first TIMEOUT
(server is busy, no point queuing more), restart for next tool.
"""
from __future__ import annotations

import argparse
import importlib
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from probe import (  # noqa: E402  pylint: disable=wrong-import-position
    McpClient, classify, discover, load_repos,
)

LOG_PATH = os.environ.get("TOKENSAVE_PROBE_LOG_ISO", "/tmp/tokensave_isolated.log")
TIMEOUT = float(os.environ.get("TOKENSAVE_PROBE_TIMEOUT_ISO", "60"))


def run_tool_fresh(repo_name: str, repo_path: str, tool: str, queries: list[dict], log) -> None:
    cli = McpClient(repo_path)
    try:
        for query in queries:
            resp, dt = cli.call(tool, query, timeout=TIMEOUT)
            st, detail = classify(resp, dt)
            log.write(f"{repo_name}\t{tool}\t{json.dumps(query)[:120]}\t{st}\t{detail}\n")
            log.flush()
            if st == "TIMEOUT":
                # The server is busy with the timed-out call; no point queuing more.
                break
    finally:
        cli.close()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("repo", help="name of the repo (from repos.toml)")
    ap.add_argument("--tools", nargs="*", help="restrict to these tool names")
    args = ap.parse_args()

    repos = {r["name"]: r for r in load_repos()}
    if args.repo not in repos:
        print(f"unknown repo '{args.repo}'. Known: {sorted(repos)}", file=sys.stderr)
        return 2
    repo = repos[args.repo]

    print(f"=== isolated probe: {repo['name']} ({repo['path']}) ===", file=sys.stderr)
    # Quick discover pass against a primer server so per-tool fresh servers
    # have plausible node ids etc.
    primer = McpClient(repo["path"])
    try:
        discovered = discover(primer)
    finally:
        primer.close()

    probes: dict = {}
    for lang in repo.get("languages", []):
        mod = importlib.import_module(f"tools.{lang}")
        probes.update(mod.probes_for(discovered))

    if args.tools:
        wanted = set(args.tools)
        probes = {k: v for k, v in probes.items() if k in wanted}
        missing = wanted - set(probes)
        if missing:
            print(f"warning: tools not in probe set: {sorted(missing)}", file=sys.stderr)

    with open(LOG_PATH, "w") as log:
        for tool, queries in probes.items():
            print(f"# {tool}", file=sys.stderr, flush=True)
            run_tool_fresh(repo["name"], repo["path"], tool, queries, log)

    print(f"log written to {LOG_PATH}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
