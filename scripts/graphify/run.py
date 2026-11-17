#!/usr/bin/env python3
"""Offline fixture acceptance and process measurements for issue #791.

Requires a prepared Graphify checkout/venv; never installs dependencies or hooks.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import tempfile
import time

TARGET = "0a8cd4e1e342235fe53e881fb5d591bf0460071f"


def run(argv, cwd, env, samples, phase, expect=0):
    start = time.perf_counter()
    # Files keep command output out of an unbounded in-memory capture buffer.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=stdout, stderr=stderr,
                                   start_new_session=(os.name == "posix"))
        try:
            process.wait(timeout=60)
        except BaseException:
            if os.name == "posix":
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            else:
                process.kill()
            process.wait()
            raise
        elapsed = time.perf_counter() - start
        stdout.seek(0)
        stderr.seek(0)
        out, err = stdout.read(1024 * 1024 + 1), stderr.read(1024 * 1024 + 1)
    if len(out) > 1024 * 1024 or len(err) > 1024 * 1024:
        raise RuntimeError(f"{phase}: output exceeds 1 MiB capture budget")
    samples.append({"phase": phase, "seconds": elapsed, "exit_code": process.returncode})
    if (expect == 0 and process.returncode != 0) or (expect != 0 and process.returncode == 0):
        raise RuntimeError(f"{phase}: unexpected exit {process.returncode}: {err.decode(errors='replace')[:4096]}")
    return out.decode(errors="replace")


def check_graph(graph):
    if graph.get("graph", {}).get("schema_version") != 1:
        raise ValueError("unsupported Graphify schema")
    meta = graph["graph"]
    if meta.get("tokensave_interchange_version") != 1 or meta.get("target_graphify_commit") != TARGET:
        raise ValueError("unsupported producer contract")
    assert graph["directed"] and graph["multigraph"] and not meta["truncated"]
    nodes = graph["nodes"]
    ids = {n["id"] for n in nodes}
    assert len(nodes) == len(ids) and nodes
    for node in nodes:
        assert node["source_file"] == "core.rs" and node["source_location"].startswith("L")
        assert not {"signature", "docstring", "body"}.intersection(node)
    for edge in graph["links"]:
        assert edge["source"] in ids and edge["target"] in ids
        assert edge["confidence"] != "EXTRACTED"
        assert "tokensave_resolved_by" in edge
    assert any(e["relation"] == "calls" for e in graph["links"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tokensave", required=True, type=Path)
    parser.add_argument("--graphify-python", required=True, type=Path)
    parser.add_argument("--graphify-source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--runs", type=int, default=5)
    args = parser.parse_args()
    if not 2 <= args.runs <= 100:
        parser.error("--runs must be 2..100")
    if args.output.exists():
        parser.error("output exists; choose a new report path")
    ts, python, upstream = (str(p.absolute()) for p in (args.tokensave, args.graphify_python, args.graphify_source))
    samples = []
    with tempfile.TemporaryDirectory(prefix="tokensave-graphify-") as temp:
        base = Path(temp)
        home, repo, work = base / "home", base / "repo", base / "render"
        for directory in (home, repo, work):
            directory.mkdir()
        env = dict(os.environ)
        env.update(HOME=str(home), USERPROFILE=str(home), XDG_CONFIG_HOME=str(home / "config"),
                   XDG_CACHE_HOME=str(home / "cache"), TOKENSAVE_SKIP_AGENT_MAINTENANCE="1",
                   TOKENSAVE_CPU_USAGE="balanced", GRAPHIFY_BACKEND="none", GRAPHIFY_QUERY_LOG_DISABLE="1")
        def call(argv, phase, cwd=repo, expect=0):
            return run(argv, cwd, env, samples, phase, expect)
        pinned = call(["git", "-C", upstream, "rev-parse", "HEAD"], "graphify_commit").strip()
        if pinned != TARGET:
            raise ValueError(f"Graphify checkout must be {TARGET}")
        if call(["git", "-C", upstream, "status", "--porcelain", "--untracked-files=no"], "graphify_clean").strip():
            raise ValueError("Graphify checkout has tracked modifications")
        version = call([python, "-c", "from importlib.metadata import version; import graphify; print(version('graphifyy')); print(graphify.__file__)"], "graphify_package").splitlines()
        if version[0] != "0.9.97" or not Path(version[1]).resolve().is_relative_to(Path(upstream)):
            raise ValueError("Graphify Python must import the pinned editable checkout (0.9.97)")
        dependency_versions = json.loads(call([python, "-c", "import json; from importlib.metadata import distributions; print(json.dumps({d.metadata['Name']:d.version for d in distributions()}))"], "graphify_dependencies"))
        ts_version = call([ts, "--version"], "tokensave_version").strip()
        call(["git", "init", "-b", "main"], "git_init")
        call(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-m", "fixture"], "git_commit")
        source = repo / "core.rs"
        source.write_text("pub fn alpha() -> i32 { beta() }\nfn beta() -> i32 { 7 }\n")
        call(["git", "add", "core.rs"], "git_add")
        call(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "source fixture"], "git_source_commit")
        call([ts, "init", str(repo), "--no-git-hook", "--cpu-usage", "balanced"], "tokensave_init")
        def export(dest, expect=0, extra=()):
            return call([ts, "graphify-export", "--project", str(repo), "--file", "core.rs", "--output", str(dest), *extra], "export" if expect == 0 else "export_refusal", expect=expect)
        snapshots = []
        for i in range(args.runs):
            snapshot = base / f"snapshot-{i}.json"
            export(snapshot)
            graph = json.loads(snapshot.read_text())
            check_graph(graph)
            snapshots.append(snapshot)
            # Actual Graphify validator and NetworkX reader, including multiedges.
            call([python, "-c", "import json,sys; from graphify.validate import assert_valid; from networkx.readwrite.json_graph import node_link_graph; d=json.load(open(sys.argv[1])); assert_valid(d); g=node_link_graph(d, edges='links'); assert g.is_directed() and g.is_multigraph(); assert g.number_of_edges()==len(d['links'])", str(snapshot)], "graphify_load")
            symbols = {n["label"].split("::")[-1]: n["id"] for n in graph["nodes"]}
            answer = call([python, "-m", "graphify", "explain", symbols["alpha"], "--graph", str(snapshot)], "graphify_explain")
            assert "alpha" in answer
            answer = call([python, "-m", "graphify", "query", "alpha", "--graph", str(snapshot)], "graphify_query")
            assert "alpha" in answer
            path = call([python, "-m", "graphify", "path", symbols["alpha"], symbols["beta"], "--directed", "--graph", str(snapshot)], "graphify_path")
            assert "alpha" in path and "beta" in path
        first = snapshots[0]
        assert all(p.read_bytes() == first.read_bytes() for p in snapshots)
        export(first, expect=1)
        rejected = base / "rejected.json"
        export(rejected, expect=1, extra=("--max-nodes", "1"))
        assert not rejected.exists()
        source.write_text("pub fn changed() {}\n")
        export(rejected, expect=1)
        assert not rejected.exists()
        call([ts, "sync", str(repo)], "tokensave_sync")
        updated = base / "updated.json"
        export(updated)
        assert json.loads(updated.read_text())["graph"]["generation"] != graph["graph"]["generation"]
        assert "alpha" not in updated.read_text()
        # Conflicting dirty views in linked worktrees use independent explicit indexes.
        other = base / "worktree"
        call(["git", "worktree", "add", "-b", "other", str(other)], "worktree_add")
        (other / "core.rs").write_text("pub fn other_dirty_view() {}\n")
        call([ts, "init", str(other), "--no-git-hook", "--cpu-usage", "balanced"], "worktree_init", cwd=other)
        call([ts, "branch", "add", "other", "--path", str(other)], "worktree_track", cwd=other)
        other_snapshot = base / "other.json"
        call([ts, "graphify-export", "--project", str(other), "--file", "core.rs", "--output", str(other_snapshot)], "worktree_export", cwd=other)
        other_graph = json.loads(other_snapshot.read_text())
        assert other_graph["graph"]["generation"] != json.loads(updated.read_text())["graph"]["generation"]
        assert "other_dirty_view" in other_snapshot.read_text() and "other_dirty_view" not in updated.read_text()
        assert source.read_text() == "pub fn changed() {}\n"
        # Real visualization generation; do not launch a browser or fetch CDN JS.
        call([python, "-m", "graphify", "export", "html", "--graph", str(first), "--node-limit", "1000"], "graphify_html", cwd=work)
        html = list(base.glob("*.html"))
        assert html and all(p.stat().st_size > 0 for p in html)
        incompatible = dict(graph, graph=dict(graph["graph"], schema_version=999))
        try:
            check_graph(incompatible)
        except ValueError:
            pass
        else:
            raise AssertionError("schema guard accepted incompatible graph")
        phases = sorted({s["phase"] for s in samples})
        summary = {phase: {"count": len(values), "median_seconds": statistics.median(values), "min_seconds": min(values), "max_seconds": max(values)} for phase in phases if (values := [s["seconds"] for s in samples if s["phase"] == phase])}
        report = {"kind": "fixture_compatibility_smoke", "graphify_commit": TARGET, "graphify_version": version[0], "tokensave_version": ts_version,
                  "dependency_versions": dependency_versions, "platform": platform.platform(), "python": platform.python_version(), "runs": args.runs,
                  "nodes": len(graph["nodes"]), "edges": len(graph["links"]), "snapshot_bytes": first.stat().st_size,
                  "snapshot_sha256": hashlib.sha256(first.read_bytes()).hexdigest(), "samples": samples, "summary": summary,
                  "limitations": ["Synthetic single-file fixture, not a representative benchmark", "Separate CLI processes; no warm resident-server latency", "No aggregate RSS/PSS, idle CPU, disk-growth, relevance or model-cost evaluation", "No production rollout or document enrichment decision", "HTML generated only; external assets and browser behavior untested", "Windows process-tree cancellation untested", "Process wall timings include wait/backoff and interpreter startup overhead; not performance acceptance data"]}
    # Exclusive create avoids overwriting a prior report.
    with args.output.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(f"Passed fixture compatibility; report: {args.output}")


if __name__ == "__main__":
    main()
