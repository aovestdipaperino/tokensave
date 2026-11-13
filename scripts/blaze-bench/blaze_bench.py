#!/usr/bin/env python3
"""Local, opt-in comparison of Cargo and Blaze builds of TokenSave (#984).

This harness is for one developer's machine only. It is not part of CI, the
release workflow or the standard contributor workflow, and Cargo stays the
source of truth for validation. See README.md beside this file and
docs/BLAZE-EVALUATION.md.

For each tool it makes a disposable clone of the repository at a pinned
revision, then times a cold build, a no-op build, a series of one-function
edits, and an integration test subset. Each step records wall time, CPU time
(the command and, for Blaze, its daemon), peak memory and disk usage.

Linux only (it reads /proc). Python 3.9+, standard library only.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import threading
import time
from pathlib import Path

DEFAULT_EDIT_FILE = "src/tokensave/staleness.rs"
DEFAULT_EDIT_ANCHOR = "pub async fn find_stale_files(&self) -> Vec<String> {"
EDIT_MARKER = "_blaze_bench_edit"
# Process names that belong to Blaze's daemon and helpers, which outlive the
# command that started them and so are not counted by its rusage.
BLAZE_PROCESS_PREFIXES = ("blaze", "ra-jit")
SAMPLE_INTERVAL_S = 0.2
TOOLS = ("cargo", "blaze")


# ----------------------------------------------------------------------------
# Pure helpers (unit-tested)


def apply_edit(text: str, anchor: str, counter: int) -> str:
    """Returns `text` with a one-line body change after `anchor`.

    The inserted statement differs per `counter`, so each edit changes the
    function body without changing any signature. A previous benchmark edit
    after the anchor is replaced rather than stacked.
    """
    index = text.find(anchor)
    if index < 0:
        raise ValueError(f"edit anchor not found: {anchor!r}")
    if text.find(anchor, index + 1) >= 0:
        raise ValueError(f"edit anchor is not unique: {anchor!r}")
    head = text[: index + len(anchor)]
    tail = text[index + len(anchor) :]
    statement = (
        f"\n        let {EDIT_MARKER}: u64 = std::hint::black_box({counter});"
    )
    previous = re.match(
        r"\n[ \t]*let " + EDIT_MARKER + r": u64 = std::hint::black_box\(\d+\);",
        tail,
    )
    if previous:
        tail = tail[previous.end() :]
    return head + statement + tail


def summarize(values: list[float]) -> dict[str, float] | None:
    """Returns median, min and max of `values`, or None when empty."""
    if not values:
        return None
    return {
        "median": statistics.median(values),
        "min": min(values),
        "max": max(values),
        "n": len(values),
    }


def format_seconds(value: float | None) -> str:
    if value is None:
        return "n/a"
    if value < 1:
        return f"{value * 1000:.0f} ms"
    return f"{value:.2f} s"


def format_mib(value: float | None) -> str:
    if value is None:
        return "n/a"
    if value >= 1024:
        return f"{value / 1024:.2f} GiB"
    return f"{value:.0f} MiB"


def step_summary(steps: list[dict], name: str, field: str) -> dict | None:
    values = [s[field] for s in steps if s["name"] == name and s["ok"]]
    values = [v for v in values if v is not None]
    return summarize(values)


def render_markdown(result: dict) -> str:
    """Renders a results dictionary as a Markdown report."""
    env = result["environment"]
    lines = [
        "# Cargo vs Blaze measurements",
        "",
        f"- Revision: `{result['revision']}`",
        f"- Features: `{result['features']}`",
        f"- Machine: {env['cpus']} CPUs, {format_mib(env['mem_total_mib'])} RAM, "
        f"{env['kernel']}",
        f"- Cargo: `{env.get('cargo_version', 'n/a')}`",
        f"- Blaze: `{env.get('blaze_version', 'n/a')}`",
        "",
        "Median (min-max) over the successful runs of each step. CPU is user+system "
        "time of the command and, for Blaze, the change in its daemon's CPU time. "
        "Peak memory is the highest sampled RSS of the command's process tree plus "
        "Blaze's daemon processes.",
        "",
        "| Step | Tool | Runs ok | Wall | CPU | Peak memory |",
        "|---|---|---:|---:|---:|---:|",
    ]
    names: list[str] = []
    for tool in result["tools"]:
        for step in result["tools"][tool]["steps"]:
            if step["name"] not in names:
                names.append(step["name"])
    for name in names:
        for tool, data in result["tools"].items():
            steps = [s for s in data["steps"] if s["name"] == name]
            if not steps:
                continue
            ok = sum(1 for s in steps if s["ok"])
            wall = step_summary(steps, name, "wall_s")
            cpu = step_summary(steps, name, "cpu_s")
            mem = step_summary(steps, name, "peak_rss_mib")

            def cell(summary, fmt):
                if summary is None:
                    return "failed"
                if summary["n"] == 1:
                    return fmt(summary["median"])
                return (
                    f"{fmt(summary['median'])} "
                    f"({fmt(summary['min'])}-{fmt(summary['max'])})"
                )

            lines.append(
                f"| {name} | {tool} | {ok}/{len(steps)} | "
                f"{cell(wall, format_seconds)} | {cell(cpu, format_seconds)} | "
                f"{cell(mem, format_mib)} |"
            )
    lines += ["", "| Tool | Target dir after cold build | Target dir at end | Tool home growth |",
              "|---|---:|---:|---:|"]
    for tool, data in result["tools"].items():
        disk = data.get("disk", {})
        lines.append(
            f"| {tool} | {format_mib(disk.get('target_after_cold_mib'))} | "
            f"{format_mib(disk.get('target_at_end_mib'))} | "
            f"{format_mib(disk.get('tool_home_growth_mib'))} |"
        )
    failures = [
        (tool, s)
        for tool, data in result["tools"].items()
        for s in data["steps"]
        if not s["ok"]
    ]
    if failures:
        lines += ["", "## Failed steps", ""]
        for tool, step in failures:
            lines.append(
                f"- {tool} `{step['name']}` exit {step['exit_code']}: "
                f"`{step.get('error_tail', '').strip()[:300]}`"
            )
    status = result["tools"].get("blaze", {}).get("status")
    if status:
        lines += ["", "## `blaze status` at the end", "", "```text", status.strip(), "```"]
    return "\n".join(lines) + "\n"


# ----------------------------------------------------------------------------
# Measurement (Linux /proc)


def _read_proc_stat(pid: int) -> tuple[str, int, float] | None:
    """Returns (comm, ppid, cpu seconds) for `pid`, or None if it is gone."""
    try:
        raw = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return None
    close = raw.rfind(")")
    comm = raw[raw.find("(") + 1 : close]
    fields = raw[close + 2 :].split()
    ticks = os.sysconf("SC_CLK_TCK")
    return comm, int(fields[1]), (int(fields[11]) + int(fields[12])) / ticks


def _rss_mib(pid: int) -> float:
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) / 1024
    except OSError:
        pass
    return 0.0


def _process_table() -> dict[int, tuple[str, int, float]]:
    table = {}
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            stat = _read_proc_stat(int(entry))
            if stat is not None:
                table[int(entry)] = stat
    return table


def _is_blaze(comm: str) -> bool:
    return comm.startswith(BLAZE_PROCESS_PREFIXES)


def blaze_daemon_cpu_s(exclude_tree_of: int | None = None) -> float:
    """CPU seconds of running Blaze processes outside a given process tree."""
    table = _process_table()
    excluded = _descendants(table, exclude_tree_of) if exclude_tree_of else set()
    return sum(cpu for pid, (comm, _, cpu) in table.items()
               if _is_blaze(comm) and pid not in excluded)


def _descendants(table: dict[int, tuple[str, int, float]], root: int) -> set[int]:
    children: dict[int, list[int]] = {}
    for pid, (_, ppid, _) in table.items():
        children.setdefault(ppid, []).append(pid)
    seen, stack = set(), [root]
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        stack.extend(children.get(pid, []))
    return seen


class PeakSampler:
    """Samples the RSS of a process tree plus Blaze processes until stopped."""

    def __init__(self, root_pid: int):
        self.root_pid = root_pid
        self.peak_mib = 0.0
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, daemon=True)

    def __enter__(self):
        self._thread.start()
        return self

    def __exit__(self, *exc):
        self._stop.set()
        self._thread.join()

    def _run(self):
        while not self._stop.is_set():
            table = _process_table()
            pids = _descendants(table, self.root_pid)
            pids |= {pid for pid, (comm, _, _) in table.items() if _is_blaze(comm)}
            total = sum(_rss_mib(pid) for pid in pids)
            self.peak_mib = max(self.peak_mib, total)
            self._stop.wait(SAMPLE_INTERVAL_S)


def dir_size_mib(path: Path) -> float | None:
    if not path.exists():
        return None
    out = subprocess.run(["du", "-sk", str(path)], capture_output=True, text=True,
                         check=False)
    if out.returncode != 0 or not out.stdout:
        return None
    return int(out.stdout.split()[0]) / 1024


def child_env() -> dict[str, str]:
    """A clean environment: Linux toolchains only, no TokenSave overrides.

    Under WSL the Windows PATH is appended to Linux's, and a Windows
    `cargo.exe` there could shadow the Linux one, so PATH is rebuilt.
    """
    home = Path.home()
    env = {k: v for k, v in os.environ.items() if not k.startswith("TOKENSAVE_")}
    env["PATH"] = os.pathsep.join([
        str(home / ".cargo" / "bin"), "/usr/local/bin", "/usr/bin", "/bin",
    ])
    env.pop("RUSTFLAGS", None)
    env.pop("CARGO_TARGET_DIR", None)
    env["CARGO_TERM_COLOR"] = "never"
    return env


def run_step(name: str, argv: list[str], cwd: Path, env: dict[str, str],
             log_dir: Path, tool: str, index: int, timeout_s: int) -> dict:
    log_path = log_dir / f"{tool}-{index:02d}-{name}.log"
    daemon_before = blaze_daemon_cpu_s() if tool == "blaze" else 0.0
    start = time.monotonic()
    with open(log_path, "w", encoding="utf-8") as log:
        log.write("$ " + " ".join(argv) + "\n")
        log.flush()
        proc = subprocess.Popen(argv, cwd=cwd, env=env, stdout=log,
                                stderr=subprocess.STDOUT)
        timed_out = False
        with PeakSampler(proc.pid) as sampler:
            deadline = start + timeout_s
            while True:
                pid, status, usage = os.wait4(proc.pid, os.WNOHANG)
                if pid:
                    break
                if time.monotonic() > deadline:
                    proc.kill()
                    timed_out = True
                time.sleep(0.05)
    wall = time.monotonic() - start
    exit_code = os.waitstatus_to_exitcode(status)
    proc.returncode = exit_code
    cpu = usage.ru_utime + usage.ru_stime
    if tool == "blaze":
        cpu += max(0.0, blaze_daemon_cpu_s() - daemon_before)
    tail = ""
    if exit_code != 0:
        tail = "\n".join(log_path.read_text(errors="replace").splitlines()[-15:])
    return {
        "name": name,
        "argv": argv,
        "ok": exit_code == 0 and not timed_out,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "wall_s": round(wall, 3),
        "cpu_s": round(cpu, 3),
        "peak_rss_mib": round(max(sampler.peak_mib, usage.ru_maxrss / 1024), 1),
        "log": str(log_path),
        "error_tail": tail,
    }


def capture(argv: list[str], cwd: Path | None, env: dict[str, str]) -> str:
    try:
        out = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True,
                             timeout=120, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        return f"<{error}>"
    return (out.stdout + out.stderr).strip()


# ----------------------------------------------------------------------------
# Benchmark plan


def feature_args(features: str) -> list[str]:
    if features == "default":
        return []
    return ["--no-default-features", "--features", features]


def bench_tool(tool: str, args, tree: Path, env: dict[str, str], log_dir: Path) -> dict:
    features = feature_args(args.features)
    build = [tool, "build", "--locked", *features]
    test = [tool, "test", "--locked", *features, "--test", "integration",
            args.test_filter]
    edit_path = tree / args.edit_file
    original = edit_path.read_text(encoding="utf-8")
    steps: list[dict] = []
    index = 0
    tool_home = Path.home() / (".blaze" if tool == "blaze" else ".cargo")
    home_before = dir_size_mib(tool_home)
    disk: dict[str, float | None] = {}

    def step(name, argv):
        nonlocal index
        index += 1
        result = run_step(name, argv, tree, env, log_dir, tool, index, args.timeout)
        steps.append(result)
        state = "ok" if result["ok"] else f"FAILED (exit {result['exit_code']})"
        print(f"  [{tool}] {name}: {format_seconds(result['wall_s'])} {state}",
              flush=True)
        return result

    def edit(counter):
        edit_path.write_text(apply_edit(original, args.edit_anchor, counter),
                             encoding="utf-8")

    try:
        for cold in range(args.cold_runs):
            if tool == "blaze":
                capture(["blaze", "stop"], tree, env)
            shutil.rmtree(tree / "target", ignore_errors=True)
            step("cold-build", build)
            if cold == 0:
                disk["target_after_cold_mib"] = dir_size_mib(tree / "target")
        if not any(s["ok"] for s in steps):
            return {"steps": steps, "disk": disk,
                    "status": capture(["blaze", "status"], tree, env)
                    if tool == "blaze" else None}
        step("noop-build", build)
        for counter in range(1, args.edits + 1):
            edit(counter)
            step("edit-build", build)
        edit_path.write_text(original, encoding="utf-8")
        if not args.skip_tests:
            step("test-first", test)
            for counter in range(1, args.test_edits + 1):
                edit(1000 + counter)
                step("edit-test", test)
    finally:
        edit_path.write_text(original, encoding="utf-8")
        disk["target_at_end_mib"] = dir_size_mib(tree / "target")
        home_after = dir_size_mib(tool_home)
        if home_before is not None and home_after is not None:
            disk["tool_home_growth_mib"] = round(home_after - home_before, 1)
    status = None
    if tool == "blaze":
        status = capture(["blaze", "status"], tree, env)
        if not args.keep_daemon:
            capture(["blaze", "stop"], tree, env)
    return {"steps": steps, "disk": disk, "status": status}


def prepare_tree(source: str, revision: str, work: Path, tool: str,
                 env: dict[str, str]) -> Path:
    tree = work / f"tree-{tool}"
    if not (tree / ".git").exists():
        subprocess.run(["git", "clone", "--quiet", "--no-checkout", source, str(tree)],
                       check=True, env=env)
    subprocess.run(["git", "-C", str(tree), "fetch", "--quiet", "origin", revision],
                   check=False, env=env)
    subprocess.run(["git", "-C", str(tree), "checkout", "--quiet", "--force",
                    revision], check=True, env=env)
    subprocess.run(["git", "-C", str(tree), "clean", "-fdxq", "-e", "target"],
                   check=True, env=env)
    return tree


def environment(env: dict[str, str], tree: Path) -> dict:
    mem_total = 0.0
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemTotal:"):
            mem_total = int(line.split()[1]) / 1024
    info = {
        "cpus": os.cpu_count(),
        "mem_total_mib": round(mem_total),
        "kernel": platform.release(),
        "python": platform.python_version(),
        "cargo_version": capture(["cargo", "--version"], tree, env),
        "rustc_version": capture(["rustc", "--version"], tree, env),
        "cc_version": capture(["cc", "--version"], tree, env).splitlines()[0:1],
    }
    if shutil.which("blaze", path=env["PATH"]):
        info["blaze_version"] = capture(["blaze", "--version"], tree, env)
    return info


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--source", required=True,
                        help="repository URL or local path to clone")
    parser.add_argument("--rev", default="HEAD", help="revision to check out")
    parser.add_argument("--work", required=True, type=Path,
                        help="disposable directory for the clones and logs")
    parser.add_argument("--tools", default="cargo,blaze",
                        help="comma-separated subset of cargo,blaze")
    parser.add_argument("--features", default="lite",
                        help="lite, medium, full, or 'default' (default: lite)")
    parser.add_argument("--cold-runs", type=int, default=1)
    parser.add_argument("--edits", type=int, default=5)
    parser.add_argument("--test-edits", type=int, default=2)
    parser.add_argument("--test-filter", default="sync",
                        help="test name filter for the integration subset")
    parser.add_argument("--skip-tests", action="store_true")
    parser.add_argument("--edit-file", default=DEFAULT_EDIT_FILE)
    parser.add_argument("--edit-anchor", default=DEFAULT_EDIT_ANCHOR)
    parser.add_argument("--timeout", type=int, default=3600,
                        help="per-step timeout in seconds")
    parser.add_argument("--keep-daemon", action="store_true",
                        help="leave the Blaze daemon running at the end")
    parser.add_argument("--output", type=Path, help="JSON results file")
    parser.add_argument("--markdown", type=Path, help="Markdown report file")
    args = parser.parse_args(argv)

    if sys.platform != "linux":
        parser.error("this harness reads /proc and runs on Linux (or WSL) only")
    tools = [t.strip() for t in args.tools.split(",") if t.strip()]
    unknown = [t for t in tools if t not in TOOLS]
    if unknown:
        parser.error(f"unknown tools: {', '.join(unknown)}")

    env = child_env()
    for tool in tools:
        if not shutil.which(tool, path=env["PATH"]):
            parser.error(f"{tool} is not on PATH {env['PATH']}")
    args.work.mkdir(parents=True, exist_ok=True)
    log_dir = args.work / "logs"
    log_dir.mkdir(exist_ok=True)

    result = {"revision": None, "features": args.features, "tools": {},
              "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    for tool in tools:
        tree = prepare_tree(args.source, args.rev, args.work, tool, env)
        # Downloads are not part of a build's time: fetch the registry first.
        subprocess.run(["cargo", "fetch", "--locked"], cwd=tree, env=env, check=True)
        if result["revision"] is None:
            result["revision"] = capture(["git", "rev-parse", "HEAD"], tree, env)
            result["environment"] = environment(env, tree)
        print(f"{tool}: {tree}", flush=True)
        result["tools"][tool] = bench_tool(tool, args, tree, env, log_dir)

    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    report = render_markdown(result)
    if args.markdown:
        args.markdown.write_text(report, encoding="utf-8")
    print(report)
    return 0


if __name__ == "__main__":
    sys.exit(main())
