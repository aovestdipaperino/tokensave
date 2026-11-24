#!/usr/bin/env python3
"""Measure additive SCIP imports on fresh, disposable fixture graphs."""

import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import statistics
import subprocess
import tempfile
import time


FIXTURES = {
    "ambiguous": "bd809e524fcfb6870c31c596fb3a8887bb0ac452c7f11013d1c0b5b41f16d4b7",
    "typescript": "c2147d9fa7daceb5dc28c33b51d8e41fbda3c0b27273bd846fc9683ef7fed4ec",
}
SOURCES = {
    "ambiguous": ("alpha.rs", "beta.rs", "lib.rs"),
    "typescript": ("alpha.ts", "beta.ts", "index.ts", "validate.ts"),
}
MAX_FIXTURE_FILE_BYTES = 64 * 1024
SAMPLE_LIMIT = 1000  # scip-compare CLI maximum
QUERY_REPEATS = 101
COMMAND_TIMEOUT_SECONDS = 60
CALLERS = "SELECT source, target, kind, line, resolved_by FROM edges WHERE target = ?"


def copy_snapshot(fixture, project, name):
    digest = hashlib.sha256()
    paths = [Path("index.scip"), *(Path("src") / file for file in SOURCES[name])]
    (project / "src").mkdir(parents=True)
    for relative in sorted(paths):
        with (fixture / relative).open("rb") as source:
            data = source.read(MAX_FIXTURE_FILE_BYTES + 1)
        if len(data) > MAX_FIXTURE_FILE_BYTES:
            raise ValueError(f"fixture file exceeds size limit: {relative}")
        digest.update(relative.as_posix().encode())
        digest.update(b"\0")
        digest.update(hashlib.sha256(data).digest())
        (project / relative).write_bytes(data)
    if digest.hexdigest() != FIXTURES[name]:
        raise ValueError("fixture snapshot differs from the pinned source and index")


def run(args, root, environment):
    return subprocess.run(
        args, cwd=root, env=environment, stdout=subprocess.PIPE, check=True,
        timeout=COMMAND_TIMEOUT_SECONDS
    ).stdout


def verify_node(db, node_id, qualified_name):
    row = db.execute(
        "SELECT file_path, qualified_name FROM nodes WHERE id = ?", (node_id,)
    ).fetchone()
    if row is None or row[1] != qualified_name:
        raise ValueError("node identity differs from the comparison snapshot")
    return row[0]


def caller_probe(db, targets):
    samples = []
    rows = 0
    for _ in range(QUERY_REPEATS):
        start = time.perf_counter_ns()
        rows = sum(len(db.execute(CALLERS, (target,)).fetchall()) for target in targets)
        samples.append(time.perf_counter_ns() - start)
    return {"rows_per_sweep": rows, "median_sweep_us": statistics.median(samples) / 1000}


def graph_bytes(db):
    page_count = db.execute("PRAGMA page_count").fetchone()[0]
    page_size = db.execute("PRAGMA page_size").fetchone()[0]
    return page_count * page_size


def apply_plan(db, report):
    proposals = report.get("samples", {}).get("proposed_import", [])
    if len(proposals) != report["proposed_import"]["edges"]:
        raise ValueError("comparison did not return the full import plan")
    candidates = []
    for proposal in proposals:
        source, target = proposal["source_id"], proposal["target_id"]
        if verify_node(db, source, proposal["source"]) != proposal["file"]:
            raise ValueError("source file differs from the comparison snapshot")
        verify_node(db, target, proposal["target"])
        if proposal["kind"] not in ("calls", "uses") or proposal["line"] < 1:
            raise ValueError("invalid edge in import plan")
        candidates.append(
            (source, target, proposal["kind"], proposal["line"] - 1,
             proposal["provenance"])
        )
    targets = sorted({target for _, target, _, _, _ in candidates})
    before = {"bytes": graph_bytes(db), "callers": caller_probe(db, targets)}
    start = time.perf_counter_ns()
    # This table exists only in the disposable experiment database. Its FK
    # keeps provenance attached to the inserted edge, without a storage code
    # that would change the application schema.
    with db:
        db.execute("BEGIN IMMEDIATE")
        db.execute("""
            CREATE TABLE scip_preview_provenance (
                edge_id INTEGER PRIMARY KEY REFERENCES edges(id) ON DELETE CASCADE,
                tool TEXT NOT NULL,
                tool_version TEXT NOT NULL,
                symbol TEXT NOT NULL
            )
        """)
        inserted = 0
        for source, target, kind, line, provenance in candidates:
            result = db.execute(
                "INSERT INTO edges(source, target, kind, line) VALUES (?, ?, ?, ?) "
                "ON CONFLICT DO NOTHING",
                (source, target, kind, line),
            )
            if result.rowcount:
                db.execute(
                    "INSERT INTO scip_preview_provenance VALUES (?, ?, ?, ?)",
                    (result.lastrowid, provenance["tool"], provenance["tool_version"],
                     provenance["symbol"]),
                )
                inserted += 1
    import_ms = (time.perf_counter_ns() - start) / 1_000_000
    after = {"bytes": graph_bytes(db), "callers": caller_probe(db, targets)}
    if after["callers"]["rows_per_sweep"] - before["callers"]["rows_per_sweep"] != inserted:
        raise RuntimeError("incoming edge query did not return every inserted edge")
    return {
        "proposed": len(proposals),
        "inserted": inserted,
        "targets": len(targets),
        "import_ms": import_ms,
        "before": before,
        "after": after,
    }


def preview(binary, fixture, name):
    with tempfile.TemporaryDirectory(prefix="tokensave-scip-preview-") as temp:
        work = Path(temp)
        project = work / "project"
        (work / "home").mkdir()
        copy_snapshot(fixture, project, name)
        index = work / "index.scip"
        (project / index.name).rename(index)
        environment = {
            key: value for key, value in os.environ.items()
            if not key.startswith(("TOKENSAVE_", "GIT_"))
        }
        environment.update(HOME=str(work / "home"), USERPROFILE=str(work / "home"),
                           XDG_CONFIG_HOME=str(work / "home"), TOKENSAVE_TELEMETRY="0")
        run(["git", "init", "--quiet"], project, environment)
        run([binary, "init", project, "--no-git-hook", "--cpu-usage", "low"], project, environment)
        state = project / ".tokensave"
        config_path = state / "config.json"
        config = json.loads(config_path.read_text(encoding="utf-8"))
        config["enable_scip"] = True
        config_path.write_text(json.dumps(config), encoding="utf-8")
        report = json.loads(run(
            [binary, "scip-compare", "--project", project, "--index", index,
             "--rebase-root", "--samples", str(SAMPLE_LIMIT)], project, environment
        ))
        if report["documents"]["compared"] != len(SOURCES[name]):
            raise ValueError("comparison skipped fixture documents")
        with closing(sqlite3.connect(state / "tokensave.db")) as db:
            db.execute("PRAGMA foreign_keys = ON")
            return apply_plan(db, report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tokensave", required=True, type=Path)
    parser.add_argument("--fixture", choices=FIXTURES, required=True)
    options = parser.parse_args()
    fixture = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "scip" / options.fixture
    result = preview(options.tokensave.resolve(strict=True), fixture, options.fixture)
    print(json.dumps({"fixture": options.fixture, **result}, indent=2))


if __name__ == "__main__":
    main()
