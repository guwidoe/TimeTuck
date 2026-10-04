import unittest
from composition_collector_checks import expected,generate,states,validate
class CompositionCollectorChecksTest(unittest.TestCase):
    def case(self,mode):return dict(id='x',mode=mode,items=[4,1],operations=[('c',0,0),('c',1,1),('c',2,1),('c',1,-1),('c',2,-1),('c',0,-1)])
    def test_all_modes_empty_duplicate_and_remove(self):
        expected_fields={'two':['-,-','4,4','1,4','1,4','1,4','4,4','-,-'],'then':['0','10','20','30','20','10','0'],'three':['-,-,-','4,4,4616189618054758400','1,4,4612811918334230528','1,4,4611686018427387904','1,4,4612811918334230528','4,4,4616189618054758400','-,-,-']}
        expected_fields['four']=[f'{n},{r}' for n,r in zip([0,1,2,3,2,1,0],expected_fields['three'])]
        for mode,fields in expected_fields.items():self.assertEqual(expected(self.case(mode)),[f'x\t{i}\t{v}' for i,v in enumerate(fields)])
    def test_trial_restores_replaced_and_absent_slots(self):
        c=self.case('then');c['operations']=[('c',0,0),('t',0,1),('t',1,1),('c',0,-1),('c',0,-1)]
        self.assertEqual([list(s.values()) for s in states(c)],[[],[0],[1],[0],[0,1],[0],[],[]])
    def test_negative_and_zero_average_bits(self):
        c=dict(id='x',mode='three',items=[-1,1],operations=[('c',0,0),('c',1,1)])
        self.assertEqual(expected(c),['x\t0\t-,-,-','x\t1\t-1,-1,13830554455654793216','x\t2\t-1,1,0'])
    def test_validator_rejects_incomplete_extra_reordered_and_invalid_results(self):
        c=self.case('four');r=expected(c)
        for rows in [r[:-1],r+r[-1:],r[::-1],[r[0],r[0],*r[2:]],['y\t0\t0,-,-,-',*r[1:]],['x\tERROR\terror'],[r[0],r[1]+'0',*r[2:]]]:
            with self.assertRaises(ValueError):validate([c],rows)
    def test_seeded_generation_is_reproducible_complete_and_drains(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),132);self.assertEqual(len({c['id'] for c in cases}),132)
        for c in cases:self.assertEqual(list(states(c))[-1],{})
        rows=[r for c in cases for r in expected(c)];self.assertEqual(validate(cases,rows),len(rows))
if __name__=='__main__':unittest.main()
