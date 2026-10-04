"""Original checks that construction measurements cannot hide invalid or poorer results."""
import copy
import unittest
from benchmark_construction import reference, parse_result, quality_summary, workload, MODES
from value_queue_construction_checks import generate_cases
from differential_solver import score


class ConstructionBenchmarkTests(unittest.TestCase):
    def setUp(self):
        self.case = dict(id='check', value_count=2, initial=[0, 0], pins=[True, False],
                         edges=[(0, 1)], weights=[[0, 0], [0, 0]])

    def test_valid_result_does_not_require_equal_counts_or_preserved_movable_initials(self):
        r = parse_result(self.case, 'check\t100\t0\t0\t0,1\t5', (0, 0))
        self.assertEqual(r, dict(elapsed_ns=100, score=[0, 0], values=[0, 1], score_calls=5))
        # Poorer valid construction quality is retained as evidence, not discarded.
        r = parse_result(self.case, 'check\t100\t-1\t0\t0,0\t8', (0, 0))
        self.assertEqual(r['score'], [-1, 0])

    def test_invalid_results_fail_closed(self):
        for row in ['wrong\t100\t0\t0\t0,1\t5', 'check\t0\t0\t0\t0,1\t5',
                    'check\t-1\t0\t0\t0,1\t5', 'check\t100\t0\t0\t0,1\t0',
                    'check\t100\t1\t0\t0,1\t5', 'check\t100\t0\t0\t1,0\t5',
                    'check\t100\t0\t0\t0,2\t5', 'check\t100\t0\t0\t0,-\t5',
                    'check\t100\t0\t0\t0\t5', 'check\t100\t0\t0\t0,1\t5\textra']:
            with self.subTest(row=row), self.assertRaises(ValueError): parse_result(self.case, row)
        with self.assertRaisesRegex(ValueError, 'exceeds'):
            parse_result(self.case, 'check\t100\t0\t0\t0,1\t5', (-1, 0))

    def test_references_keep_algorithm_order_and_pin_semantics(self):
        case = generate_cases()[-3]
        self.assertEqual(reference(case, 'sequential'), ([0, 0, 0], (0, 16)))
        self.assertEqual(reference(case, 'pooled'), ([0, 0, 0], (0, 16)))
        self.assertEqual(reference(case, 'value-queue'), ([1, 0, 0], (0, 15)))
        case = copy.deepcopy(case)
        case['initial'] = [1, 1, 1]
        case['pins'] = [True, False, False]
        for mode in MODES:
            self.assertEqual(reference(case, mode)[0][0], 1)
        self.assertEqual(reference(case, 'sequential'), ([1, 0, 0], (0, 15)))
        for mode in MODES:
            for change in [dict(domains=[[0]]*3), dict(optional=[False]*3), dict(value_count=0)]:
                with self.subTest(mode=mode, change=change), self.assertRaises(ValueError):
                    reference(dict(case, **change), mode)
        with self.assertRaises(ValueError): reference(case, 'unknown')
        with self.assertRaises(ValueError): reference(dict(case, initial=[None]*3), 'sequential')

    def test_planted_optimum_and_inputs_are_shared_between_strategies(self):
        for size in [32, 96]:
            c = workload(size)
            planted = [i % 4 for i in range(size)]
            self.assertEqual(score(c, planted), (0, 0))
            self.assertTrue(all(v <= 0 for row in c['weights'] for v in row))
            self.assertTrue(all(not pin or initial == v for pin, initial, v in zip(c['pins'], c['initial'], planted)))
            self.assertEqual(c, workload(size))

    def test_quality_agreement_optimality_and_identity_are_distinct(self):
        rows = [dict(java=dict(score=[0, 2], values=[0]), rust=dict(score=[0, 3], values=[1]),
                     reference_values=[1], optimal_score=[0, 3])]
        self.assertEqual(quality_summary(rows), dict(total=1, passed=1, equal_scores=0, equal_assignments=0,
                                                   native_reference_passed=1, optimal=dict(java=0, rust=1)))
        rows.append(dict(java=dict(score=[0, 3], values=[0]), rust=dict(score=[0, 3], values=[1]),
                         reference_values=[1], optimal_score=[0, 3]))
        self.assertEqual(quality_summary(rows), dict(total=2, passed=2, equal_scores=1, equal_assignments=0,
                                                   native_reference_passed=2, optimal=dict(java=1, rust=2)))
