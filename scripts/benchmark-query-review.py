#!/usr/bin/env python3
"""Read-only Q0119/Q0120/FTS comparisons; synthetic fixtures use native SQLite.

Run from any directory. Optional --database points at a real tokensave index
(opened read-only); only FTS is benchmarked there. Timings are local evidence,
not bundled-libSQL/production guarantees. Full fixture results and limited FTS
score sequences are compared before timing; tied FTS IDs may differ.
"""
import argparse
from contextlib import closing
import json
import math
from pathlib import Path
import re
import sqlite3
import statistics
import time

ROOT = Path(__file__).resolve().parent.parent
EDGES_SOURCE = ROOT / "src" / "db" / "queries" / "edges.rs"
SEARCH_SOURCE = ROOT / "src" / "db" / "queries" / "search.rs"


def source_sql(file, name, formatted=False):
    source = file.read_text()
    function = source[source.index("fn " + name + "("):]
    if formatted:
        raw = function.split('let sql = format!(\n            "', 1)[1].split('"\n        );', 1)[0]
    else:
        match = re.search(r'\.query\(\s*"((?:\\.|[^"\\])*)"', function, re.S)
        if not match:
            raise ValueError("SQL literal not found: " + name)
        raw = match[1]
    return json.loads('"' + raw.replace("\n", "\\n") + '"')


def compare(connection, old, new, params, repetitions, full_limit=None):
    if full_limit is not None:
        values = list(params[:-1]) + [full_limit]
        assert sorted(connection.execute(old, values).fetchall()) == sorted(connection.execute(new, values).fetchall())
    before = connection.execute(old, params).fetchall()
    after = connection.execute(new, params).fetchall()
    assert len(before) == len(after)
    # The last output column is the score/count. The existing queries do not
    # promise an ID tie-breaker, so compare score sequences at a tied cutoff.
    assert all(math.isclose(a[-1], b[-1], rel_tol=1e-12, abs_tol=1e-12) for a, b in zip(before, after))
    samples = {"old": [], "new": []}
    for sql in (old, new):
        for _ in range(2):
            connection.execute(sql, params).fetchall()
    for i in range(repetitions):
        pair = [("old", old), ("new", new)]
        for label, sql in pair if i % 2 == 0 else reversed(pair):
            start = time.perf_counter()
            connection.execute(sql, params).fetchall()
            samples[label].append((time.perf_counter() - start) * 1000)
    result = {}
    for label, values in samples.items():
        ordered = sorted(values)
        result[label] = {"median_ms": round(statistics.median(values), 3), "p95_ms": round(ordered[math.ceil(len(values) * .95) - 1], 3)}
    result["rows"] = len(after)
    result["new_plan"] = [row[3] for row in connection.execute("EXPLAIN QUERY PLAN " + new, params)]
    return result


def fixture():
    connection = sqlite3.connect(":memory:")
    largest = source_sql(EDGES_SOURCE, "get_largest_nodes", True)
    columns = [c.strip() for c in largest.split("SELECT ", 1)[1].split(",\n                    (end_line", 1)[0].split(",")]
    texts = {"id", "kind", "name", "qualified_name", "file_path", "docstring", "signature", "visibility", "parent_id"}
    connection.execute("CREATE TABLE nodes (" + ",".join(c + (" TEXT PRIMARY KEY" if c == "id" else " TEXT" if c in texts else " INTEGER") for c in columns) + ",search_terms TEXT)")
    connection.executescript("""CREATE TABLE edges(source TEXT,target TEXT,kind TEXT,line INTEGER);
    CREATE INDEX idx_nodes_kind ON nodes(kind);
    CREATE INDEX idx_nodes_file_path ON nodes(file_path);
    CREATE INDEX idx_edges_source_kind ON edges(source,kind);
    CREATE INDEX idx_edges_target_kind ON edges(target,kind);
    CREATE INDEX idx_edges_kind ON edges(kind);
    CREATE VIRTUAL TABLE nodes_fts USING fts5(name,qualified_name,docstring,signature,search_terms,content='nodes',content_rowid='rowid',tokenize='porter unicode61');""")
    nodes = []
    edges = []
    for i in range(12000):
        kind = "file" if i % 25 == 0 else "doc" if i % 25 == 1 else "function"
        nodes.append((f"n{i:05}", kind, f"common_engine_{i}", f"crate::module{i % 17}::common_engine", f"src/f{i % 96}.rs", 1, 1 + i * 17 % 1999, "common engine helper", "fn common_engine()", "common engine"))
        for j in range(1, 9):
            edges.append((f"n{i:05}", f"n{(i + j * 17) % 12000:05}", "calls" if j % 2 else "uses", j))
    connection.executemany("INSERT INTO nodes(id,kind,name,qualified_name,file_path,start_line,end_line,docstring,signature,search_terms) VALUES(?,?,?,?,?,?,?,?,?,?)", nodes)
    connection.executemany("INSERT INTO edges VALUES(?,?,?,?)", edges)
    connection.commit()
    connection.execute("ANALYZE")
    return connection, largest, columns


