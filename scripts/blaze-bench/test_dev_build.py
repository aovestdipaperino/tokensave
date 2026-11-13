"""Tests of scripts/dev-build with stub cargo, blaze, cc and uname (#984).

Each test runs the real script under /bin/sh with a PATH that holds only
stubs and the few system tools the script needs, so neither Cargo nor Blaze
has to be installed. POSIX only.
"""

from __future__ import annotations

import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "dev-build"
CARGO_CONFIG = ROOT / ".cargo" / "config.toml"
# The shell that runs the script: /bin/sh by default. macOS's /bin/sh is bash
# in POSIX mode; run the suite with TOKENSAVE_DEV_BUILD_TEST_SHELL="bash" to
# cover it on Linux too.
TEST_SHELL = os.environ.get("TOKENSAVE_DEV_BUILD_TEST_SHELL", "/bin/sh").split()
# Resolved here: a test's PATH holds only stubs.
TEST_SHELL[0] = shutil.which(TEST_SHELL[0]) or TEST_SHELL[0]

# A stub records its arguments and environment, then exits with the code
# named by STUB_<TOOL>_EXIT (default 0).
STUB = """#!/bin/sh
n=$(ls "$STUB_LOG" | wc -l | tr -d ' ')
{{
  printf 'tool={tool}\\n'
  for a in "$@"; do printf 'arg=%s\\n' "$a"; done
  env | sed 's/^/env=/'
}} > "$STUB_LOG/$n-{tool}"
exit "${{STUB_{upper}_EXIT:-0}}"
"""
BLAZE_STUB = """#!/bin/sh
if [ "${{1:-}}" = --version ]; then
  echo "blaze 0.0.0-stub"
  exit "${{STUB_BLAZE_VERSION_EXIT:-0}}"
fi
""" + STUB.split("\n", 1)[1]


