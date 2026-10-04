"""Original independent checks of cyclic k-opt benchmark evidence."""
from copy import deepcopy
import unittest
from benchmark_kopt import case, generate, optimum, parse_result, score, summarize

class KOptEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.c=dict(id='square',points=[(0,0),(10,0),(10,10),(0,10)],depots=[(100,100)],capacities=[4],initial=[[0,2,1,3]],pins=[False],prefixes=[0])
        self.good='square\t10\t0\t-40\t0,1,2,3\t8'
    def test_cycle_metric_ignores_depot_and_is_rotation_reversal_invariant(self):
        self.assertEqual(score(self.c,[0,2,1,3]),(0,-60))
        for row in [[0,1,2,3],[2,3,0,1],[3,2,1,0]]:
            self.assertEqual(score(self.c,row),(0,-40))
        self.assertEqual(optimum(self.c),(0,-40))
        self.assertEqual(parse_result(self.c,self.good,(0,-40))['lists'],[[0,1,2,3]])
    def test_duplicate_missing_unknown_identity_and_invalid_wire_rejected(self):
        for bad in [self.good.replace('square','other'),self.good.replace('0,1,2,3','0,1,2,2'),self.good.replace('0,1,2,3','0,1,2'),self.good.replace('0,1,2,3','0,1,2,4'),self.good.replace('0,1,2,3','0,1;2,3'),self.good+'\textra',self.good.replace('\t10\t','\t0\t'),self.good[:-1]+'0']:
            with self.subTest(bad=bad),self.assertRaises(ValueError):parse_result(self.c,bad)
    def test_scoring_nonregression_optimum_and_ranges_are_not_trusted(self):
        for bad in [self.good.replace('\t-40\t','\t-39\t'),self.good.replace('\t0\t-40','\t1\t-40')]:
            with self.assertRaisesRegex(ValueError,'Score mismatch'):parse_result(self.c,bad)
        c=deepcopy(self.c);c['initial']=[[0,1,2,3]]
        with self.assertRaisesRegex(ValueError,'regressed'):parse_result(c,'square\t10\t0\t-60\t0,2,1,3\t8')
        with self.assertRaisesRegex(ValueError,'optimum'):parse_result(self.c,self.good,(0,-41))
        c=deepcopy(self.c);c['ranges']=[[0,1,2]]
        with self.assertRaisesRegex(ValueError,'range'):parse_result(c,self.good)
    def test_degenerate_points_keep_distinct_identities(self):
        c=case(8,0)
        self.assertEqual(optimum(c),(0,0))
        line=c['id']+'\t1\t0\t0\t'+','.join(map(str,range(8)))+'\t1'
        # Boundary probes with no candidate scoring remain valid behavior evidence.
        self.assertEqual(parse_result(c,line)['score_calls'],1)
        with self.assertRaisesRegex(ValueError,'identity'):parse_result(c,line.replace('0,1,2,3,4,5,6,7','0,0,2,3,4,5,6,7'))
    def test_generated_corpora_are_complete_repeatable_and_optima_dominate_starts(self):
        rows=generate();self.assertEqual(rows,generate());self.assertEqual(len(rows),32)
        self.assertEqual(len({c['id'] for c in rows}),32)
        for c in rows:
            self.assertEqual(sorted(c['initial'][0]),list(range(8)))
            self.assertEqual(c['pins'],[False]);self.assertEqual(c['prefixes'],[0])
            self.assertGreaterEqual(optimum(c),score(c,c['initial'][0]))
        for n in [12,16]:
            c=case(n,99);self.assertEqual(c,case(n,99));self.assertEqual(len(c['initial'][0]),n)
    def test_summary_retains_time_range_score_work_and_rejects_nondeterminism(self):
        rows=[parse_result(self.c,self.good.replace('\t10\t',f'\t{n}\t')) for n in [10,30,20]]
        rows[1]['score_calls']=12
        s=summarize(rows)
        self.assertEqual(s['median'],0.00002);self.assertEqual(s['min'],0.00001);self.assertEqual(s['max'],0.00003)
        self.assertEqual(s['score_calls'],dict(min=8,max=12));self.assertEqual(s['score'],[0,-40])
        rows[1]['score']=[0,-42]
        with self.assertRaisesRegex(ValueError,'vary'):summarize(rows)

if __name__=='__main__':unittest.main()
