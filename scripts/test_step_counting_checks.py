"""Original independent-reference checks, including counter boundaries and parsing."""
import copy
import unittest
from step_counting_checks import MODES, generate, reference, encode
from termination_checks import check_java, check_rust


class StepCountingReferenceTest(unittest.TestCase):
    def case(self, mode='STEP', weights=None, selection='best'):
        weights = [0,-4,-1] if weights is None else weights
        return dict(id='reference',value_count=len(weights),initial=[0],pins=[False],weights=[weights],edges=[],steps=4,mode=mode,period=2,selection=selection)

    def test_fallback_recovery_counts_previous_score_and_keeps_initial_best(self):
        counts = {'STEP':[1,0,1,0], 'EQUAL_OR_IMPROVING_STEP':[0,1,1,0],
                  'IMPROVING_STEP':[0,1,1,0], 'SELECTED_MOVE':[0,0,0,0], 'ACCEPTED_MOVE':[0,1,1,0]}
        for mode in MODES:
            r=reference(self.case(mode))
            self.assertEqual(r['values'],[0]);self.assertEqual(r['score'],[0,0])
            self.assertEqual(r['steps'],4);self.assertEqual(r['accepted'],2)
            self.assertEqual([x['values'] for x in r['trace']],[[2],[0],[2],[0]])
            self.assertEqual([x['count'] for x in r['trace']],counts[mode])

    def test_plateau_separates_trial_counts_and_strict_improvement(self):
        for selection in ['first','best']:
            for mode in MODES:
                r=reference(self.case(mode,[0,0,0],selection))
                counts=([0]*4 if mode=='IMPROVING_STEP' or (selection=='best' and mode in ['SELECTED_MOVE','ACCEPTED_MOVE']) else [1,0,1,0])
                self.assertEqual([x['count'] for x in r['trace']],counts)
                self.assertEqual([x['trials'] for x in r['trace']],[1 if selection=='first' else 2]*4)

    def test_no_step_zero_steps_and_unchanged_inputs(self):
        c=self.case();original=copy.deepcopy(c);reference(c);self.assertEqual(c,original)
        c['pins']=[True];self.assertEqual(reference(c)['reason'],'NoAcceptedMove')
        c['steps']=0;self.assertEqual(reference(c)['reason'],'StepLimit')
        self.assertEqual(reference(c)['trace'],[])

    def test_matrix_reproducible_and_responses_reject_corruption(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),600)
        self.assertEqual(len({c['id'] for c in cases}),600)
        self.assertEqual({c['mode'] for c in cases},set(MODES))
        c=self.case();expected=reference(c)
        self.assertEqual(len(encode(c).split('\t')),10)
        check_java(c,'reference\t0\t0\t0',expected)
        check_rust(c,'reference\tscoped\t0\t0\t0\t4\t4\tStepLimit\t2',expected,'scoped')
        for line in ['wrong\t0\t0\t0','reference\t0\t1\t0','reference\t0\t0\t3']:
            with self.assertRaises(ValueError):check_java(c,line,expected)
        with self.assertRaises(ValueError):check_rust(c,'reference\tscoped\t0\t0\t0\t4\t4\tStepLimit\t3',expected,'scoped')


if __name__ == '__main__':unittest.main()
