"""Ensure list evidence cannot pass stale shadows or broken undo."""
import unittest
from differential_lists import apply, encode, expected, generate, state, validate

class ListEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.case = {'id':'list','count':3,'initial':[[0,1],[2]],'moves':[['c',0,0,1,1]]}
    def test_reference_preserves_post_removal_indices_and_neighbors(self):
        self.assertEqual(expected(self.case),'list\t1;2,0/1:1:2:-,0:0:-:-,1:0:-:0\t0,1;2/0:0:-:1,0:1:0:-,1:0:-:-\t1;2,0/1:1:2:-,0:0:-:-,1:0:-:0')
        self.assertEqual(encode(self.case),'list\t3\t0,1;2\tc,0,0,1,1')
    def test_wrong_candidate_undo_commit_and_shadow_cannot_pass(self):
        good = expected(self.case)
        self.assertIsNone(validate(self.case,good))
        for bad in ['ERROR',good.replace('list\t','wrong\t'),good.replace('1:1:2:-','1:0:-:-'),good.replace('0,1;2/','1;2,0/'),good.rsplit('\t',1)[0]+'\t0,1;2/']:
            with self.subTest(response=bad): self.assertIsNotNone(validate(self.case,bad))
    def test_duplicate_and_unknown_values_are_rejected(self):
        for lists in [[[0],[0]],[[3]]]:
            with self.assertRaises(ValueError): state(lists,3)
    def test_sublist_reference_reversal_cross_owner_and_post_removal_bounds(self):
        self.assertEqual(apply([[0,1,2,3,4]],['r',0,1,3,0,3,0]),[[0,3,4,1,2]])
        self.assertEqual(apply([[0,1,2],[3,4]],['r',0,0,3,1,1,1]),[[],[3,2,1,0,4]])
        for op in [['r',0,1,3,0,4,0],['r',0,3,2,0,0,0],['r',0,0,6,0,0,0],['r',0,0,2,0,0,2]]:
            with self.assertRaises(ValueError): apply([[0,1,2,3,4]],op)

    def test_generator_is_deterministic_and_has_unique_cases(self):
        cases = generate()
        self.assertEqual(cases,generate());self.assertEqual(len(cases),384)
        self.assertEqual(len({c['id'] for c in cases}),384)
        for case in cases: self.assertIsNone(validate(case,expected(case)))

if __name__ == '__main__': unittest.main()
