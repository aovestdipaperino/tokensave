#!/usr/bin/env python3
"""Audit native compilation before accepting a sanitizer evaluation."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    args = sys.argv[1:]
    sources = [arg for arg in args if Path(arg).suffix in (".c", ".cc", ".cpp", ".cxx")]
    compiling = "-c" in args and sources
    if compiling and "-fsanitize=address" not in args:
        sys.exit("refusing native compilation without -fsanitize=address")
    cpp = any(Path(arg).suffix in (".cc", ".cpp", ".cxx") for arg in sources)
    compiler = shutil.which("clang++" if cpp else "clang")
    if compiler is None:
        sys.exit("Clang is required")
    result = subprocess.run([compiler, *args], check=False)
    if result.returncode or not compiling:
        return result.returncode
    output = Path(args[args.index("-o") + 1])
    symbols = subprocess.run(["nm", "-u", str(output)], check=True, capture_output=True, text=True)
    instrumented = "__asan_" in symbols.stdout
    if not instrumented:
        sys.exit(f"native object has no ASan references: {output}")
    with output.open("rb") as object_file:
        digest = hashlib.file_digest(object_file, "sha256").hexdigest()
    record = {
        "source": sources[0], "object": str(output), "instrumented": instrumented,
        "cwd": os.getcwd(),
        "sha256": digest,
        "command": [compiler, *args],
    }
    with open(os.environ["TOKENSAVE_ASAN_AUDIT"], "ab", buffering=0) as audit:
        import fcntl
        fcntl.flock(audit, fcntl.LOCK_EX)
        audit.write((json.dumps(record) + "\n").encode())
    return 0


if __name__ == "__main__":
    sys.exit(main())
