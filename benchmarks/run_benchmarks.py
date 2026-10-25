"""Tokensave vs Token-Savior — side-by-side benchmark on Python repos.

Adapted from token-savior's benchmarks/run_benchmarks.py
(https://github.com/Mibayy/token-savior). Runs both tools against the same
clone of FastAPI and CPython, sharing a single random symbol sample so the
query-timing column is apples-to-apples, and emits a comparison report.

Prerequisites:
    pip install token-savior        # for the token-savior column
    tokensave --version             # tokensave CLI on PATH

Usage:
    python benchmarks/run_benchmarks.py
    python benchmarks/run_benchmarks.py --repos fastapi
    python benchmarks/run_benchmarks.py --skip-clone
    python benchmarks/run_benchmarks.py --skip-token-savior   # tokensave only
    python benchmarks/run_benchmarks.py --skip-tokensave      # token-savior only
"""

from __future__ import annotations

import argparse
import json
import os
import random
import resource
import shutil
import subprocess
import sys
import time
import tracemalloc
from pathlib import Path

REPOS: dict[str, str] = {
    "fastapi": "https://github.com/tiangolo/fastapi.git",
    "cpython": "https://github.com/python/cpython.git",
}

CLONE_DIR = Path("/tmp/tokensave-bench")
RESULTS_DIR = Path(__file__).resolve().parent
RESULTS_JSON = RESULTS_DIR / "comparison-results.json"
REPORT_MD = RESULTS_DIR / "comparison-report.md"

RANDOM_SEED = 42
NUM_QUERY_SAMPLES = 10

TOKENSAVE_BIN = os.environ.get("TOKENSAVE_BIN", "tokensave")


def log(msg: str) -> None:
    print(f"[bench] {msg}", file=sys.stderr, flush=True)


# ---------------------------------------------------------------------------
# Clone + naive baseline
# ---------------------------------------------------------------------------


def clone_repo(name: str, url: str, skip_clone: bool) -> Path:
    dest = CLONE_DIR / name
    if dest.exists():
        log(f"{name}: reusing existing clone at {dest}")
        return dest
    CLONE_DIR.mkdir(parents=True, exist_ok=True)
    log(f"{name}: cloning {url} (shallow, depth=1) ...")
    subprocess.run(
        ["git", "clone", "--depth=1", "--single-branch", url, str(dest)],
        check=True,
        capture_output=True,
        text=True,
    )
    return dest


def naive_source_size(root: str) -> int:
    total = 0
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            if f.endswith(".py"):
                try:
                    total += os.path.getsize(os.path.join(dirpath, f))
                except OSError:
                    pass
    return total


# ---------------------------------------------------------------------------
# token-savior side (in-process)
# ---------------------------------------------------------------------------


def have_token_savior() -> bool:
    try:
        import token_savior  # noqa: F401
        return True
    except ImportError:
        return False


def benchmark_token_savior(name: str, root: Path) -> tuple[dict, list[str]]:
    from token_savior.cache_ops import CacheManager
    from token_savior.project_indexer import ProjectIndexer
    from token_savior.query_api import create_project_query_functions

    root_str = str(root)
    out: dict = {}

    log(f"[ts] {name}: cold index ...")
    tracemalloc.start()
    t0 = time.perf_counter()
    indexer = ProjectIndexer(root_str)
    index = indexer.index()
    out["cold_index_seconds"] = round(time.perf_counter() - t0, 3)
    out["cold_index_peak_memory_bytes"] = tracemalloc.get_traced_memory()[1]
    tracemalloc.stop()
    out["total_files"] = index.total_files
    out["total_lines"] = index.total_lines
    out["total_functions"] = index.total_functions
    out["total_classes"] = index.total_classes
    out["symbol_table_size"] = len(index.symbol_table)

    log(f"[ts] {name}: warm index ...")
    t0 = time.perf_counter()
    ProjectIndexer(root_str).index()
    out["warm_index_seconds"] = round(time.perf_counter() - t0, 3)

    queries = create_project_query_functions(index)
    rng = random.Random(RANDOM_SEED)
    symbols = list(index.symbol_table.keys())
    sample = rng.sample(symbols, min(NUM_QUERY_SAMPLES, len(symbols)))

    def avg_ms(fn, items: list[str]) -> float | None:
        times: list[float] = []
        for it in items:
            t = time.perf_counter()
            fn(it)
            times.append(time.perf_counter() - t)
        return round(sum(times) / len(times) * 1000, 3) if times else None

    out["find_symbol_avg_ms"] = avg_ms(queries["find_symbol"], sample)
    out["get_function_source_avg_ms"] = avg_ms(queries["get_function_source"], sample)
    impact_sample = [s for s in sample if s in index.reverse_dependency_graph]
    out["get_change_impact_avg_ms"] = (
        avg_ms(queries["get_change_impact"], impact_sample) if impact_sample else None
    )

    cache = CacheManager(root_path=root_str, cache_version=1)
    cache.save(index)
    cp = cache.path()
    out["cache_size_bytes"] = os.path.getsize(cp) if os.path.exists(cp) else 0
    try:
        os.remove(cp)
    except OSError:
        pass

    return out, sample


