import copy
import unittest
from pooled_construction_checks import generate_cases, reference, observed


class PooledReferenceTests(unittest.TestCase):
    def setUp(self):
        self.case = generate_cases()[-2]

    def test_global_choice_and_first_ties(self):
        self.assertEqual(reference(self.case), ([1, 0], (0, 10)))
        self.assertEqual(reference(generate_cases()[-1]), ([0, 0, 0], (0, 0)))

    def test_pins_and_initial_values(self):
        c = copy.deepcopy(self.case)
        c['initial'] = [0, 1]
        self.assertEqual(reference(c), ([1, 0], (0, 10)))
        c['pins'] = [True, False]
        self.assertEqual(reference(c), ([0, 1], (0, 6)))

    def test_response_validation_rejects_corrupt_scores_ranges_and_identity(self):
        good = 'pool-global-order\t0\t10\t1,0'
        self.assertEqual(observed(self.case, good), ([1, 0], (0, 10)))
        for line in ['wrong\t0\t10\t1,0', 'pool-global-order\t0\t11\t1,0',
                     'pool-global-order\t0\t10\t1', 'pool-global-order\t0\t10\t-,0',
                     'pool-global-order\t0\t10\t2,0', 'pool-global-order\t0\t10\tx,0']:
            with self.subTest(line=line), self.assertRaises(ValueError):
                observed(self.case, line)
        c = copy.deepcopy(self.case)
        c['initial'], c['pins'] = [0, None], [True, False]
        with self.assertRaises(ValueError):
            observed(c, good)

    def test_rejects_unrepresented_reference_scope(self):
        for update in [dict(optional=[True, False]), dict(domains=[[0], [1]]),
                       dict(value_count=0), dict(pins=[True, False])]:
            with self.subTest(update=update), self.assertRaises(ValueError):
                reference(dict(self.case, **update))
