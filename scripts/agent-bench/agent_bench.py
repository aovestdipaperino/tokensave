#!/usr/bin/env python3
"""End-to-end agent workflow benchmark for TokenSave (#948).

    agent_bench.py prepare --work DIR [--repo-source NAME=PATH_OR_URL]
    agent_bench.py run     --work DIR --model MODEL [--arms a,b] [--tasks x,y]
                           [--repetitions N] [--seed S] [--warmup]
    agent_bench.py report  --work DIR [--out report.md] [--json]

`prepare` builds pinned, disposable checkouts (one unindexed, one indexed
per repository). `run` drives the agent headless once per (task, arm,
repetition) in a seeded random order, checks each answer independently and
appends one metrics record per trial to DIR/results.jsonl. `report`
aggregates those records. See README.md for privacy and limitations.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import queue
import random
import re
import secrets
import shutil
import subprocess
import sys
import threading
import time
import uuid
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import profiling  # noqa: E402
import report as report_mod  # noqa: E402
import agent_trace as trace  # noqa: E402

RESULTS_SCHEMA = 1
SNAPSHOT_TAG = "bench-snapshot"
GIT_ENV = {
    "GIT_AUTHOR_NAME": "agent-bench",
    "GIT_AUTHOR_EMAIL": "agent-bench@invalid",
    "GIT_COMMITTER_NAME": "agent-bench",
    "GIT_COMMITTER_EMAIL": "agent-bench@invalid",
    "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z",
    "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z",
}


def die(msg: str) -> None:
    print(f"agent_bench: {msg}", file=sys.stderr)
    sys.exit(2)


def load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as fh:
        return json.load(fh)


def git(cwd: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    env = {**os.environ, **GIT_ENV, "GIT_CONFIG_NOSYSTEM": "1"}
    return subprocess.run(
        ["git", "-c", "core.hooksPath=", "-c", "commit.gpgsign=false", *args],
        cwd=cwd,
        env=env,
        check=check,
        capture_output=True,
        text=True,
    )


def sha256_file(path: Path) -> str | None:
    try:
        digest = hashlib.sha256()
        with path.open("rb") as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b""):
                digest.update(chunk)
        return digest.hexdigest()
    except OSError:
        return None


def tokensave_env() -> dict[str, str]:
    return {**os.environ, "TOKENSAVE_SKIP_AGENT_MAINTENANCE": "1"}


def resolve_binary(name: str) -> Path:
    found = shutil.which(name)
    if not found:
        die(f"`{name}` not found on PATH")
    return Path(found).resolve()


# ---------------------------------------------------------------------------
# prepare
# ---------------------------------------------------------------------------


def workspace(work: Path, repo: str, indexed: bool) -> Path:
    return work / "ws" / f"{repo}-{'indexed' if indexed else 'plain'}"


def checkout(spec: dict[str, Any], source: str | None, dest: Path) -> None:
    if spec["source"] == "fixture":
        shutil.copytree(HERE / spec["path"], dest)
        git(dest, "init", "-q")
        return
    url = source or spec["url"]
    ref = spec["ref"]
    dest.mkdir(parents=True)
    git(dest, "init", "-q")
    fetched = git(dest, "fetch", "-q", "--depth", "1", url, ref, check=False)
    if fetched.returncode != 0:
        # Some servers refuse fetching an unadvertised commit; fall back.
        remove_tree(dest)
        subprocess.run(["git", "clone", "-q", "--no-checkout", url, str(dest)], check=True)
        git(dest, "checkout", "-q", ref)
    else:
        git(dest, "checkout", "-q", "FETCH_HEAD")
    for rel in spec.get("strip", []):
        target = dest.joinpath(*rel.split("/"))
        if target.is_dir():
            shutil.rmtree(target)
        elif target.exists():
            target.unlink()


def snapshot_commit(dest: Path) -> None:
    exclude = dest / ".git" / "info" / "exclude"
    exclude.parent.mkdir(parents=True, exist_ok=True)
    with exclude.open("a", encoding="utf-8") as fh:
        fh.write("\n.tokensave/\n__pycache__/\n")
    git(dest, "checkout", "-q", "-B", "bench")
    git(dest, "add", "-A")
    git(dest, "commit", "-q", "--allow-empty", "-m", "agent-bench snapshot")
    git(dest, "tag", "-f", SNAPSHOT_TAG)


def cmd_prepare(args: argparse.Namespace) -> None:
    tasks = load_json(args.tasks)
    work: Path = args.work
    sources = dict(s.split("=", 1) for s in args.repo_source)
    tokensave = resolve_binary(args.tokensave)
    manifest_path = work / "manifest.json"
    manifest: dict[str, Any] = {"repos": {}}
    if manifest_path.is_file():
        manifest = load_json(manifest_path)
    manifest["prepared_at"] = now_iso()
    for name, spec in tasks["repos"].items():
        if args.repos and name not in args.repos.split(","):
            continue
        for indexed in (False, True):
            dest = workspace(work, name, indexed)
            if dest.exists():
                remove_tree(dest)
            dest.parent.mkdir(parents=True, exist_ok=True)
            print(f"prepare {dest.name} ...", flush=True)
            checkout(spec, sources.get(name), dest)
            snapshot_commit(dest)
            index_s = None
            if indexed:
                started = time.monotonic()
                subprocess.run(
                    [str(tokensave), "init", "--no-git-hook", str(dest)],
                    env=tokensave_env(),
                    check=True,
                    stdout=subprocess.DEVNULL,
                )
                index_s = round(time.monotonic() - started, 2)
            manifest["repos"][dest.name] = {
                "repo": name,
                "indexed": indexed,
                "ref": spec.get("ref"),
                "snapshot": git(dest, "rev-parse", "HEAD").stdout.strip(),
                "index_seconds": index_s,
            }
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(f"prepared workspaces under {work / 'ws'}")


def remove_tree(path: Path) -> None:
    """Removes a checkout, including git's read-only object files on Windows."""

    def force(func, target, _exc) -> None:
        os.chmod(target, 0o700)
        func(target)

    if sys.version_info >= (3, 12):
        shutil.rmtree(path, onexc=force)
    else:
        shutil.rmtree(path, onerror=force)


