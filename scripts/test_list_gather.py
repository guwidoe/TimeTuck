import unittest
from differential_lists import apply, expected
from differential_list_gather import generate

class GatherReferenceTests(unittest.TestCase):
    def test_original_boundary_and_member_order(self):
        self.assertEqual(apply([[5,0,2],[4,1]], ['g',0,1,2,4,0]), [[5,2,4,0],[1]])
        self.assertEqual(apply([[0,1,2,3]], ['g',0,2,3,0,2]), [[1,3,0,2]])
        self.assertEqual(apply([[0],[1]], ['g',1,0,2,0]), [[],[2,0,1]])
        self.assertEqual(apply([[0,1],[2]], ['g',-1,0,2,0]), [[1],[]])
    def test_invalid_boundaries_and_duplicates(self):
        for op in [['g',0,3,0],['g',2,0,0],['g',0,0,0,0]]:
            with self.assertRaises(ValueError): apply([[0]],op)
    def test_generator_is_deterministic_and_covers_insert_and_unassign(self):
        cases = generate()
        self.assertEqual(cases,generate())
        self.assertEqual(len(cases),256)
        self.assertEqual(len({c['id'] for c in cases}),256)
        self.assertGreater(sum(c['moves'][0][1]==-1 for c in cases),40)
        self.assertGreater(sum(any(v not in sum(c['initial'],[]) for v in c['moves'][0][3:]) for c in cases),40)
        for c in cases:
            self.assertEqual(len(expected(c).split('\t')),4)
