#!/usr/bin/env python3
"""Refs #832: serial, bounded matched real-Pi RPC evaluation. No faux provider."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import shutil
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
PIN = "1.4.0"
SOURCE_PIN = "d5b3de22d0e24a3e9c38a9410d3329b099717283"
LIMIT = 4 * 1024 * 1024
TASKS = [
    ("navigation", "Find normalize_key's definition and all its callers. Explain how lookup_user calls it across files."),
    ("docs", "Read README.md and settings.json. Report the documentation marker and configured retry count."),
    ("unindexed", "Find the recovery_marker in excluded/recovery.py and report its exact value."),
    ("verification", "Verify the exact current source of normalize_key before proposing any edit. Quote its return expression. Do not modify files."),
    ("stale", "Find normalize_key and report its exact CURRENT return expression. Verify against current source; the index may lag."),
    ("missing", "Find normalize_key and its callers even if indexed retrieval is unavailable."),
    ("disconnected", "Find normalize_key and its callers even if indexed retrieval fails."),
    ("long", "Find normalize_key and its callers again, preserving correct native fallback when needed."),
]


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def verify_model(state, provider, model):
    actual = state.get("model", {})
    identity = {"provider": actual.get("provider"), "id": actual.get("id")}
    if identity != {"provider": provider, "id": model}:
        raise RuntimeError("Pi did not select the exact requested provider/model; do not compare aliases")
    return identity


def checked(command, cwd=None):
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, start_new_session=True,
                               env={**os.environ, "TOKENSAVE_SKIP_AGENT_MAINTENANCE": "1"})
    try:
        stdout, _stderr = process.communicate(timeout=120)
        if process.returncode:
            raise RuntimeError("Evaluation setup command failed; no credentials or output echoed")
        return stdout.strip()
    finally:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


def fixture(path, binary, scenario):
    path.mkdir()
    (path / "excluded").mkdir()
    (path / "keys.py").write_text('def normalize_key(value):\n    return value.strip().lower()\n')
    (path / "users.py").write_text('from keys import normalize_key\n\ndef lookup_user(value):\n    return normalize_key(value)\n')
    (path / "README.md").write_text("Documentation marker: amber-lantern\n")
    (path / "settings.json").write_text('{"retries": 3}\n')
    (path / "excluded" / "recovery.py").write_text('recovery_marker = "violet-anchor"\n')
    checked(["git", "init", "--quiet", str(path)])
    if scenario != "missing":
        checked([binary, "init", str(path), "--no-git-hook", "--cpu-usage", "low",
                 "--skip-folder", "excluded"])
    if scenario == "stale":
        (path / "keys.py").write_text('def normalize_key(value):\n    return value.strip().casefold()\n')


class Rpc:
    """One owned process group, no reader thread/queue, deadlines and byte bounds."""
    def __init__(self, command, cwd, env, trace):
        self.trace = trace
        self.buffer = b""
        self.total = 0
        self.sequence = 0
        self.turns = 0
        self.started = time.monotonic()
        self.process = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                        start_new_session=True)
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)

    def record(self, value):
        if value.get("type") not in {"message_update", "message_start"}:
            self.trace.write(json.dumps({"elapsed_s": time.monotonic() - self.started,
                                         "wall_time_ms": time.time_ns() // 1_000_000,
                                         "event": value}) + "\n")
            self.trace.flush()

    def read(self, deadline):
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not self.selector.select(remaining):
                raise TimeoutError("Pi RPC deadline exceeded")
            chunk = os.read(self.process.stdout.fileno(), 65536)
            if not chunk:
                raise RuntimeError(f"Pi exited before completion (status {self.process.poll()})")
            self.total += len(chunk)
            self.buffer += chunk
            if len(self.buffer) > LIMIT or self.total > 256 * 1024 * 1024:
                raise RuntimeError("Pi RPC output exceeded evaluation byte limit")
        line, self.buffer = self.buffer.split(b"\n", 1)
        value = json.loads(line)
        if value.get("type") == "turn_start":
            self.turns += 1
            if self.turns > 128:
                raise RuntimeError("Pi exceeded 128 assistant turns in one evaluation session")
        self.record(value)
        return value

    def command(self, kind, timeout, **fields):
        self.sequence += 1
        identifier = str(self.sequence)
        deadline = time.monotonic() + timeout
        self.record({"type": "evaluation_command", "command": kind, **fields})
        self.process.stdin.write((json.dumps({"id": identifier, "type": kind, **fields}) + "\n").encode())
        self.process.stdin.flush()
        response = None
        settled = None
        while response is None or (kind == "prompt" and settled is None):
            event = self.read(deadline)
            if event.get("id") == identifier and event.get("type") == "response":
                response = event
                if not event.get("success"):
                    raise RuntimeError(f"Pi command failed: {kind}")
                if kind == "prompt" and event.get("data", {}).get("disposition") != "started":
                    raise RuntimeError("Evaluation prompt did not start a fresh run")
            if event.get("type") == "agent_settled":
                settled = event
                if event.get("aborted"):
                    raise RuntimeError("Evaluation run aborted")
            if event.get("type") == "message_end":
                message = event.get("message", {})
                if message.get("role") == "assistant" and message.get("stopReason") in {"error", "aborted"}:
                    raise RuntimeError("Provider failed or aborted; retain trace, do not score as success")
        return response

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=5)
        except (BrokenPipeError, subprocess.TimeoutExpired):
            pass
        finally:
            # Reap Pi and terminate any MCP descendants, even after parent exit.
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait()
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            self.process.stdout.close()
            self.selector.close()


def run_arm(args, scratch, output, scenario, prompt, arm, repeat):
    cwd = scratch / "fixture"
    fixture(cwd, str(args.tokensave), scenario)
    profile = scratch / "agent"
    profile.mkdir(mode=0o700)
    if args.auth_file:
        shutil.copyfile(args.auth_file, profile / "auth.json")
        (profile / "auth.json").chmod(0o600)
    source = (ROOT / "src" / "agents" / "integrations" / "pi_extension.ts").read_text()
    source = source.replace('"__TOKENSAVE_BIN__"', json.dumps(str(args.tokensave)))
    if arm == "baseline":
        assignment = "event.systemPromptOptions.sections.tokensave_retrieval = RETRIEVAL_GUIDANCE;"
        if source.count(assignment) != 1:
            raise RuntimeError("Baseline transformation no longer matches managed extension")
        source = source.replace(assignment, "// Evaluation baseline: retrieval section omitted.")
    extension = profile / "tokensave.ts"
    extension.write_text(source)
    server = {"command": str(args.tokensave), "args": ["serve", "--path", str(cwd)], "exposure": "direct"}
    if scenario == "disconnected":
        server["command"] = str(scratch / "deliberately-missing-server")
    write_json(profile / "mcp.json", {"mcpServers": {"tokensave": server}})
    write_json(profile / "settings.json", {"compaction": {"enabled": False}, "retry": {"enabled": False}})
    command = [*args.pi, "--mode", "rpc", "--no-session", "--offline", "--no-approve",
               "--no-extensions", "--no-skills", "--no-prompt-templates", "--no-context-files",
               "--no-themes", "-e", "builtin:mcp", "-e", str(extension),
               "-e", str(ROOT / "scripts" / "pi-eval" / "observer.mjs"),
               "--provider", args.provider, "--model", args.model, "--thinking", "off"]
    env = {**os.environ, "PI_CODING_AGENT_DIR": str(profile),
           "TOKENSAVE_PI_EVAL_CONTEXT": str(output / "contexts.jsonl"),
           "TOKENSAVE_SKIP_AGENT_MAINTENANCE": "1"}
    metadata = {"schema": 1, "scenario": scenario, "arm": arm, "repeat": repeat,
                "provider": args.provider, "model": args.model, "pi_version": args.pi_version,
                "tokensave_version": args.tokensave_version, "tokensave_commit": args.commit,
                "tokensave_binary_sha256": args.binary_sha256,
                "host": platform.platform(), "python": platform.python_version(),
                "pi_source_commit": args.pi_source_commit,
                "extension_sha256": hashlib.sha256(source.encode()).hexdigest(),
                "prompt": prompt, "status": "running", "interface": "builtin-direct",
                "note": "stale scenario may be refreshed by production MCP startup sync"}
    write_json(output / "run.json", metadata)
    started = time.monotonic()
    rpc = None
    try:
        with (output / "events.jsonl").open("w") as trace:
            rpc = Rpc(command, cwd, env, trace)
            state = rpc.command("get_state", args.timeout)["data"]
            metadata["observed_model"] = verify_model(state, args.provider, args.model)
            rpc.command("set_auto_retry", args.timeout, enabled=False)
            rpc.command("set_auto_compaction", args.timeout, enabled=False)
            if scenario == "long":
                for turn in range(args.long_turns):
                    rpc.command("prompt", args.timeout, message=f"{prompt} This is independent review turn {turn + 1}. Recheck your evidence.")
                rpc.command("compact", args.timeout)
                metadata["compacted"] = True
            rpc.command("prompt", args.timeout, message=prompt)
            metadata["session_stats"] = rpc.command("get_session_stats", args.timeout)["data"]
        metadata["status"] = "completed"
    except Exception as error:
        metadata["status"] = "failed"
        metadata["error_type"] = type(error).__name__
        raise
    finally:
        if rpc:
            rpc.close()
        metadata["elapsed_s"] = time.monotonic() - started
        write_json(output / "run.json", metadata)


def main():
    def interrupt(_signum, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupt)
    parser = argparse.ArgumentParser(description=__doc__)
    host = parser.add_mutually_exclusive_group(required=True)
    host.add_argument("--pi", nargs="+", help="Executable prefix, e.g. node /path/to/pi/cli.js")
    host.add_argument("--pi-command-file", type=Path, help="JSON argv array for a source runtime with Node flags")
    parser.add_argument("--pi-source", type=Path, help="Verify an isolated Pi source checkout against the pinned commit")
    parser.add_argument("--tokensave", type=Path, required=True)
    parser.add_argument("--provider", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--auth-file", type=Path, help="Optional Pi auth.json, copied only to private temporary profiles")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=3, choices=range(2, 11))
    parser.add_argument("--long-turns", type=int, default=12, choices=range(2, 41), metavar="COUNT")
    parser.add_argument("--timeout", type=int, default=180, choices=range(10, 601), metavar="SECONDS")
    parser.add_argument("--max-failures", type=int, default=3, choices=range(1, 11))
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("Process-group cleanup currently requires POSIX")
    if args.pi_command_file:
        args.pi = json.loads(args.pi_command_file.read_text())
    if not isinstance(args.pi, list) or not args.pi or not all(isinstance(part, str) and part for part in args.pi):
        parser.error("Pi command must be a nonempty JSON argv array of strings")
    args.pi_source_commit = None
    if args.pi_source:
        args.pi_source_commit = checked(["git", "rev-parse", "HEAD"], args.pi_source)
        if args.pi_source_commit != SOURCE_PIN or checked(["git", "status", "--porcelain", "--untracked-files=no"], args.pi_source):
            parser.error("Pi source must be clean at the documented pinned commit")
    args.tokensave = args.tokensave.resolve(strict=True)
    args.output = args.output.resolve()
    args.pi_version = checked([*args.pi, "--version"])
    if args.pi_version != PIN:
        parser.error(f"Expected Pi {PIN}, got {args.pi_version!r}")
    args.tokensave_version = checked([str(args.tokensave), "--version"])
    with args.tokensave.open("rb") as binary:
        args.binary_sha256 = hashlib.file_digest(binary, "sha256").hexdigest()
    args.commit = checked(["git", "rev-parse", "HEAD"], ROOT)
    if checked(["git", "status", "--porcelain", "--untracked-files=no"], ROOT):
        parser.error("Commit tracked source changes before collecting reproducible evidence")
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    failures = 0
    for repeat in range(args.repeats):
        for scenario, prompt in TASKS:
            for arm in (["baseline", "enabled"] if repeat % 2 == 0 else ["enabled", "baseline"]):
                output = args.output / f"{repeat}-{scenario}-{arm}"
                output.mkdir(mode=0o700)
                with tempfile.TemporaryDirectory(prefix="tokensave-pi-eval-") as temporary:
                    try:
                        run_arm(args, Path(temporary), output, scenario, prompt, arm, repeat)
                    except Exception as error:
                        failures += 1
                        if not (output / "run.json").exists():
                            write_json(output / "run.json", {"schema": 1, "status": "setup-failed",
                                       "scenario": scenario, "arm": arm, "repeat": repeat,
                                       "error_type": type(error).__name__})
                        print(f"{output.name}: failed ({type(error).__name__}); retain artifacts", flush=True)
                        if failures >= args.max_failures:
                            raise SystemExit("Failure budget exhausted; remaining pairs were not run")
    raise SystemExit(1 if failures else 0)


if __name__ == "__main__":
    main()
