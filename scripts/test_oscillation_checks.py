"""Original reference/protocol tests; none grant source-method coverage."""
import unittest
from oscillation_checks import generate, encode, reference, decode, compare, validate_native

class OscillationChecksTest(unittest.TestCase):
    def case(self, **kwargs):
        return dict(id='case', mode='last', steps=1,
                    scores=[(6,0,0),(2,8,-2),(6,0,0)], **kwargs)

    def line(self, c, **updates):
        r=reference(c);r.update(updates)
        return '\t'.join(map(str,[c['id'],*r['step'],*r['best'],r['best_id'],r['selected'],r['accepted']]))

    def test_matrix_reproducibility_prefixes_and_equal_reference_permutations(self):
        a=generate();self.assertEqual(a,generate());self.assertEqual(len(a),4824)
        self.assertEqual(len({c['id'] for c in a}),len(a))
        self.assertTrue(all(len(encode(c).split('\t'))==4 for c in a))
        for name in ['equal-first','equal-last']:
            c=next(c for c in a if c['id']==name+'-last-1')
            self.assertEqual(reference(c)['step'],[2,8,-2])
        self.assertEqual(reference(self.case())['best_id'],0)

    def test_reference_modes_overall_improvement_and_level_priority(self):
        c=self.case();c['scores']=[(0,-4,-5),(0,-4,-7),(-2,1,-2),(-1,-1,-3),(-1,-2,2)];c['steps']=4
        c['mode']='last';self.assertEqual(reference(c)['trace'],[3,0,3,0])
        c['mode']='best';self.assertEqual(reference(c)['trace'],[3,4,3,4])
        c['mode']='highest';self.assertEqual(reference(c)['trace'],[1,0,1,0])
        c['scores']=[(0,-2,-3),(-1,-1,-3),(0,-2,-1)];c['steps']=1;c['mode']='last'
        self.assertEqual(reference(c)['trace'],[2])

    def test_decode_rejects_corrupted_identity_scores_domain_and_counts(self):
        c=self.case();valid=self.line(c);self.assertEqual(decode(c,valid)['step'],[2,8,-2])
        for bad in [valid.replace('case','other'),valid+'\t0',valid.replace('\t2\t8','\tx\t8'),
                    self.line(c,best_id=-1),self.line(c,best_id=1),self.line(c,step=[8,8,8]),
                    self.line(c,best=[0,0,0]),self.line(c,accepted=1),self.line(c,selected=3)]:
            with self.subTest(bad=bad),self.assertRaises(ValueError):decode(c,bad)

    def test_differing_java_result_never_counts_as_agreement(self):
        c=self.case();native=self.line(c);java=self.line(c,step=[6,0,0])
        self.assertIsNone(compare(c,native,native))
        difference=compare(c,native,java)
        self.assertEqual(difference['java']['step'],[6,0,0]);self.assertEqual(difference['native']['step'],[2,8,-2])
        with self.assertRaises(ValueError):validate_native(c,java)
        with self.assertRaises(ValueError):compare(c,java,java)

    def test_tied_retained_best_keeps_initial_identity(self):
        c=self.case();c['scores']=[(1,1,1),(1,1,1)];c['steps']=4
        self.assertEqual(reference(c)['trace'],[1,0,1,0]);self.assertEqual(reference(c)['best_id'],0)
        self.assertEqual(compare(c,self.line(c),self.line(c)),None)

if __name__=='__main__':unittest.main()
