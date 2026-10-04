import copy
import unittest
from benchmark_aggregates import expected,observation,checksum,encode_state,parse_result,parse_batch,generate,workload
class AggregateBenchmarkTest(unittest.TestCase):
    def case(self,mode='map'):return dict(id='x',mode=mode,items=[[2,4],[2,-4],[-1,3]],slots=[0,1,1],slot=1,replacement=2)
    def response(self,c,iterations=3):
        a,b=expected(c);return '\t'.join([c['id'],'123',str(iterations*(31*checksum(b)+checksum(a))),encode_state(b),encode_state(a),encode_state(b)])
    def test_complete_observations_preserve_duplicates_zero_and_key_order(self):
        items=[[2,4],[2,-4],[-1,3]]
        self.assertEqual(observation('map',items,[0,1,2]),[2,-1,3,2,0])
        self.assertEqual(observation('set',items,[0,1,1,2]),[3,-4,3,4])
        self.assertEqual(observation('sum',items,[0,1,1,2]),[-1])
        for mode in ['map','set','sum']:self.assertEqual(observation(mode,items,[-1]),[0])
    def test_trial_restore_commit_and_checksum(self):
        for mode in ['sum','set','map']:
            c=self.case(mode);r=parse_result(c,self.response(c),3);a,b=expected(c);self.assertEqual((r['trial'],r['restored'],r['committed']),(b,a,b));self.assertEqual(c['slots'],[0,1,1])
    def test_reject_every_corrupt_observation_and_checksum(self):
        c=self.case();r=self.response(c).split('\t')
        for pos in range(6):
            bad=r.copy();bad[pos]='wrong' if pos==0 else '0'
            with self.assertRaises(ValueError):parse_result(c,'\t'.join(bad),3)
        for line in ['\t'.join(r[:-1]),'\t'.join(r+['0'])]:
            with self.assertRaises(ValueError):parse_result(c,line,3)
    def test_invalid_inputs_cannot_enter_timing_scope(self):
        c=self.case()
        for field,value in [('id','bad\tname'),('mode','bad'),('items',[]),('items',[[0,10001]]),('items',[[0,True]]),('items',[[1]]),('slots',[]),('slots',[99]),('slot',-1),('replacement',-2)]:
            bad=copy.deepcopy(c);bad[field]=value
            with self.assertRaises(ValueError):expected(bad)
    def test_batch_count_duplicates_and_order_checked(self):
        c=self.case();r=self.response(c)
        self.assertEqual(len(parse_batch([c],r+'\n',3,1)),1)
        for cases,output,samples in [([c],r,2),([c],r+'\n'+r,1),([c,c],r+'\n'+r,1),([], '',1),([c],r,0)]:
            with self.assertRaises(ValueError):parse_batch(cases,output,3,samples)
    def test_reproducible_cases_and_workload_sizes(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),96)
        for c in cases:parse_result(c,self.response(c),3)
        for mode in ['sum','set','map']:
            for n in [64,512]:
                c=workload(n,mode);a,b=expected(c);self.assertNotEqual(a,b);self.assertEqual(len(c['slots']),n)
                if mode=='set':self.assertEqual((a[0],b[0]),(n//2,n//2+1))
                if mode=='map':self.assertEqual((a[0],b[0]),(n//8,n//8+1))
        with self.assertRaises(ValueError):workload(10,'sum')
if __name__=='__main__':unittest.main()
