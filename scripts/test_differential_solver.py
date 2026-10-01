"""Guard against accepting invalid or merely feasible solver responses."""
import unittest
from differential_solver import optimum, validate

class SolverEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.case = {'id':'case','value_count':2,'initial':[0,None], 'pins':[True,False],
                     'edges':[(0,1)], 'weights':[[0,100],[20,1]]}

    def test_independent_optimum_respects_hard_priority_and_pins(self):
        self.assertEqual(optimum(self.case),(0,1))
        self.assertIsNone(validate(self.case,'case\t0\t1\t0,1',(0,1)))

    def test_invalid_assignments_and_scores_cannot_pass(self):
        for response in ['wrong\t0\t1\t0,1', 'case\t0\t1\t0', 'case\t0\t1\t0,2',
                         'case\t0\t1\t1,0', 'case\t0\t999\t0,1', 'case\t0\t1\t0,-']:
            with self.subTest(response=response):
                self.assertIsNotNone(validate(self.case,response,(0,1)))

    def test_self_consistent_suboptimal_result_cannot_pass(self):
        self.assertIsNotNone(validate(self.case,'case\t-1\t20\t0,0',(0,1)))

if __name__=='__main__': unittest.main()
