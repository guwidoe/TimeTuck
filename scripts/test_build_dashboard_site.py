"""Publication boundaries: preserve evidence and prevent private report disclosure."""
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from build_dashboard_site import ASSETS, build_site, public_report, validate_public


class PublicDashboardTests(unittest.TestCase):
    def source(self):
        return {'schema_version': 1, 'generated_at': '2026-10-01T10:00:00Z',
                'verification_digest': 'old-evidence', 'summary': {'passed': 0, 'unverified': 2},
                'families': [{'name': 'Scores', 'passed': 0, 'unverified': 2}], 'methods': [],
                'runs': [{'stdout': 'PRIVATE LOG'}],
                'application_benchmarks': {'private_application': {
                    'origin': 'external_application', 'status': 'stale', 'qualified': True,
                    'machine': {'hostname': 'PRIVATE HOST'}, 'commands': ['PRIVATE COMMAND'],
                    'future_private_field': 'PRIVATE APPLICATION DATA',
                    'source_repository': {'path': '/home/private/PrivateApplication'},
                    'summaries': [{'case': 'sailing-trip-v1', 'throughput_target_met': False,
                        'rust_over_java_streams_evaluations_per_second': 0.85,
                        'runtimes': {'rust-streams': {'runs': 3, 'scores': [[-1, 10], [0, 8]]}},
                        'quality_note': 'Sailing and PrivateApplication limits still apply.'}]}},
                'test_origins': {'external_suites': [{'id': 'private_application', 'expected_tests': [
                    {'local_path': 'tests/private_application.rs', 'local_test': 'sailing_score', 'status': 'unverified'}]}]}}

    def export(self):
        return public_report(self.source(), source_sha256='original-file-hash',
            source_commit='source-commit', exported_at='2026-10-21T10:00:00Z',
            freshness={'verification': False, 'completion': False, 'native': False})

    def test_private_details_removed_without_turning_old_evidence_current(self):
        report = self.export()
        text = json.dumps(report)
        for forbidden in ['PrivateApplication', 'private_application', 'sailing', '/home/', 'PRIVATE']:
            self.assertNotIn(forbidden, text)
        self.assertEqual(report['generated_at'], self.source()['generated_at'])
        self.assertEqual(report['verification_digest'], 'old-evidence')
        self.assertEqual(report['summary'], self.source()['summary'])
        app = report['application_benchmarks']['real_world']
        self.assertEqual(app['status'], 'stale')
        self.assertFalse(app['summaries'][0]['throughput_target_met'])
        self.assertEqual(app['summaries'][0]['rust_over_java_streams_evaluations_per_second'], 0.85)
        self.assertEqual(app['summaries'][0]['runtimes']['rust-streams']['scores'], [[-1, 10], [0, 8]])
        self.assertEqual(app['summaries'][0]['runtimes']['rust-streams']['runs'], 3)
        self.assertFalse(report['publication']['verification_fresh'])
        self.assertEqual(report['publication']['source_report_sha256'], 'original-file-hash')

    def test_rejects_identity_or_private_fields_in_a_committed_snapshot(self):
        for injection in [{'stdout': 'leak'}, {'name': 'PrivateApplication'}, {'note': '/home/private/file'}, {'runs': [{}]}]:
            with self.subTest(injection=injection):
                report = self.export()
                report['leak'] = injection
                with self.assertRaises(ValueError):
                    validate_public(report)

    def test_build_includes_only_site_assets_and_rejects_contaminated_output(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp) / 'repo'
            for source in ASSETS.values():
                path = root / source
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('approved asset')
            (root / 'dashboard/private.log').write_text('PRIVATE')
            output = Path(tmp) / 'site'
            build_site(self.export(), output, root=root)
            self.assertEqual({p.relative_to(output).as_posix() for p in output.rglob('*') if p.is_file()},
                             set(ASSETS) | {'data/report.json', '.nojekyll'})
            self.assertEqual(json.loads((output / 'data/report.json').read_text())['summary'], self.source()['summary'])
            (output / 'private.log').write_text('PRIVATE')
            with self.assertRaises(ValueError):
                build_site(self.export(), output, root=root)


if __name__ == '__main__':
    unittest.main()
