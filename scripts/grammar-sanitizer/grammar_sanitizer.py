#!/usr/bin/env python3
"""Run TokenSave's grammar paths under AddressSanitizer on Linux."""

from __future__ import annotations

import argparse
import json
import os
import platform
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

try:
    import resource
except ModuleNotFoundError:  # Windows: import for unit tests; execution is Linux-only.
    resource = None


PINNED_NIGHTLY = "nightly-2026-11-10"
DEFAULT_TARGET = "x86_64-unknown-linux-gnu"
ASAN_FLAGS = "-Zsanitizer=address -Cforce-frame-pointers=yes"
CC_ASAN_FLAGS = "-DNDEBUG -fsanitize=address -fno-omit-frame-pointer"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Build every enabled tree-sitter grammar with AddressSanitizer, "
            "run extraction fixtures, and optionally index real corpora."
        )
    )
    parser.add_argument(
        "--work-dir",
        type=Path,
        required=True,
        help="Private directory outside the checkout for Cargo output and logs.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="JSON report path; an existing ordinary report is replaced.",
    )
    parser.add_argument(
        "--corpus",
        type=Path,
        action="append",
        default=[],
        help="Existing source tree to index read-only; repeat for more corpora.",
    )
    parser.add_argument("--toolchain", default=PINNED_NIGHTLY)
    parser.add_argument("--target", default=DEFAULT_TARGET)
    parser.add_argument("--timeout-secs", type=int, default=3600)
    parser.add_argument(
        "--skip-fixtures",
        action="store_true",
        help="Build the sanitized binary and run only the supplied corpora.",
    )
    return parser.parse_args(argv)


def require_linux() -> None:
    if platform.system() != "Linux":
        raise RuntimeError("the grammar sanitizer harness requires Linux")


def directory_size(path: Path) -> int:
    total = 0
    if not path.exists():
        return total
    for entry in path.rglob("*"):
        try:
            if entry.is_file():
                total += entry.stat().st_size
        except OSError:
            # A concurrent Cargo cleanup may remove an intermediate file.
            continue
    return total


def sanitizer_environment(target_dir: Path) -> dict[str, str]:
    environment = os.environ.copy()
    environment.update(
        {
            "ASAN_OPTIONS": "detect_leaks=1:halt_on_error=1:abort_on_error=1",
            "CARGO_TARGET_DIR": str(target_dir),
            "CFLAGS": CC_ASAN_FLAGS,
            "CXXFLAGS": CC_ASAN_FLAGS,
            "RUST_BACKTRACE": "1",
            "RUSTFLAGS": ASAN_FLAGS,
        }
    )
    return environment


def cargo_command(
    toolchain: str, target: str, *, test: bool
) -> list[str]:
    action = "test" if test else "build"
    command = [
        "cargo",
        f"+{toolchain}",
        action,
        "--locked",
        "--target",
        target,
        "--no-default-features",
        "--features",
        "full,test-transport",
    ]
    if test:
        command.extend(["--test", "integration", "extraction_test::", "--", "--nocapture"])
    else:
        command.extend(["--bin", "tokensave"])
    return command


