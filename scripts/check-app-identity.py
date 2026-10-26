#!/usr/bin/env python3
"""Smoke-check the packaged CLI identity and, on Windows, PE resources."""
import argparse
import json
import pathlib
import struct
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--channel", choices=("stable", "beta"), required=True)
parser.add_argument("executable", type=pathlib.Path)
args = parser.parse_args()
identity = json.loads((ROOT / "build" / "identity.json").read_text())[args.channel]
executable = args.executable.resolve()
windows = sys.platform == "win32"
expected_name = identity["windows_executable" if windows else "unix_executable"]
assert executable.name == expected_name, (executable.name, expected_name)
version = subprocess.check_output([str(executable), "--version"], text=True).strip()
assert version.startswith(identity["cli_name"] + " "), version
if args.channel == "beta":
    assert "TokenSave Beta" in version and "-beta." in version, version
    icon = (ROOT / identity["icon"]).read_bytes()
    reserved, kind, count = struct.unpack_from("<HHH", icon)
    assert (reserved, kind) == (0, 1)
    sizes = {icon[6 + index * 16] or 256 for index in range(count)}
    assert {16, 20, 24, 32, 40, 48, 64, 128, 256} <= sizes, sizes
    if windows:
        # Pass the path as a parameter, never interpolate it into PowerShell code.
        info = subprocess.check_output([
            "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
            str(ROOT / "scripts" / "check-app-resources.ps1"), str(executable)
        ], text=True)
        assert "TokenSave Beta" in info, info
else:
    assert "Beta" not in version and "-beta." not in version, version
print(version)
