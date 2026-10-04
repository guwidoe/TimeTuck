import unittest
from period_collector_checks import arithmetic,expected,generate,validate,LOW,HIGH

class PeriodCollectorChecksTest(unittest.TestCase):
    def case(self,items,ops):return dict(id='x',items=items,operations=ops)
    def test_empty_and_unnormalized_signed_components(self):
        c=self.case([[1,12,-3],[-2,-1,4]],[('c',0,0),('c',1,1)])
        self.assertEqual(expected(c),['x\t0\t0,0,0','x\t1\t1,12,-3','x\t2\t-1,11,1'])
        self.assertEqual(expected(self.case([[0,0,0]],[])),['x\t0\t0,0,0'])
    def test_each_component_overflow_and_minimum_subtraction(self):
        for axis in range(3):
            for a,b,subtract in [(HIGH,1,False),(LOW,-1,False),(HIGH,-1,True),(LOW,1,True)]:
                x=[0]*3;y=[0]*3;x[axis]=a;y[axis]=b
                with self.assertRaises(OverflowError):arithmetic(x,y,subtract)
        self.assertEqual(arithmetic([LOW]*3,[LOW]*3,True),[0]*3)
    def test_same_and_equal_replacements_check_subtraction_before_addition(self):
        items=[[0,0,-1],[0,0,HIGH],[0,0,1],[0,0,-1],[0,0,-2]]
        prefix=[('c',0,0),('c',1,1),('c',2,2)]
        for item in [0,3,4,-1]:
            c=self.case(items,prefix+[('c',0,item),('c',1,-1)])
            rows=expected(c);self.assertEqual(len(rows),5);self.assertEqual(rows[-1],'x\t4\tOVERFLOW')
        self.assertEqual(items[0],[0,0,-1])
    def test_trial_restore_duplicate_zero_and_missing_retraction(self):
        c=self.case([[1,-2,3],[0,0,0]],[('c',0,0),('c',1,0),('t',0,1),('c',1,-1),('c',0,-1),('c',0,-1)])
        rows=expected(c)
        self.assertEqual([r.split('\t')[-1] for r in rows],['0,0,0','1,-2,3','2,-4,6','1,-2,3','2,-4,6','1,-2,3','0,0,0','0,0,0'])
    def test_validator_counts_expected_rejection_as_rejection_not_update(self):
        c=self.case([[HIGH,0,0],[1,0,0]],[('c',0,0),('c',1,1),('c',1,-1)])
        self.assertEqual(validate([c],expected(c)),dict(observations=3,expected_overflow_rejections=1))
        with self.assertRaises(ValueError):validate([c],expected(c)[:-1]+['x\t2\t-2147483648,0,0'])
    def test_validator_rejects_missing_extra_reordered_wrong_identity_and_fields(self):
        c=self.case([[1,12,-3]],[('c',0,0),('c',0,-1)]);rows=expected(c)
        for bad in [rows[:-1],rows+rows[-1:],rows[::-1],[rows[0],rows[0],rows[2]],
                    [rows[0],'y\t1\t1,12,-3',rows[2]],[rows[0],'x\t1\t2,0,-3',rows[2]],
                    [rows[0],'x\t1\t1,12',rows[2]],[rows[0],'x\t1\tOVERFLOW']]:
            with self.assertRaises(ValueError):validate([c],bad)
    def test_seeded_cases_retain_all_boundaries_and_observations(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),227)
        rows=[r for c in cases for r in expected(c)];self.assertEqual(len(rows),8771)
        self.assertEqual(sum(r.endswith('\tOVERFLOW') for r in rows),120)
        self.assertEqual(len({c['id'] for c in cases}),227)
        for c in cases:
            r=expected(c)
            if c['id'].startswith(('period-0-', 'period-1-')) and not r[-1].endswith('\tOVERFLOW'):
                self.assertTrue(r[-1].endswith('\t0,0,0'))

if __name__=='__main__':unittest.main()
