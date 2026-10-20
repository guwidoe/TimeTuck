"""Original reference boundaries and divergence accounting; no method credit."""
import copy
import unittest
from great_deluge_checks import generate, reference, encode, known_java_boundary, binary64_case, arithmetic_probes
from termination_checks import check_java, check_rust


class GreatDelugeReferenceTest(unittest.TestCase):
    def case(self, weights=None, **updates):
        weights=[0,0,1] if weights is None else weights
        return dict(id='reference', value_count=len(weights), initial=[0], pins=[False], weights=[weights], edges=[], steps=2, rain='fixed', parameters=[0,1], selection='first', **updates)

    def test_strict_aspiration_skips_plateau_below_water(self):
        c=self.case();r=reference(c)
        self.assertEqual([t['values'] for t in r['trace']],[[1],[2]])
        self.assertEqual([t['trials'] for t in r['trace']],[1,2])
        self.assertEqual(r['score'],[0,1]);self.assertEqual(r['accepted'],2)
        check_java(c,'reference\t0\t1\t2',r)
        check_rust(c,'reference\tscoped\t0\t1\t2\t2\t0\tStepLimit\t2',r,'scoped')
        with self.assertRaises(ValueError):check_java(c,'reference\t0\t0\t1',r)
        with self.assertRaises(ValueError):check_rust(c,'reference\tscoped\t0\t1\t2\t2\t0\tStepLimit\t1',r,'scoped')

    def test_rejected_fallback_advances_water_without_promoting_best(self):
        c=self.case([0,-4,-1]);c['steps']=4
        r=reference(c)
        self.assertEqual([t['values'] for t in r['trace']],[[2],[0],[2],[0]])
        self.assertEqual([t['threshold'] for t in r['trace']],[[0,1],[0,2],[0,3],[0,4]])
        self.assertEqual(r['score'],[0,0]);self.assertEqual(r['accepted'],2)
        original=copy.deepcopy(c);reference(c);self.assertEqual(original,c)
        c['pins']=[True];self.assertEqual(reference(c)['reason'],'NoAcceptedMove')
        c['steps']=0;self.assertEqual(reference(c)['reason'],'StepLimit')

    def test_fractional_rain_and_observed_binary_boundary_remain_distinct(self):
        c=self.case([-8,-9,-7]);c.update(rain='ratio',parameters=[1,10],steps=3)
        self.assertEqual([t['threshold'] for t in reference(c)['trace']],[[0,-8],[0,-7],[0,-6]])
        c=next(c for c in generate() if known_java_boundary(c))
        expected=known_java_boundary(c)
        self.assertEqual(expected,{'score':[0,1],'values':[2,1,0]})
        self.assertEqual(reference(c)['score'],[0,9])
        self.assertEqual(reference(c,floating_ratio=True)['score'],[0,1])
        for steps in [11,12]:
            c['steps']=steps
            self.assertNotEqual(reference(c)['score'],reference(c,floating_ratio=True)['score'])
        c['steps']=13
        self.assertEqual(reference(c)['score'],reference(c,floating_ratio=True)['score'])

    def test_generated_matrix_is_reproducible_and_has_one_explicit_boundary(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),276)
        self.assertEqual(len({c['id'] for c in cases}),276)
        self.assertEqual(sum(known_java_boundary(c) is not None for c in cases),1)
        self.assertTrue(all(len(encode(c).split('\t'))==11 for c in cases))

    def test_selected_policy_preserves_original_cases_and_extra_probe_denominator(self):
        cases=generate();before=copy.deepcopy(cases)
        selected=list(map(binary64_case,cases))
        self.assertEqual(cases,before)
        self.assertEqual(len(selected),276)
        for old,new in zip(cases,selected):
            self.assertEqual({k:v for k,v in old.items() if k!='rain'}, {k:v for k,v in new.items() if k!='rain'})
            self.assertEqual(new['rain'],'ratio-binary64' if old['rain']=='ratio' else 'fixed')
        extra=arithmetic_probes(cases)
        self.assertEqual(len(extra),184)
        self.assertEqual(len({c['id'] for c in extra}),184)
        self.assertFalse({c['id'] for c in extra}&{c['id'] for c in cases})

    def test_floating_reference_casts_operands_before_division_and_floors_negative_water(self):
        c=self.case([-2,-3]);c.update(rain='ratio',parameters=[1,3],steps=3)
        self.assertEqual([t['threshold'] for t in reference(c,floating_ratio=True)['trace']], [[0,-2],[0,-1],[0,0]])
        c=self.case([-8,-9,-7]);c.update(rain='ratio',parameters=[1,10],steps=10)
        self.assertEqual(reference(c,floating_ratio=True)['trace'][-1]['threshold'],[0,-1])


if __name__=='__main__':unittest.main()
