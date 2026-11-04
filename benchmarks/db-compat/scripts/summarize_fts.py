#!/usr/bin/env python3
"""Summarize fts_compare JSON results into CSV files and Markdown tables.

    python3 -I scripts/summarize_fts.py results/fts

Reads functional-*.json, relevance-*.json and perf-*.json from the directory
and writes one CSV per JSON file plus summary.md next to
them. Medians are taken across repetitions; the [min-max] range across
repetitions is printed next to each median so run-to-run variability stays
visible. A ratio is marked "~" when the two engines' ranges overlap, which
means the difference is not distinguishable in these runs.
"""

import csv
import glob
import json
import os
import statistics
import sys


def median(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def rng(xs):
    xs = [x for x in xs if x is not None]
    return (min(xs), max(xs)) if xs else (None, None)


def fmt(x, digits=1):
    if x is None:
        return "-"
    if abs(x) >= 1000:
        return f"{x:,.0f}"
    return f"{x:.{digits}f}"


def cell(xs, digits=1):
    m = median(xs)
    lo, hi = rng(xs)
    if m is None:
        return "-"
    if len([x for x in xs if x is not None]) == 1 or lo == hi:
        return fmt(m, digits)
    return f"{fmt(m, digits)} [{fmt(lo, digits)}–{fmt(hi, digits)}]"


def ratio(a, b):
    """b/a with an overlap marker; a = libsql, b = turso."""
    ma, mb = median(a), median(b)
    if not ma or mb is None:
        return "-"
    (alo, ahi), (blo, bhi) = rng(a), rng(b)
    overlap = alo is not None and blo is not None and blo <= ahi and alo <= bhi
    return f"{'~' if overlap else ''}{mb / ma:.2f}×"


def get(d, *path):
    for p in path:
        if not isinstance(d, dict) or p not in d:
            return None
        d = d[p]
    return d


def functional(path, out_dir, md):
    doc = json.load(open(path))
    rows = doc["results"]
    with open(os.path.join(out_dir, os.path.basename(path).replace(".json", ".csv")), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["number", "scenario", "adapter", "class", "got", "reason"])
        for r in rows:
            w.writerow([r["number"], r["scenario"], r["adapter"], r["class"], r["got"], r["reason"]])
    adapters = list(dict.fromkeys(r["adapter"] for r in rows))
    md.append(f"## Functional scenarios ({os.path.basename(path)})\n")
    md.append("| # | Scenario | " + " | ".join(adapters) + " |")
    md.append("|---|---|" + "---|" * len(adapters))
    by = {}
    for r in rows:
        by.setdefault((r["number"], r["title"]), {})[r["adapter"]] = r["class"]
    for (num, title), cls in by.items():
        md.append(f"| {num} | {title} | " + " | ".join(cls.get(a, "-") for a in adapters) + " |")
    md.append("")
    counts = {}
    for r in rows:
        counts.setdefault(r["adapter"], {}).setdefault(r["class"], 0)
        counts[r["adapter"]][r["class"]] += 1
    md.append("| Adapter | " + " | ".join(sorted({c for v in counts.values() for c in v})) + " |")
    classes = sorted({c for v in counts.values() for c in v})
    md.append("|---|" + "---|" * len(classes))
    for a in adapters:
        md.append(f"| {a} | " + " | ".join(str(counts[a].get(c, 0)) for c in classes) + " |")
    md.append("")


RELEVANCE_COLS = ["ndcg_at_10", "mrr_at_10", "recall_at_10", "precision_at_5", "overlap_at_10_vs_libsql", "rbo_vs_libsql"]


def relevance(path, out_dir, md):
    doc = json.load(open(path))
    flat = []
    for c in doc["corpora"]:
        for r in c["rows"]:
            flat.append(r)
    with open(os.path.join(out_dir, os.path.basename(path).replace(".json", ".csv")), "w", newline="") as f:
        w = csv.writer(f)
        cols = ["corpus", "mode", "intent", "arm", "queries", "unsupported", "errors",
                "queries_without_relevant", "recall_at_5"] + RELEVANCE_COLS
        w.writerow(cols + ["identical_on_repeat", "identical_after_reopen", "stability_queries"])
        for r in flat:
            s = r["stability_exact_mode"]
            w.writerow([r[k] for k in cols] + [s["identical_on_repeat"], s["identical_after_reopen"], s["queries"]])
    md.append(f"## Relevance ({os.path.basename(path)}, k={doc['k']}, seed={doc['seed']:#x})\n")
    md.append("| Corpus | Nodes | Nodes with an indexed token > 40 bytes | …in the name |")
    md.append("|---|---|---|---|")
    for c in doc["corpora"]:
        md.append(f"| {c['corpus']} | {c['nodes']} | {c['nodes_with_indexed_token_over_40_bytes']} | {c['nodes_with_name_token_over_40_bytes']} |")
    md.append("")
    arms = list(dict.fromkeys(r["arm"] for r in flat))
    for mode in ["exact", "production"]:
        md.append(f"### Mode `{mode}`: mean over corpora (macro-average of per-corpus means)\n")
        md.append("| Intent | Arm | nDCG@10 | MRR@10 | R@10 | P@5 | overlap@10 | RBO | unsupported |")
        md.append("|---|---|---|---|---|---|---|---|---|")
        intents = list(dict.fromkeys(r["intent"] for r in flat if r["mode"] == mode))
        for intent in intents:
            for arm in arms:
                rs = [r for r in flat if r["mode"] == mode and r["intent"] == intent and r["arm"] == arm]
                if not rs:
                    continue
                vals = [statistics.mean(r[k] for r in rs) for k in RELEVANCE_COLS]
                uns = sum(r["unsupported"] for r in rs)
                n = sum(r["queries"] for r in rs)
                md.append(f"| {intent} | {arm} | " + " | ".join(f"{v:.3f}" for v in vals) + f" | {uns}/{n} |")
        md.append("")
    md.append("### Ranking stability (exact mode, all intents, all corpora)\n")
    md.append("| Arm | Identical on repeat | Identical after reopen |")
    md.append("|---|---|---|")
    for arm in arms:
        seen = {}
        for r in flat:
            if r["arm"] == arm:
                seen[r["corpus"]] = r["stability_exact_mode"]
        q = sum(s["queries"] for s in seen.values())
        rep = sum(s["identical_on_repeat"] for s in seen.values())
        reo = sum(s["identical_after_reopen"] for s in seen.values())
        md.append(f"| {arm} | {rep}/{q} | {reo}/{q} |")
    md.append("")


def perf(path, out_dir, md):
    doc = json.load(open(path))
    env = doc["environment"]
    allocator = env.get("allocator")
    rows = []

    def add(corpus, metric, adapter, values, unit):
        rows.append((corpus, metric, adapter, unit, values))

    for c in doc["corpora"]:
        name = c["corpus"]
        ph = c["phases"]
        adapters = sorted({a for p in ph.values() for a in p})
        for a in adapters:
            def runs(phase):
                return [r for r in ph.get(phase, {}).get(a, []) if "error" not in r]
            for phase in ["build-bulk", "build-incremental"]:
                rs = runs(phase)
                add(name, f"{phase} total", a, [get(r, "result", "total_ms") for r in rs], "ms")
                add(name, f"{phase} records/s", a, [get(r, "result", "records_per_sec") for r in rs], "rec/s")
                add(name, f"{phase} peak RSS", a, [get(r, "memory_end", "peak_rss_kib") / 1024 for r in rs], "MiB")
                for part in ["nodes_200_row_inserts", "bodies_single_row_inserts", "decisions_single_row_inserts"]:
                    add(name, f"{phase} {part}", a, [get(r, "result", "row_load_split_ms", part) for r in rs], "ms")
                if phase == "build-bulk":
                    add(name, "build-bulk fts build", a, [get(r, "result", "fts_build_ms") for r in rs], "ms")
                    add(name, "disk total", a, [get(r, "result", "disk_bytes") / 2**20 for r in rs], "MiB")
                    if a.startswith("turso"):
                        fts = [(get(r, "result", "disk_bytes") - get(r, "result", "disk_bytes_before_fts_build")) / 2**20 for r in rs]
                    else:
                        fts = [sum(get(r, "result", "fts_storage_bytes").values()) / 2**20 for r in rs]
                    add(name, "disk fts structures", a, fts, "MiB")
            for phase in ["startup-cold-cache", "startup-warm-cache"]:
                rs = runs(phase)
                add(name, f"{phase} open", a, [get(r, "result", "open_ms") for r in rs], "ms")
                add(name, f"{phase} first query", a, [get(r, "result", "first_query_ms") for r in rs], "ms")
                add(name, f"{phase} peak RSS", a, [get(r, "memory_end", "peak_rss_kib") / 1024 for r in rs], "MiB")
            rs = runs("query")
            cats = sorted({k for r in rs for k in get(r, "result", "categories") or {}})
            for cat in cats:
                for p in ["p50", "p95", "p99"]:
                    vals = [get(r, "result", "categories", cat, "latency_us", p) for r in rs]
                    add(name, f"query {cat} {p}", a, vals, "µs")
                uns = [get(r, "result", "categories", cat, "unsupported") for r in rs]
                if any(uns):
                    add(name, f"query {cat} unsupported", a, uns, "queries")
            add(name, "query repeated throughput", a, [get(r, "result", "categories", "repeated", "queries_per_sec") for r in rs], "q/s")
            add(name, "query steady-state RSS", a, [get(r, "result", "steady_state", "rss_kib") / 1024 for r in rs], "MiB")
            add(name, "query peak RSS", a, [get(r, "memory_end", "peak_rss_kib") / 1024 for r in rs], "MiB")
            rs = runs("update")
            add(name, "file edit txn p50", a, [get(r, "result", "file_edits", "latency_ms", "p50") for r in rs], "ms")
            add(name, "file edit txn p95", a, [get(r, "result", "file_edits", "latency_ms", "p95") for r in rs], "ms")
            add(name, "file edit rows/s", a, [get(r, "result", "file_edits", "rows_per_sec") for r in rs], "rows/s")
            add(name, "row update p50", a, [get(r, "result", "row_updates", "latency_ms", "p50") for r in rs], "ms")
            add(name, "file delete txn p50", a, [get(r, "result", "file_deletes", "latency_ms", "p50") for r in rs], "ms")
            add(name, "exact_name p50 after updates", a, [get(r, "result", "exact_name_after_updates_latency_us", "p50") for r in rs], "µs")
            add(name, "optimize nodes fts", a, [get(r, "result", "optimize_nodes_fts_ms") for r in rs], "ms")
            add(name, "exact_name p50 after optimize", a, [get(r, "result", "exact_name_after_optimize_latency_us", "p50") for r in rs], "µs")
            add(name, "disk after updates", a, [get(r, "result", "disk_bytes_after_updates") / 2**20 for r in rs], "MiB")
            for r in rs[:1]:
                fe = get(r, "result", "file_edits") or {}
                ru = get(r, "result", "row_updates") or {}
                fd = get(r, "result", "file_deletes") or {}
                add(name, "checks (added/renamed/updated/deleted)", a,
                    [f"{fe.get('check_added_symbols_found')} {fe.get('check_renamed_found')} {ru.get('check_new_text_found')} {fd.get('check_deleted_not_found')}"], "")
            rs = runs("concurrency")
            add(name, "concurrency 1-conn q/s", a, [get(r, "result", "single_connection_qps") for r in rs], "q/s")
            add(name, "concurrency 4-conn q/s", a, [get(r, "result", "multi_connection_qps") for r in rs], "q/s")
            for phase, p in ph.items():
                errs = [r["error"] for r in p.get(a, []) if "error" in r]
                if errs:
                    add(name, f"{phase} errors", a, [errs[0]], "")

    with open(os.path.join(out_dir, os.path.basename(path).replace(".json", ".csv")), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["corpus", "metric", "adapter", "unit", "median", "min", "max", "n", "values"])
        for corpus, metric, adapter, unit, vals in rows:
            num = [v for v in vals if isinstance(v, (int, float))]
            lo, hi = rng(num)
            w.writerow([corpus, metric, adapter, unit, median(num), lo, hi, len(vals), json.dumps(vals)])

    md.append(f"## Performance ({os.path.basename(path)}, {doc['repetitions']} repetitions, allocator: {allocator})\n")
    md.append("Cells: median [min–max] across repetitions. Ratio = turso / libsql of medians; `~` = ranges overlap.\n")
    corpora = list(dict.fromkeys(r[0] for r in rows))
    metrics = list(dict.fromkeys(r[1] for r in rows))
    for corpus in corpora:
        md.append(f"### {corpus}\n")
        adapters = sorted({r[2] for r in rows if r[0] == corpus})
        md.append("| Metric | Unit | " + " | ".join(adapters) + " | Ratio |")
        md.append("|---|---|" + "---|" * len(adapters) + "---|")
        for metric in metrics:
            vals = {r[2]: r[4] for r in rows if r[0] == corpus and r[1] == metric}
            if not vals:
                continue
            unit = next(r[3] for r in rows if r[0] == corpus and r[1] == metric)
            cells = []
            for a in adapters:
                v = vals.get(a, [])
                if v and isinstance(v[0], str):
                    cells.append(v[0])
                else:
                    cells.append(cell(v))
            rt = "-"
            if len(adapters) == 2 and all(vals.get(a) and not isinstance(vals[a][0], str) for a in adapters):
                rt = ratio(vals[adapters[0]], vals[adapters[1]])
            md.append(f"| {metric} | {unit} | " + " | ".join(cells) + f" | {rt} |")
        md.append("")


def main():
    out_dir = sys.argv[1] if len(sys.argv) > 1 else "results/fts"
    md = ["# FTS comparison: generated summary", "",
          "Generated by `scripts/summarize_fts.py` from the JSON files in this directory. Do not edit by hand.", ""]
    for p in sorted(glob.glob(os.path.join(out_dir, "functional-*.json"))):
        functional(p, out_dir, md)
    for p in sorted(glob.glob(os.path.join(out_dir, "relevance-*.json"))):
        relevance(p, out_dir, md)
    for p in sorted(glob.glob(os.path.join(out_dir, "perf-*.json"))):
        perf(p, out_dir, md)
    with open(os.path.join(out_dir, "summary.md"), "w") as f:
        f.write("\n".join(md) + "\n")


if __name__ == "__main__":
    main()
