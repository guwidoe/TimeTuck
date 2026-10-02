"""Original regression tests for truthful list-solving evidence."""
from copy import deepcopy
import unittest
from benchmark_lists import encode, generate, optimum, parse_result, score

class ListSolverEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.case={'id':'case','points':[(1,0),(9,0)],'depots':[(0,0),(10,0)],'capacities':[1,1],
                   'initial':[[0],[1]],'pins':[False,False],'prefixes':[0,0]}
        self.good='case\t10\t0\t-4\t0;1\t3'
    def test_independent_optimum_and_scores(self):
        self.assertEqual(optimum(self.case),(0,-4))
        self.assertEqual(score(self.case,[[1],[0]]),(0,-36))
        self.assertEqual(score(self.case,[[0,1],[]]),(-1,-18))
        c=deepcopy(self.case);c['pins']=[True,True];c['initial']=[[1],[0]]
        self.assertEqual(optimum(c),(0,-36))
        c['pins']=[False,False];c['prefixes']=[1,1]
        self.assertEqual(optimum(c),(0,-36))
    def test_valid_output_and_wire_format(self):
        self.assertEqual(encode(self.case),'case\t1:0,9:0\t0:0,10:0\t1,1\t0;1\t00\t0,0')
        r=parse_result(self.case,self.good,'hill',(0,-4))
        self.assertEqual(r['score'],[0,-4]);self.assertEqual(r['lists'],[[0],[1]])
    def test_missing_duplicate_unknown_wrong_shape_score_and_time_rejected(self):
        for bad in [self.good.replace('case\t','other\t'),self.good.replace('\t10\t','\t0\t'),
                    self.good.replace('\t-4\t','\t-3\t'),self.good.replace('0;1','0;0'),self.good.replace('0;1','0;2'),
                    self.good.replace('0;1','0;-'),self.good.replace('0;1','0,1'),self.good[:-1]+'0']:
            with self.subTest(bad=bad), self.assertRaises(ValueError):parse_result(self.case,bad,'hill')
    def test_pin_prefix_and_construction_order_rejected(self):
        c=deepcopy(self.case);c['initial']=[[1],[0]]
        for pins,prefixes in [([True,False],[0,0]),([False,False],[1,0])]:
            c['pins']=pins;c['prefixes']=prefixes
            with self.assertRaisesRegex(ValueError,'Pin'):parse_result(c,self.good,'hill')
        c['pins']=[False,False];c['prefixes']=[0,0]
        with self.assertRaisesRegex(ValueError,'Construction'):parse_result(c,self.good,'construct')
    def test_non_regression_and_optimum_upper_bound(self):
        with self.assertRaisesRegex(ValueError,'regressed'):parse_result(self.case,'case\t10\t0\t-36\t1;0\t3','hill')
        with self.assertRaisesRegex(ValueError,'optimum'):parse_result(self.case,self.good,'hill',(0,-5))
    def test_generation_is_reproducible_and_optima_account_for_pins(self):
        generated=generate();self.assertEqual(generated,generate());self.assertEqual(sum(map(len,generated.values())),64)
        ids=[c['id'] for cases in generated.values() for c in cases];self.assertEqual(len(set(ids)),64)
        for mode,cases in generated.items():
            for c in cases:
                self.assertEqual(len(c['initial']),len(c['depots']))
                self.assertIsNotNone(optimum(c))
                if mode=='hill':self.assertGreaterEqual(optimum(c),score(c,c['initial']))

if __name__=='__main__':unittest.main()
