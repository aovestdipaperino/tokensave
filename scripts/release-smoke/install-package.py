#!/usr/bin/env python3
"""Install the packed .crate into a disposable prefix and exercise its CLI."""

import argparse
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time

from smoke import smoke


def cargo(*args, cwd=None):
    cmd = ["cargo", *map(str, args)]
    print(cmd, flush=True)
    subprocess.run(cmd, cwd=cwd, timeout=3600, check=True)


def run(source, version, lite):
    with tempfile.TemporaryDirectory(prefix="tokensave-package-") as directory:
        work = Path(directory)
        root = work / "installed"
        target = work / "target"
        install = ["install", "--locked", "--root", root, "--target-dir", target]
        if lite:
            install.extend(["--no-default-features", "--features", "lite"])
        if version:
            # A successful publish can precede registry index propagation.
            for attempt in range(6):
                try:
                    cargo("info", f"tokensave@{version}", "--registry", "crates-io")
                    break
                except subprocess.CalledProcessError:
                    if attempt == 5:
                        raise
                    time.sleep(10)
            cargo(*install, "--version", f"={version}", "tokensave")
        else:
            cargo("package", "--locked", "--no-verify", "--target-dir", target, cwd=source)
            (archive,) = (target / "package").glob("tokensave-*.crate")
            extracted = work / "crate"
            with tarfile.open(archive) as package:
                package.extractall(extracted, filter="data")
            cargo(*install, "--path", extracted / archive.stem)
        smoke(root / "bin" / ("tokensave.exe" if os.name == "nt" else "tokensave"))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--source", type=Path)
    mode.add_argument("--registry-version")
    parser.add_argument("--lite", action="store_true", help="Use the lite feature tier for local iteration")
    args = parser.parse_args()
    run(args.source, args.registry_version, args.lite)
