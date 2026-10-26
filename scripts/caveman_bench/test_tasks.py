import json
from pathlib import Path
import tempfile
import unittest

from run import verify_artifacts
from tasks import FILES, FIXTURE, prepare, verify


class TaskSuiteTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name in FILES:
            (self.root / name).write_bytes((FIXTURE / name).read_bytes())

    def test_understanding_requires_complete_correct_answer_and_unchanged_source(self):
        self.assertFalse(verify('understanding', self.root))
        answer = self.root / 'answer.json'
        answer.write_text('{}')
        self.assertFalse(verify('understanding', self.root))
        answer.write_text(json.dumps(dict(entrypoint='checkout', units='integer cents',
                                         rounding='down to whole cents after summing')))
        self.assertTrue(verify('understanding', self.root))
        (self.root / 'report.py').write_text('')
        self.assertFalse(verify('understanding', self.root))

    def test_impact_rejects_partial_or_duplicate_callers(self):
        answer = dict(direct_callers=['checkout', 'preview'],
                      transitive_callers=['checkout', 'invoice_total', 'preview', 'receipt'])
        path = self.root / 'answer.json'
        path.write_text(json.dumps(answer))
        self.assertTrue(verify('impact', self.root))
        answer['transitive_callers'].remove('receipt')
        path.write_text(json.dumps(answer))
        self.assertFalse(verify('impact', self.root))
        answer['transitive_callers'] += ['receipt', 'receipt']
        path.write_text(json.dumps(answer))
        self.assertFalse(verify('impact', self.root))

    def test_edit_negative_control_then_fix_and_source_tampering(self):
        self.assertFalse(verify('edit', self.root))
        source = self.root / 'pricing.py'
        source.write_text(source.read_text().replace(
            'sum(price * (100 - percent) // 100 for price in prices)',
            'sum(prices) * (100 - percent) // 100'))
        self.assertTrue(verify('edit', self.root))
        (self.root / 'checkout.py').write_text('')
        self.assertFalse(verify('edit', self.root))

    def test_extra_source_cannot_shadow_probe_imports(self):
        (self.root / 'json.py').write_text('raise RuntimeError("shadow")')
        self.assertFalse(verify('edit', self.root))

    def test_prepare_pins_fixture_and_external_verifier(self):
        output = self.root / 'prepared'
        prepare(output)
        manifest = json.loads((output / 'manifest.json').read_text())
        self.assertEqual(len(manifest['commit']), 40)
        self.assertEqual(len(manifest['verification_artifacts']), 5)
        self.assertEqual([t['id'] for t in manifest['tasks']],
                         ['understanding', 'impact', 'edit'])
        for task in manifest['tasks']:
            self.assertNotIn(str(output), task['verify'][2])
        with self.assertRaises(FileExistsError):
            prepare(output)

    def test_changed_verification_artifact_is_rejected(self):
        import hashlib
        path = self.root / 'verifier.py'
        path.write_bytes(b'original')
        config = dict(verification_artifacts={str(path): hashlib.sha256(b'original').hexdigest()})
        verify_artifacts(config)
        path.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'artifact changed'):
            verify_artifacts(config)

    def test_symlinked_answer_rejected(self):
        with tempfile.NamedTemporaryFile() as target:
            target.write(json.dumps(dict(entrypoint='checkout', units='integer cents',
                rounding='down to whole cents after summing')).encode())
            target.flush()
            (self.root / 'answer.json').symlink_to(target.name)
            self.assertFalse(verify('understanding', self.root))
