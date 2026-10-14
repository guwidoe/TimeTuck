#!/usr/bin/env python3
"""Original contract checks; synthetic rows are validation inputs, not measurements."""
import json
import unittest
from copy import deepcopy
import benchmark_mixed as m

class MixedBenchmarkContractTest(unittest.TestCase):
    def setUp(self):self.small,self.large=m.workloads()
    def row(self,case=None,runtime='dense',iterations=2):
        c=case or self.small
        actual=m.score(c,c['certificate_depots'],c['certificate_routes'])
        return {'id':c['id'],'elapsed_ns':2000,'score':[v*iterations for v in actual],'depots':c['certificate_depots'][:],
            'routes':deepcopy(c['certificate_routes']),'score_calls':8,'evaluations':None if runtime=='java' else 4,'steps':None if runtime=='java' else 2,'termination':'synthetic contract input'}
    def test_small_independent_exhaustive_certificate(self):
        self.assertEqual(m.optimum(self.small),(0,-12))
        self.assertEqual(self.small['initial_score'],[0,-78])
        self.assertGreater(tuple(self.small['reference_score']),tuple(self.small['initial_score']))
    def test_larger_feasible_certificate_is_not_claimed_optimal(self):
        self.assertEqual(len(self.large['points']),24)
        self.assertEqual(self.large['reference_kind'],'independently_scored_feasible_certificate')
        self.assertEqual(m.validate_assignment(self.large,self.large['certificate_depots'],self.large['certificate_routes'])[0],0)
        with self.assertRaises(ValueError):m.optimum(self.large)
    def test_actual_work_metrics_remain_distinct(self):
        for runtime in ['dense','stream','java']:
            row=self.row(runtime=runtime);parsed=m.parse_result(self.small,json.dumps(row),runtime,2)
            self.assertEqual(parsed['per_solve_score'],[0,-12])
        row=self.row(runtime='java');row['evaluations']=4
        with self.assertRaises(ValueError):m.parse_result(self.small,json.dumps(row),'java',2)
    def test_rejects_missing_duplicate_unknown_and_bad_depot(self):
        for change in [lambda r:r['routes'][1].pop(),lambda r:r['routes'][1].append(0),lambda r:r['routes'][1].append(99),lambda r:r['depots'].__setitem__(0,99)]:
            row=self.row();change(row)
            with self.assertRaises(ValueError):m.parse_result(self.small,json.dumps(row),'dense',2)
    def test_whole_and_prefix_pins_rejected_even_with_rescored_output(self):
        for whole in [True,False]:
            row=self.row(self.large)
            if whole:row['routes'][0].reverse()
            else:row['routes'][1].reverse()
            row['score']=[v*2 for v in m.score(self.large,row['depots'],row['routes'])]
            with self.assertRaises(ValueError):m.parse_result(self.large,json.dumps(row),'dense',2)
    def test_rejects_checksum_floor_work_and_no_improvement(self):
        changes=[lambda r:r['score'].__setitem__(1,r['score'][1]+1),lambda r:r.__setitem__('elapsed_ns',0),lambda r:r.__setitem__('evaluations',20001),lambda r:r.__setitem__('steps',100)]
        for change in changes:
            row=self.row();change(row)
            with self.assertRaises(ValueError):m.parse_result(self.small,json.dumps(row),'dense',2)
        row=self.row();row['depots']=self.small['initial_depots'];row['routes']=self.small['initial_routes'];row['score']=[v*2 for v in self.small['initial_score']]
        with self.assertRaises(ValueError):m.parse_result(self.small,json.dumps(row),'dense',2)
    def test_rotates_three_runtimes_and_reuses_one_measured_java_group(self):
        from types import SimpleNamespace
        from unittest.mock import patch
        order=[]
        def execute(command,**kwargs):
            runtime='java' if 'MixedSolverBenchmark' in command else command[-1]
            order.append(runtime)
            return SimpleNamespace(returncode=0,stderr='',stdout=json.dumps(self.row(runtime=runtime))+'\n')
        args=SimpleNamespace(iterations=2,warmup=0,samples=1,forks=3)
        with patch.object(m.subprocess,'run',side_effect=execute):
            rows=m.measure_case(self.small,args,'contract-test-classpath',0)
        self.assertEqual(order,['java','dense','stream','dense','stream','java','stream','java','dense'])
        self.assertEqual(len(rows),2)
        self.assertEqual(order.count('java'),3)
        self.assertEqual(rows[0]['samples']['java'],rows[1]['samples']['java'])
        for row in rows:
            self.assertTrue(row['java_samples_shared_between_native_rows'])
            self.assertEqual(row['java_sample_group'],self.small['id'])
            self.assertEqual(row['rust']['evaluations'],{'min':2,'max':2})
            self.assertEqual(row['java']['score_calls'],{'min':4,'max':4})
            self.assertEqual(row['rust']['score'],[0,-12])
            self.assertEqual(row['optimal_score'],[0,-12])
            self.assertTrue(row['equal_scores'])
    def test_protocol_and_shared_java_reference_contract(self):
        fields=m.encode(self.large).split('\t');self.assertEqual(len(fields),10)
        self.assertEqual(fields[6],'1000');self.assertEqual(fields[7],'0,1,1,1')
        self.assertEqual(fields[8:],['24','40000'])

if __name__=='__main__':unittest.main()
