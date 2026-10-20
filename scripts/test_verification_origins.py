"""Origin accounting preserves source-method inventory and execution evidence."""
import copy
import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from verification_origins import build_test_origins


class TestOrigins(unittest.TestCase):
    def setUp(self):
        directory = TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.path = 'crates/solver/tests/application.rs'
        self.fixture = 'tests/application/public/problem.json'
        fixture_path = self.root / self.fixture
        fixture_path.parent.mkdir(parents=True)
        fixture_path.write_bytes(b'{"public": true}\n')
        self.suite = {
            'id': 'application', 'label': 'Application regressions',
            'origin': 'external_application', 'upstream_method_credit': 0,
            'benchmark_job': 'application-performance', 'native_test_paths': [self.path],
            'expected_tests': [{'local_path': self.path, 'local_test': name}
                               for name in ('score', 'search')],
            'fixtures': [{'id': 'public-case', 'snapshots': [{
                'role': 'problem', 'local_path': self.fixture,
                'source_path': 'scenarios/cases/public/problem.json',
                'sha256': hashlib.sha256(fixture_path.read_bytes()).hexdigest()}]}],
        }
        self.write_manifest()

    def write_manifest(self, suites=None):
        path = self.root / 'compat/additional-tests.json'
        path.parent.mkdir(exist_ok=True)
        path.write_text(json.dumps({'schema_version': 1, 'suites': suites or [self.suite]}))

    def report(self, results, ledger=()):
        return build_test_origins(results, ledger, root=self.root)

    def category(self, report, name):
        return next(row for row in report['categories'] if row['id'] == name)['counts']

    def test_exact_method_and_path_mapping_never_mutates_ledger(self):
        path = 'crates/solver/tests/upstream.rs'
        ledger = [{'id': 'Java.method', 'status': 'implemented',
                   'local_path': path, 'local_test': 'module::method'},
                  {'id': 'Java.pending', 'status': 'pending',
                   'local_path': path, 'local_test': 'pending'}]
        original = copy.deepcopy(ledger)
        results = {(path, 'module::method'): 'passed', (path, 'method'): 'passed',
                   (path, 'module::method_extra'): 'passed',
                   (path, 'pending'): 'passed', (path + '.extra', 'module::method'): 'passed'}
        report = self.report(results, ledger)
        self.assertEqual(self.category(report, 'upstream_adapted')['total'], 1)
        self.assertEqual(self.category(report, 'native_regression')['total'], 4)
        self.assertEqual(ledger, original)
        self.assertEqual(report['counts']['total'], len(results))

    def test_multiple_upstream_mappings_never_double_count_one_native_test(self):
        identity = (self.path, 'score')
        ledger = [{'status': 'implemented', 'local_path': identity[0], 'local_test': identity[1]}] * 2
        report = self.report({identity: 'passed'}, ledger)
        self.assertEqual(self.category(report, 'upstream_adapted')['passed'], 1)
        self.assertEqual(self.category(report, 'external_application')['total'], 0)
        self.assertEqual(report['counts']['total'], 1)
        self.assertEqual(sum(c['counts']['total'] for c in report['categories']), 1)

    def test_external_origin_requires_exact_declared_file(self):
        report = self.report({(self.path, 'score'): 'passed',
                              (self.path.replace('.rs', '_extra.rs'), 'score'): 'passed',
                              (self.path + '/child.rs', 'score'): 'passed'})
        self.assertEqual(self.category(report, 'external_application')['total'], 1)
        self.assertEqual(self.category(report, 'native_regression')['total'], 2)

    def test_application_failure_keeps_upstream_passes_separate(self):
        upstream = 'crates/solver/tests/adapted.rs'
        ledger = [{'status': 'implemented', 'local_path': upstream, 'local_test': 'score'}]
        report = self.report({(upstream, 'score'): 'passed',
                              (self.path, 'score'): 'failed',
                              (self.path, 'search'): 'passed'}, ledger)
        self.assertEqual(self.category(report, 'upstream_adapted')['failed'], 0)
        self.assertEqual(self.category(report, 'external_application')['failed'], 1)
        suite = report['external_suites'][0]
        self.assertEqual(suite['execution_status'], 'failed')
        self.assertEqual(suite['upstream_method_credit'], 0)
        self.assertEqual({test['id']: test['status'] for test in suite['tests']},
                         {self.path + '#score': 'failed', self.path + '#search': 'passed'})

    def test_existing_files_never_prove_test_execution(self):
        source = self.root / self.path
        source.parent.mkdir(parents=True)
        source.write_text('#[test] fn score() {}\n#[test] fn search() {}')
        report = self.report({})
        suite = report['external_suites'][0]
        self.assertEqual(report['counts']['total'], 0)
        self.assertEqual(suite['counts']['total'], 0)
        self.assertEqual(suite['tests'], [])
        self.assertEqual(suite['missing_test_count'], 2)
        self.assertEqual(suite['execution_status'], 'unverified')
        self.assertTrue(all(t['status'] == 'unverified' and not t['recorded']
                            for t in suite['expected_tests']))
        self.assertEqual(suite['fixture_status'], 'valid')

    def test_partial_or_ignored_execution_is_unverified(self):
        for results in ({(self.path, 'score'): 'passed'},
                        {(self.path, 'score'): 'passed', (self.path, 'search'): 'unverified'}):
            with self.subTest(results=results):
                suite = self.report(results)['external_suites'][0]
                self.assertEqual(suite['execution_status'], 'unverified')
                self.assertEqual(suite['counts']['passed'], 1)

    def test_extra_actual_external_test_is_listed_without_expanding_expected_tests(self):
        results = {(self.path, name): 'passed' for name in ('score', 'search', 'new_control')}
        suite = self.report(results)['external_suites'][0]
        self.assertEqual(suite['counts']['passed'], 3)
        self.assertEqual(len(suite['tests']), 3)
        self.assertEqual(len(suite['expected_tests']), 2)
        self.assertEqual(suite['execution_status'], 'passed')

    def test_fixture_hash_mismatch_and_missing_file_remain_visible(self):
        fixture = self.root / self.fixture
        fixture.write_bytes(b'changed')
        suite = self.report({(self.path, 'score'): 'passed'})['external_suites'][0]
        self.assertEqual(suite['fixture_status'], 'mismatch')
        evidence = suite['fixtures'][0]['files'][0]
        self.assertNotEqual(evidence['actual_sha256'], evidence['expected_sha256'])
        fixture.unlink()
        suite = self.report({})['external_suites'][0]
        self.assertEqual(suite['fixture_status'], 'missing')
        self.assertIsNone(suite['fixtures'][0]['files'][0]['actual_sha256'])

    def test_rejects_duplicate_external_file_declarations(self):
        other = copy.deepcopy(self.suite)
        other['id'] = 'another'
        self.write_manifest([self.suite, other])
        with self.assertRaisesRegex(ValueError, 'multiple suites'):
            self.report({})

    def test_unknown_status_cannot_be_counted_as_a_pass(self):
        with self.assertRaisesRegex(ValueError, 'Unknown native test status'):
            self.report({(self.path, 'score'): 'ok'})

    def test_missing_manifest_allows_existing_native_results(self):
        (self.root / 'compat/additional-tests.json').unlink()
        report = self.report({('crates/solver/src/lib.rs', 'test'): 'passed'})
        self.assertEqual(self.category(report, 'native_regression')['passed'], 1)
        self.assertEqual(report['external_suites'], [])


if __name__ == '__main__':
    unittest.main()
