"""Privacy transformations preserve attribution, measured values, and source history."""
import json
import os
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
import unittest

from prepare_public_history import Redactor, audit, prepare


POLICY = {'schema_version': 1,
          'literal_replacements': [['PrivateProduct', 'RealWorld'], ['privateproduct', 'real_world']],
          'forbidden_patterns': {'product': 'privateproduct', 'email': 'owner@example.test'},
          'email_replacements': {'owner@example.test': 'owner@users.noreply.github.com'}}


class PublicHistoryTests(unittest.TestCase):
    def test_preserves_copyright_and_historical_measurement_identity(self):
        rules = Redactor(POLICY)
        before = b'Copyright (c) 2026 Original Author.\n{"name":"PrivateProduct","source_digest":"old-evidence","ratio":0.85}'
        result = rules.rewrite(before)
        self.assertIn(b'Copyright (c) 2026 Original Author.', result)
        self.assertIn(b'"source_digest":"old-evidence","ratio":0.85', result)
        self.assertNotIn(b'PrivateProduct', result)
        with self.assertRaises(ValueError):
            rules.rewrite(b'Copyright (c) 2026 PrivateProduct contributors.')

    def test_refuses_incomplete_rules(self):
        with self.assertRaises(ValueError):
            Redactor({**POLICY, 'literal_replacements': []}).rewrite(b'PrivateProduct')

    def test_filters_all_refs_objects_and_messages_without_modifying_source(self):
        tool = os.environ.get('PUBLIC_HISTORY_FILTER_TOOL')
        self.assertTrue(tool, 'Set PUBLIC_HISTORY_FILTER_TOOL to the pinned local filter tool.')
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'source'
            source.mkdir()
            def git(*args):
                return subprocess.check_output(['git', '-C', str(source), *args], stderr=subprocess.DEVNULL)
            git('init', '-b', 'main')
            git('config', 'user.name', 'Original Author')
            git('config', 'user.email', 'owner@example.test')
            (source / 'privateproduct.txt').write_text('Copyright (c) 2026 Original Author.\nPrivateProduct\n')
            git('add', '.')
            git('commit', '-m', 'feat(privateproduct): add a workload')
            git('branch', 'privateproduct-history')
            git('tag', '-a', 'privateproduct-v1', '-m', 'PrivateProduct release note')
            (source / 'privateproduct.txt').write_text('Copyright (c) 2026 Original Author.\npublic current content\n')
            git('add', '.')
            git('commit', '-m', 'docs: update current content')
            original = git('rev-parse', 'HEAD')
            policy = root / 'private-policy.json'
            policy.write_text(json.dumps(POLICY))
            target = root / 'public.git'
            receipt = prepare(source, target, policy, Path(tool), root / 'private-receipts')
            self.assertEqual(git('rev-parse', 'HEAD'), original)
            self.assertEqual(git('show', 'privateproduct-history:privateproduct.txt').splitlines()[-1], b'PrivateProduct')
            self.assertFalse(receipt['audit']['findings'])
            self.assertEqual(receipt['audit']['objects']['commit'], 2)
            self.assertEqual(receipt['audit']['refs'], 3)
            self.assertFalse(audit(target, policy)['findings'])
            public_refs = subprocess.check_output(['git', '-C', str(target), 'for-each-ref', '--format=%(refname)'])
            self.assertIn(b'real_world-history', public_refs)
            history = subprocess.check_output(['git', '-C', str(target), 'log', '--all', '--format=%an <%ae> %s'])
            self.assertIn(b'Original Author <owner@users.noreply.github.com>', history)
            self.assertNotIn(b'PrivateProduct', history)
            self.assertTrue((root / 'private-receipts/original.bundle').is_file())


if __name__ == '__main__':
    unittest.main()
