"""Guard membership evidence against duplicate pairs and stale payloads."""
import unittest
from membership_join_checks import encode, expected, generate, states, validate, boundary_probes, validate_boundaries


class MembershipChecksTest(unittest.TestCase):
    def test_relations_partitioning_null_and_pair_deduplication(self):
        state = {'L': {0: (1, 2, 9, 3, [None, 0, 0, 1])}, 'R': {
            0: (1, 2, None, 5, [9, 0, 1, 1]),
            1: (1, 9, 1, -5, [0, 9]), 2: (9, 2, 2, 0, [])}}
        for mode in range(6):
            self.assertEqual(expected(mode, state)[0], (2, 0))
        for mode in range(6, 9):
            self.assertEqual(expected(mode, state)[0], (1, 15))
        state = {'L': {0: (0, 0, None, 1, [])}, 'R': {0: (0, 0, None, 2, [None, None])}}
        self.assertEqual(expected(0, state)[0], (0, 0))
        self.assertEqual(expected(1, state)[0], (1, 2))
        self.assertEqual(expected(2, state)[0], (0, 0))
        state['L'][0] = (0, 0, 4, 1, [None, None])
        self.assertEqual(expected(0, state)[0], (1, 2))
        self.assertEqual(expected(1, state)[0], (0, 0))
        self.assertEqual(expected(2, state)[0], (1, 2))

    def test_trials_restore_both_collection_order_and_duplicate_payloads(self):
        row = [1, 2, None, 3, [None, 0, 0]]
        case = {'id': 'undo', 'mode': 2, 'operations': [
            ('c', 'L', 0, row), ('t', 'L', 0, [9, 8, 7, 6, []]),
            ('t', 'L', 0, None), ('t', 'R', 1, row), ('c', 'L', 0, None)]}
        out = list(states(case))
        self.assertEqual(len(out), 9)
        for i in [3, 5, 7]: self.assertEqual(out[1], out[i])
        self.assertEqual(out[2]['L'][0], (9, 8, 7, 6, ()))
        self.assertEqual(out[4]['L'], {})
        self.assertEqual(out[6]['R'][1], (1, 2, None, 3, (None, 0, 0)))
        self.assertEqual(out[-1], {'L': {}, 'R': {}})

    def test_validator_rejects_wrong_score_identity_payload_and_duplicate_pairs(self):
        cases = [{'id': 'x', 'mode': 2, 'operations': [
            ('c', 'L', 0, [0, 0, None, 0, [1, 1]]), ('c', 'R', 1, [0, 0, 3, 9, [1]])]}]
        match = '0,1,0,0,n,0,1/1|0,0,3,9,1'
        rust = ['x\t0\t0\t0\t', 'x\t1\t0\t0\t', 'x\t2\t1\t0\t' + match]
        java = ['x\t0\t0\t0\t-', 'x\t1\t0\t0\t-', 'x\t2\t1\t0\t-']
        for runtime, correct in [('rust', rust), ('java', java)]:
            self.assertEqual(validate(cases, correct, runtime), 3)
            for bad in [correct[:-1], correct + [correct[-1]], ['wrong\t0\t0\t0\t-'] + correct[1:],
                        correct[:2] + [correct[2].replace('\t2\t', '\t8\t', 1)],
                        correct[:2] + [correct[2].replace('\t1\t0\t', '\t2\t0\t')],
                        correct[:2] + [correct[2] + '\textra']]:
                with self.assertRaises(ValueError): validate(cases, bad, runtime)
        for bad in ['', match + ';' + match, match.replace('1/1', '1'), match.replace('0,1,', '1,0,', 1)]:
            with self.assertRaises(ValueError): validate(cases, rust[:2] + ['x\t2\t1\t0\t' + bad], 'rust')
        with self.assertRaises(ValueError): validate(cases, java, 'unknown')

    def test_generated_cases_are_repeatable_and_end_empty(self):
        cases = generate()
        self.assertEqual(cases, generate())
        self.assertEqual(len(cases), 54)
        self.assertEqual(len({c['id'] for c in cases}), 54)
        self.assertEqual({c['mode'] for c in cases}, set(range(9)))
        for case in cases:
            self.assertEqual(len(encode(case).split('\t')), 3)
            out = list(states(case))
            self.assertEqual(len(out), 1 + sum(2 if op[0] == 't' else 1 for op in case['operations']))
            self.assertEqual(out[-1], {'L': {}, 'R': {}})

    def test_boundary_probes_preserve_explicit_known_disagreements(self):
        cases = boundary_probes()
        self.assertEqual(len(cases), 18)
        self.assertEqual(sum(c['java_final_score'] != [1, 6] for c in cases), 8)
        lines = []
        for c in cases:
            for i, score in enumerate([[0, 0], [0, 0], c['java_final_score']]):
                lines.append(f'{c["id"]}\t{i}\t{score[0]}\t{score[1]}\t-')
            self.assertEqual(expected(c['mode'], list(states(c))[-1])[0], (1, 6))
        self.assertEqual(validate_boundaries(cases, lines), 54)
        for bad in [lines[:-1], lines[:2]+['duplicate-0\t2\t1\t6\t-']+lines[3:]]:
            with self.assertRaises(ValueError): validate_boundaries(cases, bad)
        for c in generate():
            for _, _, _, row in c['operations']:
                if row is not None:
                    self.assertNotIn(None, row[4])
                    self.assertIsNotNone(row[2])
                    self.assertEqual(len(row[4]), len(set(row[4])))
        self.assertNotEqual(generate(), generate(include_edge_keys=True))
