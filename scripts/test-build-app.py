#!/usr/bin/env python3
"""Check channel builds without invoking a compiler or needing cross toolchains."""
import contextlib
import importlib.util
import io
import json
import pathlib
import tempfile
import sys
import unittest
from unittest import mock

sys.dont_write_bytecode = True

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("build_app", ROOT / "scripts" / "build-app.py")
build_app = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build_app)


class BuildAppTests(unittest.TestCase):
    def test_one_canonical_executable_per_channel_and_platform(self):
        identity = json.loads((ROOT / "build" / "identity.json").read_text())
        for channel in ("stable", "beta"):
            for target, filename in (
                ("x86_64-pc-windows-msvc", "tokensave.exe"),
                ("x86_64-unknown-linux-gnu", "tokensave"),
                ("aarch64-apple-darwin", "tokensave"),
            ):
                with self.subTest(channel=channel, target=target), tempfile.TemporaryDirectory() as tmp:
                    # Match production path resolution (macOS symlinks and Windows short names).
                    root = pathlib.Path(tmp).resolve()
                    output = root / "target" / target / "debug"
                    executable = output / filename

                    def compile_once(command, **kwargs):
                        self.assertEqual(command[:5], ["cargo", "build", "--locked", "--bin", "tokensave"])
                        self.assertEqual("--features" in command, channel == "beta")
                        self.assertNotIn("TOKENSAVE_BETA_VERSION", kwargs["env"])
                        output.mkdir(parents=True)
                        executable.write_bytes(b"compiled application")

                    metadata = {"packages": [{
                        "manifest_path": str(root / "Cargo.toml"), "version": "7.16.0"
                    }]}
                    stdout = io.StringIO()
                    with mock.patch.object(build_app, "ROOT", root), mock.patch(
                        "sys.argv", ["build-app.py", "--channel", channel, "--target", target,
                                     "--profile", "dev", "--target-dir", str(root / "target")]
                    ), mock.patch.object(build_app.subprocess, "check_output", return_value=json.dumps(metadata)), mock.patch.object(
                        build_app.subprocess, "run", side_effect=compile_once
                    ) as run, mock.patch.dict(build_app.os.environ, {"TOKENSAVE_BETA_VERSION": "stale"}), contextlib.redirect_stdout(stdout):
                        build_app.main()
                    run.assert_called_once()
                    self.assertEqual(stdout.getvalue().strip(), str(executable))
                    self.assertEqual(list(output.iterdir()), [executable])
                    key = "windows_executable" if filename.endswith(".exe") else "unix_executable"
                    self.assertEqual(identity[channel][key], filename)

    def test_single_cargo_application_target(self):
        import tomllib
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertEqual(manifest["bin"], [{"name": "tokensave", "path": "src/main.rs"}])


if __name__ == "__main__":
    unittest.main()
