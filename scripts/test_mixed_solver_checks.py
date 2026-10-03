"""Independent quality-accounting guards, without upstream method credit."""
import unittest
from mixed_solver_checks import generate, reference, score, validate


class MixedSolverChecksTest(unittest.TestCase):
    def setUp(self):
        self.case = generate()[0]
        self.optimum = reference(self.case)

    def test_coupled_optimum_and_complete_space_have_literal_independent_answers(self):
        # Depot 0 serves point 1; depot 10 serves points 9 and 10. Each route
        # costs at least two, and this assignment attains the lower bound.
        self.assertEqual(self.optimum, {'score':[0,-4], 'assignments':96})
        self.assertEqual(score(self.case,[0,0],[[0],[1,2]]),(0,-22))
        self.assertEqual(score(self.case,[0,10],[[0],[1,2]]),(0,-4))
        self.assertEqual(reference(generate()[1])['score'],[-1,-20])

    def test_valid_java_quality_gap_is_reported_without_becoming_score_agreement(self):
        r = validate(self.case,'coupled-depots\t0\t-22\t0,0\t0;1,2',self.optimum,'java')
        self.assertFalse(r['optimal'])
        self.assertEqual(r['score'],[0,-22])
        r = validate(self.case,'coupled-depots\t0\t-4\t0,10\t0;1,2',self.optimum,'java')
        self.assertTrue(r['optimal'])

    def test_validator_rejects_invalid_ownership_domains_and_forged_scores(self):
        for line in [
            'coupled-depots\t0\t-4\t0,0\t0;1,2',  # Forged quality.
            'coupled-depots\t0\t-4\t0,10\t0;0,2', # Duplicate/missing ID.
            'coupled-depots\t0\t-4\t0,10\t0;1,3', # Unknown ID.
            'coupled-depots\t0\t-4\t0,10\t0;1',   # Incomplete.
            'coupled-depots\t0\t-4\t0,9\t0;1,2',  # Invalid depot.
            'coupled-depots\t0\t-4\t0\t0;1,2',    # Wrong shape.
            'other\t0\t-4\t0,10\t0;1,2',
        ]:
            with self.subTest(line=line), self.assertRaises(ValueError):
                validate(self.case,line,self.optimum,'java')

    def test_native_exact_requires_score_exhaustion_and_every_joint_assignment(self):
        fields = ['coupled-depots','exact','0','-4','0,10','0;1,2','96','0','Exhausted']
        self.assertTrue(validate(self.case,'\t'.join(fields),self.optimum,'exact')['optimal'])
        for i, v in [(1,'stream'),(6,'95'),(7,'1'),(8,'EvaluationLimit')]:
            bad = fields[:]
            bad[i] = v
            with self.assertRaises(ValueError): validate(self.case,'\t'.join(bad),self.optimum,'exact')
        with self.assertRaises(ValueError):
            validate(self.case,'coupled-depots\texact\t0\t-22\t0,0\t0;1,2\t96\t0\tExhausted',self.optimum,'exact')

    def test_local_validation_enforces_work_caps_without_claiming_optimality(self):
        fields = ['coupled-depots','scoped','0','-22','0,0','0;1,2','45','2','NoAcceptedMove']
        self.assertFalse(validate(self.case,'\t'.join(fields),self.optimum,'scoped')['optimal'])
        for i,v in [(6,'0'),(6,'1000001'),(7,'21'),(8,'StepLimit'),(8,'EvaluationLimit')]:
            bad=fields[:]
            bad[i]=v
            with self.assertRaises(ValueError): validate(self.case,'\t'.join(bad),self.optimum,'scoped')


if __name__ == '__main__': unittest.main()
