"""Original completion-index contracts; no benchmark/test evidence is fabricated."""
from copy import deepcopy
import json
import unittest

from completion_progress import (ALL_JOBS, BEHAVIOR_JOBS, PERFORMANCE_JOBS,
                                 UPDATE_JOBS, build_completion, model_digest)


def solve_row(rust=1, java=1, rust_score=None, java_score=None):
    return {'name': 'case', 'rust': {'median': rust, 'score': rust_score or [0, 1]},
            'java': {'median': java, 'score': java_score or [0, 1]},
            'samples': {'rust': [{'elapsed_ns': 1, 'score': rust_score or [0, 1]}],
                        'java': [{'elapsed_ns': 1, 'score': java_score or [0, 1]}]},
            'equal_scores': True}


def complete_report():
    report = {'verification': {'successful': True},
              'summary': {'failed': 0, 'unverified': 0, 'pending': 0},
              'families': [{'name': 'family', 'passed': 1, 'total': 1, 'excluded': 0}]}
    for name in BEHAVIOR_JOBS:
        report[name] = {'status': 'current', 'passed': 1, 'total': 1}
    for name in PERFORMANCE_JOBS:
        row = solve_row()
        if name in ('performance', 'solver_performance'):
            row['samples'] = {'rust': [1], 'java': [1]}
            row['checksum'] = 17
            row['optimal_score'] = [0, 1]
        if name in UPDATE_JOBS:
            row['samples'] = {'rust': [{'elapsed_ns': 1, 'checksum': 17}],
                              'java': [{'elapsed_ns': 1, 'checksum': 17}]}
        report[name] = {'status': 'current', 'workloads': [row], 'validation': {'total': 1, 'passed': 1}}
    return report


def component(report, name):
    return next(c for c in build_completion(report)['components'] if c['id'] == name)


class CompletionTests(unittest.TestCase):
    def test_empty_malformed_and_no_work_never_imply_ready(self):
        for report in ({}, None, [], {'families': [], 'verification': None},
                       {'families': [{'total': 'invalid'}]}):
            result = build_completion(report)
            self.assertFalse(result['ready'])
            self.assertEqual(result['index_percent'], 0)
            json.dumps(result, allow_nan=False)
        valid = complete_report()
        before = deepcopy(valid)
        self.assertTrue(build_completion(valid)['ready'])
        self.assertEqual(build_completion(valid)['index_percent'], 100)
        self.assertEqual(valid, before)
        for field in ('failed', 'unverified'):
            report = complete_report(); report['summary'].pop(field)
            self.assertFalse(build_completion(report)['ready'])
            self.assertLess(build_completion(report)['index_percent'], 100)

    def test_features_are_macro_weighted_include_zero_pass_and_exclude_only_out_of_scope(self):
        report = complete_report()
        report['families'] = [{'name': 'large', 'total': 1000, 'passed': 1000},
                              {'name': 'small', 'total': 1, 'passed': 0},
                              {'name': 'outside', 'total': 9, 'excluded': 9, 'passed': 0}]
        features = component(report, 'features')
        self.assertEqual(features['percent'], 50)
        self.assertEqual(features['total'], 2)
        self.assertFalse(build_completion(report)['ready'])

    def test_behavior_uses_worst_nested_java_passes_not_native_references(self):
        report = complete_report()
        report['stream_differential']['collector'] = {'total': 10, 'passed': 6,
                                                      'native_reference_passed': 10, 'java_error_cases': 4}
        self.assertEqual(component(report, 'behavior')['percent'], 90)
        report['stream_differential']['collector'].pop('passed')
        self.assertEqual(component(report, 'behavior')['percent'], 75)

    def test_fast_rows_cannot_hide_slow_row_or_worse_quality_and_better_counts(self):
        report = complete_report()
        report['mixed_performance']['workloads'] = [solve_row(0.00001, 1000), solve_row(4, 1)]
        family = next(i for i in component(report, 'performance')['items'] if i['name'] == 'mixed_performance')
        self.assertEqual(family['percent'], 25)
        self.assertEqual(family['bottleneck']['rust_median'], 4)
        worse = solve_row(0.00001, 1000, [0, -2], [0, -1])
        report['mixed_performance']['workloads'] = [worse]
        family = next(i for i in component(report, 'performance')['items'] if i['name'] == 'mixed_performance')
        self.assertEqual(family['percent'], 0)
        better = solve_row(1, 1, [0, 2], [0, 1]); better['equal_scores'] = False
        report['mixed_performance']['workloads'] = [better]
        self.assertEqual(component(report, 'performance')['percent'], 100)
        better['rust']['score'] = [0]
        self.assertLess(component(report, 'performance')['percent'], 100)

    def test_invalid_timing_and_missing_captured_samples_are_unknown_zero(self):
        for value in [0, -1, float('nan'), float('inf'), None, True]:
            report = complete_report(); report['mixed_performance']['workloads'][0]['rust']['median'] = value
            item = next(i for i in component(report, 'performance')['items'] if i['name'] == 'mixed_performance')
            self.assertEqual(item['percent'], 0)
            self.assertEqual(item['status'], 'unknown')
            json.dumps(build_completion(report), allow_nan=False)
        report = complete_report(); report['mixed_performance']['workloads'][0]['samples']['rust'] = []
        self.assertLess(component(report, 'performance')['percent'], 100)

    def test_update_results_and_full_validation_are_required(self):
        report = complete_report()
        report['aggregate_performance']['workloads'][0]['samples']['rust'][0]['checksum'] = 18
        self.assertLess(component(report, 'performance')['percent'], 100)
        report = complete_report(); report['aggregate_performance']['validation']['passed'] = 0
        self.assertLess(component(report, 'performance')['percent'], 100)

    def test_stale_progress_remains_but_current_evidence_and_gate_are_separate(self):
        report = complete_report(); report['mixed_performance']['status'] = 'stale'
        result = build_completion(report)
        self.assertEqual(component(report, 'performance')['percent'], 100)
        self.assertLess(component(report, 'evidence')['percent'], 100)
        self.assertFalse(result['ready']); self.assertEqual(result['current_jobs'], 17)
        self.assertEqual(result['stale_jobs'], ['mixed_performance'])
        report = complete_report(); report['verification']['successful'] = False
        self.assertLess(component(report, 'evidence')['percent'], 100)
        self.assertFalse(build_completion(report)['ready'])

    def test_policy_digest_is_deterministic_and_uses_resolved_policy(self):
        self.assertEqual(model_digest(), model_digest({'java_time_multiplier': 1}))
        self.assertNotEqual(model_digest(), model_digest({'java_time_multiplier': 2}))
        report = complete_report(); report['mixed_performance']['workloads'] = [solve_row(2, 1)]
        self.assertTrue(build_completion(report, {'java_time_multiplier': 2})['ready'])
        self.assertEqual(set(ALL_JOBS), set(BEHAVIOR_JOBS) | set(PERFORMANCE_JOBS))
        with self.assertRaises(ValueError):
            build_completion(report, [])


if __name__ == '__main__':
    unittest.main()
