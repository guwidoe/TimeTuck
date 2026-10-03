"""Protect ordered-join evidence against silently accepted incorrect results."""
import unittest
from ordered_join_checks import MIN, MAX, encode, expected, generate, states, validate


class OrderedJoinChecksTest(unittest.TestCase):
    def test_hand_calculated_relations_equalities_and_cancellation(self):
        state = {'L': {0: (1, 2, 4, 3)}, 'R': {
            0: (1, 2, 4, 5), 1: (1, 9, 6, -5), 2: (9, 2, 2, 0)}}
        # Exact count/weight pairs, separately specified for each relation and
        # equality arity, catch relation reversal and accidental lexicographic joins.
        for arity, scores in enumerate([
            [(1, 15), (1, -15), (2, 0), (1, 0), (2, 15)],
            [(1, 15), (1, -15), (2, 0), (0, 0), (1, 15)],
            [(1, 15), (0, 0), (1, 15), (0, 0), (1, 15)],
        ]):
            for relation, score in enumerate(scores):
                self.assertEqual(expected(5 * arity + relation, state)[0], score)
        self.assertEqual(expected(10, state)[1], '0,0,1,2,4,3,1,2,4,5')
        boundary = {'L': {0: (0, 0, MIN, 2)}, 'R': {1: (0, 0, MAX, -3)}}
        self.assertEqual(expected(1, boundary)[0], (1, -6))
        self.assertEqual(expected(3, boundary)[0], (0, 0))

    def test_trial_updates_and_removals_restore_exact_captured_rows(self):
        case = {'id': 'undo', 'mode': 0, 'operations': [
            ('c', 'L', 0, [1, 2, 3, 4]), ('t', 'L', 0, [9, 8, 7, 6]),
            ('t', 'L', 0, None), ('t', 'R', 1, [1, 2, 3, 4]), ('c', 'L', 0, None)]}
        out = list(states(case))
        self.assertEqual(len(out), 9)
        self.assertEqual(out[1], out[3])
        self.assertEqual(out[1], out[5])
        self.assertEqual(out[1], out[7])
        self.assertEqual(out[-1], {'L': {}, 'R': {}})
        self.assertEqual(out[2]['L'][0], (9, 8, 7, 6))
        self.assertEqual(out[4]['L'], {})
        self.assertEqual(out[6]['R'][1], (1, 2, 3, 4))

    def test_validator_rejects_wrong_scores_payloads_identity_and_response_counts(self):
        cases = [{'id': 'x', 'mode': 0, 'operations': [
            ('c', 'L', 0, [0, 0, 3, 0]), ('c', 'R', 1, [0, 0, 3, 9])]}]
        rust = ['x\t0\t0\t0\t', 'x\t1\t0\t0\t', 'x\t2\t1\t0\t0,1,0,0,3,0,0,0,3,9']
        java = ['x\t0\t0\t0\t-', 'x\t1\t0\t0\t-', 'x\t2\t1\t0\t-']
        self.assertEqual(validate(cases, rust, 'rust'), 3)
        self.assertEqual(validate(cases, java, 'java'), 3)
        for runtime, correct in [('rust', rust), ('java', java)]:
            for bad in [correct[:-1], correct + [correct[-1]],
                        ['wrong\t0\t0\t0\t-'] + correct[1:],
                        correct[:2] + [correct[2].replace('\t2\t', '\t7\t', 1)],
                        correct[:2] + [correct[2].replace('\t1\t0\t', '\t0\t0\t')],
                        correct[:2] + [correct[2] + '\textra']]:
                with self.subTest(runtime=runtime, bad=bad), self.assertRaises(ValueError):
                    validate(cases, bad, runtime)
        # Zero score does not excuse stale payloads or a swapped match identity.
        for row in ['0,1,0,0,3,0,0,0,3,8', '1,0,0,0,3,0,0,0,3,9', '']:
            with self.assertRaises(ValueError): validate(cases, rust[:2] + ['x\t2\t1\t0\t' + row], 'rust')
        with self.assertRaises(ValueError): validate(cases, rust, 'java')
        with self.assertRaises(ValueError): validate(cases, java, 'unknown')

    def test_generated_matrix_is_deterministic_complete_and_finishes_empty(self):
        cases = generate()
        self.assertEqual(cases, generate())
        self.assertEqual(len(cases), 135)
        self.assertEqual(len({c['id'] for c in cases}), 135)
        self.assertEqual({c['mode'] for c in cases}, set(range(15)))
        for case in cases:
            self.assertEqual(len(encode(case).split('\t')), 3)
            observed = list(states(case))
            self.assertEqual(len(observed), 1 + sum(2 if op[0] == 't' else 1 for op in case['operations']))
            self.assertEqual(observed[-1], {'L': {}, 'R': {}})
        self.assertEqual(sum(len(list(states(c))) for c in cases), 8976)


if __name__ == '__main__':
    unittest.main()
