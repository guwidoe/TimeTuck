import unittest
from benchmark_heuristics import parse_result, workload

class HeuristicResultTests(unittest.TestCase):
    def setUp(self):
        self.case={'id':'test','value_count':2,'initial':[0,0],'pins':[True,False],
                   'edges':[(0,1)],'weights':[[0,0],[0,0]]}
    def test_complete_rescored_result(self):
        self.assertEqual(parse_result(self.case,'test\t100\t0\t0\t0,1\t5')['score'],[0,0])
    def test_invalid_evidence_is_rejected(self):
        for row in ['bad\t100\t0\t0\t0,1\t5','test\t0\t0\t0\t0,1\t5',
                    'test\t100\t0\t0\t0,1\t0','test\t100\t1\t0\t0,1\t5',
                    'test\t100\t0\t0\t1,0\t5','test\t100\t0\t0\t0,2\t5',
                    'test\t100\t0\t0\t0,-\t5','test\t100\t0\t0\t0\t5']:
            with self.subTest(row=row),self.assertRaises(ValueError): parse_result(self.case,row)
    def test_returned_best_must_not_regress(self):
        self.case['initial']=[0,1]
        with self.assertRaises(ValueError): parse_result(self.case,'test\t100\t-1\t0\t0,0\t5')
    def test_planted_workload_has_known_optimum(self):
        from differential_solver import score
        for n in [32,96]:
            case=workload(n)
            self.assertEqual(score(case,[i%4 for i in range(n)]),(0,0))
            self.assertTrue(all(w<=0 for row in case['weights'] for w in row))