# ---------------------------------------------------------------------------
# run
# ---------------------------------------------------------------------------


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def reset_workspace(ws: Path) -> None:
    git(ws, "reset", "-q", "--hard", SNAPSHOT_TAG)
    git(ws, "clean", "-q", "-fd", "-e", ".tokensave")


def mcp_config(arm: dict[str, Any], tokensave: Path, ws: Path, profile: bool) -> dict[str, Any]:
    if not arm.get("tokensave"):
        return {"mcpServers": {}}
    env = {"TOKENSAVE_SKIP_AGENT_MAINTENANCE": "1"}
    if profile:
        env["TOKENSAVE_PROFILE"] = "light"
    return {
        "mcpServers": {
            "tokensave": {
                "type": "stdio",
                "command": str(tokensave),
                "args": ["serve", "--path", str(ws)],
                "env": env,
            }
        }
    }


def build_command(
    claude: Path,
    arms_cfg: dict[str, Any],
    arm: dict[str, Any],
    prompt: str,
    model: str,
    mcp_path: Path,
    budget: float | None,
    effort: str | None,
) -> list[str]:
    cmd = [str(claude), "-p", prompt, "--output-format", "stream-json", "--verbose"]
    cmd += ["--model", model, "--strict-mcp-config", "--mcp-config", str(mcp_path)]
    cmd += list(arms_cfg.get("common_args", []))
    if budget is not None:
        cmd += ["--max-budget-usd", str(budget)]
    if effort:
        cmd += ["--effort", effort]
    guide = arm.get("append_system_prompt_file")
    if guide:
        cmd += ["--append-system-prompt", Path(guide).expanduser().read_text(encoding="utf-8")]
    return cmd


def kill_tree(proc: subprocess.Popen) -> None:
    if proc.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)], capture_output=True)
    else:
        proc.kill()


# Variables kept from the CLAUDE* namespace: configuration the user chose for
# the agent (where its config and credentials live, the Windows shell).
# Everything else in that namespace describes the calling session (its id,
# entrypoint, messaging socket and token, effort) and must not reach a trial.
KEPT_AGENT_VARS = frozenset({"CLAUDE_CONFIG_DIR", "CLAUDE_CODE_GIT_BASH_PATH"})


def agent_env() -> dict[str, str]:
    return {
        k: v
        for k, v in os.environ.items()
        if not (k.startswith("CLAUDE") or k.startswith("MCP_CONNECTION")) or k in KEPT_AGENT_VARS
    }


ISOLATION_PROMPT = (
    "List the markdown headings of every rules file, CLAUDE.md, AGENTS.md or memory file "
    "loaded into your context, verbatim. If there are none, reply exactly NONE. Use no tools."
)


