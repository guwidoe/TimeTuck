"""Original reference/validator regressions; no upstream coverage credit."""
import unittest
from termination_checks import generate, reference, check_java, check_rust


class TerminationChecksTest(unittest.TestCase):
    def setUp(self):
        self.cases = {c['id']:c for c in generate()}

    def test_stagnation_boundary_changes_returned_quality(self):
        # Literal independently calculated milestones for the delayed fixture.
        for limit, steps, score in [(0,0,[-1,-2]),(1,3,[0,1]),(2,4,[0,1]),(3,8,[0,4]),(4,9,[0,4])]:
            result = reference(self.cases[f'delayed-stagnation-{limit}'])
            self.assertEqual(result['steps'], steps)
            self.assertEqual(result['score'], score)
            self.assertEqual(result['reason'], 'UnimprovedStepLimit')
            self.assertEqual(result['unimproved'], limit)

    def test_score_and_feasibility_stop_before_later_improvements(self):
        for name, steps, score, reason in [
            ('delayed-target-0--2',1,[0,-2],'BestScoreLimit'),
            ('delayed-target-0-0',2,[0,1],'BestScoreLimit'),
            ('delayed-target-0-4',5,[0,4],'BestScoreLimit'),
            ('delayed-feasibility',1,[0,-2],'FeasibleScore'),
            ('delayed-target-1-0',12,[0,4],'StepLimit'),
        ]:
            r = reference(self.cases[name])
            self.assertEqual((r['steps'],r['score'],r['reason']),(steps,score,reason))

    def test_validator_rejects_later_valid_but_incorrect_java_solution(self):
        case = self.cases['delayed-feasibility']
        expected = reference(case)
        later = reference(self.cases['delayed-target-0-4'])
        def line(result):
            return '\t'.join([case['id'],*map(str,result['score']),','.join(map(str,result['values']))])
        check_java(case,line(expected),expected)
        with self.assertRaises(ValueError):
            check_java(case,line(later),expected)

    def test_native_validator_checks_every_counter_reason_backend_and_identity(self):
        case = self.cases['delayed-stagnation-2']
        r = reference(case)
        fields = [case['id'],'scoped',*map(str,r['score']),','.join(map(str,r['values'])),
                  str(r['steps']),str(r['unimproved']),r['reason'],str(r['accepted'])]
        check_rust(case,'\t'.join(fields),r,'scoped')
        for i, changed in [(0,'wrong'),(1,'stream'),(5,'3'),(6,'0'),(7,'StepLimit'),(8,'99')]:
            bad = fields[:]
            bad[i] = changed
            with self.assertRaises(ValueError):
                check_rust(case,'\t'.join(bad),r,'scoped')

    def test_no_moves_and_zero_budgets_do_not_create_steps(self):
        for name, reason in [('all-pinned','NoAcceptedMove'),('singleton-domain','NoAcceptedMove'),
                             ('zero-steps','StepLimit'),('zero-stagnation','UnimprovedStepLimit'),
                             ('all-criteria-initial','StepLimit')]:
            r = reference(self.cases[name])
            self.assertEqual((r['steps'],r['unimproved'],r['accepted']),(0,0,0))
            self.assertEqual(r['reason'],reason)


if __name__ == '__main__': unittest.main()
