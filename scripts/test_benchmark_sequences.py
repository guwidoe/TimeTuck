import unittest
from copy import deepcopy
from benchmark_sequences import expected, observation, checksum, encode_state, parse_result, parse_batch, generate, workload

class SequenceBenchmarkTest(unittest.TestCase):
    def case(self): return dict(id='bridge',points=[0,2,1,1],slots=[0,1,-1],slot=2,replacement=2)
    def response(self,c):
        a,b=expected(c)
        return '\t'.join([c['id'],'123',str(3*(31*checksum(b)+checksum(a))),encode_state(b),encode_state(a),encode_state(b)])
    def test_independent_components_and_complete_metadata(self):
        c=self.case();saved=deepcopy(c);a,b=expected(c)
        self.assertEqual(c,saved)
        self.assertEqual(a[:2],[2,1]);self.assertEqual(b[:2],[1,0])
        self.assertEqual(b[2:14],[1,0,0,1,2,0,1,1,0,-1,-1,-1])
        self.assertEqual(b[14:],[3,3,0,2,1,1,-1,-1])
        self.assertEqual(a[-5:],[2,0,2,1,1])
    def test_duplicates_unassignment_and_noop(self):
        p=[-1,0,0,5]
        self.assertEqual(observation(p,[0,0,1]),observation(p,[0,1]))
        self.assertNotEqual(observation(p,[0,1]),observation(p,[0,1,2]))
        self.assertEqual(observation(p,[-1]),[0,0]+[-1]*12)
        c=self.case();c['replacement']=-1
        self.assertEqual(*expected(c))
    def test_every_observation_and_checksum_required(self):
        c=self.case();line=self.response(c)
        self.assertEqual(parse_result(c,line,3)['elapsed_ns'],123)
        for col,value in [(0,'wrong'),(1,'0'),(2,'0'),(3,'0'),(4,'0'),(5,'0')]:
            f=line.split('\t');f[col]=value
            with self.subTest(col=col),self.assertRaises(ValueError):parse_result(c,'\t'.join(f),3)
        for field in [3,4,5]:
            for col in range(len(line.split('\t')[field].split(','))):
                f=line.split('\t');values=f[field].split(',');values[col]='999';f[field]=','.join(values)
                with self.subTest(field=field,col=col),self.assertRaises(ValueError):parse_result(c,'\t'.join(f),3)
    def test_batch_rejects_missing_duplicate_or_reordered_cases(self):
        c=self.case();line=self.response(c)
        self.assertEqual(len(parse_batch([c],line+'\n'+line,3,2)[0]),2)
        for cases,out,samples in [([], '',1),([c],'',1),([c],line+'\n'+line,1),([c,c],line+'\n'+line,1),([c],line,0)]:
            with self.assertRaises(ValueError):parse_batch(cases,out,3,samples)
        d={**c,'id':'other'}
        with self.assertRaises(ValueError):parse_batch([c,d],self.response(d)+'\n'+line,3,1)
    def test_bounded_scope_and_indices(self):
        for change in [dict(points=[]),dict(points=[2**31]),dict(points=[True]),dict(slots=[]),dict(slot=-1),dict(slot=99),dict(replacement=-2),dict(replacement=4),dict(slots=[4]),dict(id='a\nb')]:
            c=self.case();c.update(change)
            with self.subTest(change=change),self.assertRaises(ValueError):expected(c)
    def test_seeded_cases_and_all_workloads(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),64)
        pairs=[expected(c) for c in cases]
        self.assertTrue(any(a==b for a,b in pairs));self.assertTrue(any(a!=b for a,b in pairs))
        for mode in ['dense','fragmented']:
            for n in [64,512]:
                a,b=expected(workload(n,mode));self.assertNotEqual(a,b)
                self.assertEqual(a[0],1 if mode=='dense' else n)

if __name__=='__main__':unittest.main()