def run_phase(
    name: str,
    command: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    log_dir: Path,
    timeout_secs: int,
) -> dict[str, object]:
    if resource is None:
        raise RuntimeError("resource accounting requires Linux")
    log_path = log_dir / f"{name}.log"
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.monotonic()
    with log_path.open("w", encoding="utf-8", errors="backslashreplace") as log:
        log.write(f"command: {subprocess.list2cmdline(command)}\n")
        log.flush()
        try:
            process = subprocess.Popen(
                command,
                cwd=cwd,
                env=environment,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        except OSError as error:
            log.write(f"spawn error: {error}\n")
            return_code = None
            timed_out = False
            spawn_error = str(error)
        else:
            spawn_error = None
            try:
                return_code = process.wait(timeout=timeout_secs)
                timed_out = False
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                return_code = None
                timed_out = True
    elapsed = time.monotonic() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    return {
        "name": name,
        "command": command,
        "elapsed_seconds": elapsed,
        "user_cpu_seconds": after.ru_utime - before.ru_utime,
        "system_cpu_seconds": after.ru_stime - before.ru_stime,
        "child_high_water_rss_kib": after.ru_maxrss,
        "return_code": return_code,
        "spawn_error": spawn_error,
        "timed_out": timed_out,
        "log": str(log_path),
    }


def safe_name(path: Path, index: int) -> str:
    stem = "".join(character if character.isalnum() else "-" for character in path.name)
    return f"corpus-{index:02}-{stem or 'root'}"


def validate_paths(repository: Path, args: argparse.Namespace) -> None:
    if args.timeout_secs <= 0:
        raise ValueError("--timeout-secs must be positive")
    work_dir = args.work_dir.resolve()
    if work_dir == repository or repository in work_dir.parents:
        raise ValueError("--work-dir must be outside the repository")
    for corpus in args.corpus:
        if not corpus.resolve().is_dir():
            raise ValueError(f"corpus is not a directory: {corpus}")
    if args.skip_fixtures and not args.corpus:
        raise ValueError("--skip-fixtures requires at least one --corpus")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    require_linux()
    repository = Path(__file__).resolve().parents[2]
    validate_paths(repository, args)

    work_dir = args.work_dir.resolve()
    target_dir = work_dir / "target"
    log_dir = work_dir / "logs"
    target_dir.mkdir(parents=True, exist_ok=True)
    log_dir.mkdir(parents=True, exist_ok=True)
    environment = sanitizer_environment(target_dir)

    phases: list[dict[str, object]] = []
    if not args.skip_fixtures:
        phases.append(
            run_phase(
                "fixtures",
                cargo_command(args.toolchain, args.target, test=True),
                cwd=repository,
                environment=environment,
                log_dir=log_dir,
                timeout_secs=args.timeout_secs,
            )
        )

    if args.corpus:
        phases.append(
            run_phase(
                "build",
                cargo_command(args.toolchain, args.target, test=False),
                cwd=repository,
                environment=environment,
                log_dir=log_dir,
                timeout_secs=args.timeout_secs,
            )
        )
        binary = target_dir / args.target / "debug" / "tokensave"
        with tempfile.TemporaryDirectory(prefix="tokensave-grammar-asan-", dir=work_dir) as tmp:
            temporary = Path(tmp)
            for index, corpus in enumerate(args.corpus, start=1):
                name = safe_name(corpus, index)
                corpus_environment = environment.copy()
                corpus_environment.update(
                    {
                        "HOME": str(temporary / f"home-{index}"),
                        "TOKENSAVE_DISABLE_SUBPROCESS": "1",
                        "TOKENSAVE_STATE_DIR": str(temporary / f"state-{index}"),
                        "TOKENSAVE_UPDATE_CHECK": "off",
                    }
                )
                Path(corpus_environment["HOME"]).mkdir(parents=True)
                phases.append(
                    run_phase(
                        name,
                        [str(binary), "init", str(corpus.resolve())],
                        cwd=repository,
                        environment=corpus_environment,
                        log_dir=log_dir,
                        timeout_secs=args.timeout_secs,
                    )
                )

    failed = any(phase["return_code"] != 0 for phase in phases)
    report = {
        "schema": 1,
        "platform": platform.platform(),
        "python": platform.python_version(),
        "repository_commit": subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repository,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip(),
        "toolchain": args.toolchain,
        "target": args.target,
        "asan_options": environment["ASAN_OPTIONS"],
        "rustflags": environment["RUSTFLAGS"],
        "cflags": environment["CFLAGS"],
        "corpora": [str(path.resolve()) for path in args.corpus],
        "phases": phases,
        "target_bytes": directory_size(target_dir),
        "success": not failed,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 1 if failed else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        print(f"grammar sanitizer: {error}", file=sys.stderr)
        sys.exit(2)
