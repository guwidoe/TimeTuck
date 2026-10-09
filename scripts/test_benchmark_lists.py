"""Original regression tests for truthful list-solving evidence."""
from copy import deepcopy
import unittest
from benchmark_lists import encode, generate, generate_extended, generate_two_opt, generate_search, workload, optimum, parse_result, score

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


    def test_range_constraints_affect_optimum_and_reject_illegal_results(self):
        c=deepcopy(self.case);c['initial']=[[1],[0]];c['ranges']=[[1],[0]]
        self.assertEqual(optimum(c),(0,-36))
        with self.assertRaisesRegex(ValueError,'entity range'):parse_result(c,self.good,'hill')
        c['ranges']=[None,[]];c['initial']=[[],[]]
        self.assertTrue(encode(c).endswith('\t*;-'))
        self.assertEqual(optimum(c),(-1,-18))
        c['ranges']=[[],[]]
        with self.assertRaisesRegex(ValueError,'No complete'):optimum(c)
    def test_extended_generation_is_reproducible_and_retains_initial_membership(self):
        cases=generate_extended();self.assertEqual(cases,generate_extended())
        self.assertEqual(sum(map(len,cases.values())),64)
        original_ids={c['id'] for rows in generate().values() for c in rows}
        for mode,rows in cases.items():
            for c in rows:
                self.assertNotIn(c['id'],original_ids)
                for e,row in enumerate(c['initial']):self.assertTrue(set(row)<=set(c['ranges'][e]))
                self.assertIsNotNone(optimum(c))
                if mode=='hill':self.assertGreaterEqual(optimum(c),score(c,c['initial']))
    def test_two_opt_inputs_preserve_starts_and_workload_equality(self):
        rows=generate_two_opt();self.assertEqual(rows,generate_two_opt());self.assertEqual(len(rows),64)
        self.assertEqual(len({c['id'] for c in rows}),64)
        originals=generate()['hill']+generate_extended()['hill']
        for c,before in zip(rows,originals):
            expected=deepcopy(before);expected['id']=expected['id'].replace('hill','two-opt')
            self.assertEqual(c,expected)
            self.assertGreaterEqual(optimum(c),score(c,c['initial']))
        for n in (16,32):
            c=workload(n,'two-opt');before=workload(n,'hill');before['id']=c['id']
            self.assertEqual(c,before)
            self.assertEqual(sorted(v for row in c['initial'] for v in row),list(range(n)))

    def test_two_opt_cannot_hide_regression_or_impossible_scores(self):
        self.assertEqual(parse_result(self.case,self.good,'two-opt',(0,-4))['score'],[0,-4])
        with self.assertRaisesRegex(ValueError,'regressed'):parse_result(self.case,'case\t10\t0\t-36\t1;0\t3','two-opt')
        with self.assertRaisesRegex(ValueError,'optimum'):parse_result(self.case,self.good,'two-opt',(0,-5))
        with self.assertRaisesRegex(ValueError,'Unknown mode'):parse_result(self.case,self.good,'unknown')

    def test_value_tabu_validates_quality_ranges_and_identical_starts(self):
        originals=generate()['hill']+generate_extended()['hill']
        for mode in ('value-tabu-change', 'value-tabu-swap'):
            cases=generate_search(mode)
            self.assertEqual(len(cases),64)
            for c,before in zip(cases,originals):
                expected=deepcopy(before);expected['id']=expected['id'].replace('hill',mode)
                self.assertEqual(c,expected)
            self.assertEqual(parse_result(self.case,self.good,mode,(0,-4))['score'],[0,-4])
            with self.assertRaisesRegex(ValueError,'regressed'):
                parse_result(self.case,'case\t10\t0\t-36\t1;0\t3',mode)
            c=deepcopy(self.case);c['ranges']=[[1],[0]]
            with self.assertRaisesRegex(ValueError,'entity range'):
                parse_result(c,self.good,mode)
            with self.assertRaisesRegex(ValueError,'optimum'):
                parse_result(self.case,self.good,mode,(0,-5))

    def test_construction_range_rejection_is_independent_of_completeness_and_score(self):
        c=deepcopy(self.case);c['initial']=[[],[]];c['ranges']=[[1],[0]]
        with self.assertRaisesRegex(ValueError,'entity range'):parse_result(c,self.good,'construct')
        r=parse_result(c,'case\t10\t0\t-36\t1;0\t3','construct',(0,-36))
        self.assertEqual(r['lists'],[[1],[0]])

if __name__=='__main__':unittest.main()
