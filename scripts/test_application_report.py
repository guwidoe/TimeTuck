"""Additional application reports cannot silently become upstream evidence."""
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

from application_report import load_application_benchmark
from completion_progress import build_completion
from test_completion_progress import complete_report


class ApplicationReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.path = self.root / 'work/real_world-performance.json'
        self.path.parent.mkdir()
        self.key = patch('benchmark_real_world.real_world_digest', return_value='actual-input-key')
        self.key.start()
        self.addCleanup(self.key.stop)

    def record(self, **changes):
        record = dict(schema_version=1, kind='application_benchmark', upstream_method_credit=0,
                      input_sha256='actual-input-key', valid=True, qualified=True,
                      runs=[], summaries=[])
        record.update(changes)
        self.path.write_text(json.dumps(record))

    def test_unmeasured_or_bad_application_evidence_is_not_a_pass(self):
        self.assertEqual(load_application_benchmark(self.root)['status'], 'not_measured')
        self.path.write_text('{invalid json')
        self.assertEqual(load_application_benchmark(self.root)['status'], 'invalid')
        self.record(kind='upstream')
        self.assertEqual(load_application_benchmark(self.root)['status'], 'invalid')

    def test_freshness_uses_application_key_without_changing_runtime_outcomes(self):
        self.record(valid=False, qualified=False)
        current = load_application_benchmark(self.root)
        self.assertEqual(current['status'], 'current')
        self.assertFalse(current['valid'])
        self.assertFalse(current['qualified'])
        self.record(input_sha256='previous-input-key')
        self.assertEqual(load_application_benchmark(self.root)['status'], 'stale')

    def test_separate_application_failures_never_change_upstream_index(self):
        original = complete_report()
        expected = build_completion(original)
        self.record(valid=False)
        original['application_benchmarks'] = {'real_world': load_application_benchmark(self.root)}
        self.assertEqual(build_completion(original), expected)
        self.assertEqual(original['application_benchmarks']['real_world']['upstream_method_credit'], 0)
