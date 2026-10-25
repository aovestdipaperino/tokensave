import unittest
from run import summarize


class TelemetryTests(unittest.TestCase):
    def test_counts_usage_and_only_identical_reads(self):
        read = dict(type='file_read', path='src/lib.rs', range='1:10', sha256='abc')
        result = summarize([
            dict(type='usage', input_tokens=100, output_tokens=20, cost_usd=.02),
            dict(type='usage', input_tokens=50, output_tokens=5, cost_usd=.01),
            dict(type='mcp_call', name='tokensave_search'), read, read,
            dict(read, sha256='changed'), dict(read, range='11:20')])
        self.assertEqual(result, dict(input_tokens=150, output_tokens=25, cost_usd=.03,
                                     mcp_calls=1, redundant_file_reads=1))

    def test_missing_invalid_or_estimated_usage_rejected(self):
        for events in ([], [dict(type='usage', input_tokens=-1, output_tokens=0, cost_usd=0)],
                       [dict(type='usage', input_tokens=1, output_tokens=0, cost_usd=float('nan'))],
                       [dict(type='estimated_usage')]):
            with self.assertRaises(ValueError):
                summarize(events)

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
assert pathlib.Path('answer').read_text() == 'before'
pathlib.Path('answer').write_text('after')
print(json.dumps({'complete': True, 'trace_sha256': '0' * 64,
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
            rows = [json.loads(line) for line in (output / 'trials.jsonl').read_text().splitlines()]
            self.assertEqual([row['arm'] for row in rows], ['A', 'B', 'B', 'A'])


if __name__ == '__main__':
    unittest.main()
