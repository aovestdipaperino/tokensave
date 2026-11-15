#!/usr/bin/env python3
"""Bounded, opt-in grammar ASan evaluation. See README.md for prerequisites."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import selectors
import signal
import subprocess
import sys
import time


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
NIGHTLY = "nightly-2026-11-11"
NATIVE_FLAGS = "-DNDEBUG -fsanitize=address -fno-omit-frame-pointer -g -O1"
# Unoptimized AArch64 builtins call instrumented core before ASan maps shadow memory.
BUILD_STD = ["-Zbuild-std", "--config", "profile.dev.package.compiler_builtins.opt-level=1",
             "--config", "profile.test.package.compiler_builtins.opt-level=1"]


def run_command(command, cwd, env, log, timeout, max_log_bytes=64 * 1024 * 1024):
    if not math.isfinite(timeout) or timeout <= 0 or max_log_bytes <= 0:
        raise ValueError("timeout and log limit must be positive and finite")
    started = time.monotonic()
    status = None
    written = 0
    with log.open("wb") as output:
        child = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(child.stdout, selectors.EVENT_READ)
                while True:
                    remaining = timeout - (time.monotonic() - started)
                    if remaining <= 0:
                        status = "timed_out"
                        break
                    if not selector.select(min(remaining, .2)):
                        continue
                    chunk = os.read(child.stdout.fileno(), 65536)
                    if not chunk:
                        break
                    room = max_log_bytes - written
                    output.write(chunk[:room])
                    output.flush()
                    written += min(room, len(chunk))
                    if len(chunk) > room:
                        status = "output_limit"
                        break
            if status is None:
                try:
                    child.wait(timeout=max(.001, timeout - (time.monotonic() - started)))
                except subprocess.TimeoutExpired:
                    status = "timed_out"
        finally:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()
            child.stdout.close()
    return {"command": command, "status": status or ("passed" if child.returncode == 0 else "failed"),
            "returncode": child.returncode, "seconds": round(time.monotonic() - started, 3),
            "log": str(log)}


def reserve_output(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.mkdir()


def snapshot_inputs(roots, output, max_bytes=1_048_576):
    files = []
    entries_seen = 0
    for root in roots:
        if not root.is_dir():
            raise ValueError(f"corpus directory is missing: {root}")
        if output.resolve().is_relative_to(root.resolve()):
            raise ValueError("output directory must be outside the corpus")
        before = len(files)
        pending = [root]
        while pending:
            with os.scandir(pending.pop()) as entries:
                for entry in entries:
                    entries_seen += 1
                    if entries_seen > 2000:
                        raise ValueError("corpus traversal exceeds 2000 filesystem entries")
                    if entry.is_symlink():
                        continue
                    if entry.is_dir(follow_symlinks=False):
                        pending.append(Path(entry.path))
                    elif entry.is_file(follow_symlinks=False):
                        files.append(Path(entry.path))
                        if len(files) > 1000:
                            raise ValueError("corpus exceeds 1000 files")
        if len(files) == before:
            raise ValueError(f"corpus is empty: {root}")
    files.sort()
    records = []
    for index, path in enumerate(files):
        with path.open("rb") as source:
            data = source.read(max_bytes + 1)
        if len(data) > max_bytes:
            raise ValueError(f"input exceeds {max_bytes} bytes: {path}")
        name = f"{index:04d}-{path.name}"
        (output / name).write_bytes(data)
        records.append({"name": name, "source": str(path), "bytes": len(data),
                        "sha256": hashlib.sha256(data).hexdigest()})
    return records


def control_detected(result, log):
    return (result["status"] == "failed" and result["returncode"] != 0
            and b"ERROR: AddressSanitizer: heap-buffer-overflow" in log.read_bytes())


def build_paths(output, previous, identity):
    if previous is None:
        return output / "target", output / "native-audit.jsonl"
    previous = previous.resolve()
    stored = json.loads((previous / "report.json").read_text())
    if any(stored.get(key) != identity[key] for key in ("toolchain", "target", "lock_sha256")):
        raise ValueError("resume requires the same toolchain, target and Cargo.lock")
    target = Path(stored.get("build_target_dir", previous / "target")).resolve()
    audit = Path(stored.get("native_audit", previous / "native-audit.jsonl")).resolve()
    if not target.is_dir() or not audit.is_file():
        raise ValueError("resume requires the previous target directory and native audit")
    return target, audit


def build_environment(env, target_dir, audit, target, jobs):
    result = dict(env, RUSTFLAGS="-Zsanitizer=address -Cforce-frame-pointers=yes -Cdebuginfo=1",
                  CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS=str(jobs), CC=str(HERE / "compiler.py"),
                  CXX=str(HERE / "compiler.py"), CFLAGS=NATIVE_FLAGS, CXXFLAGS=NATIVE_FLAGS,
                  TOKENSAVE_ASAN_AUDIT=str(audit), CARGO_TARGET_DIR=str(target_dir))
    for key in ("CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        result.pop(key, None)
    for name in ("CC", "CXX", "CFLAGS", "CXXFLAGS"):
        for qualified in (f"{name}_{target}", f"{name}_{target.replace('-', '_')}", f"TARGET_{name}"):
            result[qualified] = result[name]
    return result


def native_symbols(text):
    return set(re.findall(r"\b[TWtw]\s+(tree_sitter_[A-Za-z0-9_]+)\s*$", text, re.MULTILINE))


def evaluate(args, report):
    output = args.output.resolve()
    env = dict(os.environ)
    env["ASAN_OPTIONS"] = "detect_leaks=1:halt_on_error=1:abort_on_error=0"
    report["platform"] = platform.platform()
    report["toolchain"] = args.toolchain
    report["revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    report["lock_sha256"] = hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()
    report["driver_sha256"] = hashlib.sha256((ROOT / "examples" / "grammar_asan.rs").read_bytes()).hexdigest()
    report["steps"] = []

    def step(name, command, timeout=60, child_env=None):
        result = run_command(command, ROOT, child_env or env, output / f"{name}.log", timeout)
        report["steps"].append(dict(name=name, **result))
        print(f"{name}: {result['status']} ({result['seconds']}s)", flush=True)
        if result["status"] != "passed":
            raise RuntimeError(f"{name}: {result['status']}; see {result['log']}")
        return result

    for name, command in (("rust-version", ["rustc", f"+{args.toolchain}", "-Vv"]),
                          ("clang-version", ["clang", "--version"]),
                          ("clang-cxx-version", ["clang++", "--version"])):
        step(name, command)
    host = (output / "rust-version.log").read_text()
    target = next(line.removeprefix("host: ") for line in host.splitlines() if line.startswith("host: "))
    if target not in ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"):
        raise RuntimeError(f"Linux GNU target required, got {target}")
    report["target"] = target
    for language, compiler in (("c", "clang"), ("cxx", "clang++")):
        mode = ["-x", "c++"] if language == "cxx" else []
        control = output / f"native-control-{language}"
        step(f"control-build-{language}", [compiler, *mode, *NATIVE_FLAGS.split(),
             str(HERE / "control.c"), "-o", str(control)])
        log = output / f"control-{language}.log"
        detected = run_command([str(control)], ROOT, env, log, 10)
        report["steps"].append(dict(name=f"control-{language}", **detected))
        if not control_detected(detected, log):
            raise RuntimeError(f"{language} memory-error control did not produce the expected ASan diagnostic")
        control = output / f"negative-control-{language}"
        step(f"negative-control-build-{language}", [compiler, *mode, "-O1",
             str(HERE / "control.c"), "-o", str(control)])
        log = output / f"negative-control-{language}.log"
        negative = run_command([str(control)], ROOT, env, log, 10)
        report["steps"].append(dict(name=f"negative-control-{language}", **negative))
        if control_detected(negative, log):
            raise RuntimeError("uninstrumented control unexpectedly produced an ASan diagnostic")

    inputs = output / "inputs"
    inputs.mkdir()
    report["inputs"] = snapshot_inputs([ROOT / "tests" / "fixtures", *args.corpus], inputs)
    seeds = {
        "empty": b"", "delimiters": b"([{\"'/*\\\n" * 64,
        "binary": bytes(range(256)), "nul": b"\x00" * 256,
        "unicode": "\u03bb\u4e2d\U0001f600\n".encode() * 128,
        "nested": b"(" * 256 + b")" * 256,
        "long-string": b'"' + b"x" * 16384 + b'"',
    }
    for name, data in seeds.items():
        name = f"seed-{name}"
        (inputs / name).write_bytes(data)
        report["inputs"].append({"name": name, "source": "generated", "bytes": len(data),
                                 "sha256": hashlib.sha256(data).hexdigest()})
    target_dir, audit = build_paths(output, args.resume_from, report)
    report["build_target_dir"] = str(target_dir)
    report["native_audit"] = str(audit)
    report["build_jobs"] = args.build_jobs
    if args.resume_from:
        report["resume_from"] = str(args.resume_from.resolve())
    build_env = build_environment(env, target_dir, audit, target, args.build_jobs)
    step("build", ["cargo", f"+{args.toolchain}", "build", "--locked", "--example", "grammar_asan",
                   "--target", target, *BUILD_STD, "-vv"], args.build_timeout, build_env)
    step("driver-tests", ["cargo", f"+{args.toolchain}", "test", "--locked", "--example", "grammar_asan",
                          "--target", target, *BUILD_STD], args.build_timeout, build_env)
    records = [json.loads(line) for line in audit.read_text().splitlines()]
    objects = {}
    for row in records:
        if not row["instrumented"] or "-fsanitize=address" not in row["command"]:
            raise RuntimeError("native audit includes an uninstrumented compilation")
        objects[row["object"]] = row
    for path, row in objects.items():
        with open(path, "rb") as object_file:
            if hashlib.file_digest(object_file, "sha256").hexdigest() != row["sha256"]:
                raise RuntimeError(f"native object changed since its instrumentation audit: {path}")
    expected = ["tree-sitter-wgsl/src/parser.c", "tree-sitter-wgsl/src/scanner.c",
                "tree-sitter-actionscript/src/parser.c", "tree-sitter-gdscript/src/parser.c",
                "tree-sitter-gdscript/src/scanner.c"]
    for suffix in expected:
        if not any(row["source"].endswith(suffix) and row["instrumented"] for row in records):
            raise RuntimeError(f"no instrumented native object recorded for {suffix}")
    if not any(row["source"].endswith("parser.c") and "vendor/" not in row["source"] for row in records):
        raise RuntimeError("no bundled grammar parser recorded in native audit")
    report["native_compilations"] = len(records)
    report["native_objects"] = len(objects)
    binary = target_dir / target / "debug" / "examples" / "grammar_asan"
    step("native-symbols", ["nm", "-g", "--defined-only", *objects])
    step("linked-symbols", ["nm", "-g", "--defined-only", str(binary)])
    linked = native_symbols((output / "linked-symbols.log").read_text())
    audited = native_symbols((output / "native-symbols.log").read_text())
    if not linked or linked - audited:
        raise RuntimeError(f"linked native grammar/scanner symbols lack audited objects: {sorted(linked - audited)}")
    report["audited_linked_symbols"] = len(linked)
    step("grammars", [str(binary), "--list"])
    keys = json.loads((output / "grammars.log").read_text())
    if not keys or len(keys) != len(set(keys)) or any(not re.fullmatch(r"[a-z][a-z0-9_-]{0,63}", key) for key in keys):
        raise RuntimeError("invalid grammar inventory")
    report["grammars"] = keys
    report["runs"] = []
    for key in keys:
        result = run_command([str(binary), key, *[str(inputs / row["name"]) for row in report["inputs"]]],
                             ROOT, env, output / f"grammar-{key}.log", args.timeout)
        report["runs"].append(dict(grammar=key, **result))
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"{key}: {result['status']} ({result['seconds']}s)", flush=True)
    if any(row["status"] != "passed" for row in report["runs"]):
        raise RuntimeError("grammar evaluation has failures or incomplete runs; inspect per-grammar logs")
    report["status"] = "completed"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new directory; existing paths are refused")
    parser.add_argument("--corpus", type=Path, action="append", default=[])
    parser.add_argument("--toolchain", default=NIGHTLY)
    parser.add_argument("--timeout", type=float, default=60, help="seconds per grammar's complete corpus")
    parser.add_argument("--build-timeout", type=float, default=2400)
    parser.add_argument("--build-jobs", type=int, choices=(1, 2), default=2)
    parser.add_argument("--resume-from", type=Path, help="reuse an earlier evaluation's isolated target and native audit")
    args = parser.parse_args(argv)
    if sys.platform != "linux":
        parser.error("evaluation requires Linux; unit tests can run on other POSIX systems")
    for value in (args.timeout, args.build_timeout):
        if not math.isfinite(value) or value <= 0:
            parser.error("timeouts must be finite and positive")
    reserve_output(args.output)
    report = {"status": "incomplete"}
    started = time.monotonic()

    def terminate(_signum, _frame):
        raise KeyboardInterrupt

    previous_handler = signal.signal(signal.SIGTERM, terminate)
    try:
        evaluate(args, report)
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, KeyboardInterrupt) as error:
        report["error"] = str(error) or type(error).__name__
        print(report["error"], file=sys.stderr)
    finally:
        signal.signal(signal.SIGTERM, previous_handler)
        report["total_seconds"] = round(time.monotonic() - started, 3)
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["status"] == "completed" else 1


if __name__ == "__main__":
    sys.exit(main())
