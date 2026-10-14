"""Original repair measurement regressions: validity, work and quality stay separate."""
import itertools
import unittest
from benchmark_repair import generate_cases, workload, group_size, validate_scope, parse_result, quality_summary, validate_scoring_equivalence
from differential_solver import score, optimum


class RepairBenchmarkTests(unittest.TestCase):
    def setUp(self):
        self.case = dict(id='check', value_count=2, initial=[0, 0], pins=[False, False],
                         edges=[(0, 1)], weights=[[-1, 0], [-1, 0]])

    def test_valid_differing_quality_and_work_counts_are_retained(self):
        best = parse_result(self.case, 'check\t100\t0\t-1\t0,1\t50\t8', 8, (0, -1))
        poor = parse_result(self.case, 'check\t200\t-1\t0\t1,1\t30\t8', 8, (0, -1))
        rows = [dict(java=poor, rust=best, optimal_score=[0, -1])]
        self.assertEqual(quality_summary(rows), dict(total=1, passed=1, equal_scores=0, optimal=dict(java=0, rust=1)))
        self.assertEqual(best['repairs'], 8)
        self.assertEqual(poor['score_calls'], 30)

    def test_incremental_baseline_must_preserve_result_and_logical_work(self):
        result = parse_result(self.case, 'check\t100\t0\t-1\t0,1\t50\t8', 8, (0, -1))
        validate_scoring_equivalence(result, dict(result, elapsed_ns=999))
        for key, value in [('score', [-1, 0]), ('values', [1, 0]),
                           ('score_calls', 49), ('repairs', 7)]:
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                validate_scoring_equivalence(result, dict(result, **{key: value}))

    def test_protocol_rejects_wrong_work_identity_score_and_assignment(self):
        valid = 'check\t100\t0\t-1\t0,1\t50\t8'
        for column, replacement in [(0, 'other'), (1, '0'), (2, '1'), (4, '0,2'),
                                     (4, '0,-'), (4, '0'), (5, '0'), (6, '0'), (6, '7'), (6, '9'), (6, 'nan')]:
            fields = valid.split('\t'); fields[column] = replacement
            with self.subTest(column=column, replacement=replacement), self.assertRaises(ValueError):
                parse_result(self.case, '\t'.join(fields), 8, (0, -1))
        for line in [valid+'\textra', valid.rsplit('\t', 1)[0]]:
            with self.assertRaises(ValueError): parse_result(self.case, line, 8, (0, -1))
        with self.assertRaisesRegex(ValueError, 'exceeds'):
            parse_result(self.case, valid, 8, (-1, 0))

    def test_retained_result_cannot_regress_initial_quality(self):
        # Parser is defensive even outside the benchmark's worst-initial scope.
        case = dict(self.case, initial=[0, 1])
        with self.assertRaisesRegex(ValueError, 'Regressed'):
            parse_result(case, 'check\t100\t-1\t-2\t0,0\t50\t8', 8, (0, -1))

    def test_bounded_acceptance_assumption_is_exhaustively_true(self):
        cases = generate_cases()
        self.assertEqual(cases, generate_cases())
        self.assertTrue(any(optimum(c)[0] < 0 for c in cases))
        for case in cases:
            validate_scope(case)
            initial = score(case, case['initial'])
            self.assertTrue(all(score(case, v) >= initial for v in itertools.product(range(3), repeat=len(case['initial']))))
            self.assertGreaterEqual(optimum(case), initial)

    def test_scope_rejects_inputs_that_break_comparison_contract(self):
        for change in [dict(domains=[[0, 1]]*2), dict(optional=[False]*2), dict(value_count=1),
                       dict(initial=[None, 0]), dict(initial=[1, 0]), dict(initial=[]),
                       dict(pins=[True, False]), dict(weights=[[0, -1], [-1, 0]])]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_scope(dict(self.case, **change))
        self.assertEqual(group_size(self.case, 'half'), 1)
        self.assertEqual(group_size(self.case, 'all'), 2)
        with self.assertRaises(ValueError): group_size(self.case, 'typo')

    def test_timing_fixtures_have_proven_planted_optima_and_fixed_group_sizes(self):
        for size in [32, 96]:
            case = workload(size)
            validate_scope(case)
            self.assertEqual(score(case, [1+i % 3 for i in range(size)]), (0, 0))
            self.assertTrue(all(w <= 0 for row in case['weights'] for w in row))
            self.assertEqual(group_size(case, 'half'), size//2)
            self.assertEqual(group_size(case, 'all'), size)
