import unittest
from copy import deepcopy
from benchmark_concurrent import expected, encode, encode_state, checksum, parse_result, parse_batch, generate, workload

class ConcurrentBenchmarkTest(unittest.TestCase):
    def case(self):return dict(id='cycle',count=4,groups=[[0,2]],lists=[[0,1],[2,3]],move=['c',1,0,0,2])
    def response(self,c):
        initial,trial=expected(c)
        return '\t'.join([c['id'],'123',str(3*(31*checksum(trial)+checksum(initial))),encode_state(trial),encode_state(initial),encode_state(trial)])
    def test_cycle_trial_retains_independent_ready_and_restores_all_fields(self):
        c=self.case();before=deepcopy(c);initial,trial=expected(c)
        self.assertEqual(c,before)
        self.assertEqual(trial[5],0)
        self.assertIsNone(trial[6])
        self.assertEqual([trial[9*i+8] for i in range(4)],[1,1,1,0])
        self.assertEqual(initial[6],0)
        self.assertEqual(len(initial),36)
    def test_every_observation_and_checksum_required(self):
        c=self.case();line=self.response(c)
        self.assertEqual(parse_result(c,line,3)['elapsed_ns'],123)
        for i,value in [(0,'other'),(1,'0'),(2,'0'),(3,'0'),(4,'0'),(5,'0')]:
            f=line.split('\t');f[i]=value
            with self.subTest(i=i),self.assertRaises(ValueError):parse_result(c,'\t'.join(f),3)
        # Change each of the nine columns independently, retaining response shape.
        for i in range(9):
            f=line.split('\t');s=f[3].split(',');s[i]='99999';f[3]=','.join(s)
            with self.subTest(column=i),self.assertRaises(ValueError):parse_result(c,'\t'.join(f),3)
    def test_batch_requires_complete_ordered_unique_results(self):
        c=self.case();line=self.response(c)
        self.assertEqual(len(parse_batch([c],line+'\n'+line,3,2)[0]),2)
        for cases,out,samples in [([], '',1),([c],'',1),([c],line+'\n'+line,1),([c,c],line+'\n'+line,1),([c],line,0)]:
            with self.assertRaises(ValueError):parse_batch(cases,out,3,samples)
    def test_scope_rejects_unassigned_noop_and_invalid_moves(self):
        for change in [dict(lists=[[0,1],[2]]),dict(move=['c',0,0,0,0]),dict(move=['s',0,0,0,0]),dict(move=['a',0,1,0]),dict(move=['c',0,9,1,0])]:
            c=self.case();c.update(change)
            with self.subTest(change=change),self.assertRaises(ValueError):expected(c)
    def test_seeded_cases_and_workloads_cover_cycle_and_recovery(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),64)
        pairs=[expected(c) for c in cases]
        self.assertTrue(any(sum(a[8::9])<sum(b[8::9]) for a,b in pairs))
        self.assertTrue(any(sum(a[8::9])>sum(b[8::9]) for a,b in pairs))
        self.assertTrue(any(not c['groups'] for c in cases))
        for c in cases:self.assertEqual(len(encode(c).split('\t')),7)
        for mode in ['swap','relocate']:
            for n in [16,64]:
                initial,trial=expected(workload(n,mode));self.assertNotEqual(initial,trial)
                self.assertEqual(sum(initial[8::9]),0);self.assertGreater(sum(trial[8::9]),0)
