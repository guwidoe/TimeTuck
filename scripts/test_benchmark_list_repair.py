"""Original list-repair evidence checks; no additional adapted-method credit."""
from itertools import product
import unittest
from benchmark_list_repair import (encode, generate_cases, group_limit, initial, optimum,
                                  parse_result, quality_summary, score, states, validate_scope, workload)


class ListRepairBenchmarkTests(unittest.TestCase):
    def setUp(self):
        self.case = dict(id='check', optional=True, caps=[0, 2], costs=[[20, 0], [20, 9], [20, 1]], missing=[5, 3, 5])

    def test_owner_and_permutation_enumeration_keeps_omissions_and_order(self):
        self.assertEqual(len(list(states(self.case))), 49)
        self.assertEqual(optimum(self.case), (0, -4))
        self.assertEqual(score(self.case, [[], [2, 0]]), (0, -4))
        self.assertEqual(score(self.case, [[], [0, 2]]), (0, -6))
        self.assertEqual(score(self.case, [[], []]), (0, -13))
        required = dict(self.case, optional=False)
        self.assertEqual(len(list(states(required))), 24)
        self.assertEqual(optimum(required), (0, -11))
        self.assertEqual(encode(self.case), 'check\t1\t0,2\t20,0;20,9;20,1\t5,3,5')

    def test_every_quality_case_has_worst_initial_and_independently_bounded_optimum(self):
        cases = generate_cases()
        self.assertEqual(cases, generate_cases())
        self.assertEqual(len(cases), 32)
        for c in cases:
            validate_scope(c)
            scores = [score(c, rows) for rows in states(c)]
            self.assertEqual(min(scores), score(c, initial(c)))
            # The best ordering is descending within each owner; enumerate only
            # owners and calculate costs directly without the shared score helper.
            candidates = []
            for owners in product(list(range(len(c['caps'])))+([-1] if c['optional'] else []), repeat=len(c['costs'])):
                costs = sum(c['missing'][v] if e == -1 else c['costs'][v][e] for v, e in enumerate(owners))
                costs += sum(max(0, owners.count(e)-cap)**2 for e, cap in enumerate(c['caps']))
                candidates.append(-costs)
            self.assertEqual(max(scores), (0, max(candidates)))

    def test_parser_checks_omissions_and_keeps_quality_and_actual_work_separate(self):
        best = parse_result(self.case, 'check\t100\t0\t-4\t-;2,0\t30\t8', 8, (0, -4))
        worse = parse_result(self.case, 'check\t200\t0\t-6\t-;0,2\t50\t8', 8, (0, -4))
        self.assertEqual(best['unassigned'], 1)
        self.assertEqual(quality_summary([dict(java=best, rust=worse, optimal_score=[0, -4])]),
                         dict(total=1, passed=1, equal_scores=0, optimal=dict(java=1, rust=0)))
        short = 'check\t100\t0\t-4\t-;2,0\t30\t1'
        with self.assertRaisesRegex(ValueError, 'repair count'): parse_result(self.case, short, 8, (0, -4))
        self.assertEqual(parse_result(self.case, short, 8, (0, -4), False)['repairs'], 1)
        with self.assertRaises(ValueError): parse_result(dict(self.case, optional=False), short, 8, (0, -4), False)

    def test_corrupt_identity_score_values_counters_and_bounds_are_rejected(self):
        good = 'check\t100\t0\t-4\t-;2,0\t30\t8'
        for col, value in [(0, 'wrong'), (1, '0'), (2, '1'), (3, '-3'), (4, '-;2,2'),
                           (4, '-;2,3'), (4, '-;2,-1'), (4, '-'), (4, '-;-;-'),
                           (5, '0'), (6, '9'), (6, '-1'), (6, 'nan')]:
            row = good.split('\t');row[col] = value
            with self.subTest(col=col, value=value), self.assertRaises(ValueError):
                parse_result(self.case, '\t'.join(row), 8, (0, -4))
        for line in [good+'\textra', good.rsplit('\t', 1)[0]]:
            with self.assertRaises(ValueError): parse_result(self.case, line, 8, (0, -4))
        with self.assertRaisesRegex(ValueError, 'exceeds'): parse_result(self.case, good, 8, (0, -5))

    def test_invalid_scope_cannot_support_worst_initial_assumption(self):
        for change in [dict(caps=[1, 2]), dict(caps=[0]), dict(missing=[5, 3]), dict(optional=1),
                       dict(costs=[[20, 0], [20, 21], [20, 1]]), dict(missing=[5, 21, 5]),
                       dict(costs=[[20, 0], [20], [20, 1]]), dict(caps=[0, -1])]:
            with self.subTest(change=change), self.assertRaises(ValueError): validate_scope(dict(self.case, **change))
        self.assertEqual(group_limit(self.case, 'half'), 2)
        self.assertEqual(group_limit(self.case, 'all'), 3)
        with self.assertRaises(ValueError): group_limit(self.case, 'typo')

    def test_large_planted_zero_optima_include_real_optional_omissions(self):
        for optional in [False, True]:
            for n in [24, 48]:
                c, rows = workload(n, optional)
                validate_scope(c)
                self.assertEqual(score(c, rows), (0, 0))
                omitted = n-sum(map(len, rows))
                self.assertEqual(omitted, (n-1)//5 if optional else 0)
                result = parse_result(c, f'{c["id"]}\t1\t0\t0\t'+ ';'.join(','.join(map(str, r)) or '-' for r in rows)+'\t1\t8', 8, (0, 0))
                self.assertEqual(result['unassigned'], omitted)
