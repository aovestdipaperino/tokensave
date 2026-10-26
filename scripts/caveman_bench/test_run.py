import unittest
from run import summarize


class TelemetryTests(unittest.TestCase):
    def test_counts_usage_and_only_identical_reads(self):
        read = dict(type='file_read', path='/repo/src/lib.rs', range='1:10', sha256='a' * 64)
        result = summarize([
            dict(type='usage', input_tokens=100, output_tokens=20, cost_usd=.02),
            dict(type='usage', input_tokens=50, output_tokens=5, cost_usd=.01),
            dict(type='mcp_call', name='tokensave_search'), read, read,
            dict(read, sha256='b' * 64), dict(read, range='11:20')])
        self.assertEqual(result, dict(input_tokens=150, output_tokens=25, cost_usd=.03,
                                     mcp_calls=1, redundant_file_reads=1))

    def test_missing_invalid_or_estimated_usage_rejected(self):
        for events in ([], [dict(type='usage', input_tokens=-1, output_tokens=0, cost_usd=0)],
                       [dict(type='usage', input_tokens=1, output_tokens=0, cost_usd=float('nan'))],
                       [dict(type='estimated_usage')]):
            with self.assertRaises(ValueError):
                summarize(events)


    def test_missing_or_ambiguous_tool_observations_rejected(self):
        usage = dict(type='usage', input_tokens=1, output_tokens=1, cost_usd=.01)
        read = dict(type='file_read', path='/repo/source.py', range='1:2', sha256='a' * 64)
        for event in (dict(type='mcp_call'), dict(type='mcp_call', name=''),
                      dict(read, path='source.py'), dict(read, range='all'),
                      dict(read, range='3:2'), dict(read, sha256='estimated')):
            with self.assertRaises(ValueError):
                summarize([usage, event])

class TrialTests(unittest.TestCase):
    def test_paired_runner_uses_fresh_checkouts_and_independent_verification(self):
        import contextlib
        import io
        import json
        from pathlib import Path
        import subprocess
        import sys
        import tempfile
        from run import run

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = root / 'repo'
            repo.mkdir()
            subprocess.run(['git', 'init', '--quiet', str(repo)], check=True)
            (repo / 'answer').write_text('before')
            subprocess.run(['git', 'add', 'answer'], cwd=repo, check=True)
            subprocess.run(['git', '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                            'commit', '--quiet', '-m', 'fixture'], cwd=repo, check=True)
            commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
            driver = root / 'driver.py'
            driver.write_text("""import json, pathlib, sys
request = json.load(sys.stdin)
trace = pathlib.Path(request['trace_directory']) / 'native.json'
trace.write_text('synthetic')
import hashlib
assert pathlib.Path('answer').read_text() == 'before'
pathlib.Path('answer').write_text('after')
print(json.dumps({'complete': True, 'trace_sha256': hashlib.sha256(trace.read_bytes()).hexdigest(), 'trace_file': str(trace),
 'events': [{'type': 'usage', 'input_tokens': 1, 'output_tokens': 1, 'cost_usd': 0.01}]}))
""")
            metadata = dict(tokensave_commit=commit, agent_version='test', model='test',
                            provider='test', pricing_snapshot='synthetic')
            config = dict(repository=str(repo), commit=commit, repeats=2,
                arms={arm: dict(driver=[sys.executable, str(driver)], metadata=metadata) for arm in ('A', 'B')},
                tasks=[dict(id='fixture', prompt='synthetic', verify=[sys.executable, '-c',
                    "from pathlib import Path; assert Path('answer').read_text() == 'after'"])])
            output = root / 'results'
            with contextlib.redirect_stdout(io.StringIO()):
                run(config, output)
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual(summary['A']['successful'], 2)
            self.assertEqual(summary['B']['cost_per_successful_task'], .01)
            self.assertEqual((repo / 'answer').read_text(), 'before')
            self.assertEqual(len(list((output / 'private-traces').glob('*/native.json'))), 4)
            rows = [json.loads(line) for line in (output / 'trials.jsonl').read_text().splitlines()]
            self.assertEqual([row['arm'] for row in rows], ['A', 'B', 'B', 'A'])



class TraceTests(unittest.TestCase):
    def test_retained_trace_must_match_digest_and_private_directory(self):
        import hashlib
        from pathlib import Path
        import tempfile
        from run import verify_trace
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            private = root / 'private'
            private.mkdir()
            path = private / 'trace'
            path.write_bytes(b'original')
            telemetry = dict(trace_file=str(path), trace_sha256=hashlib.sha256(b'original').hexdigest())
            verify_trace(telemetry, private)
            path.write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                verify_trace(telemetry, private)
            outside = root / 'outside'
            outside.write_bytes(b'original')
            telemetry['trace_file'] = str(outside)
            with self.assertRaisesRegex(ValueError, 'private trial directory'):
                verify_trace(telemetry, private)


if __name__ == '__main__':
    unittest.main()
