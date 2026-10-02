import unittest
from duration_collector_checks import MIN, MAX, LOW, HIGH, add, subtract, expected, generate, validate

class DurationValidationTest(unittest.TestCase):
    def test_normalized_seconds_and_subtraction_boundaries(self):
        with self.assertRaises(OverflowError): add(MIN+1,-1)
        self.assertEqual(subtract(MIN+1,1),MIN)
        self.assertEqual(subtract(0,MIN+1),MAX)
        self.assertEqual(subtract(MIN,MIN),0)
        with self.assertRaises(OverflowError): subtract(0,MIN)

    def test_average_truncates_and_rejects_total_before_division(self):
        def result(ops): return expected({'id':'case','kind':'average','operations':ops})[-1]
        self.assertEqual(result([('p',0,-1),('p',1,0)]),'case\t2\t0')
        self.assertEqual(result([('p',0,-3),('p',1,0)]),'case\t2\t-1')
        self.assertTrue(result([('p',0,HIGH),('p',1,1)]).endswith('OVERFLOW:finish'))
        self.assertTrue(result([('p',0,LOW),('p',1,-1)]).endswith('OVERFLOW:finish'))

    def test_validator_rejects_wrong_values_stages_and_incomplete_output(self):
        cases=generate();correct=[line for c in cases for line in expected(c)]
        observations,rejected=validate(cases,correct)
        self.assertEqual(len(cases),288)
        self.assertGreater(observations,7000)
        self.assertGreater(rejected,100)
        for bad in [[],correct[:-1],correct+['extra'],[line.replace('OVERFLOW:finish','OVERFLOW:update') for line in correct],['wrong']+correct[1:]]:
            with self.assertRaises(ValueError):validate(cases,bad)
