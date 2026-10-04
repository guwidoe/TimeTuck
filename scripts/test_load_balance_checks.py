import unittest
from load_balance_checks import canonical,generate,reference,parse,validate
from sequence_collector_checks import states

class LoadBalanceChecksTest(unittest.TestCase):
    def case(self):return dict(id='x',items=[[0,2,0],[1,1,0]],operations=[('c',0,0),('c',1,1),('t',2,1)])
    def lines(self):return ['x\t0\t-\t0\t0','x\t1\t0:2\t0\t0','x\t2\t0:2;1:1\t707107\t6','x\t3\t0:2;1:2\t0\t0','x\t4\t0:2;1:1\t707107\t6']
    def test_initial_multiplicity_negative_and_zero_keys(self):
        c=dict(items=[[0,2,10],[0,-12,10],[1,0,-4]])
        self.assertEqual(reference(c,{0:0,1:0,2:1,3:2})['loads'],[[0,2],[1,-4]])
        self.assertEqual(reference(c,{0:0,1:1})['loads'],[[0,0]])
        c['items'][1][2]=11
        with self.assertRaises(ValueError):reference(c,{0:0,1:1})
    def test_exact_large_offset_and_overflow_reference(self):
        c=dict(items=[[0,10**9,0],[1,10**9+1,0]])
        self.assertEqual(reference(c,{0:0,1:1})['unfairness'],[707107,6])
        c=dict(items=[[0,2**63-1,2**63-1],[1,-2**63,-2**63]])
        r=reference(c,{0:0,1:0,2:1,3:1})
        self.assertEqual(r['loads'],[[0,3*(2**63-1)],[1,-3*2**63]])
        self.assertEqual(r['unfairness'],[391315,-14])
    def test_decimal_normalization_is_explicit(self):
        self.assertEqual(canonical(1000,0),[1,-3]);self.assertEqual(canonical(0,6),[0,0])
        c=self.case();self.assertEqual(parse(c,0,'x\t0\t0:2\t7071070\t7')['unfairness'],[707107,6])
    def test_complete_observations_reject_missing_duplicate_reordered_and_wrong_fields(self):
        c=self.case();lines=self.lines();self.assertEqual(validate([c],lines)['passed'],1)
        for bad in [lines[:-1],lines+lines[:1],lines[::-1],lines[:2]+[lines[1]]+lines[3:]]:
            with self.assertRaises(ValueError):validate([c],bad)
        for bad in ['x\t2\t0:2;1:2\t707107\t6','x\t2\t0:2;1:1\t707108\t6','x\t2\t0:2;1:1\t707107\t5']:
            with self.assertRaises(ValueError):validate([c],lines[:2]+[bad]+lines[3:])
    def test_errors_and_differences_are_not_agreement_passes(self):
        c=self.case();lines=self.lines();bad=lines.copy();bad[2]=bad[2].replace('707107','0')
        r=validate([c],bad,True);self.assertEqual((r['passed'],r['divergent_cases'],len(r['differences'])),(0,1,1))
        err=lines[:2]+['x\tERROR\tjava.lang.ArithmeticException'];r=validate([c],err,True)
        self.assertEqual((r['passed'],r['error_cases'],r['observations']),(0,1,2))
        with self.assertRaises(ValueError):validate([c],err)
    def test_malformed_identity_maps_and_numeric_fields_fail(self):
        c=self.case()
        for raw in ['x\t0\t0:1;0:2\t0\t0','x\t0\t1:1;0:2\t0\t0','x\t0\t9:2\t0\t0','x\t0\t0:2:3\t0\t0','x\t0\t-\t-1\t0','x\t0\t-\t1\t999','y\t0\t-\t0\t0','x\t0\t-\t0']:
            with self.subTest(raw=raw),self.assertRaises(ValueError):parse(c,0,raw)
    def test_seeded_cases_retain_all_boundaries_and_trial_states(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),101)
        self.assertEqual(sum(c['boundary'] for c in cases),5)
        self.assertEqual(sum(len(list(states(c))) for c in cases),8290)
        for c in cases:
            self.assertEqual(list(states(c))[-1],{})
            for s in states(c):reference(c,s)

if __name__=='__main__':unittest.main()