def preflight_isolation(claude: Path, arms_cfg: dict[str, Any], model: str, cwd: Path) -> str | None:
    """Asks the agent, with the arms' CLI arguments and no MCP servers, which
    instruction files it loaded. Returns the reply when anything leaked."""
    empty = cwd.parent.parent / "mcp-preflight.json"
    empty.write_text('{"mcpServers": {}}', encoding="utf-8")
    cmd = [str(claude), "-p", ISOLATION_PROMPT, "--model", model, "--strict-mcp-config",
           "--mcp-config", str(empty), *arms_cfg.get("common_args", []), "--tools", ""]
    out = subprocess.run(cmd, cwd=cwd, env=agent_env(), capture_output=True, text=True,
                         encoding="utf-8", timeout=300)
    reply = (out.stdout or "").strip()
    empty.unlink(missing_ok=True)
    if out.returncode != 0:
        return f"preflight failed (exit {out.returncode})"
    return None if reply.splitlines()[-1:] == ["NONE"] or reply == "NONE" else reply


def run_agent(cmd: list[str], cwd: Path, timeout_s: float):
    """Runs the agent, timestamping every stream-json line on receipt.
    Returns `(events, end_ms, exit_code, timed_out)`. Raw lines never leave
    this function unless the caller keeps them."""
    started = time.monotonic()
    proc = subprocess.Popen(
        cmd,
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env=agent_env(),
    )
    lines: queue.Queue = queue.Queue()

    def pump() -> None:
        assert proc.stdout is not None
        for raw in proc.stdout:
            lines.put(((time.monotonic() - started) * 1000.0, raw))
        lines.put(None)

    threading.Thread(target=pump, daemon=True).start()
    events: list[trace.Event] = []
    raw_lines: list[bytes] = []
    timed_out = False
    while True:
        remaining = timeout_s - (time.monotonic() - started)
        if remaining <= 0:
            timed_out = True
            kill_tree(proc)
            break
        try:
            item = lines.get(timeout=min(remaining, 1.0))
        except queue.Empty:
            continue
        if item is None:
            break
        t_ms, raw = item
        raw_lines.append(raw)
        try:
            obj = json.loads(raw)
        except ValueError:
            continue
        if isinstance(obj, dict):
            events.append(trace.Event(t_ms, obj))
    try:
        exit_code = proc.wait(timeout=30)
    except subprocess.TimeoutExpired:
        kill_tree(proc)
        exit_code = proc.wait()
    end_ms = (time.monotonic() - started) * 1000.0
    return events, raw_lines, end_ms, exit_code, timed_out


def check_task(task: dict[str, Any], ws: Path, answer: str | None) -> dict[str, Any]:
    """Independent correctness check. Returns booleans only."""
    checks = task.get("checks", {})
    out: dict[str, Any] = {}
    text = answer or ""
    if "answer_all" in checks:
        out["answer_all"] = all(
            re.search(p, text, re.IGNORECASE) for p in checks["answer_all"]
        )
    if "answer_none" in checks:
        out["answer_none"] = not any(
            re.search(p, text, re.IGNORECASE) for p in checks["answer_none"]
        )
    if "command" in checks:
        cmd = [sys.executable if a == "{python}" else a for a in checks["command"]]
        try:
            proc = subprocess.run(cmd, cwd=ws, capture_output=True, timeout=600)
            out["command"] = proc.returncode == 0
        except (OSError, subprocess.TimeoutExpired):
            out["command"] = False
    for rel in checks.get("unchanged", []):
        diff = git(ws, "status", "--porcelain", "--", rel, check=False).stdout.strip()
        out[f"unchanged:{rel}"] = diff == ""
    if checks.get("read_only"):
        setup = set(task.get("setup_files", {}))
        changed = [
            line[3:].strip()
            for line in git(ws, "status", "--porcelain", check=False).stdout.splitlines()
        ]
        out["read_only"] = all(path in setup for path in changed)
    return out


def classify(m: trace.TraceMetrics, checks: dict[str, Any], exit_code: int, timed_out: bool) -> str:
    if timed_out:
        return "timeout"
    if m.result_subtype is None:
        return "agent_crash"
    if m.is_error or (m.result_subtype or "").startswith("error"):
        return f"agent_{m.result_subtype or 'error'}"
    if checks.get("read_only") is False:
        return "modified_workspace"
    if any(k.startswith("unchanged:") and v is False for k, v in checks.items()):
        return "modified_protected_files"
    if checks.get("command") is False:
        return "check_failed"
    if checks.get("answer_all") is False or checks.get("answer_none") is False:
        return "wrong_answer"
    if exit_code != 0:
        return "agent_nonzero_exit"
    return "pass"


