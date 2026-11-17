import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest


HERE = Path(__file__).resolve().parent


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue((HERE / "run.py").is_file(), "ASan runner is missing")
        spec = importlib.util.spec_from_file_location("grammar_asan", HERE / "run.py")
        self.runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.runner)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def command(self, code, timeout=2, limit=8192):
        return self.runner.run_command(
            [sys.executable, "-B", "-c", code], self.root, dict(os.environ),
            self.root / "child.log", timeout, limit,
        )

    def test_command_preserves_exit_code_and_output(self):
        result = self.command("print('parsed'); raise SystemExit(7)")
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["returncode"], 7)
        self.assertEqual((self.root / "child.log").read_text().strip(), "parsed")

    def test_successful_command(self):
        self.assertEqual(self.command("print('parsed')")["status"], "passed")

    def test_timeout_terminates_descendants(self):
        code = (
            "import subprocess,sys,time; "
            "subprocess.Popen([sys.executable,'-c',"
            "\"import time,pathlib; time.sleep(.5); pathlib.Path('escaped').touch()\"]); "
            "time.sleep(10)"
        )
        result = self.command(code, timeout=.15)
        self.assertEqual(result["status"], "timed_out")
        import time
        time.sleep(.6)
        self.assertFalse((self.root / "escaped").exists())

    def test_output_is_bounded(self):
        result = self.command("import os; os.write(1, b'x' * 100000)", limit=1024)
        self.assertEqual(result["status"], "output_limit")
        self.assertLessEqual((self.root / "child.log").stat().st_size, 1024)

    @unittest.skipUnless(sys.platform == "linux", "Linux evaluation entry point")
    def test_termination_records_incomplete_and_reaps_compiler(self):
        import time
        tools = self.root / "bin"
        tools.mkdir()
        started = self.root / "started"
        escaped = self.root / "escaped"
        compiler = tools / "rustc"
        compiler.write_text(
            f"#!{sys.executable}\nimport pathlib,subprocess,sys,time\n"
            f"subprocess.Popen([sys.executable, '-c', {repr('import pathlib,time; time.sleep(1); pathlib.Path(' + repr(str(escaped)) + ').touch()')}])\n"
            f"pathlib.Path({str(started)!r}).touch()\ntime.sleep(10)\n"
        )
        compiler.chmod(0o755)
        env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"])
        output = self.root / "result"
        child = subprocess.Popen([sys.executable, "-B", str(HERE / "run.py"), "--output", str(output)],
                                 env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            deadline = time.monotonic() + 3
            while not started.exists() and time.monotonic() < deadline:
                time.sleep(.02)
            self.assertTrue(started.exists(), "compiler did not start")
            child.send_signal(signal.SIGTERM)
            self.assertEqual(child.wait(timeout=3), 1)
            report = json.loads((output / "report.json").read_text())
            self.assertEqual(report["status"], "incomplete")
            self.assertEqual(report["error"], "KeyboardInterrupt")
            time.sleep(1.1)
            self.assertFalse(escaped.exists())
        finally:
            if child.poll() is None:
                child.kill()
            child.wait()

    def test_existing_output_is_preserved(self):
        sentinel = self.root / "keep"
        sentinel.write_text("user data")
        with self.assertRaises(FileExistsError):
            self.runner.reserve_output(self.root)
        self.assertEqual(sentinel.read_text(), "user data")

    def test_corpus_snapshots_raw_bytes_and_rejects_oversize(self):
        corpus = self.root / "corpus"
        corpus.mkdir()
        (corpus / "binary").write_bytes(b"\xff\x00abc")
        output = self.root / "inputs"
        output.mkdir()
        records = self.runner.snapshot_inputs([corpus], output, max_bytes=5)
        self.assertEqual(len(records), 1)
        self.assertEqual((output / records[0]["name"]).read_bytes(), b"\xff\x00abc")
        self.assertEqual(records[0]["bytes"], 5)
        with self.assertRaises(ValueError):
            self.runner.snapshot_inputs([corpus], output, max_bytes=4)

    def test_empty_or_missing_corpus_is_refused(self):
        for corpus in (self.root, self.root / "missing"):
            with self.subTest(corpus=corpus), self.assertRaises(ValueError):
                self.runner.snapshot_inputs([corpus], self.root)

    @unittest.skipIf(os.geteuid() == 0, "root can read permission-denied directories")
    def test_unreadable_subdirectory_is_not_silently_skipped(self):
        corpus = self.root / "corpus"
        corpus.mkdir()
        (corpus / "readable").write_bytes(b"ok")
        denied = corpus / "denied"
        denied.mkdir()
        (denied / "missing").write_bytes(b"must not be skipped")
        output = self.root / "inputs"
        output.mkdir()
        denied.chmod(0)
        try:
            with self.assertRaises(OSError):
                self.runner.snapshot_inputs([corpus], output)
        finally:
            denied.chmod(0o700)

    def test_discovery_stops_before_enumerating_an_oversize_corpus(self):
        from unittest import mock
        corpus = self.root / "corpus"
        corpus.mkdir()
        for index in range(1002):
            (corpus / f"{index:04d}").touch()
        output = self.root / "inputs"
        output.mkdir()
        discovered = 0
        original_rglob = Path.rglob
        original_scandir = os.scandir

        def count(iterator):
            nonlocal discovered
            for item in iterator:
                discovered += 1
                yield item

        class Scandir:
            def __init__(self, path):
                self.iterator = original_scandir(path)

            def __enter__(self):
                return count(self.iterator)

            def __exit__(self, *_args):
                self.iterator.close()

        with mock.patch.object(Path, "rglob", lambda path, pattern: count(original_rglob(path, pattern))), \
             mock.patch.object(os, "scandir", Scandir):
            with self.assertRaises(ValueError):
                self.runner.snapshot_inputs([corpus], output)
        self.assertLessEqual(discovered, 1001)

    def test_control_requires_actual_sanitizer_diagnostic(self):
        log = self.root / "control.log"
        result = {"status": "failed", "returncode": 1}
        log.write_text("compiler or loader failed")
        self.assertFalse(self.runner.control_detected(result, log))
        log.write_text("ERROR: AddressSanitizer: heap-buffer-overflow")
        self.assertTrue(self.runner.control_detected(result, log))
        self.assertFalse(self.runner.control_detected({"status": "passed", "returncode": 0}, log))

    def test_resume_requires_matching_build_identity_and_native_audit(self):
        self.assertTrue(callable(getattr(self.runner, "build_paths", None)), "resume validation missing")
        previous = self.root / "previous"
        previous.mkdir()
        metadata = {"lock_sha256": "lock-a", "target": "aarch64-unknown-linux-gnu",
                    "toolchain": "nightly-2026-11-11"}
        (previous / "target").mkdir()
        audit = previous / "native-audit.jsonl"
        audit.write_text('{"instrumented": true}\n')
        (previous / "report.json").write_text(json.dumps(metadata))
        paths = self.runner.build_paths(self.root / "new", previous, metadata)
        self.assertTrue(paths[0].samefile(previous / "target"))
        self.assertTrue(paths[1].samefile(audit))
        resumed = self.root / "resumed"
        resumed.mkdir()
        (resumed / "report.json").write_text(json.dumps(dict(
            metadata, build_target_dir=str(previous / "target"), native_audit=str(audit))))
        resumed_paths = self.runner.build_paths(self.root / "newer", resumed, metadata)
        self.assertTrue(resumed_paths[0].samefile(paths[0]))
        self.assertTrue(resumed_paths[1].samefile(paths[1]))
        for field in metadata:
            with self.subTest(field=field):
                mismatch = dict(metadata, **{field: "different"})
                with self.assertRaises(ValueError):
                    self.runner.build_paths(self.root / "new", previous, mismatch)
        audit.unlink()
        with self.assertRaises(ValueError):
            self.runner.build_paths(self.root / "new", previous, metadata)

    @unittest.skipUnless(shutil.which("clang") and shutil.which("nm"), "Clang and nm required")
    def test_compiler_records_native_instrumentation(self):
        source = self.root / "scanner.c"
        source.write_text("int scan(int *p) { return p[10]; }\n")
        audit = self.root / "audit.jsonl"
        env = dict(os.environ, TOKENSAVE_ASAN_AUDIT=str(audit))
        command = [sys.executable, str(HERE / "compiler.py"), "-c", str(source),
                   "-o", str(self.root / "scanner.o")]
        rejected = subprocess.run(command, env=env, capture_output=True, text=True)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertFalse(audit.exists())
        accepted = subprocess.run(command + ["-fsanitize=address"], env=env,
                                  capture_output=True, text=True)
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        records = [json.loads(line) for line in audit.read_text().splitlines()]
        self.assertEqual(len(records), 1)
        self.assertTrue(records[0]["instrumented"])
        self.assertEqual(records[0]["source"], str(source))


if __name__ == "__main__":
    unittest.main()
