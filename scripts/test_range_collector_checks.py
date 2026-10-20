import unittest
from range_collector_checks import reference, parse, validate, generate

class RangeCollectorChecksTest(unittest.TestCase):
    def case(self):return dict(id='x',intervals=[[0,2],[2,4],[1,3]],operations=[('c',0,0),('c',1,1)])
    def lines(self):return ['x\t0\t-\t-','x\t1\t0,2,2,1,1,1,0:0\t-','x\t2\t0,4,4,2,1,1,0:0,1\t-']
    def test_independent_connectivity_multiplicity_and_touching(self):
        c=self.case();r=reference(c,{0:0,1:1});self.assertEqual(r['ranges'][0]['maximum'],1);self.assertFalse(r['ranges'][0]['overlap'])
        r=reference(c,{0:0,1:1,2:2,3:2});self.assertEqual(r['ranges'][0]['maximum'],3);self.assertEqual(r['ranges'][0]['count'],4);self.assertEqual(r['ranges'][0]['minimum'],1)
    def test_zero_length_sentinels_and_native_semantics(self):
        c=dict(id='z',intervals=[[1,1],[0,2]],operations=[])
        self.assertEqual(parse(c,0,'z\t0\t1,1,0,1,2147483647,-2147483648,0:0\t-',True),reference(c,{0:0}))
        r=reference(c,{0:0,1:1});self.assertEqual(r['ranges'][0]['minimum'],1);self.assertFalse(r['ranges'][0]['overlap'])
        self.assertNotEqual(parse(c,0,'z\t0\t0,2,2,2,1,1,1:1,0\t-',True),r)
    def test_complete_states_and_response_integrity(self):
        c=self.case();lines=self.lines();self.assertEqual(validate([c],lines)['passed'],1)
        for bad in [lines[:-1],lines+lines[:1],lines[::-1],[lines[0],lines[2],lines[1]]]:
            with self.assertRaises(ValueError):validate([c],bad)
        for old,new in [('0,4,4,2','0,4,9,2'),('0,4,4,2','0,4,4,3'),('1,1,0:0,1','1,2,0:0,1'),(':0,1',':0,0')]:
            with self.subTest(new=new),self.assertRaises(ValueError):validate([c],lines[:-1]+[lines[-1].replace(old,new)])
    def test_java_differences_and_errors_never_receive_agreement_credit(self):
        c=self.case();lines=self.lines();bad=lines[:-1]+[lines[-1].replace('0,4,4,2','0,4,-1,2')]
        r=validate([c],bad,True);self.assertEqual((r['passed'],r['divergent_cases'],r['observations']),(0,1,3))
        r=validate([c],lines[:1]+['x\tERROR\tjava.lang.IllegalStateException'],True)
        self.assertEqual((r['passed'],r['error_cases'],r['observations']),(0,1,1))
        with self.assertRaises(ValueError):validate([c],lines[:1]+['x\tERROR\tjava.lang.IllegalStateException'])
    def test_shared_identity_residue_is_recorded(self):
        c=dict(id='d',intervals=[[0,2]],operations=[('c',0,0),('c',1,0),('c',1,-1),('c',0,-1)])
        lines=['d\t0\t-\t-','d\t1\t0,2,2,1,1,1,0:0\t-','d\t2\t0,2,2,2,2,2,1:0,0\t-','d\t3\t0,2,2,2,2,2,1:0,0\t-','d\t4\t0,2,2,1,1,1,0:0\t-']
        r=validate([c],lines,True);self.assertEqual((r['passed'],r['divergent_cases'],len(r['differences'])),(0,1,2))
    def test_unknown_items_bad_shapes_and_flags(self):
        c=self.case()
        for line in ['x\t0\t0,2,2,1,1,1,0:9\t-','x\t0\t0,2,2,1,1,1,2:0\t-','x\t0\t0,2:0\t-','x\t0\t-\t1,2']:
            with self.subTest(line=line),self.assertRaises(ValueError):parse(c,0,line,True)
    def test_distinct_cohort_retains_original_boundary_cases(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),197)
        self.assertEqual(sum(not c['boundary'] for c in cases),64)
        self.assertEqual(sum(c['id'].startswith('range-0-') for c in cases),64)
        from sequence_collector_checks import states
        for c in cases:
            if not c['boundary']:
                for s in states(c):self.assertEqual(len(s),len(set(s.values())))

    def test_explicit_wrapping_keeps_exact_controls_and_all_nonlength_fields(self):
        c=dict(id='wide',intervals=[[-2**31,2**31-1]],operations=[('c',0,0)])
        exact=['wide\t0\t-\t-', 'wide\t1\t-2147483648,2147483647,4294967295,1,1,1,0:0\t-']
        wrapped=[line.replace('4294967295','-1') for line in exact]
        self.assertEqual(validate([c],exact)['passed'],1)
        self.assertEqual(validate([c],wrapped,wrapping=True)['passed'],1)
        self.assertEqual(validate([c],wrapped,java=True,wrapping=True)['passed'],1)
        self.assertEqual(validate([c],wrapped,java=True)['passed'],0)
        with self.assertRaises(ValueError):validate([c],wrapped)
        with self.assertRaises(ValueError):validate([c],exact,wrapping=True)
        for old,new in [('1,1,1,0:0','1,1,1,1:0'),('1,1,1,0:0','2,1,1,0:0')]:
            with self.subTest(field=new),self.assertRaises(ValueError):
                validate([c],wrapped[:1]+[wrapped[1].replace(old,new)],wrapping=True)

if __name__=='__main__':unittest.main()
