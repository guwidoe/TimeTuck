import unittest
from copy import deepcopy
from benchmark_pillars import reference, flatten, checksum, encode, parse_result, parse_batch, generate, workload

class PillarBenchmarkTest(unittest.TestCase):
    def case(self):
        return {'id':'example', 'kind':'s', 'columns':[0,2], 'left':[2,0], 'right':[3], 'target':None,
                'rows':[[None,3,0],[None,2,0],[None,1,0],[2,-1,None]]}
    def response(self, c, iterations=3):
        initial, trial = flatten(c['rows']), flatten(reference(c))
        def state(s): return ','.join('-' if v is None else str(v) for v in s)
        return '\t'.join([c['id'],'42',str(iterations*(31*checksum(trial)+checksum(initial))),state(trial),state(initial),state(trial)])
    def test_simultaneous_swap_preserves_other_columns_and_matching_nonmember(self):
        c=self.case(); original=deepcopy(c)
        self.assertEqual(reference(c),[[2,3,None],[None,2,0],[2,1,None],[None,-1,0]])
        self.assertEqual(c,original)
    def test_change_unassignment_and_noop(self):
        c=self.case();c.update(kind='c',columns=[2],right=[])
        self.assertEqual(reference(c),[[None,3,None],[None,2,0],[None,1,None],[2,-1,None]])
        c['target']=0
        self.assertEqual(reference(c),c['rows'])
    def test_invalid_groups_rejected(self):
        for key,value in [('left',[]),('right',[0]),('columns',[0,0]),('columns',[3]),('left',[0,3]),('left',[4])]:
            c=self.case();c[key]=value
            with self.subTest(key=key,value=value), self.assertRaises(ValueError):reference(c)
    def test_observations_and_checksum_are_all_required(self):
        c=self.case();line=self.response(c)
        self.assertEqual(parse_result(c,line,3)['elapsed_ns'],42)
        for index,replacement in [(0,'wrong'),(1,'0'),(1,'-1'),(2,'0'),(3,'0'),(4,'0'),(5,'0')]:
            fields=line.split('\t');fields[index]=replacement
            with self.subTest(index=index),self.assertRaises(ValueError):parse_result(c,'\t'.join(fields),3)
    def test_batch_requires_complete_ordered_nonempty_results(self):
        c=self.case();line=self.response(c)
        self.assertEqual(len(parse_batch([c],line+'\n'+line,3,2)[0]),2)
        for cases,out,samples in [([], '',1),([c],'',1),([c],line+'\n'+line,1),([c,c],line+'\n'+line,1),([c],line,0)]:
            with self.assertRaises(ValueError):parse_batch(cases,out,3,samples)
    def test_generator_deterministic_with_optional_and_noop_coverage(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),128)
        self.assertEqual({len(c['columns']) for c in cases},{1,2,3})
        self.assertTrue(any(reference(c)==c['rows'] for c in cases))
        self.assertTrue(any(None in flatten(reference(c)) for c in cases))
        for c in cases: self.assertEqual(len(encode(c).split('\t')),7)
    def test_timing_workloads_are_nontrivial_subgroup_transactions(self):
        for mode in ['change','swap','tuple-swap']:
            for count in [16,64]:
                c=workload(count,mode);self.assertNotEqual(reference(c),c['rows'])
                self.assertEqual(len(c['rows']),count)
                self.assertLess(len(c['left'])+len(c['right']),count)
