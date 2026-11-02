"""Real child lifecycle tests. Run under an outer timeout as a hang guard."""
import importlib.util
import os
from pathlib import Path
import signal
import select
import subprocess
import sys
import time
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location('bench', Path(__file__).resolve().parents[1] / 'run_benchmarks.py')
bench = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bench)
HANDSHAKE = 'import sys,json; sys.stdin.readline(); print(json.dumps({"id":1,"result":{}}),flush=True); '


class McpCleanupTests(unittest.TestCase):
    def client(self, code, **options):
        client = bench.TokensaveMcp(Path('unused'), initialization_timeout=.15,
                                   request_timeout=.15, shutdown_timeout=.15, **options)
        spawn = subprocess.Popen
        children = []
        def start(*args, **kwargs):
            child = spawn([sys.executable, '-u', '-c', code], **kwargs)
            children.append(child)
            return child
        patch = mock.patch.object(bench.subprocess, 'Popen', side_effect=start)
        patch.start()
        self.addCleanup(patch.stop)
        def repair():
            for child in children:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait(timeout=2)
                child.stdin.close()
                child.stdout.close()
        self.addCleanup(repair)
        return client, children

    def assert_closed(self, client, children):
        self.assertIsNone(client.proc)
        for child in children:
            self.assertIsNotNone(child.returncode)
            self.assertTrue(child.stdin.closed)
            self.assertTrue(child.stdout.closed)

    def test_success_repeated_cleanup_and_body_error(self):
        for error in (False, True):
            client, children = self.client(HANDSHAKE + 'sys.stdin.read()')
            try:
                with client:
                    if error:
                        raise ValueError('body')
            except ValueError:
                self.assertTrue(error)
            client.__exit__()
            self.assert_closed(client, children)

    def test_stalled_initialize_and_partial_line(self):
        for prefix in ('', 'print("partial",end="",flush=True); '):
            client, children = self.client('import time; ' + prefix + 'time.sleep(60)')
            with self.assertRaises(TimeoutError):
                with client:
                    pass
            self.assert_closed(client, children)

    def test_closed_stdout(self):
        client, children = self.client('import os,sys; sys.stdin.readline(); os.close(1); sys.stdin.read()')
        with self.assertRaisesRegex(RuntimeError, 'closed stdout'):
            client.__enter__()
        self.assert_closed(client, children)

    def test_stalled_notification_write(self):
        client, children = self.client(HANDSHAKE + 'import time; time.sleep(60)')
        notify = client._notify
        with mock.patch.object(client, '_notify', side_effect=lambda *args: notify('x', {'data':'x' * 1000000})):
            with self.assertRaises(TimeoutError):
                client.__enter__()
        self.assert_closed(client, children)

    def test_utf8_notifications_and_followup_response(self):
        code = HANDSHAKE + 'sys.stdin.readline(); req=json.loads(sys.stdin.readline()); print("[]",flush=True); print(json.dumps({"method":"notice"}),flush=True); print(json.dumps({"id":req["id"],"result":"café"},ensure_ascii=False),flush=True); sys.stdin.read()'
        client, children = self.client(code)
        with client:
            self.assertEqual(client.call_tool('hello', {})['result'], 'café')
        self.assert_closed(client, children)

    def test_request_deadline(self):
        client, children = self.client(HANDSHAKE + 'sys.stdin.read()')
        with client:
            with self.assertRaises(TimeoutError):
                client.call_tool('stalled', {})
        self.assert_closed(client, children)

    def test_repeated_interruption_during_shutdown_preserves_body_error(self):
        client, children = self.client(HANDSHAKE + 'sys.stdin.read()')
        with self.assertRaisesRegex(ValueError, 'original'):
            with client:
                wait = client.proc.wait
                interruptions = [KeyboardInterrupt(), KeyboardInterrupt()]
                def interrupted(*args, **kwargs):
                    if interruptions:
                        raise interruptions.pop()
                    return wait(*args, **kwargs)
                client.proc.wait = interrupted
                raise ValueError('original')
        self.assert_closed(client, children)

    def test_interruption_on_normal_exit_is_reraised(self):
        client, children = self.client(HANDSHAKE + 'sys.stdin.read()')
        with self.assertRaises(KeyboardInterrupt):
            with client:
                wait = client.proc.wait
                # Use the actual wait after the first interrupted attempt.
                calls = iter([True, False])
                def interrupted(*args, **kwargs):
                    if next(calls, False):
                        raise KeyboardInterrupt
                    return wait(*args, **kwargs)
                client.proc.wait = interrupted
        self.assert_closed(client, children)

    def test_overlapping_clients(self):
        one, children = self.client(HANDSHAKE + 'sys.stdin.read()')
        with one:
            two, other = self.client(HANDSHAKE + 'sys.stdin.read()')
            with two:
                pass
            self.assertIsNone(one.proc.poll())
        self.assert_closed(one, children)
        self.assert_closed(two, other)

    def test_descendant_holding_stdout(self):
        code = HANDSHAKE + 'import subprocess,time; subprocess.Popen([sys.executable,"-c","import time; time.sleep(60)"]); sys.exit(0)'
        client, children = self.client(code)
        with client:
            descriptor = os.dup(client.proc.stdout.fileno())
            self.addCleanup(os.close, descriptor)
            with self.assertRaises(TimeoutError):
                client.call_tool('stalled', {})
        self.assert_closed(client, children)
        self.assertTrue(select.select([descriptor], [], [], 1)[0])
        self.assertEqual(os.read(descriptor, 1), b'')

    def test_entry_interruption_preserves_original(self):
        client, children = self.client('import sys; sys.stdin.read()')
        error = KeyboardInterrupt()
        with mock.patch.object(client, '_request', side_effect=error):
            with self.assertRaises(KeyboardInterrupt) as raised:
                client.__enter__()
        self.assertIs(raised.exception, error)
        self.assert_closed(client, children)

    def test_oversized_unterminated_response(self):
        client, children = self.client('import sys; sys.stdin.readline(); sys.stdout.write("x" * (17 * 1024 * 1024)); sys.stdout.flush(); sys.stdin.read()')
        client.initialization_timeout = 3
        with self.assertRaisesRegex(RuntimeError, '16 MiB'):
            client.__enter__()
        self.assert_closed(client, children)

    def test_invalid_timeouts_and_spawn_failure(self):
        for value in (0, -1, float('nan'), float('inf')):
            with self.assertRaises(ValueError):
                bench.TokensaveMcp(Path('unused'), initialization_timeout=value)
        client = bench.TokensaveMcp(Path('unused'))
        with mock.patch.object(bench.subprocess, 'Popen', side_effect=OSError('spawn')):
            with self.assertRaises(OSError):
                client.__enter__()
        client.__exit__()


if __name__ == '__main__':
    unittest.main()
