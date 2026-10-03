"""Original correctness checks for the independent decimal comparison oracle."""
import unittest
from differential_decimal_scores import expected, generate, result_matches


class DecimalReferenceTests(unittest.TestCase):
    def test_reference_keeps_representation_and_flooring(self):
        cases = [('simple','parse','0structural/-0.000','', '0|0.000|0|true|0@3'),
                 ('simple','neg','1E+2','', '0|-100|-100|true|-100@0'),
                 ('simple','neg','0structural/0.00','', '0|0.00|0|true|0@2'),
                 ('simple','mul','-1.99','1.2', '0|-2.39|-2.39|true|-239@2'),
                 ('hard_soft','parse','-0.1hard/10soft','', '0|-0.1hard/10soft|-0.1hard/10soft|false|-1@1,10@0')]
        for kind,op,a,b,result in cases:
            self.assertEqual(expected(dict(type=kind,operation=op,a=a,b=b)),result)

    def test_generator_is_deterministic_and_errors_are_explicit(self):
        cases=generate()
        self.assertEqual(cases,generate())
        self.assertEqual(len(cases),8170)
        for c in cases:
            self.assertEqual(expected(c)=='ERROR',c.get('expect_error',False),c)

    def test_identical_unexpected_errors_cannot_pass(self):
        self.assertFalse(result_matches({},['ERROR']*3))
        self.assertTrue(result_matches({'expect_error':True},['ERROR']*3))
        self.assertFalse(result_matches({'expect_error':True},['ok']*3))
        self.assertFalse(result_matches({},['one','one','two']))

    def test_bendable_preserves_scales_and_empty_shapes(self):
        cases = [('bendable_1_2','neg','[1E+2]hard/[0.00/-1.20]soft','', '0|[-1E+2]hard/[0.00/1.20]soft|[-1E+2]hard/[0.00/1.20]soft|false|-1@-2,0@2,120@2'),
                 ('bendable_0_0','div','-2structural/[]hard/[]soft','0','0|[]hard/[]soft|0|true|'),
                 ('bendable_2_0','parse','[1/-0.01]hard/[]soft','','0|[1/-0.01]hard/[]soft|[1/-0.01]hard|false|1@0,-1@2'),
                 ('bendable_1_2','parse','[0.00]hard/[0.0/0E+2]soft','','0|[0.00]hard/[0.0/0E+2]soft|0|true|0@2,0@1,0@-2'),
                 ('bendable_0_0','parse','[0]hard/[]soft','','ERROR')]
        for kind,op,a,b,result in cases:
            self.assertEqual(expected(dict(type=kind,operation=op,a=a,b=b)),result)
