"""MCP client lifecycle regression tests without a TokenSave build or database."""
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("run_benchmarks", Path(__file__).resolve().parents[1] / "run_benchmarks.py")
bench = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bench)


def process():
    child = mock.Mock()
    child.stdin = io.StringIO()
    child.stdout = io.StringIO('{"id":1,"result":{}}\n')
    child.wait.return_value = 0
    return child


class McpCleanupTests(unittest.TestCase):
    def assert_closed(self, child, client):
        self.assertTrue(child.stdin.closed)
        self.assertTrue(child.stdout.closed)
        child.wait.assert_called()
        self.assertIsNone(client.proc)

    def test_success_and_repeated_cleanup(self):
        child = process()
        client = bench.TokensaveMcp(Path("project"))
        with mock.patch.object(bench.subprocess, "Popen", return_value=child):
            with client:
                self.assertIs(client.proc, child)
        self.assert_closed(child, client)
        client.__exit__()
        self.assertEqual(child.wait.call_count, 1)

    def test_body_failure(self):
        child = process()
        client = bench.TokensaveMcp(Path("project"))
        with mock.patch.object(bench.subprocess, "Popen", return_value=child):
            with self.assertRaisesRegex(ValueError, "body failed"):
                with client:
                    raise ValueError("body failed")
        self.assert_closed(child, client)

    def test_initialization_failure_and_interruption(self):
        for method in ["_request", "_notify"]:
            for error in [RuntimeError("startup failed"), KeyboardInterrupt()]:
                with self.subTest(method=method, error=type(error).__name__):
                    child = process()
                    client = bench.TokensaveMcp(Path("project"))
                    with mock.patch.object(bench.subprocess, "Popen", return_value=child), \
                         mock.patch.object(client, method, side_effect=error):
                        with self.assertRaises(type(error)) as raised:
                            client.__enter__()
                    self.assertIs(raised.exception, error)
                    self.assert_closed(child, client)

    def test_spawn_failure(self):
        client = bench.TokensaveMcp(Path("project"))
        with mock.patch.object(bench.subprocess, "Popen", side_effect=OSError("cannot spawn")):
            with self.assertRaises(OSError):
                client.__enter__()
        self.assertIsNone(client.proc)
        client.__exit__()

    def test_timeout_kills_and_reaps(self):
        child = process()
        child.wait.side_effect = [subprocess.TimeoutExpired("fake", 3), 0]
        client = bench.TokensaveMcp(Path("project"))
        with mock.patch.object(bench.subprocess, "Popen", return_value=child):
            with client:
                pass
        self.assert_closed(child, client)
        child.kill.assert_called_once()
        self.assertEqual(child.wait.call_count, 2)

    def test_independent_clients(self):
        first, second = process(), process()
        one, two = bench.TokensaveMcp(Path("one")), bench.TokensaveMcp(Path("two"))
        with mock.patch.object(bench.subprocess, "Popen", side_effect=[first, second]):
            with one:
                with two:
                    self.assertIs(one.proc, first)
                    self.assertIs(two.proc, second)
                self.assertFalse(first.stdin.closed)
        self.assert_closed(first, one)
        self.assert_closed(second, two)

    def test_real_child_is_reaped_after_failed_entry(self):
        # Close stdout during initialize, then wait for EOF on stdin. Without
        # entry cleanup, the child remains alive waiting for that EOF.
        spawn = subprocess.Popen
        children = []
        def start(*args, **kwargs):
            child = spawn([sys.executable, "-u", "-c",
                           "import os,sys; sys.stdin.readline(); os.close(1); sys.stdin.read()"], **kwargs)
            children.append(child)
            return child
        client = bench.TokensaveMcp(Path("unused"))
        try:
            with mock.patch.object(bench.subprocess, "Popen", side_effect=start):
                with self.assertRaisesRegex(RuntimeError, "closed stdout"):
                    client.__enter__()
            self.assertIsNotNone(children[0].returncode)
            self.assertTrue(children[0].stdin.closed)
            self.assertTrue(children[0].stdout.closed)
            self.assertIsNone(client.proc)
        finally:
            for child in children:
                child.stdin.close()
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
                child.stdout.close()


if __name__ == "__main__":
    unittest.main()
