"""Independent boundary and corruption checks for generated follower evidence."""
import unittest
from copy import deepcopy
from follower_checks import apply, encode, expected, generate, states, validate_outputs


class FollowerChecksTest(unittest.TestCase):
    def test_interface_source_retains_all_null_rows(self):
        case = generate()[0]
        state = next(states(case))
        # Two leaders, four followers: four nulls and two ones => -(16+4).
        self.assertEqual(expected(case, 0, state),
                         'follower-000\t0\t0\t-20\t-,1\t0,0,1,-\t-,-,1,-')

    def test_shared_fixture_scores_and_undo(self):
        case = generate()[3]
        rows = list(states(case))
        self.assertEqual([int(expected(case, i, s).split('\t')[3]) for i, s in enumerate(rows)],
                         [-64, -32, -26, -34, -26, -34])
        self.assertEqual(rows[-1], rows[-3])

    def test_fixed_missing_references_are_not_genuine_variables(self):
        case = generate()[4]
        first = next(states(case))
        self.assertEqual(expected(case, 0, first).split('\t')[2:4], ['1', '-8'])
        dynamic = dict(case, model='dynamic')
        self.assertEqual(expected(dynamic, 0, first).split('\t')[2], '0')

    def test_batches_are_sequential_and_trials_do_not_mutate_initial(self):
        case = generate()[0]
        initial = next(states(case))
        original = deepcopy(initial)
        result = apply(case, initial, [['v', 0, 0], ['v', 0, 2], ['r', 0, 1]])
        self.assertEqual(result, {'leaders': [2, 1], 'refs': [1, 0, 1, None]})
        self.assertEqual(initial, original)
        rows = list(states(case))
        self.assertEqual(rows[0], rows[2])
        rows[2]['leaders'][0] = 999
        self.assertEqual(rows[0], original)

    def test_invalid_moves_and_domains_are_rejected(self):
        case = generate()[0]
        state = next(states(case))
        for move in [['v', -1, 0], ['r', 4, 0], ['r', 0, 2], ['v', 0, 3], ['x', 0, 0]]:
            with self.subTest(move=move), self.assertRaises(ValueError):
                apply(case, state, [move])
        with self.assertRaises(ValueError):
            apply(dict(case, model='shared'), state, [['r', 0, None]])
        with self.assertRaises(ValueError):
            expected(case, 0, {'leaders': [0], 'refs': state['refs']})

    def test_responses_require_both_runtimes_and_every_exact_observation(self):
        cases = generate()[:2]
        rows = [expected(c, i, s) for c in cases for i, s in enumerate(states(c))]
        self.assertEqual(len(validate_outputs(cases, {'java': rows, 'rust': rows})), len(rows))
        for outputs in [{'java': rows}, {'java': rows[:-1], 'rust': rows},
                        {'java': rows, 'rust': rows + [rows[-1]]}]:
            with self.assertRaises(ValueError):
                validate_outputs(cases, outputs)
        for field, replacement in [(0,'wrong'),(1,'99'),(2,'1'),(3,'0'),(4,'0,1'),(5,'1,0,1,-'),(6,'0,0,1,-')]:
            corrupted = rows[:]
            parts = corrupted[0].split('\t'); parts[field] = replacement
            corrupted[0] = '\t'.join(parts)
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_outputs(cases, {'java': corrupted, 'rust': rows})

    def test_seeded_cases_are_reproducible_and_cover_both_models(self):
        cases = generate()
        self.assertEqual(cases, generate())
        self.assertEqual(len(cases), 128)
        self.assertEqual({c['model'] for c in cases}, {'dynamic', 'shared'})
        self.assertEqual(len({c['id'] for c in cases}), 128)
        for case in cases:
            self.assertEqual(len(encode(case).split('\t')), 6)
            for i, state in enumerate(states(case)):
                self.assertEqual(len(expected(case, i, state).split('\t')), 7)


if __name__ == '__main__':
    unittest.main()
