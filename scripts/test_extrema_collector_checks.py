import unittest
from extrema_collector_checks import allowed,expected,generate,states,validate
class ExtremaCollectorChecksTest(unittest.TestCase):
    def case(self):return dict(id='x',mode='min',items=[[1,10],[1,20],[2,30]],operations=[('c',0,0),('c',1,1),('t',0,2),('c',0,-1),('c',1,-1)])
    def test_empty_ties_and_inverse_restore(self):
        c=self.case();self.assertEqual([allowed(c,s) for s in states(c)],[[],[0],[0,1],[1],[0,1],[1],[]])
        self.assertEqual(validate([c],expected(c),True),7)
        c['mode']='max';self.assertEqual([allowed(c,s) for s in states(c)],[[],[0],[0,1],[2],[0,1],[1],[]])
    def test_java_tie_choice_is_allowed_but_native_is_stable(self):
        c=self.case();rows=expected(c);rows[4]='x\t4\t1,1,20'
        self.assertEqual(validate([c],rows),7)
        with self.assertRaises(ValueError):validate([c],rows,True)
    def test_removed_representative_wrong_payload_and_wrong_extreme_rejected(self):
        c=self.case();good=expected(c)
        for pos,value in [(0,'0,1,10'),(1,'-'),(1,'1,1,20'),(2,'0,1,20'),(3,'2,2,30'),(5,'0,1,10'),(6,'1,1,20'),(2,'0,1'),(2,'nonsense')]:
            rows=good.copy();rows[pos]=f'x\t{pos}\t{value}'
            with self.assertRaises(ValueError):validate([c],rows)
    def test_protocol_missing_extra_duplicate_and_wrong_case_rejected(self):
        c=self.case();r=expected(c)
        for rows in [r[:-1],r+r[-1:],r[::-1],[r[0],r[0],*r[2:]],['y\t0\t-',*r[1:]],['x\tERROR\tjava.error']]:
            with self.assertRaises(ValueError):validate([c],rows)
    def test_duplicate_handles_preserve_item_until_last_removal(self):
        c=dict(id='d',mode='max',items=[[-2**63,9],[2**63-1,8]],operations=[('c',0,1),('c',1,1),('c',0,-1),('c',1,0),('c',1,-1)])
        self.assertEqual([allowed(c,s) for s in states(c)],[[],[1],[1],[1],[0],[]])
        validate([c],expected(c),True)
    def test_seeded_generation_is_reproducible_and_drains(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),196);self.assertEqual(len({c['id'] for c in cases}),196)
        for c in cases:self.assertEqual(list(states(c))[-1],{})
        validate(cases,[r for c in cases for r in expected(c)],True)
if __name__=='__main__':unittest.main()
