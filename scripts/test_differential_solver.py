"""Guard against accepting invalid or merely feasible solver responses."""
import unittest
from differential_solver import encode, generate_extended, optimum, score, validate

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

class ExtendedSolverEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.case = {'id':'mixed', 'value_count':3, 'domains':[[1],[0,2],[]],
                     'optional':[False,True,True], 'initial':[1,None,None], 'pins':[True,False,True],
                     'edges':[(0,1),(1,2)], 'weights':[[0,5,0],[2,0,7],[0,0,0]],
                     'unassigned_weights':[0,8,-2]}

    def test_optional_optimum_and_null_weights_are_independent_of_domain_order(self):
        self.assertEqual(optimum(self.case),(0,11))
        self.assertEqual(score(self.case,[1,None,None]),(0,11))
        self.assertIsNone(validate(self.case,'mixed\t0\t11\t1,-,-',(0,11)))
        self.case['domains'][1].reverse()
        self.assertEqual(optimum(self.case),(0,11))

    def test_optional_support_does_not_weaken_range_pin_score_or_optimum_checks(self):
        for row in ['mixed\t0\t11\t-,-,-', 'mixed\t0\t11\t1,1,-',
                    'mixed\t0\t11\t0,-,-', 'mixed\t0\t11\t1,-,0',
                    'mixed\t0\t999\t1,-,-', 'mixed\t0\t10\t1,2,-']:
            with self.subTest(row=row): self.assertIsNotNone(validate(self.case,row,(0,11)))
        self.case['pins'][1] = True
        self.assertIsNotNone(validate(self.case,'mixed\t0\t10\t1,2,-',(0,10)))

    def test_extended_wire_fields_preserve_nulls_and_empty_ranges(self):
        self.assertEqual(encode(self.case).split('\t'), [
            'mixed','3','1,-,-','101','0:1,1:2','0,5,0,2,0,7,0,0,0','1;0:2;','011','0,8,-2'])

    def test_extended_generator_has_valid_bounded_complete_spaces(self):
        cases = generate_extended()
        self.assertEqual(len(cases),64)
        self.assertEqual(len({c['id'] for c in cases}),64)
        for case in cases:
            self.assertIsInstance(optimum(case),tuple)
            for domain,allow,initial,pin in zip(case['domains'],case['optional'],case['initial'],case['pins']):
                self.assertEqual(len(domain),len(set(domain)))
                self.assertTrue(all(0<=v<case['value_count'] for v in domain))
                self.assertTrue(domain or allow)
                self.assertTrue(initial is None or initial in domain)
                self.assertFalse(pin and initial is None and not allow)

if __name__=='__main__': unittest.main()
