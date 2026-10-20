"""Native policy accounting contracts; these synthetic fixtures claim no execution."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from completion_progress import build_completion, model_digest as strict_digest
from native_readiness import (COHORTS, PENDING_METHODS, PERFORMANCE_COUNTS, build_native_readiness, cohort_digest,
                              load_manifest, model_digest, named_test_evidence, workload_declaration)
from test_completion_progress import complete_report, solve_row
from verification_inputs import MEASUREMENTS, measurement_digest, rust_digest


class NativeReadinessTests(unittest.TestCase):
    def setUp(self):
        self.directory = TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.manifest = deepcopy(load_manifest())
        self.report = complete_report()
        self.report['native_source_digest'] = 'a' * 64
        results = {(test['path'], test['name']): 'passed'
                   for requirement in self.manifest['requirements'] for test in requirement['tests']}
        self.report['verification']['native_tests'] = named_test_evidence(results, 'a' * 64, full_gate=True)
        self.report['methods'] = [{'id': identity, 'status': 'pending'} for identity in PENDING_METHODS]
        self.report['summary']['pending'] = 10
        self.report['families'][0].update(total=11, passed=1)
        # The frozen 109-row universe is exercised with explicitly synthetic
        # inputs. These fixtures are model contracts, never execution evidence.
        for name, count in PERFORMANCE_COUNTS.items():
            template = self.report[name]['workloads'][0]
            rows = []
            for index in range(count):
                row = deepcopy(template)
                row.update(name=f'synthetic {name} {index}', case={'id': index, 'values': [0, 1]},
                           unit='synthetic units', limit=100)
                rows.append(row)
            self.report[name].update(config={'synthetic_fixture': True, 'iterations': 1, 'max_evaluations': 100},
                                     workloads=rows)
            self.manifest['performance_targets'][name] = {
                'config': deepcopy(self.report[name]['config']),
                'workloads': [workload_declaration(row) for row in rows]}
        for boundary in self.manifest['boundaries']:
            cohort = {'total': 3, 'passed': 2, 'native_reference_passed': 3, 'cases': [{'id': 'case-1'}],
                      'results': [{'id': 'case-1', 'java': None, 'rust': 'actual'}],
                      'failures': [], 'divergent_cases': 1, 'java_error_cases': 0, 'unsupported_cases': 0}
            if boundary['disposition'] == 'reference_unavailable':
                cohort.update(divergent_cases=0, java_error_cases=1,
                              failures=[{'case': 'case-1', 'kind': 'java_runtime_error'}])
                record = {'concurrent_checks': cohort}
            else:
                answer = {'connected-range-history': 'Keep consistent native behavior',
                          'list-planning-value-metadata': 'Keep stable native capture'}[boundary['id']]
                record = {'completion_policy_decision': {'authority': boundary['authority'],
                          'answer': answer, 'target': 'The specifically approved native contract'}}
            self.report[boundary['job']][boundary['check']] = cohort
            boundary['cohort_sha256'] = cohort_digest(cohort)
            boundary['evidence'] = self.write_json('evidence/' + boundary['id'] + '.json', record)
        for requirement in self.manifest['requirements']:
            # Minimal structural fixture, not executable solver proof.
            requirement['files'] = [self.write_json('evidence/structure.json', {'native': True})]
            if requirement['id'] == 'dashboard':
                assets = {}
                for path in ('dashboard/app.js', 'dashboard/index.html', 'dashboard/styles.css'):
                    assets[path] = self.write_bytes(path, b'test fixture')['sha256']
                requirement['browser_evidence'] = self.write_json('evidence/browser.json', {
                    'passed': True, 'errors': [], 'input_sha256': assets,
                    'proof': [{'label': label, 'width': width, 'overflow': False}
                              for label, width in [('desktop', 1440), ('mobile', 390)]]})

    def write_bytes(self, path, data):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        return {'path': path, 'sha256': hashlib.sha256(data).hexdigest()}

    def write_json(self, path, data):
        return self.write_bytes(path, json.dumps(data, sort_keys=True).encode())

    def evaluate(self):
        return build_native_readiness(self.report, build_completion(self.report), self.manifest, self.root)

    def boundary(self, identity):
        return next(item for item in self.manifest['boundaries'] if item['id'] == identity)

    def test_separate_native_readiness_preserves_all_strict_inputs_and_results(self):
        before = deepcopy(self.report)
        strict = build_completion(self.report)
        self.assertFalse(strict['ready'])
        result = self.evaluate()
        self.assertTrue(result['native_ready'], result['blockers'])
        self.assertEqual(self.report, before)
        self.assertEqual(build_completion(self.report), strict)
        self.assertEqual(len(result['requirements']), 7)
        self.assertEqual(len(result['method_boundaries']), 10)
        self.assertEqual({b['status'] for b in result['boundaries']}, {'resolved'})
        json.dumps(result, allow_nan=False)

    def test_approved_range_never_resolves_metadata_or_new_difference(self):
        metadata = self.boundary('list-planning-value-metadata')
        metadata['authority'] = ''
        self.assertFalse(self.evaluate()['native_ready'])
        metadata['authority'] = 'Explicit user reply in this project chat'
        self.report['stream_differential']['new_range_check'] = {'total': 1, 'passed': 0, 'native_reference_passed': 1}
        result = self.evaluate()
        self.assertFalse(result['native_ready'])
        self.assertTrue(any('new_range_check' in message for message in result['blockers']))

    def test_exact_cohort_binds_every_input_outcome_counter_and_retained_control(self):
        boundary = self.boundary('connected-range-history')
        original = deepcopy(self.report[boundary['job']][boundary['check']])
        for key, value in [('cases', []), ('results', []), ('passed', 3), ('new_control', {'total': 1, 'passed': 0})]:
            with self.subTest(key=key):
                self.report[boundary['job']][boundary['check']] = {**original, key: value}
                result = self.evaluate()
                self.assertFalse(result['native_ready'])
                self.assertEqual(next(b for b in result['boundaries'] if b['id'] == boundary['id'])['status'], 'unresolved')

    def test_wrong_record_hash_missing_authority_and_unrelated_user_decision_block(self):
        boundary = self.boundary('connected-range-history')
        boundary['evidence']['sha256'] = '0' * 64
        self.assertFalse(self.evaluate()['native_ready'])
        boundary['evidence'] = self.write_json('evidence/wrong-decision.json', {
            'completion_policy_decision': {'authority': boundary['authority'], 'answer': 'Another policy', 'target': 'Another target'}})
        self.assertFalse(self.evaluate()['native_ready'])
        boundary['evidence'] = self.write_json('evidence/no-authority.json', {'completion_policy_decision': {'answer': 'Keep consistent native behavior'}})
        self.assertFalse(self.evaluate()['native_ready'])

    def test_native_reference_passes_alone_do_not_prove_named_native_requirement(self):
        proof = self.report['verification']['native_tests']
        original = deepcopy(proof)
        for mutation in ('no-tests', 'wrong-test-name', 'wrong-test-path', 'failed', 'duplicate', 'wrong-source', 'not-full-gate'):
            with self.subTest(mutation=mutation):
                self.report['verification']['native_tests'] = proof = deepcopy(original)
                if mutation == 'no-tests': proof['tests'] = []
                elif mutation == 'wrong-test-name': proof['tests'][0]['name'] = 'unrelated'
                elif mutation == 'wrong-test-path': proof['tests'][0]['path'] = 'unrelated.rs'
                elif mutation == 'failed': proof['tests'][0]['status'] = 'failed'
                elif mutation == 'duplicate': proof['tests'].append(proof['tests'][0])
                elif mutation == 'wrong-source': proof['source_digest'] = 'b' * 64
                elif mutation == 'not-full-gate': proof['full_gate'] = False
                self.assertFalse(self.evaluate()['native_ready'])

    def test_errors_and_unsupported_are_retained_without_java_pass_credit(self):
        before = build_completion(self.report)
        result = self.evaluate()
        counts = next(b['counts'] for b in result['boundaries'] if b['id'] == 'cyclic-supplier-reference-errors')
        self.assertEqual(counts['java_error_cases'], 1)
        self.assertEqual(counts['passed'], 2)
        self.assertEqual(build_completion(self.report), before)
        # Error-only disposition cannot absorb even an exactly re-bound successful divergence.
        boundary = self.boundary('cyclic-supplier-reference-errors')
        cohort = self.report[boundary['job']][boundary['check']]
        cohort['divergent_cases'] = 1
        boundary['cohort_sha256'] = cohort_digest(cohort)
        boundary['evidence'] = self.write_json('evidence/rebound-errors.json', {'concurrent_checks': cohort})
        self.assertFalse(self.evaluate()['native_ready'])

    def test_fixed_targets_and_exact_pending_method_mappings_cannot_shrink(self):
        original = deepcopy(self.manifest)
        for mutation in ('remove-requirement', 'remove-boundary', 'remove-mapping', 'unknown-mapping', 'blanket-policy'):
            with self.subTest(mutation=mutation):
                self.manifest = deepcopy(original)
                if mutation == 'remove-requirement': self.manifest['requirements'].pop()
                elif mutation == 'remove-boundary': self.manifest['boundaries'].pop()
                elif mutation == 'remove-mapping': self.manifest['method_boundaries'].pop()
                elif mutation == 'unknown-mapping': self.manifest['method_boundaries'][0]['method_id'] = 'new-method'
                else: self.manifest['accept_native_reference'] = True
                self.assertFalse(self.evaluate()['native_ready'])
        self.manifest = original
        self.report['methods'].append({'id': 'unmapped-new-pending-method', 'status': 'pending'})
        self.assertFalse(self.evaluate()['native_ready'])

    def test_pending_mapping_keeps_raw_pending_status_and_requires_physical_native_proof(self):
        result = self.evaluate()
        self.assertEqual(self.report['summary']['pending'], 10)
        self.assertEqual({m['original_status'] for m in result['method_boundaries']}, {'pending'})
        tests = self.report['verification']['native_tests']['tests']
        self.report['verification']['native_tests']['tests'] = [t for t in tests if 'kopt_subtour' not in t['path']]
        self.assertFalse(self.evaluate()['native_ready'])

    def test_all_original_performance_quality_and_capture_rules_are_unchanged(self):
        original = deepcopy(self.report['mixed_performance'])
        for row in (solve_row(2, 1), solve_row(.1, 1, [0, -3], [0, 1]),
                    {**solve_row(), 'samples': {}}, {**solve_row(), 'rust': {'median': float('nan')}}):
            with self.subTest(row=row):
                self.report['mixed_performance'] = {**original, 'workloads': [solve_row(.001, 100), row]}
                result = self.evaluate()
                self.assertFalse(result['native_ready'])
                self.assertEqual(result['performance'], next(c['items'] for c in build_completion(self.report)['components'] if c['id'] == 'performance'))
                json.dumps(result, allow_nan=False)

    def test_all_109_original_workload_inputs_are_required_even_with_current_passing_timings(self):
        original = deepcopy(self.report['mixed_performance'])
        self.assertEqual(sum(len(self.report[name]['workloads']) for name in PERFORMANCE_COUNTS), 109)
        for mutation in ('remove', 'rename', 'duplicate', 'change-case', 'remove-case', 'change-config',
                         'change-limits', 'new-limits', 'extra-row'):
            with self.subTest(mutation=mutation):
                job = self.report['mixed_performance'] = deepcopy(original)
                rows = job['workloads']
                if mutation == 'remove': rows.pop()
                elif mutation == 'rename': rows[0]['name'] += ' renamed'
                elif mutation == 'duplicate': rows[1] = deepcopy(rows[0])
                elif mutation == 'change-case': rows[0]['case']['values'] = [0]
                elif mutation == 'remove-case': rows[0].pop('case')
                elif mutation == 'change-config': job['config']['max_evaluations'] = 1
                elif mutation == 'change-limits': rows[0]['limit'] = 1
                elif mutation == 'new-limits': rows[0]['new_limit'] = 1
                elif mutation == 'extra-row': rows.append(deepcopy(rows[0]))
                strict = build_completion(self.report)
                self.assertEqual(next(c['percent'] for c in strict['components'] if c['id'] == 'performance'), 100)
                result = self.evaluate()
                self.assertFalse(result['native_ready'])
                self.assertEqual(next(c['status'] for c in result['checklist'] if c['id'] == 'original-performance-target'), 'not_met')
                self.assertTrue(any('Original workload universe/configuration' in error for error in result['blockers']))
                self.assertEqual(build_completion(self.report), strict)

    def test_frozen_original_performance_target_family_and_row_counts_cannot_shrink(self):
        original = deepcopy(self.manifest['performance_targets'])
        for mutation in ('remove-family', 'remove-row', 'duplicate-target'):
            with self.subTest(mutation=mutation):
                targets = self.manifest['performance_targets'] = deepcopy(original)
                if mutation == 'remove-family': targets.pop('performance')
                elif mutation == 'remove-row': targets['performance']['workloads'].pop()
                else: targets['performance']['workloads'][1] = deepcopy(targets['performance']['workloads'][0])
                self.assertFalse(self.evaluate()['native_ready'])

    def test_improved_observations_do_not_change_the_frozen_workload_input_declaration(self):
        original = deepcopy(self.report['mixed_performance']['workloads'][0])
        improved = self.report['mixed_performance']['workloads'][0]
        improved['rust']['median'] = .5
        improved['java_over_rust'] = 2
        improved['observations'] = {'complete_output': 'may improve'}
        self.assertEqual(workload_declaration(improved), workload_declaration(original))
        self.assertTrue(self.evaluate()['native_ready'])

    def test_failed_gate_stale_jobs_raw_failures_and_missing_evidence_each_block(self):
        original = deepcopy(self.report)
        for mutation in ('failed-gate', 'stale', 'missing-job', 'failed', 'unverified', 'missing-summary'):
            with self.subTest(mutation=mutation):
                self.report = deepcopy(original)
                if mutation == 'failed-gate': self.report['verification']['successful'] = False
                elif mutation == 'stale': self.report['performance']['status'] = 'stale'
                elif mutation == 'missing-job': self.report.pop('performance')
                elif mutation == 'missing-summary': self.report.pop('summary')
                else: self.report['summary'][mutation] = 1
                self.assertFalse(self.evaluate()['native_ready'])

    def test_browser_proof_must_match_actual_assets_and_both_viewports(self):
        original = self.root.joinpath('dashboard/styles.css').read_bytes()
        self.root.joinpath('dashboard/styles.css').write_bytes(b'changed')
        self.assertFalse(self.evaluate()['native_ready'])
        self.root.joinpath('dashboard/styles.css').write_bytes(original)
        dashboard = next(r for r in self.manifest['requirements'] if r['id'] == 'dashboard')
        dashboard['browser_evidence'] = self.write_json('evidence/no-mobile.json', {'passed': True, 'errors': [],
            'input_sha256': {path: hashlib.sha256(b'test fixture').hexdigest() for path in
                             ('dashboard/app.js', 'dashboard/index.html', 'dashboard/styles.css')},
            'proof': [{'label': 'desktop', 'width': 1440, 'overflow': False}]})
        self.assertFalse(self.evaluate()['native_ready'])

    def test_live_native_digest_tracks_every_bound_asset_without_rewriting_browser_receipt(self):
        dashboard = next(r for r in self.manifest['requirements'] if r['id'] == 'dashboard')
        receipt = self.root / dashboard['browser_evidence']['path']
        proof = json.loads(receipt.read_bytes())
        font = 'dashboard/assets/test-font.woff2'
        proof['input_sha256'][font] = self.write_bytes(font, b'font fixture')['sha256']
        dashboard['browser_evidence'] = self.write_json(dashboard['browser_evidence']['path'], proof)
        receipt_before = receipt.read_bytes()
        self.write_bytes('Cargo.toml', b'[workspace]\nmembers = []\n')
        strict_before = strict_digest()
        measurements_before = {name: measurement_digest(name, self.root) for name in MEASUREMENTS}
        rust_before = rust_digest(self.root)
        native_before = model_digest(self.manifest, self.root)
        self.assertTrue(self.evaluate()['native_ready'])
        for path in proof['input_sha256']:
            with self.subTest(path=path):
                asset = self.root / path
                original = asset.read_bytes()
                asset.write_bytes(original + b' edited')
                self.assertEqual(receipt.read_bytes(), receipt_before)
                self.assertNotEqual(model_digest(self.manifest, self.root), native_before)
                self.assertFalse(self.evaluate()['native_ready'])
                self.assertEqual(strict_digest(), strict_before)
                self.assertEqual({name: measurement_digest(name, self.root) for name in MEASUREMENTS}, measurements_before)
                self.assertEqual(rust_digest(self.root), rust_before)
                asset.unlink()
                missing = model_digest(self.manifest, self.root)
                self.assertNotEqual(missing, native_before)
                self.assertEqual(model_digest(self.manifest, self.root), missing)
                self.assertFalse(self.evaluate()['native_ready'])
                asset.write_bytes(original)
                self.assertEqual(model_digest(self.manifest, self.root), native_before)

    def test_browser_receipt_requires_all_three_dashboard_sources(self):
        dashboard = next(r for r in self.manifest['requirements'] if r['id'] == 'dashboard')
        proof = json.loads((self.root / dashboard['browser_evidence']['path']).read_bytes())
        original = deepcopy(proof)
        for path in original['input_sha256']:
            with self.subTest(missing=path):
                proof = deepcopy(original)
                proof['input_sha256'].pop(path)
                dashboard['browser_evidence'] = self.write_json('evidence/incomplete-inputs.json', proof)
                self.assertFalse(self.evaluate()['native_ready'])
        proof = deepcopy(original)
        proof['app_sha256'] = proof['input_sha256']['dashboard/app.js']
        proof.pop('input_sha256')
        dashboard['browser_evidence'] = self.write_json('evidence/legacy-app-only.json', proof)
        self.assertFalse(self.evaluate()['native_ready'])

    def test_browser_input_symlinks_cannot_escape_repository_when_hashing_live_sources(self):
        with TemporaryDirectory() as outside_directory:
            outside = Path(outside_directory) / 'external.css'
            outside.write_bytes(b'outside repository')
            path = 'dashboard/escaped.css'
            (self.root / path).symlink_to(outside)
            dashboard = next(r for r in self.manifest['requirements'] if r['id'] == 'dashboard')
            proof = json.loads((self.root / dashboard['browser_evidence']['path']).read_bytes())
            proof['input_sha256'][path] = hashlib.sha256(outside.read_bytes()).hexdigest()
            dashboard['browser_evidence'] = self.write_json('evidence/escaped-input.json', proof)
            before = model_digest(self.manifest, self.root)
            self.assertFalse(self.evaluate()['native_ready'])
            outside.write_bytes(b'changed outside repository')
            self.assertEqual(model_digest(self.manifest, self.root), before)

    def test_native_policy_digest_is_separate_and_does_not_modify_measurement_keys(self):
        before = deepcopy(self.report)
        strict_before = strict_digest()
        native_before = model_digest(self.manifest, self.root)
        self.manifest['boundaries'][0]['reason'] += ' Reviewed clarification.'
        self.assertNotEqual(model_digest(self.manifest, self.root), native_before)
        self.assertEqual(strict_digest(), strict_before)
        self.assertEqual(self.report, before)
        native_before = model_digest(self.manifest, self.root)
        (self.root / self.manifest['boundaries'][0]['evidence']['path']).write_bytes(b'changed record')
        self.assertNotEqual(model_digest(self.manifest, self.root), native_before)
        self.assertFalse(self.evaluate()['native_ready'])

    def test_missing_malformed_or_escaped_manifest_evidence_fails_closed(self):
        self.assertFalse(build_native_readiness(None, None, root=self.root)['native_ready'])
        for value in (None, [], {'schema_version': 99}, {'accept_native_reference': True}):
            result = build_native_readiness(None, None, value, self.root)
            self.assertFalse(result['native_ready'])
            json.dumps(result, allow_nan=False)
        self.manifest['boundaries'][0]['evidence']['path'] = '../outside.json'
        self.assertFalse(self.evaluate()['native_ready'])


if __name__ == '__main__':
    unittest.main()