def environment(tokensave: Path, claude: Path) -> dict[str, Any]:
    def version(cmd: list[str], env=None) -> str | None:
        try:
            out = subprocess.run(cmd, capture_output=True, text=True, timeout=60, env=env)
            return out.stdout.strip() or None
        except (OSError, subprocess.TimeoutExpired):
            return None

    head = git(HERE, "rev-parse", "HEAD", check=False).stdout.strip() or None
    return {
        "tokensave_version": version([str(tokensave), "--version"], tokensave_env()),
        "tokensave_sha256": sha256_file(tokensave),
        "harness_commit": head,
        "agent_version": version([str(claude), "--version"]),
        "os": platform.system(),
        "os_release": platform.release(),
        "machine": platform.machine(),
        "cpu_count": os.cpu_count(),
        "python": platform.python_version(),
    }


def cmd_run(args: argparse.Namespace) -> None:
    tasks_cfg = load_json(args.tasks)
    arms_cfg = load_json(args.arms)
    work: Path = args.work
    tokensave = resolve_binary(args.tokensave)
    claude = resolve_binary(args.claude)
    tasks = [t for t in tasks_cfg["tasks"] if not args.tasks_filter or t["id"] in args.tasks_filter.split(",")]
    arm_names = args.arms_filter.split(",") if args.arms_filter else list(arms_cfg["arms"])
    arms: dict[str, dict[str, Any]] = {}
    for name in arm_names:
        arm = arms_cfg["arms"].get(name)
        if arm is None:
            die(f"unknown arm {name}")
        guide = arm.get("append_system_prompt_file")
        if guide and not Path(guide).expanduser().is_file():
            print(f"skip arm {name}: {guide} not found (unsupported here)", file=sys.stderr)
            continue
        arms[name] = arm
    for task in tasks:
        for name, arm in arms.items():
            if not workspace(work, task["repo"], bool(arm.get("tokensave"))).is_dir():
                die(f"workspace for {task['repo']} missing; run `prepare` first")

    isolation = "not-checked"
    if not args.skip_preflight:
        leaked = preflight_isolation(claude, arms_cfg, args.model, workspace(work, tasks[0]["repo"], False))
        if leaked is not None:
            die("user or project instructions reach the agent, so arms would not differ only in "
                f"their declared fields. Agent reply:\n{leaked}")
        isolation = "verified"

    run_id = uuid.uuid4().hex[:12]
    env_info = environment(tokensave, claude)
    env_info["instruction_isolation"] = isolation
    salt = secrets.token_bytes(32)  # duplicate-detection hashes are per run only
    profile_dir = args.profile_dir or profiling.default_dir()
    results = work / "results.jsonl"
    raw_dir = None
    if args.keep_raw:
        raw_dir = work / "raw" / run_id
        raw_dir.mkdir(parents=True, exist_ok=True)
        print(f"WARNING: --keep-raw stores prompts, tool arguments, paths and output in {raw_dir}", file=sys.stderr)

    plan = [(t, a, r) for r in range(args.repetitions) for t in tasks for a in arms]
    rng = random.Random(args.seed)
    rng.shuffle(plan)
    if args.warmup:
        warm = [(t, a, -1) for t in tasks for a in arms]
        plan = warm + plan
    mcp_path = work / f"mcp-{run_id}.json"
    print(f"run {run_id}: {len(plan)} trials ({len(tasks)} tasks x {len(arms)} arms x {args.repetitions})", flush=True)
    for order, (task, arm_name, rep) in enumerate(plan):
        arm = arms[arm_name]
        ws = workspace(work, task["repo"], bool(arm.get("tokensave")))
        reset_workspace(ws)
        for rel, content in task.get("setup_files", {}).items():
            path = ws.joinpath(*rel.split("/"))
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8", newline="\n")
        mcp_path.write_text(
            json.dumps(mcp_config(arm, tokensave, ws, not args.no_profile)), encoding="utf-8"
        )
        cmd = build_command(claude, arms_cfg, arm, task["prompt"], args.model, mcp_path, args.budget, args.effort)
        before = profiling.snapshot(profile_dir)
        events, raw_lines, end_ms, exit_code, timed_out = run_agent(cmd, ws, args.timeout)
        metrics, answer = trace.parse(events, salt, end_ms)
        prof = None
        if arm.get("tokensave") and not args.no_profile:
            prof = profiling.collect(profile_dir, before)
        try:
            checks = check_task(task, ws, answer)
            outcome = classify(metrics, checks, exit_code, timed_out)
        except (re.error, OSError, subprocess.SubprocessError) as exc:
            checks = {"harness_error": type(exc).__name__}
            outcome = "harness_error"
        record = {
            "schema": RESULTS_SCHEMA,
            "run_id": run_id,
            "trial_id": uuid.uuid4().hex[:12],
            "started_at": now_iso(),
            "order": order,
            "warmup": rep < 0,
            "repetition": rep,
            "task": task["id"],
            "category": task.get("category"),
            "repo": task["repo"],
            "arm": arm_name,
            "requested_model": args.model,
            "effort": args.effort,
            "conditions": {
                "process": "cold",
                "index": "prebuilt" if arm.get("tokensave") else "none",
                "os_cache": "warm" if args.warmup else "unknown",
            },
            "exit_code": exit_code,
            "timed_out": timed_out,
            "outcome": outcome,
            "success": outcome == "pass",
            "checks": checks,
            "metrics": metrics.to_json(),
            "tokensave_profile": prof,
            "environment": env_info,
        }
        with results.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(record) + "\n")
        if raw_dir is not None:
            (raw_dir / f"{order:04d}-{task['id']}-{arm_name}.jsonl").write_bytes(b"".join(raw_lines))
        print(
            f"[{order + 1}/{len(plan)}] {task['id']:<16} {arm_name:<17} {outcome:<20} "
            f"{metrics.wall_ms / 1000:7.1f}s  calls={metrics.tool_calls:<3} "
            f"cost={metrics.cost_usd if metrics.cost_usd is not None else 'n/a'}",
            flush=True,
        )
        reset_workspace(ws)
    mcp_path.unlink(missing_ok=True)


