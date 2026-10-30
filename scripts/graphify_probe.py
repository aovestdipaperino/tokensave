#!/usr/bin/env python3
"""Local coexistence probe for #791; requires GNU time and both CLIs.

Creates and removes its own temporary corpus. Never installs agent integrations,
reads either tool's database, or calls a model. Results are observations, not a
cross-tool retrieval-quality benchmark. Run with --help for usage.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--graphify", required=True, help="Pinned Graphify CLI path")
    parser.add_argument("--tokensave", default="tokensave")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--benchmark-repo", type=Path,
                        help="Copy tracked .rs/.py files to a disposable corpus (max 64 MiB/5000 files)")
    args = parser.parse_args()
    graphify = shutil.which(args.graphify)
    tokensave = shutil.which(args.tokensave)
    if not graphify or not tokensave or not Path("/usr/bin/time").exists():
        parser.error("Both CLIs and GNU /usr/bin/time are required")
    env = dict(os.environ, GRAPHIFY_BACKEND="none", GRAPHIFY_QUERY_LOG_DISABLE="1",
               GRAPHIFY_NO_TIPS="1", TOKENSAVE_SKIP_AGENT_MAINTENANCE="1")
    results = {"measurements": [], "checks": {}}
    with tempfile.TemporaryDirectory(prefix="tokensave-graphify-probe-") as tmp:
        root = Path(tmp)
        repo = root / "corpus"
        repo.mkdir()

        def run(label, command, cwd=repo, expected=0):
            metrics = root / "metrics"
            log = root / "command.log"
            with log.open("w") as stream:
                process = subprocess.Popen(
                    ["/usr/bin/time", "-f", "%e %U %S %M %x", "-o",
                     str(metrics), *command], cwd=cwd, env=env,
                    stdout=stream, stderr=stream, start_new_session=True)
                try:
                    process.wait(timeout=120)
                finally:
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
            output = log.read_text()
            # GNU time precedes metrics with a diagnostic on a nonzero exit.
            wall, user, system, rss, status = metrics.read_text().splitlines()[-1].split()
            record = {"step": label, "wall_seconds": float(wall),
                      "cpu_seconds": float(user) + float(system),
                      "peak_rss_kib": int(rss), "status": int(status)}
            results["measurements"].append(record)
            if int(status) != expected:
                raise RuntimeError(f"{label} exited {status}:\n{output[-8000:]}")
            return output

        def git(*argv):
            return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", *argv],
                                  cwd=repo, env=env, check=True,
                                  capture_output=True, text=True)

        git("init", "-b", "main")
        git("config", "user.name", "Graphify probe")
        git("config", "user.email", "probe@example.invalid")
        (repo / ".gitignore").write_text(".tokensave/\ngraphify-out/\n")
        source = repo / "app.py"
        source.write_text("def helper():\n    return 1\n\ndef entry():\n    return helper()\n")
        (repo / "README.md").write_text("The entry function calls helper.\n")
        git("add", ".")
        git("commit", "-m", "fixture")
        results["tokensave_version"] = run("tokensave-version", [tokensave, "--version"]).strip()
        results["graphify_version"] = run("graphify-version", [graphify, "--version"]).strip()
        run("tokensave-init", [tokensave, "init", "--no-git-hook", "--cpu-usage", "low"])
        extract = [graphify, "extract", ".", "--code-only"]
        run("graphify-extract", extract)

        def graph_at(folder):
            data = json.loads((folder / "graphify-out" / "graph.json").read_text())
            nodes = data["nodes"]
            edges = data.get("links", data.get("edges", []))
            ids = {node["id"] for node in nodes}
            assert all(e["source"] in ids and e["target"] in ids for e in edges)
            return data, nodes, edges

        data, nodes, edges = graph_at(repo)
        results["initial_graph"] = {"nodes": len(nodes), "edges": len(edges),
                                    "metadata": data.get("graph", {}),
                                    "sample_nodes": nodes[:3], "sample_edges": edges[:3]}
        assert any("helper" in n["id"] for n in nodes)
        assert all(n.get("file_type") != "document" for n in nodes)
        results["checks"]["code_only_skips_document"] = True
        assert data.get("graph", {}).get("schema_version") == 1
        assert any(e.get("relation") == "calls" and e.get("confidence") == "EXTRACTED"
                   and e.get("source_location") == "L5" for e in edges)
        results["checks"]["version_and_call_provenance"] = True
        run("tokensave-search", [tokensave, "tool", "search", "helper"])
        run("graphify-explain", [graphify, "explain", "helper"])
        future = root / "future.json"
        future_data = json.loads(json.dumps(data))
        future_data["graph"]["schema_version"] = 999
        future.write_text(json.dumps(future_data))
        # Observe current behavior; this must not be treated as adapter compatibility.
        run("graphify-future-schema-explain", [graphify, "explain", "helper",
                                              "--graph", str(future)])
        results["checks"]["future_schema_accepted_by_explain"] = True
        for index in range(3):
            run(f"tokensave-noop-{index}", [tokensave, "sync"])
            run(f"graphify-noop-{index}", extract)
        source.write_text(source.read_text().replace("helper", "changed"))
        run("tokensave-edit", [tokensave, "sync"])
        run("graphify-edit", extract)
        _, nodes, _ = graph_at(repo)
        assert any("changed" in n["id"] for n in nodes)
        assert not any("helper" in n["id"] for n in nodes)
        results["checks"]["uncommitted_symbol_rename"] = True
        source.rename(repo / "renamed.py")
        run("graphify-file-rename", extract)
        _, nodes, _ = graph_at(repo)
        assert not any(Path(n.get("source_file", "")).name == "app.py" for n in nodes)
        results["checks"]["file_rename_prunes_old_source"] = True
        # Keep another code file so deleting the last code file is not a no-input run.
        (repo / "keep.py").write_text("def remaining():\n    return 2\n")
        (repo / "renamed.py").unlink()
        run("graphify-delete", extract)
        _, nodes, _ = graph_at(repo)
        assert not any("changed" in n["id"] for n in nodes)
        results["checks"]["deletion"] = True
        worktree = root / "worktree"
        git("worktree", "add", "-b", "other", str(worktree))
        (worktree / "app.py").write_text("def worktree_only():\n    return 3\n")
        run("graphify-worktree", extract, cwd=worktree)
        _, other_nodes, _ = graph_at(worktree)
        _, main_nodes, _ = graph_at(repo)
        assert any("worktree_only" in n["id"] for n in other_nodes)
        assert not any("worktree_only" in n["id"] for n in main_nodes)
        results["checks"]["separate_worktree_output"] = True
        results["artifact_bytes"] = {
            name: sum(p.stat().st_size for p in (repo / name).rglob("*") if p.is_file())
            for name in (".tokensave", "graphify-out")}
        if args.benchmark_repo:
            original = args.benchmark_repo.resolve()
            paths = subprocess.run(["git", "ls-files", "-z", "--", "*.rs", "*.py"],
                                   cwd=original, check=True, capture_output=True).stdout
            corpus = root / "benchmark"
            corpus.mkdir()
            count = size = 0
            for name in paths.decode().split("\0"):
                if not name:
                    continue
                relative = Path(name)
                source_path = original / relative
                if relative.is_absolute() or ".." in relative.parts:
                    raise ValueError("Unsafe tracked path")
                if source_path.is_symlink() or not source_path.resolve().is_relative_to(original):
                    raise ValueError("Symlink outside corpus contract")
                count += 1
                size += source_path.stat().st_size
                if count > 5000 or size > 64 * 1024 * 1024:
                    raise ValueError("Benchmark corpus exceeds 5000 files/64 MiB")
                target = corpus / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source_path, target)
            revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=original,
                                      check=True, capture_output=True, text=True).stdout.strip()
            results["benchmark_corpus"] = {"revision": revision, "files": count,
                                           "source_bytes": size,
                                           "scope": "tracked .rs/.py working-tree contents"}
            run("benchmark-tokensave-init",
                [tokensave, "init", "--no-git-hook", "--cpu-usage", "low"], cwd=corpus)
            run("benchmark-graphify-extract", extract, cwd=corpus)
            for index in range(3):
                run(f"benchmark-tokensave-noop-{index}", [tokensave, "sync"], cwd=corpus)
                run(f"benchmark-graphify-noop-{index}", extract, cwd=corpus)
            results["benchmark_artifact_bytes"] = {
                name: sum(p.stat().st_size for p in (corpus / name).rglob("*") if p.is_file())
                for name in (".tokensave", "graphify-out")}
    args.output.write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