@unittest.skipIf(os.name != "posix", "dev-build is a POSIX shell script")
class DevBuildTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="dev-build-test-"))
        self.bin = self.tmp / "bin"
        self.log = self.tmp / "log"
        self.bin.mkdir()
        self.log.mkdir()
        for tool in ("awk", "dirname", "sed", "ls", "wc", "tr", "env"):
            real = shutil.which(tool)
            self.assertIsNotNone(real, f"{tool} is needed by the test")
            os.symlink(real, self.bin / tool)
        self.stub("cargo")
        self.stub("blaze")
        self.stub("cc")
        self.set_os("Linux")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def stub(self, tool: str, text: str | None = None):
        if text is None:
            text = (BLAZE_STUB if tool == "blaze" else STUB).format(
                tool=tool, upper=tool.upper())
        path = self.bin / tool
        path.write_text(text)
        path.chmod(path.stat().st_mode | stat.S_IEXEC)

    def remove(self, tool: str):
        (self.bin / tool).unlink()

    def set_os(self, name: str):
        self.stub("uname", f"#!/bin/sh\necho {name}\n")

    def run_script(self, *args: str, env: dict[str, str] | None = None):
        child_env = {"PATH": str(self.bin), "STUB_LOG": str(self.log),
                     "HOME": str(self.tmp)}
        child_env.update(env or {})
        return subprocess.run([*TEST_SHELL, str(SCRIPT), *args], env=child_env,
                              capture_output=True, text=True, timeout=60,
                              check=False)

    def calls(self) -> list[dict]:
        records = []
        for path in sorted(self.log.iterdir(), key=lambda p: int(p.name.split("-")[0])):
            record = {"args": [], "env": {}}
            for line in path.read_text().splitlines():
                key, _, value = line.partition("=")
                if key == "tool":
                    record["tool"] = value
                elif key == "arg":
                    record["args"].append(value)
                elif key == "env":
                    name, _, val = value.partition("=")
                    record["env"][name] = val
            records.append(record)
        return records

    def tools_called(self) -> list[str]:
        return [c["tool"] for c in self.calls()]

    # Selection -----------------------------------------------------------

    def test_auto_prefers_blaze(self):
        done = self.run_script("test", "--workspace", "--locked")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self.tools_called(), ["blaze"])
        self.assertEqual(self.calls()[0]["args"], ["test", "--workspace", "--locked"])
        self.assertIn("using blaze", done.stderr)

    def test_macos_prefers_blaze_with_command_line_tools(self):
        self.set_os("Darwin")
        self.remove("cc")
        self.stub("xcode-select", "#!/bin/sh\necho /Library/Developer\n")
        self.assertEqual(self.run_script("build").returncode, 0)
        self.assertEqual(self.tools_called(), ["blaze"])

    def test_macos_without_command_line_tools_uses_cargo(self):
        self.set_os("Darwin")
        self.stub("xcode-select", "#!/bin/sh\nexit 2\n")
        done = self.run_script("build")
        self.assertEqual(self.tools_called(), ["cargo"])
        self.assertIn("command line tools", done.stderr)

    def test_missing_blaze_falls_back_to_cargo(self):
        self.remove("blaze")
        done = self.run_script("test", "--locked", env={"STUB_CARGO_EXIT": "3"})
        self.assertEqual(done.returncode, 3)
        self.assertEqual(self.tools_called(), ["cargo"])
        self.assertIn("blaze is not installed", done.stderr)

    def test_broken_blaze_falls_back_to_cargo(self):
        done = self.run_script("build", env={"STUB_BLAZE_VERSION_EXIT": "1"})
        self.assertEqual(done.returncode, 0)
        self.assertEqual(self.tools_called(), ["cargo"])

    def test_missing_linker_falls_back_to_cargo(self):
        self.remove("cc")
        done = self.run_script("build")
        self.assertEqual(self.tools_called(), ["cargo"])
        self.assertIn("cc", done.stderr)

    def test_windows_uses_cargo(self):
        for name in ("MINGW64_NT-10.0-26300", "MSYS_NT-10.0", "CYGWIN_NT-10.0"):
            with self.subTest(name=name):
                shutil.rmtree(self.log)
                self.log.mkdir()
                self.set_os(name)
                done = self.run_script("test", "--workspace")
                self.assertEqual(done.returncode, 0)
                self.assertEqual(self.tools_called(), ["cargo"])

    def test_explicit_cargo_flag(self):
        self.assertEqual(self.run_script("--cargo", "build", "--locked").returncode, 0)
        self.assertEqual(self.tools_called(), ["cargo"])
        self.assertEqual(self.calls()[0]["args"], ["build", "--locked"])

    def test_explicit_cargo_variable(self):
        self.run_script("build", env={"TOKENSAVE_BUILD_RUNNER": "cargo"})
        self.assertEqual(self.tools_called(), ["cargo"])

    def test_flag_overrides_variable(self):
        self.run_script("--blaze", "build", env={"TOKENSAVE_BUILD_RUNNER": "cargo"})
        self.assertEqual(self.tools_called(), ["blaze"])

    def test_invalid_variable_is_an_error(self):
        done = self.run_script("build", env={"TOKENSAVE_BUILD_RUNNER": "bazel"})
        self.assertEqual(done.returncode, 2)
        self.assertEqual(self.tools_called(), [])

    def test_no_arguments_is_an_error(self):
        self.assertEqual(self.run_script().returncode, 2)
        self.assertEqual(self.tools_called(), [])

    def test_other_subcommands_use_cargo(self):
        for sub in ("clippy", "fmt", "doc", "install", "fetch"):
            with self.subTest(sub=sub):
                shutil.rmtree(self.log)
                self.log.mkdir()
                self.run_script(sub)
                self.assertEqual(self.tools_called(), ["cargo"])

    def test_unsupported_arguments_use_cargo(self):
        cases = [
            ["check", "--workspace", "--all-targets"],
            ["build", "--target-dir", "/tmp/t"],
            ["build", "--target-dir=/tmp/t"],
            ["build", "--message-format=json"],
            ["build", "-Zunstable-options"],
            ["test", "--tests"],
            ["test", "--doc"],
            ["test", "--test", "integration", "--test", "env_isolated"],
            ["build", "--target", "a", "--target", "b"],
        ]
        for args in cases:
            with self.subTest(args=args):
                shutil.rmtree(self.log)
                self.log.mkdir()
                done = self.run_script(*args)
                self.assertEqual(self.tools_called(), ["cargo"], done.stderr)
                self.assertEqual(self.calls()[0]["args"], args)

    def test_arguments_after_double_dash_are_not_inspected(self):
        self.run_script("test", "--test", "integration", "--", "--test-threads=1",
                        "--tests")
        self.assertEqual(self.tools_called(), ["blaze"])

    def test_one_test_target_uses_blaze(self):
        self.run_script("test", "--test", "integration", "sync")
        self.assertEqual(self.tools_called(), ["blaze"])

    # Failures and exit codes ---------------------------------------------

    def test_blaze_failure_reruns_with_cargo(self):
        done = self.run_script("test", "--locked",
                               env={"STUB_BLAZE_EXIT": "101", "STUB_CARGO_EXIT": "7"})
        self.assertEqual(done.returncode, 7)
        self.assertEqual(self.tools_called(), ["blaze", "cargo"])
        self.assertEqual(self.calls()[1]["args"], ["test", "--locked"])
        self.assertIn("whose result counts", done.stderr)

    def test_blaze_failure_then_cargo_success(self):
        done = self.run_script("build", env={"STUB_BLAZE_EXIT": "1"})
        self.assertEqual(done.returncode, 0)
        self.assertEqual(self.tools_called(), ["blaze", "cargo"])

    def test_run_is_not_repeated(self):
        done = self.run_script("run", "--", "status", env={"STUB_BLAZE_EXIT": "4"})
        self.assertEqual(done.returncode, 4)
        self.assertEqual(self.tools_called(), ["blaze"])

    def test_interrupt_is_not_repeated(self):
        done = self.run_script("test", env={"STUB_BLAZE_EXIT": "130"})
        self.assertEqual(done.returncode, 130)
        self.assertEqual(self.tools_called(), ["blaze"])

    def test_forced_blaze_does_not_fall_back(self):
        done = self.run_script("--blaze", "test", env={"STUB_BLAZE_EXIT": "101"})
        self.assertEqual(done.returncode, 101)
        self.assertEqual(self.tools_called(), ["blaze"])

    def test_forced_blaze_when_unusable_is_an_error(self):
        self.remove("blaze")
        done = self.run_script("--blaze", "test")
        self.assertEqual(done.returncode, 2)
        self.assertEqual(self.tools_called(), [])
        self.assertIn("cannot use Blaze", done.stderr)

    def test_forced_blaze_on_windows_is_an_error(self):
        self.set_os("MINGW64_NT-10.0")
        self.assertEqual(self.run_script("--blaze", "build").returncode, 2)
        self.assertEqual(self.tools_called(), [])

    # The [env] table of .cargo/config.toml -------------------------------

    def cargo_env_table(self) -> dict:
        if sys.version_info < (3, 11):
            self.skipTest("tomllib needs Python 3.11")
        import tomllib

        with open(CARGO_CONFIG, "rb") as config:
            return tomllib.load(config).get("env", {})

    def test_blaze_gets_every_cargo_env_entry(self):
        table = self.cargo_env_table()
        self.assertTrue(table, "the repository's .cargo/config.toml has an [env] table")
        self.run_script("test")
        seen = self.calls()[0]["env"]
        for name, entry in table.items():
            value = entry["value"] if isinstance(entry, dict) else entry
            with self.subTest(name=name):
                self.assertIn(name, seen)
                self.assertEqual(seen[name], value)

    def test_cargo_env_does_not_override_unforced_values(self):
        table = self.cargo_env_table()
        unforced = [name for name, entry in table.items()
                    if isinstance(entry, dict) and not entry.get("force", False)]
        self.assertTrue(unforced)
        name = unforced[0]
        self.run_script("test", env={name: "from-the-caller"})
        self.assertEqual(self.calls()[0]["env"][name], "from-the-caller")

    def test_cargo_runs_with_the_callers_environment(self):
        self.remove("blaze")
        self.run_script("test")
        self.assertNotIn("TOKENSAVE_SKIP_AGENT_MAINTENANCE", self.calls()[0]["env"])


if __name__ == "__main__":
    unittest.main()