def analytical_benches(connection, largest, columns, repetitions):
    results = {}
    for payload in (0, 2048):
        connection.execute("UPDATE nodes SET signature=?,docstring=?", ("s" * payload, "d" * payload))
        for scoped in (False, True):
            where = "WHERE kind = ?1" + (" AND (file_path = 'src/f0.rs' OR file_path LIKE 'src/f0.rs/%' ESCAPE '\\')" if scoped else "")
            old = largest.replace("{where_clause}", where).replace("{param_idx}", "2")
            projection = ",".join("n." + c for c in columns)
            new = f"WITH ranked AS MATERIALIZED (SELECT id,(end_line-start_line+1) AS lines FROM nodes {where} ORDER BY lines DESC,id LIMIT ?2) SELECT {projection},r.lines FROM ranked r JOIN nodes n ON n.id=r.id ORDER BY r.lines DESC,r.id"
            results[f"Q0119_{payload}_bytes_each_{'scoped' if scoped else 'global'}"] = compare(connection, old, new, ["function", 20], repetitions, 20000)
    coupling = source_sql(EDGES_SOURCE, "get_file_coupling", True)
    for files in (96, 12000):
        connection.execute("UPDATE nodes SET file_path='src/f' || ((rowid-1) % ?) || '.rs'", [files])
        connection.execute("ANALYZE")
        for incoming in (False, True):
            group, count = ("n_tgt", "n_src") if incoming else ("n_src", "n_tgt")
            for scoped in (False, True):
                path = f"AND ({group}.file_path = 'src/f0.rs' OR {group}.file_path LIKE 'src/f0.rs/%' ESCAPE '\\')" if scoped else ""
                old = coupling.replace("{group_alias}", group).replace("{count_alias}", count).replace("{path_filter}", path)
                new = f"WITH pairs AS (SELECT DISTINCT {group}.file_path AS file_path,{count}.file_path AS other FROM edges e JOIN nodes n_src ON e.source=n_src.id JOIN nodes n_tgt ON e.target=n_tgt.id WHERE e.kind IN ('calls','uses','implements','extends') AND n_src.file_path!=n_tgt.file_path {path}) SELECT file_path,COUNT(*) AS coupling FROM pairs GROUP BY file_path ORDER BY coupling DESC LIMIT ?1"
                results[f"Q0120_{files}_files_{'in' if incoming else 'out'}_{'scoped' if scoped else 'global'}"] = compare(connection, old, new, [20], repetitions, 20000)
    return results


def fts_query_pair(current):
    """Support master before and after the independent FTS implementation."""
    function = "bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0)"
    constraint = "                   AND nodes_fts.rank MATCH 'bm25(10.0, 5.0, 1.0, 2.0, 3.0)'\n"
    if "nodes_fts.rank AS rank" in current:
        new = current
        old = new.replace("nodes_fts.rank AS rank", function + " AS rank").replace(constraint, "").replace("ORDER BY nodes_fts.rank", "ORDER BY " + function)
    else:
        assert function + " AS rank" in current, "FTS source shape changed; review baseline construction"
        old = current
        new = old.replace(function, "nodes_fts.rank").replace("ORDER BY rank", "ORDER BY nodes_fts.rank")
        new = new.replace("WHERE nodes_fts MATCH ?1\n", "WHERE nodes_fts MATCH ?1\n" + constraint)
    assert "nodes_fts.rank" not in old, "FTS baseline transformation incomplete"
    assert new.count("nodes_fts.rank AS rank") == 1 and constraint in new
    return old, new


def fts_benches(connection, repetitions):
    results = {}
    for name in ("search_nodes_bounded_fts", "search_nodes_fts"):
        old, new = fts_query_pair(source_sql(SEARCH_SOURCE, name))
        for term in ('"common"*', '"engine"*', '"search"*', '"node"*', '"common engine"', '"search"* OR "node"*'):
            results[name + ":" + term] = compare(connection, old, new, [term, 20], repetitions)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", type=Path, help="real project index, opened read-only (FTS only)")
    parser.add_argument("--repetitions", type=int, default=15)
    args = parser.parse_args()
    if not 5 <= args.repetitions <= 101:
        parser.error("repetitions must be between 5 and 101")
    report = {"sqlite": sqlite3.sqlite_version, "repetitions": args.repetitions, "warmups": 2, "limit": 20, "timing_order": "alternating old/new", "scope": "native SQLite, not bundled libSQL"}
    if args.database:
        with closing(sqlite3.connect(args.database.resolve().as_uri() + "?mode=ro", uri=True)) as connection:
            report["nodes"] = connection.execute("SELECT COUNT(*) FROM nodes").fetchone()[0]
            report["fts"] = fts_benches(connection, args.repetitions)
    else:
        connection, largest, columns = fixture()
        try:
            report.update(nodes=12000, edges=96000, analyzed=True)
            report["analytical"] = analytical_benches(connection, largest, columns, args.repetitions)
            connection.execute("UPDATE nodes SET docstring='common engine helper ' || substr(id,2),signature='fn common_engine()'")
            connection.execute("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')")
            report["fts"] = fts_benches(connection, args.repetitions)
        finally:
            connection.close()
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
