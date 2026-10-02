import unittest
from big_number_collector_checks import expected, generate, validate

class BigNumberChecksTest(unittest.TestCase):
    def test_half_even_rounding_has_both_signs_and_tie_parities(self):
        for value,result in [(3,2),(5,2),(7,4),(-3,-2),(-5,-2),(-7,-4)]:
            c={'id':'x','kind':'integer_average','operations':[('p',0,value,0),('p',1,0,0)]}
            self.assertEqual(expected(c)[-1],f'x\t2\t{result}:0')

    def test_decimal_scale_persists_across_empty_state(self):
        c={'id':'x','kind':'decimal_average','operations':[('p',0,0,2),('r',0),('p',1,7,0),('p',2,0,0)]}
        self.assertEqual(expected(c),['x\t0\t-','x\t1\t0:2','x\t2\t-','x\t3\t700:2','x\t4\t350:2'])

    def test_wrong_coefficients_scales_identity_and_missing_rows_fail(self):
        cases=generate();lines=[line for case in cases for line in expected(case)]
        self.assertEqual(len(cases),304);self.assertGreater(validate(cases,lines),16000)
        c=[{'id':'x','kind':'decimal_sum','operations':[('p',0,10,1)]}];correct=expected(c[0]);validate(c,correct)
        for bad in [[],correct[:-1],correct+['extra'],['wrong']+correct[1:],[correct[0],'x\t1\t1:0'],[correct[0],'x\t1\t11:1']]:
            with self.assertRaises(ValueError):validate(c,bad)
