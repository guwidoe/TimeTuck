"""Independent collection-supplier boundaries and result-integrity checks."""
import unittest
from copy import deepcopy
from collection_follower_checks import encode, expected, generate, states, validate_outputs


class CollectionFollowerChecksTest(unittest.TestCase):
    def test_utf16_and_equal_code_ties_preserve_value_identity(self):
        case = generate()[0]
        self.assertEqual(expected(case, 0, case['initial']),
                         'collection-follower-000\t0\t1\t-25\t0,1,2,3\t1,1,2,3,1')
        self.assertGreater(case['codes'][1], case['codes'][0])  # Unicode scalar order differs.
        self.assertLess(case['codes'][1].encode('utf-16-be'), case['codes'][0].encode('utf-16-be'))

    def test_all_null_and_empty_codes(self):
        case = generate()[1]
        rows = list(states(case))
        self.assertEqual([int(expected(case, i, v).split('\t')[3]) for i, v in enumerate(rows)],
                         [-16, -10, -8, -10, -10, -16, -10])
        self.assertEqual(expected(case, 4, rows[4]).split('\t')[5], '2,2')

    def test_zero_followers_keep_leader_score_and_completeness(self):
        case = generate()[2]
        rows = list(states(case))
        self.assertEqual(expected(case, 0, rows[0]).split('\t')[2:], ['0','-1','-','_'])
        self.assertEqual(expected(case, 1, rows[1]).split('\t')[2:], ['1','-1','0','_'])

    def test_repeated_writes_and_undo_are_independent_snapshots(self):
        case = generate()[0]
        rows = list(states(case))
        original = deepcopy(case)
        self.assertEqual(rows[0], rows[2])
        self.assertEqual(rows[4][:2], [None,None])
        self.assertEqual(rows[3], rows[5])
        rows[0][0] = 999
        self.assertEqual(rows[2], original['initial'])
        self.assertEqual(case, original)

    def test_invalid_dependency_collections_and_moves(self):
        base = generate()[0]
        for dependencies in [[[]], [[None]], [[-1]], [[4]]]:
            case = dict(base, dependencies=dependencies)
            with self.subTest(dependencies=dependencies), self.assertRaises(ValueError):
                list(states(case))
        for index, value in [(-1,0),(4,0),(0,4),(0,-1)]:
            case = dict(base, batches=[{'mode':'c','moves':[[index,value]]}])
            with self.subTest(index=index,value=value), self.assertRaises(ValueError):
                list(states(case))

    def test_outputs_reject_missing_reordered_or_corrupt_observations(self):
        cases = generate()[:2]
        rows = [expected(c, i, v) for c in cases for i, v in enumerate(states(c))]
        self.assertEqual(len(validate_outputs(cases, {'java':rows,'rust':rows})), len(rows))
        for bad in [rows[:-1], rows+[rows[-1]], rows[::-1]]:
            with self.assertRaises(ValueError):
                validate_outputs(cases, {'java':bad,'rust':rows})
        for field, replacement in [(0,'wrong'),(1,'9'),(2,'0'),(3,'0'),(4,'0,0,0,0'),(5,'1,1,3,2,1')]:
            corrupted = rows[:]
            parts = corrupted[0].split('\t'); parts[field] = replacement
            corrupted[0] = '\t'.join(parts)
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_outputs(cases, {'java':corrupted,'rust':rows})
        with self.assertRaises(ValueError):
            validate_outputs(cases, {'rust':rows})

    def test_generation_is_reproducible_and_protocol_complete(self):
        cases = generate()
        self.assertEqual(cases, generate())
        self.assertEqual(len(cases), 128)
        self.assertEqual(len({c['id'] for c in cases}), 128)
        for case in cases:
            self.assertEqual(len(encode(case).split('\t')), 5)
            for i, values in enumerate(states(case)):
                self.assertEqual(len(expected(case,i,values).split('\t')), 6)


if __name__ == '__main__':
    unittest.main()
