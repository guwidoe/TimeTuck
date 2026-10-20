"""Original independent-reference and response-integrity regressions."""
import unittest
from sequence_collector_checks import components, states, validate

class SequenceChecksTest(unittest.TestCase):
    def setUp(self):
        self.case = {'id':'x','points':[1,1,3], 'operations':[('c',0,0),('c',1,1),('c',2,2)]}
        self.lines = ['x\t0\t-\t-', 'x\t1\t1,1,1,1,-,-,0,0:0\t-',
                      'x\t2\t2,1,1,1,-,-,0,1:0,1\t-',
                      'x\t3\t2,1,1,0,-,2,0,1:0,1;1,1,0,1,2,-,2,2:2\t2,1,1,1,2']

    def test_reference_components_and_duplicate_occurrences(self):
        self.assertEqual(components([-4,-2,-1,0,0,5],range(6)),[[0],[1,2,3,4],[5]])
        c={'operations':[('c',0,1),('c',1,1),('t',0,-1),('c',1,-1)]}
        self.assertEqual(list(states(c)),[{}, {0:1}, {0:1,1:1}, {1:1}, {0:1,1:1}, {0:1}])

    def test_complete_fields(self):
        self.assertEqual(validate([self.case],self.lines),4)
        self.assertEqual(validate([self.case],self.lines,java=True),4)

    def test_java_ties_keep_observed_endpoints(self):
        lines=self.lines.copy()
        lines[2]='x\t2\t2,1,1,1,-,-,1,0:1,0\t-'
        self.assertEqual(validate([self.case],lines,java=True),4)
        with self.assertRaises(ValueError): validate([self.case],lines)
        lines[2]=lines[2].replace(',-,-,1,0:', ',-,-,0,1:')
        with self.assertRaises(ValueError): validate([self.case],lines,java=True)

    def test_reject_missing_extra_and_reordered_observations(self):
        for lines in [self.lines[:-1], self.lines+self.lines[:1], self.lines[::-1]]:
            with self.assertRaises(ValueError): validate([self.case],lines)

    def test_reject_corrupt_sequence_and_break_metadata(self):
        last=self.lines[-1]
        for old,new in [('2,1,1,0,-,2','3,1,1,0,-,2'),('2,1,1,0,-,2','2,9,1,0,-,2'),
                        ('2,1,1,0,-,2','2,1,0,0,-,2'),('2,1,1,0,-,2','2,1,1,0,-,3'),
                        (':0,1;',':0,0;'), ('\t2,1,1,1,2','\t2,1,1,0,2'),
                        ('\t2,1,1,1,2','\t3,1,1,1,2')]:
            with self.subTest(new=new), self.assertRaises(ValueError):
                validate([self.case],self.lines[:-1]+[last.replace(old,new)])

    def test_overflow_boundary_is_not_ordinary_agreement(self):
        case={'id':'b','points':[-2**31,2**31-1], 'operations':[('c',0,0),('c',1,1)]}
        native=['b\t0\t-\t-', 'b\t1\t1,1,1,1,-,-,0,0:0\t-',
                'b\t2\t1,1,1,0,-,4294967295,0,0:0;1,1,0,1,4294967295,-,1,1:1\t4294967295,1,1,0,1']
        java=[line.replace('4294967295','-1') for line in native]
        self.assertEqual(validate([case],native),3)
        self.assertEqual(validate([case],java,java=True),3)
        with self.assertRaises(ValueError): validate([case],java)
        with self.assertRaises(ValueError): validate([case],native,java=True)

    def test_explicit_wrapping_requires_native_identity_order_and_correct_lengths(self):
        case={'id':'b','points':[-2**31,2**31-1], 'operations':[('c',0,0),('c',1,1)]}
        wrapped=['b\t0\t-\t-', 'b\t1\t1,1,1,1,-,-,0,0:0\t-',
                 'b\t2\t1,1,1,0,-,-1,0,0:0;1,1,0,1,-1,-,1,1:1\t-1,1,1,0,1']
        self.assertEqual(validate([case],wrapped,wrapping=True),3)
        with self.assertRaises(ValueError):validate([case],wrapped)
        with self.assertRaises(ValueError):validate([case],wrapped[:-1]+[wrapped[-1].replace('\t-1,1,1,0,1','\t-2,1,1,0,1')],wrapping=True)
        reversed_ties=self.lines.copy()
        reversed_ties[2]='x\t2\t2,1,1,1,-,-,1,0:1,0\t-'
        self.assertEqual(validate([self.case],reversed_ties,java=True),4)
        with self.assertRaises(ValueError):validate([self.case],reversed_ties,wrapping=True)

if __name__ == '__main__': unittest.main()
