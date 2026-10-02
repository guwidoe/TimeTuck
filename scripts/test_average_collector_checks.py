import unittest
from average_collector_checks import MIN, MAX, expected, generate, validate

class AverageValidationTest(unittest.TestCase):
    def test_checked_and_wrapping_boundaries_are_distinguished(self):
        def lines(ops):
            return expected({'id': 'test', 'operations': ops})
        self.assertTrue(lines([('p',0,MAX),('p',1,1)])[-1].endswith('OVERFLOW'))
        self.assertTrue(lines([('p',0,MIN),('p',0,MAX)])[-1].endswith('OVERFLOW'))
        self.assertTrue(lines([('p',0,MAX),('p',1,-1),('p',1,1)])[-1].endswith('c3d0000000000000'))
        self.assertFalse(lines([('p',0,1),('p',1,MIN),('p',2,-1),('p',0,2)])[-1].endswith('OVERFLOW'))
        self.assertTrue(lines([('p',0,1),('p',1,MIN),('p',2,-1),('r',0)])[-1].endswith('OVERFLOW'))

    def test_wrong_bits_identity_missing_rows_and_unexpected_rejection_fail(self):
        cases=[{'id':'test','operations':[('p',0,4)]}]
        correct=expected(cases[0])
        validate(cases,correct)
        for bad in [[],correct[:-1],correct+['extra'],['other\t0\t-',correct[1]],[correct[0],'test\t1\t0000000000000000'],[correct[0],'test\t1\tOVERFLOW']]:
            with self.assertRaises(ValueError): validate(cases,bad)

    def test_generated_cases_cover_duplicates_empty_and_rejected_results(self):
        cases=generate()
        lines=[line for case in cases for line in expected(case)]
        observations,rejections=validate(cases,lines)
        self.assertEqual(len(cases),137)
        self.assertGreater(observations,3500)
        self.assertGreater(rejections,50)
        self.assertGreater(sum(line.endswith('\t-') for line in lines),200)
