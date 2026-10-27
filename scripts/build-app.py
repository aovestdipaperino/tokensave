#!/usr/bin/env python3
"""Build a channel release using the canonical Cargo executable."""
import argparse
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--channel", choices=("stable", "beta"), required=True)
    parser.add_argument("--target", help="Cargo target triple (defaults to host)")
    parser.add_argument("--lite", action="store_true")
    parser.add_argument("--version", help="Beta version matching the package base, e.g. 7.16.0-beta.2")
    parser.add_argument("--target-dir", type=pathlib.Path, help="Override the isolated Cargo build directory")
    parser.add_argument("--profile", choices=("dev", "release"), default="release")
    args = parser.parse_args()
    if args.channel == "stable":
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--offline", "--no-deps", "--format-version", "1"],
            cwd=ROOT, text=True))
        package = next(package for package in metadata["packages"]
                       if pathlib.Path(package["manifest_path"]).resolve() == ROOT / "Cargo.toml")
        if "-" in package["version"].split("+")[0]:
            parser.error("a prerelease Cargo package selects beta identity; use --channel beta")
    # Separate build trees allow stable and beta builds without overwriting each other.
    target_dir = args.target_dir.resolve() if args.target_dir else ROOT / "target" / args.channel
    command = ["cargo", "build", "--locked", "--bin", "tokensave",
               "--profile", args.profile, "--target-dir", str(target_dir)]
    if args.channel == "beta":
        command += ["--features", "beta"]
    if args.lite:
        command += ["--no-default-features", "--features", "lite"]
    if args.target:
        command += ["--target", args.target]
    environment = os.environ.copy()
    # Prevent a beta version override from contaminating a subsequent stable build.
    environment.pop("TOKENSAVE_BETA_VERSION", None)
    if args.version:
        if args.channel != "beta":
            parser.error("--version is only supported for beta builds")
        environment["TOKENSAVE_BETA_VERSION"] = args.version.removeprefix("v")
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    windows = "windows" in args.target if args.target else sys.platform == "win32"
    output = target_dir
    if args.target:
        output /= args.target
    output /= "release" if args.profile == "release" else "debug"
    print(output / ("tokensave.exe" if windows else "tokensave"))


if __name__ == "__main__":
    main()
