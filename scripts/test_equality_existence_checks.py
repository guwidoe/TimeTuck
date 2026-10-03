"""Guard independently enumerated existence evidence and response validation."""
import unittest
from equality_existence_checks import expected, generate, validate
from ordered_join_checks import states, encode

class EqualityExistenceChecksTest(unittest.TestCase):
    def test_literal_presence_partition_and_weights(self):
        state = {'L': {0: (1, 2, 4, 3), 1: (1, 9, 4, -3), 2: (9, 2, 2, 0)},
                 'R': {0: (1, 2, 4, 99), 1: (1, 2, 4, -99)}}
        for mode, score in enumerate([(3, 0), (0, 0), (2, 0), (1, 0), (1, 3), (2, -3)]):
            self.assertEqual(expected(mode, state)[0], score)
        self.assertEqual(expected(4, state)[1], '0,1,2,4,3')
        self.assertEqual(expected(5, state)[1], '1,1,9,4,-3;2,9,2,2,0')
        state['R'] = {}
        for mode in range(6):
            self.assertEqual(expected(mode, state)[0], (0, 0) if mode % 2 == 0 else (3, 0))
        with self.assertRaises(ValueError): expected(6, state)

    def test_duplicate_removal_and_trial_restore(self):
        case = {'id': 'x', 'mode': 2, 'operations': [
            ('c', 'L', 0, [1, 0, 0, -4]), ('c', 'R', 0, [1, 0, 0, 7]),
            ('c', 'R', 1, [1, 0, 0, 9]), ('t', 'R', 0, None),
            ('c', 'R', 1, None), ('c', 'R', 0, None)]}
        self.assertEqual([expected(2, s)[0] for s in states(case)],
                         [(0, 0), (0, 0), (1, -4), (1, -4), (1, -4), (1, -4), (1, -4), (0, 0)])

    def test_wrong_identity_multiplicity_payload_and_counts_rejected(self):
        cases = [{'id': 'x', 'mode': 1, 'operations': [('c', 'L', 0, [1, 2, 3, 0])]}]
        rust = ['x\t0\t0\t0\t', 'x\t1\t1\t0\t0,1,2,3,0']
        java = ['x\t0\t0\t0\t-', 'x\t1\t1\t0\t-']
        for runtime, lines in [('rust', rust), ('java', java)]:
            self.assertEqual(validate(cases, lines, runtime), 2)
            for bad in [lines[:-1], lines + [lines[-1]], ['wrong\t0\t0\t0\t-'] + lines[1:],
                        lines[:1] + [lines[1].replace('\t1\t1\t', '\t9\t1\t')],
                        lines[:1] + [lines[1].replace('\t1\t0\t', '\t2\t0\t')]]:
                with self.assertRaises(ValueError): validate(cases, bad, runtime)
        for match in ('', '1,1,2,3,0', '0,1,2,9,0', '0,1,2,3,0;0,1,2,3,0'):
            with self.assertRaises(ValueError): validate(cases, rust[:1] + ['x\t1\t1\t0\t' + match], 'rust')
        with self.assertRaises(ValueError): validate(cases, java, 'unknown')

    def test_generated_matrix_is_deterministic_and_ends_empty(self):
        cases = generate()
        self.assertEqual(cases, generate())
        self.assertEqual(len(cases), 54)
        self.assertEqual(len({c['id'] for c in cases}), 54)
        self.assertEqual({c['mode'] for c in cases}, set(range(6)))
        for case in cases:
            self.assertEqual(len(encode(case).split('\t')), 3)
            observed = list(states(case))
            self.assertEqual(len(observed), 1 + sum(2 if x[0] == 't' else 1 for x in case['operations']))
            self.assertEqual(observed[-1], {'L': {}, 'R': {}})

if __name__ == '__main__':
    unittest.main()