def cmd_report(args: argparse.Namespace) -> None:
    records = report_mod.load(args.work / "results.jsonl", include_warmup=args.include_warmup)
    if args.run:
        records = [r for r in records if r["run_id"] in args.run.split(",")]
    if args.json:
        text = json.dumps(report_mod.aggregate(records), indent=2)
    else:
        text = report_mod.render_markdown(records)
    if args.out:
        args.out.write_text(text, encoding="utf-8")
    else:
        sys.stdout.buffer.write(text.encode("utf-8") + b"\n")


def main(argv: list[str] | None = None) -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="command", required=True)

    def common(sp: argparse.ArgumentParser) -> None:
        sp.add_argument("--work", type=Path, required=True, help="private working directory")
        sp.add_argument("--tasks", type=Path, default=HERE / "tasks.json")

    sp = sub.add_parser("prepare", help="build pinned checkouts and indexes")
    common(sp)
    sp.add_argument("--tokensave", default="tokensave")
    sp.add_argument("--repos", help="comma-separated subset of repositories")
    sp.add_argument("--repo-source", action="append", default=[], metavar="NAME=URL_OR_PATH",
                    help="fetch a git repository from another URL or local clone")
    sp.set_defaults(func=cmd_prepare)

    sp = sub.add_parser("run", help="run trials and append results")
    common(sp)
    sp.add_argument("--arms", type=Path, default=HERE / "arms.json")
    sp.add_argument("--model", required=True, help="exact model id, recorded with each trial")
    sp.add_argument("--effort", help="agent effort level, if the agent supports it")
    sp.add_argument("--only-arms", dest="arms_filter")
    sp.add_argument("--only-tasks", dest="tasks_filter")
    sp.add_argument("--repetitions", type=int, default=3)
    sp.add_argument("--seed", type=int, default=948)
    sp.add_argument("--warmup", action="store_true", help="one unrecorded-in-report trial per task and arm first")
    sp.add_argument("--timeout", type=float, default=900.0, help="seconds per trial")
    sp.add_argument("--budget", type=float, default=2.0, help="max USD per trial (agent-enforced)")
    sp.add_argument("--tokensave", default="tokensave")
    sp.add_argument("--claude", default="claude")
    sp.add_argument("--profile-dir", type=Path, help="TokenSave profile directory (default ~/.tokensave/profile)")
    sp.add_argument("--no-profile", action="store_true", help="do not enable #754 profiling for the MCP server")
    sp.add_argument("--keep-raw", action="store_true", help="OPT-IN: keep full raw traces (sensitive)")
    sp.add_argument("--skip-preflight", action="store_true",
                    help="skip the one-call check that no user instructions reach the agent")
    sp.set_defaults(func=cmd_run)

    sp = sub.add_parser("report", help="aggregate results")
    sp.add_argument("--work", type=Path, required=True)
    sp.add_argument("--run", help="comma-separated run ids (default: all)")
    sp.add_argument("--include-warmup", action="store_true")
    sp.add_argument("--json", action="store_true")
    sp.add_argument("--out", type=Path)
    sp.set_defaults(func=cmd_report)

    args = p.parse_args(argv)
    args.func(args)


if __name__ == "__main__":
    main()
