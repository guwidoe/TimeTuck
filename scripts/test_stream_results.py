"""Original checks that stream evidence rejects incorrect scores and match identities."""
import copy
import unittest

from benchmark_streams import parse_result, workload
from differential_streams import expected_matches, optimum, score, states, validate


class StreamEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.case = {
            'id': 'stream', 'value_count': 2, 'initial': [0, 0, 1],
            'pins': [True, False, False],
            'edges': [(0, 1), (0, 1), (2, 2)],
            'weights': [[3, 0], [-3, 4], [2, -2]],
            'operations': [
                {'kind': 't', 'variable': 1, 'value': None},
                {'kind': 'c', 'variable': 1, 'value': 1},
            ],
        }

    def test_group_cancellation_duplicate_edges_and_negative_impact(self):
        self.assertEqual(score(self.case, [0, 0, 1]), (-3, -4))
        self.assertEqual(expected_matches(self.case, [0, 0, 1]), ([0, 1, 2], [(1, -2)]))
        self.assertEqual(score(self.case, [0, 1, 1]), (-1, 13))
        self.assertEqual(optimum(self.case), (-1, 41))

    def test_trial_restores_base_before_next_commit(self):
        original = copy.deepcopy(self.case)
        observed = list(states(self.case))
        self.assertEqual(observed, [[0, 0, 1], [0, None, 1], [0, 0, 1], [0, 1, 1]])
        observed[0][0] = 1
        self.assertEqual(observed[2], [0, 0, 1])
        self.assertEqual(self.case, original)

    def test_native_exact_matches_and_community_score_only(self):
        for runtime, suffix in [('rust', '0,1,2\t1:-2'), ('java', '-\t-')]:
            row = f'stream\t0\t1\t-3\t-4\t{suffix}'
            result = validate(self.case, 0, self.case['initial'], row, runtime)
            self.assertEqual(result['score'], [-3, -4])
            self.assertTrue(result['complete'])
        result = validate(self.case, 1, [0, None, 1], 'stream\t1\t0\t-1\t5\t2\t0:3,1:-2', 'rust')
        self.assertFalse(result['complete'])

    def test_incorrect_score_identity_completeness_or_native_matches_rejected(self):
        for row in [
            'other\t0\t1\t-3\t-4\t0,1,2\t1:-2',
            'stream\t1\t1\t-3\t-4\t0,1,2\t1:-2',
            'stream\t0\t0\t-3\t-4\t0,1,2\t1:-2',
            'stream\t0\t2\t-3\t-4\t0,1,2\t1:-2',
            'stream\t0\t1\t-2\t-4\t0,1,2\t1:-2',
            'stream\t0\t1\t-3\t-3\t0,1,2\t1:-2',
            'stream\t0\t1\t-3\t-4\t0,1,1\t1:-2',
            'stream\t0\t1\t-3\t-4\t0,1,2\t0:-2',
            'stream\t0\t1\t-3\t-4\t0,1,2\t0:0,1:-2',
            'stream\t0\t1\t-3\t-4\t0,1,2\t1:-2\textra',
        ]:
            with self.subTest(row=row), self.assertRaises(ValueError):
                validate(self.case, 0, self.case['initial'], row, 'rust')

    def test_java_match_data_and_unknown_runtime_rejected(self):
        for runtime, suffix in [('java', '0,1,2\t1:-2'), ('unknown', '-\t-')]:
            with self.subTest(runtime=runtime), self.assertRaises(ValueError):
                validate(self.case, 0, self.case['initial'], f'stream\t0\t1\t-3\t-4\t{suffix}', runtime)

    def test_benchmark_validates_assignment_score_and_timing(self):
        row = 'stream\t100\t-1\t13\t0,1,1\t8'
        self.assertEqual(parse_result(self.case, row)['score'], [-1, 13])
        for invalid in [
            row.replace('stream', 'other'), row.replace('\t100\t', '\t0\t'),
            row.replace('\t13\t', '\t14\t'), row.replace('0,1,1', '1,1,0'),
            row.replace('0,1,1', '0,1,2'), row.replace('0,1,1', '0,1'),
            row.replace('0,1,1', '0,1,-'), row.replace('\t8', '\t-1'),
        ]:
            with self.subTest(row=invalid), self.assertRaises(ValueError):
                parse_result(self.case, invalid)
        self.case['initial'] = [0, 1, 0]
        with self.assertRaisesRegex(ValueError, 'worse than initial'):
            parse_result(self.case, row)

    def test_zero_terminal_callbacks_are_valid_for_empty_scoring_streams(self):
        case = {'id': 'empty', 'value_count': 1, 'initial': [0], 'pins': [True],
                'edges': [], 'weights': [[0]]}
        self.assertEqual(parse_result(case, 'empty\t1\t0\t0\t0\t0')['score_calls'], 0)

    def test_large_workloads_have_planted_zero_optimum(self):
        for n in [32, 96]:
            case = workload(n)
            self.assertEqual(score(case, [i % 4 for i in range(n)]), (0, 0))
            self.assertTrue(all(w <= 0 for row in case['weights'] for w in row))


if __name__ == '__main__':
    unittest.main()
