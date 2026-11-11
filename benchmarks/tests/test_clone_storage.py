"""Storage regression tests; no downloads or actual disk exhaustion required."""
import contextlib
import errno
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("run_benchmarks", Path(__file__).resolve().parents[1] / "run_benchmarks.py")
bench = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bench)


class CloneStorageTests(unittest.TestCase):
    def test_explicit_directory_wins(self):
        with mock.patch.dict(os.environ, {"TOKENSAVE_BENCH_REPOS_DIR": "custom-cache"}):
            with mock.patch.object(bench.tempfile, "gettempdir", side_effect=AssertionError("unused")):
                self.assertEqual(bench.clone_root(), Path("custom-cache"))

    def test_standard_temp_directory(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            with mock.patch.object(bench.tempfile, "gettempdir", return_value="system-temp"):
                self.assertEqual(bench.clone_root(), Path("system-temp") / "tokensave-bench")

    def test_tmpdir_in_fresh_process(self):
        with tempfile.TemporaryDirectory() as temp:
            env = dict(os.environ, TMPDIR=temp)
            env.pop("TOKENSAVE_BENCH_REPOS_DIR", None)
            code = "import run_benchmarks; print(run_benchmarks.clone_root())"
            result = subprocess.run([os.sys.executable, "-c", code], env=env,
                                    cwd=Path(bench.__file__).parent, check=True, capture_output=True, text=True)
            self.assertEqual(Path(result.stdout.strip()), Path(temp) / "tokensave-bench")

    def test_custom_cache_is_reused_without_copy_or_clone(self):
        with tempfile.TemporaryDirectory() as temp:
            cached = Path(temp) / "fastapi"
            cached.mkdir()
            sentinel = cached / "keep.txt"
            sentinel.write_text("user-owned")
            with mock.patch.dict(os.environ, {"TOKENSAVE_BENCH_REPOS_DIR": temp}):
                with mock.patch.object(bench.subprocess, "run") as run:
                    for _ in range(2):
                        self.assertEqual(bench.clone_repo("fastapi", "unused", True), cached)
                    run.assert_not_called()
            self.assertEqual(sentinel.read_text(), "user-owned")

    def test_new_clone_uses_selected_directory(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "cache"
            with mock.patch.dict(os.environ, {"TOKENSAVE_BENCH_REPOS_DIR": str(root)}):
                with mock.patch.object(bench.subprocess, "run") as run:
                    dest = bench.clone_repo("fastapi", "https://example.invalid/repo", False)
                    self.assertEqual(dest, root / "fastapi")
                    self.assertEqual(run.call_args.args[0][-1], str(dest))
                    self.assertTrue(root.is_dir())

    def test_storage_failures_name_directory_and_workaround(self):
        failures = [OSError(errno.ENOSPC, "No space left on device"),
                    subprocess.CalledProcessError(128, ["git"], stderr="fatal: No space left on device")]
        for failure in failures:
            with self.subTest(failure=failure):
                with mock.patch.dict(os.environ, {"TOKENSAVE_BENCH_REPOS_DIR": "chosen-cache"}):
                    with mock.patch.object(bench, "clone_repo", side_effect=failure), \
                         mock.patch.object(bench.shutil, "which", return_value="tokensave"), \
                         mock.patch.object(os.sys, "argv", ["bench", "--repos", "fastapi", "--skip-token-savior"]):
                        output = io.StringIO()
                        with contextlib.redirect_stderr(output), self.assertRaises(SystemExit) as raised:
                            bench.main()
                        self.assertEqual(raised.exception.code, 1)
                        self.assertIn(str(Path("chosen-cache") / "fastapi"), output.getvalue())
                        self.assertIn("No space left on device", output.getvalue())
                        self.assertIn("TOKENSAVE_BENCH_REPOS_DIR", output.getvalue())


if __name__ == "__main__":
    unittest.main()