# ---------------------------------------------------------------------------
# tokensave side (subprocess)
# ---------------------------------------------------------------------------


def ru_maxrss_bytes() -> int:
    """ru_maxrss is bytes on macOS, KiB on Linux."""
    r = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    return r if sys.platform == "darwin" else r * 1024


def run_timed(cmd: list[str], cwd: str | None = None) -> tuple[float, int, subprocess.CompletedProcess]:
    before = ru_maxrss_bytes()
    t0 = time.perf_counter()
    cp = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    dt = time.perf_counter() - t0
    after = ru_maxrss_bytes()
    return dt, max(0, after - before), cp


class TokensaveMcp:
    """Minimal JSON-RPC 2.0 client for `tokensave serve` over stdio.

    Spawns a long-lived server so per-call latency reflects actual query work
    instead of process startup + DB open. Newline-delimited JSON, no Content-
    Length framing — see src/mcp/transport.rs.
    """

    def __init__(self, root: Path):
        self.root = root
        self.proc: subprocess.Popen | None = None
        self._next_id = 1

    def __enter__(self) -> "TokensaveMcp":
        self.proc = subprocess.Popen(
            [TOKENSAVE_BIN, "serve", "-p", str(self.root), "--timings"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        self._request("initialize", {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "tokensave-bench", "version": "0.1"},
        })
        self._notify("notifications/initialized", {})
        return self

    def __exit__(self, *_exc) -> None:
        if self.proc is None:
            return
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.proc = None

    def _write(self, payload: dict) -> None:
        assert self.proc and self.proc.stdin
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()

    def _read_response(self, expected_id: int) -> dict:
        assert self.proc and self.proc.stdout
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("tokensave serve closed stdout unexpectedly")
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("id") == expected_id:
                return msg

    def _request(self, method: str, params: dict) -> dict:
        rid = self._next_id
        self._next_id += 1
        self._write({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        return self._read_response(rid)

    def _notify(self, method: str, params: dict) -> None:
        self._write({"jsonrpc": "2.0", "method": method, "params": params})

    def call_tool(self, name: str, arguments: dict) -> dict:
        return self._request("tools/call", {"name": name, "arguments": arguments})

    @staticmethod
    def first_node_id(search_resp: dict) -> str | None:
        """Extract the first node id from a tokensave_search response."""
        try:
            text = search_resp["result"]["content"][0]["text"]
            items = json.loads(text)
            if isinstance(items, list) and items:
                return items[0].get("id")
        except (KeyError, IndexError, TypeError, json.JSONDecodeError):
            return None
        return None


def benchmark_tokensave(name: str, root: Path, sample_symbols: list[str]) -> dict:
    out: dict = {}
    ts_dir = root / ".tokensave"
    if ts_dir.exists():
        shutil.rmtree(ts_dir)

    log(f"[tk] {name}: init (cold) ...")
    dt, mem, cp = run_timed([TOKENSAVE_BIN, "init"], cwd=str(root))
    if cp.returncode != 0:
        log(f"[tk] init failed: {cp.stderr[-400:]}")
        out["error"] = cp.stderr[-2000:]
        return out
    out["cold_index_seconds"] = round(dt, 3)
    out["cold_index_peak_memory_bytes"] = mem

    log(f"[tk] {name}: sync (warm) ...")
    dt, _, cp = run_timed([TOKENSAVE_BIN, "sync"], cwd=str(root))
    out["warm_index_seconds"] = round(dt, 3) if cp.returncode == 0 else None

    _, _, cp = run_timed([TOKENSAVE_BIN, "status", "--json"], cwd=str(root))
    if cp.returncode == 0:
        try:
            status = json.loads(cp.stdout)
            out["node_count"] = status.get("node_count")
            out["edge_count"] = status.get("edge_count")
            out["file_count"] = status.get("file_count")
            out["files_by_language"] = status.get("files_by_language")
        except json.JSONDecodeError:
            pass

    log(f"[tk] {name}: query benchmarks via MCP ({len(sample_symbols)} symbols) ...")
    queries = [sym.split(".")[-1] for sym in sample_symbols]

    def handler_us(resp: dict) -> int | None:
        meta = resp.get("result", {}).get("_meta", {})
        v = meta.get("duration_us")
        return int(v) if isinstance(v, (int, float)) else None

    def avg_ms(samples: list[int]) -> float | None:
        return round(sum(samples) / len(samples) / 1000.0, 3) if samples else None

    with TokensaveMcp(root) as mcp:
        # Warm-up so first-query DB/cache fill doesn't skew the averages.
        # `_meta.duration_us` already excludes transport overhead, but the
        # first call also pays one-time index warmup we want to drop.
        if queries:
            mcp.call_tool("tokensave_search", {"query": queries[0], "limit": 5})

        # Apples-to-apples vs token-savior's find_symbol: bare-name index
        # probe via `tokensave_find_exact_symbol`, not BM25-ranked `search`.
        find_us = [
            d
            for q in queries
            if (
                d := handler_us(
                    mcp.call_tool("tokensave_find_exact_symbol", {"name": q})
                )
            )
            is not None
        ]
        out["find_symbol_avg_ms"] = avg_ms(find_us)

        body_us = [
            d
            for q in queries
            if (d := handler_us(mcp.call_tool("tokensave_body", {"symbol": q, "limit": 1})))
            is not None
        ]
        out["get_function_source_avg_ms"] = avg_ms(body_us)

        impact_us: list[int] = []
        for q in queries:
            sresp = mcp.call_tool("tokensave_search", {"query": q, "limit": 1})
            search_us = handler_us(sresp) or 0
            node_id = TokensaveMcp.first_node_id(sresp)
            if node_id is None:
                continue
            iresp = mcp.call_tool("tokensave_impact", {"node_id": node_id, "max_depth": 3})
            ius = handler_us(iresp)
            if ius is not None:
                impact_us.append(search_us + ius)
        out["get_change_impact_avg_ms"] = avg_ms(impact_us)

    db = ts_dir / "tokensave.db"
    out["cache_size_bytes"] = os.path.getsize(db) if db.exists() else 0

    return out


def fallback_sample(root: Path) -> list[str]:
    """If token-savior didn't run, derive a symbol-ish sample from .py basenames."""
    cands: list[str] = []
    for dirpath, _, files in os.walk(root):
        for f in files:
            if f.endswith(".py") and not f.startswith("_"):
                cands.append(f[:-3])
    if not cands:
        return []
    rng = random.Random(RANDOM_SEED)
    return rng.sample(cands, min(NUM_QUERY_SAMPLES, len(cands)))


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------


def fmt_bytes(b: int | None) -> str:
    if b is None:
        return "N/A"
    if b >= 1_048_576:
        return f"{b / 1_048_576:.1f} MiB"
    if b >= 1024:
        return f"{b / 1024:.1f} KiB"
    return f"{b} B"


def fmt_ms(v: float | None) -> str:
    return "N/A" if v is None else f"{v:.3f} ms"


def fmt_sec(v: float | None) -> str:
    return "N/A" if v is None else f"{v:.3f}s"


def fmt_int(v) -> str:
    if v is None:
        return "N/A"
    try:
        return f"{int(v):,}"
    except (TypeError, ValueError):
        return str(v)


def generate_report(results: list[dict], naive_sizes: dict[str, int]) -> str:
    lines = [
        "# Tokensave vs Token-Savior Benchmark",
        "",
        f"Generated: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}",
        "",
        "Side-by-side comparison on real-world Python repositories. Both tools index",
        "the same clone, and the same random sample of symbols (seed=42) is used for",
        "query timing so the find_symbol column is directly comparable.",
        "",
        "**Memory notes.** token-savior's peak memory is measured with `tracemalloc`",
        "(Python heap only). tokensave runs as a subprocess, so its peak is the",
        "`ru_maxrss` delta from `getrusage(RUSAGE_CHILDREN)` (resident set size).",
        "These are *not* identical units — treat them as order-of-magnitude.",
        "",
        "**Query timing.** token-savior is called in-process (pure Python dict",
        "lookups). tokensave is driven over MCP via `tokensave serve --timings`",
        "and the per-query column reports the handler's `_meta.duration_us` —",
        "i.e. the time spent inside the Rust handler, with JSON-RPC / stdio /",
        "Python-parse overhead stripped out. A warm-up call is issued before",
        "each timed loop. `get_change_impact` for tokensave sums the handler",
        "times of `search → impact`, mirroring how an agent must resolve the",
        "symbol to a node_id before querying.",
        "",
    ]
    for r in results:
        name = r["repo"]
        ts = r.get("token_savior") or {}
        tk = r.get("tokensave") or {}
        lines += [
            f"## {name}",
            "",
            f"Naive `.py` source size: {fmt_bytes(naive_sizes.get(name))}",
            "",
            "| Metric | token-savior | tokensave |",
            "|--------|--------------|-----------|",
            f"| Cold index time | {fmt_sec(ts.get('cold_index_seconds'))} | {fmt_sec(tk.get('cold_index_seconds'))} |",
            f"| Warm reindex time | {fmt_sec(ts.get('warm_index_seconds'))} | {fmt_sec(tk.get('warm_index_seconds'))} |",
            f"| Peak memory (cold) | {fmt_bytes(ts.get('cold_index_peak_memory_bytes'))} | {fmt_bytes(tk.get('cold_index_peak_memory_bytes'))} |",
            f"| Cache / DB size | {fmt_bytes(ts.get('cache_size_bytes'))} | {fmt_bytes(tk.get('cache_size_bytes'))} |",
            f"| Files indexed | {fmt_int(ts.get('total_files'))} | {fmt_int(tk.get('file_count'))} |",
            f"| Symbols / nodes | {fmt_int(ts.get('symbol_table_size'))} | {fmt_int(tk.get('node_count'))} |",
            f"| find_symbol avg | {fmt_ms(ts.get('find_symbol_avg_ms'))} | {fmt_ms(tk.get('find_symbol_avg_ms'))} |",
            f"| get_function_source avg | {fmt_ms(ts.get('get_function_source_avg_ms'))} | {fmt_ms(tk.get('get_function_source_avg_ms'))} |",
            f"| get_change_impact avg | {fmt_ms(ts.get('get_change_impact_avg_ms'))} | {fmt_ms(tk.get('get_change_impact_avg_ms'))} |",
            "",
        ]
        if tk.get("files_by_language"):
            langs = ", ".join(f"{k}={v}" for k, v in sorted(tk["files_by_language"].items()))
            lines += [f"_tokensave indexed languages:_ {langs}", ""]
        if tk.get("error"):
            lines += [f"_tokensave error:_ `{tk['error'][:300]}`", ""]
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(description="Tokensave vs Token-Savior benchmark")
    parser.add_argument(
        "--repos",
        nargs="+",
        choices=list(REPOS.keys()),
        default=list(REPOS.keys()),
    )
    parser.add_argument("--skip-clone", action="store_true")
    parser.add_argument("--skip-token-savior", action="store_true")
    parser.add_argument("--skip-tokensave", action="store_true")
    args = parser.parse_args()

    if not args.skip_token_savior and not have_token_savior():
        log("token-savior not installed (`pip install token-savior`); skipping its column")
        args.skip_token_savior = True
    if not args.skip_tokensave and shutil.which(TOKENSAVE_BIN) is None:
        log(f"{TOKENSAVE_BIN} not on PATH; skipping its column")
        args.skip_tokensave = True
    if args.skip_token_savior and args.skip_tokensave:
        log("Both tools unavailable; nothing to benchmark.")
        sys.exit(1)

    results: list[dict] = []
    naive_sizes: dict[str, int] = {}

    for name in args.repos:
        try:
            root = clone_repo(name, REPOS[name], skip_clone=args.skip_clone)
        except subprocess.CalledProcessError as exc:
            log(f"SKIP {name}: clone failed -- {exc.stderr.strip()[:200]}")
            continue

        naive_sizes[name] = naive_source_size(str(root))
        entry: dict = {"repo": name, "root": str(root)}
        sample: list[str] | None = None

        if not args.skip_token_savior:
            ts_result, sample = benchmark_token_savior(name, root)
            entry["token_savior"] = ts_result

        if not args.skip_tokensave:
            if not sample:
                sample = fallback_sample(root)
            entry["tokensave"] = benchmark_tokensave(name, root, sample)

        results.append(entry)

    if not results:
        log("No repos benchmarked, exiting.")
        sys.exit(1)

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    payload = {"results": results, "naive_source_size_bytes": naive_sizes}
    RESULTS_JSON.write_text(json.dumps(payload, indent=2))
    log(f"Results written to {RESULTS_JSON}")

    report = generate_report(results, naive_sizes)
    REPORT_MD.write_text(report)
    log(f"Report written to {REPORT_MD}")
    print(report)


if __name__ == "__main__":
    main()
