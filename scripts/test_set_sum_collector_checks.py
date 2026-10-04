import unittest
from set_sum_collector_checks import LOW,HIGH,expected,generate,validate
class SetSumCollectorChecksTest(unittest.TestCase):
    def case(self,mode,items,ops):return dict(id='x',mode=mode,items=items,operations=ops)
    def test_sets_preserve_duplicates_until_final_removal(self):
        for mode in ['set','sorted']:
            c=self.case(mode,[[0,2],[0,1]],[('c',0,0),('c',1,1),('c',2,1),('c',1,-1),('c',2,-1),('c',0,-1)])
            self.assertEqual([r.split('\t')[-1] for r in expected(c)],['-','2','1,2','1,2','1,2','2','-'])
    def test_merged_zero_keys_wrap_and_last_occurrence(self):
        c=self.case('map',[[2,2**31-1],[2,1],[-1,-2**31],[-1,-2**31]],[('c',0,0),('c',1,1),('c',2,2),('c',3,3),('c',2,-1),('c',3,-1)])
        self.assertEqual([r.split('\t')[-1] for r in expected(c)],['-','2:2147483647','2:-2147483648','-1:-2147483648,2:-2147483648','-1:0,2:-2147483648','-1:-2147483648,2:-2147483648','2:-2147483648'])
    def test_sum_replacement_checks_difference_and_new_total(self):
        for items,ops in [([[0,LOW],[0,HIGH]],[('c',0,0),('c',0,1)]),([[0,HIGH],[0,-1],[0,1]],[('c',0,0),('c',1,1),('c',1,2)])]:self.assertTrue(expected(self.case('sum',items,ops))[-1].endswith('\tOVERFLOW'))
    def test_equal_replacement_and_minimum_removal_do_not_overflow(self):
        c=self.case('sum',[[0,-1],[0,HIGH],[0,1]],[('c',0,0),('c',1,1),('c',2,2),('c',0,0)])
        self.assertTrue(expected(c)[-1].endswith(f'\t{HIGH}'))
        c=self.case('sum',[[0,LOW]],[('c',0,0),('c',0,-1)]);self.assertEqual(expected(c)[-1],'x\t2\t0')
    def test_trial_inverse_and_missing_removal(self):
        c=self.case('sum',[[0,2],[1,-3]],[('c',0,0),('t',0,1),('t',1,0),('c',0,-1),('c',0,-1)])
        self.assertEqual([r.split('\t')[-1] for r in expected(c)],['0','2','-3','2','4','2','0','0'])
    def test_validator_rejects_missing_extra_reordered_wrong_case_and_values(self):
        c=self.case('sum',[[0,HIGH],[0,1]],[('c',0,0),('c',1,1)]);r=expected(c)
        self.assertEqual(validate([c],r),dict(observations=3,expected_overflow_rejections=1))
        for rows in [r[:-1],r+r[-1:],r[::-1],['y\t0\t0',*r[1:]],[*r[:-1],f'x\t2\t{LOW}']]:
            with self.assertRaises(ValueError):validate([c],rows)
    def test_seeded_cases_are_reproducible_and_observations_complete(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),270);self.assertEqual(len({c['id'] for c in cases}),270)
        stats=validate(cases,[r for c in cases for r in expected(c)]);self.assertGreater(stats['expected_overflow_rejections'],0)
if __name__=='__main__':unittest.main()
