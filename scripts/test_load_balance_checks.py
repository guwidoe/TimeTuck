import unittest
from unittest.mock import patch
from types import SimpleNamespace
from load_balance_checks import canonical,generate,reference,parse,validate,wrapping_i64,wrapping_reference,comparison_report,WRAPPING_POLICY,BINARY64_POLICY,binary64_reference,run_checks
from sequence_collector_checks import states

class LoadBalanceChecksTest(unittest.TestCase):
    def case(self):return dict(id='x',items=[[0,2,0],[1,1,0]],operations=[('c',0,0),('c',1,1),('t',2,1)])
    def lines(self):return ['x\t0\t-\t0\t0','x\t1\t0:2\t0\t0','x\t2\t0:2;1:1\t707107\t6','x\t3\t0:2;1:2\t0\t0','x\t4\t0:2;1:1\t707107\t6']
    def test_initial_multiplicity_negative_and_zero_keys(self):
        c=dict(items=[[0,2,10],[0,-12,10],[1,0,-4]])
        self.assertEqual(reference(c,{0:0,1:0,2:1,3:2})['loads'],[[0,2],[1,-4]])
        self.assertEqual(reference(c,{0:0,1:1})['loads'],[[0,0]])
        c['items'][1][2]=11
        with self.assertRaises(ValueError):reference(c,{0:0,1:1})
    def test_exact_large_offset_and_overflow_reference(self):
        c=dict(items=[[0,10**9,0],[1,10**9+1,0]])
        self.assertEqual(reference(c,{0:0,1:1})['unfairness'],[707107,6])
        c=dict(items=[[0,2**63-1,2**63-1],[1,-2**63,-2**63]])
        r=reference(c,{0:0,1:0,2:1,3:1})
        self.assertEqual(r['loads'],[[0,3*(2**63-1)],[1,-3*2**63]])
        self.assertEqual(r['unfairness'],[391315,-14])
    def test_decimal_normalization_is_explicit(self):
        self.assertEqual(canonical(1000,0),[1,-3]);self.assertEqual(canonical(0,6),[0,0])
        c=self.case();self.assertEqual(parse(c,0,'x\t0\t0:2\t7071070\t7')['unfairness'],[707107,6])
    def test_complete_observations_reject_missing_duplicate_reordered_and_wrong_fields(self):
        c=self.case();lines=self.lines();self.assertEqual(validate([c],lines)['passed'],1)
        for bad in [lines[:-1],lines+lines[:1],lines[::-1],lines[:2]+[lines[1]]+lines[3:]]:
            with self.assertRaises(ValueError):validate([c],bad)
        for bad in ['x\t2\t0:2;1:2\t707107\t6','x\t2\t0:2;1:1\t707108\t6','x\t2\t0:2;1:1\t707107\t5']:
            with self.assertRaises(ValueError):validate([c],lines[:2]+[bad]+lines[3:])
    def test_errors_and_differences_are_not_agreement_passes(self):
        c=self.case();lines=self.lines();bad=lines.copy();bad[2]=bad[2].replace('707107','0')
        r=validate([c],bad,True);self.assertEqual((r['passed'],r['divergent_cases'],len(r['differences'])),(0,1,1))
        err=lines[:2]+['x\tERROR\tjava.lang.ArithmeticException'];r=validate([c],err,True)
        self.assertEqual((r['passed'],r['error_cases'],r['observations']),(0,1,2))
        with self.assertRaises(ValueError):validate([c],err)
    def test_malformed_identity_maps_and_numeric_fields_fail(self):
        c=self.case()
        for raw in ['x\t0\t0:1;0:2\t0\t0','x\t0\t1:1;0:2\t0\t0','x\t0\t9:2\t0\t0','x\t0\t0:2:3\t0\t0','x\t0\t-\t-1\t0','x\t0\t-\t1\t999','y\t0\t-\t0\t0','x\t0\t-\t0']:
            with self.subTest(raw=raw),self.assertRaises(ValueError):parse(c,0,raw)
    def test_seeded_cases_retain_all_boundaries_and_trial_states(self):
        cases=generate();self.assertEqual(cases,generate());self.assertEqual(len(cases),101)
        self.assertEqual(sum(c['boundary'] for c in cases),5)
        self.assertEqual(sum(len(list(states(c))) for c in cases),8290)
        for c in cases:
            self.assertEqual(list(states(c))[-1],{})
            for s in states(c):reference(c,s)

    def encoded(self,case,reference_fn):
        lines=[]
        for state,slots in enumerate(states(case)):
            out=reference_fn(case,slots)
            if 'error' in out:lines.append(f'{case["id"]}\t{state}\tERROR\t{out["error"]}')
            else:
                loads=';'.join(f'{k}:{v}' for k,v in out['loads']) or '-'
                coefficient,scale=out['unfairness']
                lines.append(f'{case["id"]}\t{state}\t{loads}\t{coefficient}\t{scale}')
        return lines
    def test_wrapping_reference_modular_boundaries_and_exact_offset(self):
        self.assertEqual(wrapping_i64(2**63),-2**63)
        self.assertEqual(wrapping_i64(-2**63-1),2**63-1)
        for loads,expected in [([2**63-1,0],[707107,6]),([2**63-1,2**63-2],[707107,6]),([-2,2**63-2],[0,0]),([2**63-3,0],[212132,5]),([10**9,10**9+1],[707107,6])]:
            c=dict(items=[[k,v,0] for k,v in enumerate(loads)])
            self.assertEqual(wrapping_reference(c,{0:0,1:1})['unfairness'],expected)
        c=dict(items=[[0,2**63-1,2**63-1],[1,-2**63,-2**63]])
        self.assertEqual(wrapping_reference(c,{0:0,1:1}),dict(loads=[[0,-2],[1,0]],unfairness=[141421,5]))
        c=dict(items=[[0,3,7],[0,3,7]])
        self.assertEqual(wrapping_reference(c,{0:0,1:1})['loads'],[[0,13]])
        c['items'][1][2]=8
        with self.assertRaises(ValueError):wrapping_reference(c,{0:0,1:1})
    def test_wrapping_typed_error_requires_predicted_state_and_recovery(self):
        c=dict(id='negative',boundary=True,items=[[0,3037000500,0],[1,0,0]],operations=[('c',0,0),('c',1,1),('c',0,-1)])
        lines=self.encoded(c,wrapping_reference)
        self.assertEqual(lines[2],'negative\t2\tERROR\tNegativeRadicand')
        result=validate([c],lines,arithmetic_policy=WRAPPING_POLICY)
        self.assertEqual((result['passed'],result['observations'],len(result['policy_errors'])),(1,4,1))
        for bad in [lines[:-1],lines[:2]+['negative\t2\tERROR\tOther']+lines[3:],lines[:1]+['negative\t1\tERROR\tNegativeRadicand']+lines[2:],lines[:2]+['negative\t2\t-\t0\t0']+lines[3:]]:
            with self.assertRaises(ValueError):validate([c],bad,arithmetic_policy=WRAPPING_POLICY)
        with self.assertRaises(ValueError):validate([c],lines)
        with self.assertRaises(ValueError):validate([c],lines,java=True,arithmetic_policy=WRAPPING_POLICY)
        java=self.encoded(c,reference)
        result=validate([c],java,java=True,arithmetic_policy=WRAPPING_POLICY)
        self.assertEqual((result['passed'],result['divergent_cases']),(0,1))
    def test_wrapping_validation_remains_strict_and_rejects_unknown_policy(self):
        c=self.case();lines=self.lines()
        self.assertEqual(validate([c],lines,arithmetic_policy=WRAPPING_POLICY)['passed'],1)
        for bad in [lines[:-1],lines+lines[:1],lines[::-1],lines[:2]+[lines[1]]+lines[3:]]:
            with self.assertRaises(ValueError):validate([c],bad,arithmetic_policy=WRAPPING_POLICY)
        with self.assertRaises(ValueError):validate([c],lines,arithmetic_policy='unrecognized')
    def test_report_counters_derive_all_three_actual_policies_and_full_controls(self):
        cancellation=dict(id='cancel',boundary=True,items=[[0,10**9,0],[1,10**9+1,0]],operations=[('c',0,0),('c',1,1)])
        overflow=dict(id='overflow',boundary=True,items=[[0,2**63-1,0],[1,0,0]],operations=[('c',0,0),('c',1,1)])
        cases=[cancellation,overflow]
        exact=sum((self.encoded(c,reference) for c in cases),[])
        wrapping=sum((self.encoded(c,wrapping_reference) for c in cases),[])
        binary=sum((self.encoded(c,binary64_reference) for c in cases),[])
        java=binary.copy()
        outputs=dict(java=java,rust=binary,rust_exact=exact,rust_exact_root=wrapping)
        result=comparison_report(cases,outputs)
        self.assertEqual(result['arithmetic_policy'],BINARY64_POLICY)
        self.assertEqual((result['passed'],result['exact_root_controls']['passed'],result['exact_controls']['passed']),(2,1,0))
        self.assertEqual(result['outputs']['rust_exact'],exact)
        self.assertEqual(result['outputs']['rust_exact_root'],wrapping)
        self.assertEqual(result['exact_controls']['outputs']['java'],java)
        self.assertEqual(result['exact_root_controls']['outputs']['java'],java)
        self.assertEqual(result['exact_controls']['results']['rust']['passed'],2)
        self.assertEqual(result['exact_root_controls']['results']['rust']['passed'],2)
        self.assertEqual(result['results']['rust']['observations'],6)
        # A real Java-output change changes the three derived counters.
        java[2]=wrapping[2]
        result=comparison_report(cases,outputs)
        self.assertEqual((result['passed'],result['exact_root_controls']['passed'],result['exact_controls']['passed']),(1,2,1))
        java[:]=['cancel\tERROR\tjava.lang.ArithmeticException']+binary[3:]
        result=comparison_report(cases,outputs)
        for cohort in [result,result['exact_controls'],result['exact_root_controls']]:
            self.assertEqual(cohort['java_error_cases'],1)
            self.assertEqual(cohort['java_observations'],3)
        self.assertEqual((result['passed'],result['exact_root_controls']['passed'],result['exact_controls']['passed']),(1,1,0))
    def test_binary_typed_errors_require_exact_prediction_shape_and_recovery(self):
        case=dict(id='negative',boundary=True,items=[[0,3037000500,0],[1,0,0]],operations=[('c',0,0),('c',1,1),('c',0,-1)])
        lines=self.encoded(case,binary64_reference)
        result=validate([case],lines,arithmetic_policy=BINARY64_POLICY)
        self.assertEqual((result['passed'],result['observations'],len(result['policy_errors'])),(1,4,1))
        malformed=[lines[:-1],lines+lines[:1],lines[::-1],
                   lines[:2]+['negative\t2\tERROR\tOther']+lines[3:],
                   lines[:2]+['negative\t2\tERROR\t']+lines[3:],
                   lines[:2]+['negative\t2\tERROR\tNegativeRadicand\textra']+lines[3:],
                   lines[:2]+['wrong\t2\tERROR\tNegativeRadicand']+lines[3:],
                   lines[:1]+['negative\t1\tERROR\tNegativeRadicand']+lines[2:],
                   lines[:2]+['negative\t2\t-\t0\t0']+lines[3:]]
        for bad in malformed:
            with self.subTest(bad=bad),self.assertRaises(ValueError):validate([case],bad,arithmetic_policy=BINARY64_POLICY)
        with self.assertRaises(ValueError):validate([case],lines)
        with self.assertRaises(ValueError):validate([case],lines,java=True,arithmetic_policy=BINARY64_POLICY)
        # Even a real Java failure at this same negative state is not agreement.
        java=self.encoded(case,reference)[:2]+['negative\tERROR\tjava.lang.ArithmeticException']
        result=validate([case],java,java=True,arithmetic_policy=BINARY64_POLICY)
        self.assertEqual((result['passed'],result['error_cases'],result['observations']),(0,1,2))
    def test_binary_success_output_retains_strict_parser_and_native_reference(self):
        case=self.case();lines=self.lines()
        self.assertEqual(validate([case],lines,arithmetic_policy=BINARY64_POLICY)['passed'],1)
        for bad in [lines[:-1],lines+lines[:1],lines[::-1],lines[:2]+[lines[1]]+lines[3:],
                    lines[:2]+['x\t2\t0:2;1:2\t707107\t6']+lines[3:],
                    lines[:2]+['x\t2\t0:2;1:1\t707108\t6']+lines[3:]]:
            with self.assertRaises(ValueError):validate([case],bad,arithmetic_policy=BINARY64_POLICY)
        for raw in ['x\t2\t0:1;0:2\t0\t0','x\t2\t1:1;0:2\t0\t0','x\t2\t9:2\t0\t0','x\t2\t0:2:3\t0\t0','x\t2\t-\t-1\t0','x\t2\t-\t1\t999']:
            with self.assertRaises(ValueError):validate([case],lines[:2]+[raw]+lines[3:],arithmetic_policy=BINARY64_POLICY)
    def test_all_three_native_modes_must_validate_and_bounded_java_is_mandatory(self):
        case=self.case();case['boundary']=False;lines=self.lines()
        baseline=dict(java=lines.copy(),rust=lines.copy(),rust_exact=lines.copy(),rust_exact_root=lines.copy())
        for key in ['rust','rust_exact','rust_exact_root']:
            outputs={name:value.copy() for name,value in baseline.items()};outputs[key]=outputs[key][:-1]
            with self.subTest(mode=key),self.assertRaises(ValueError):comparison_report([case],outputs)
        baseline['java'][2]='x\t2\t0:2;1:1\t0\t0'
        with self.assertRaises(ValueError):comparison_report([case],baseline)

    def test_all_original_inputs_and_states_survive_selected_reference(self):
        cases=generate();count=0
        for case in cases:
            for slots in states(case):
                result=wrapping_reference(case,slots)
                self.assertNotIn('error',result)
                count+=1
        self.assertEqual((len(cases),count),(101,8290))

    def test_runtime_commands_keep_one_unchanged_java_and_native_payload(self):
        case=self.case();case['boundary']=False
        output='\n'.join(self.lines())+'\n'
        with patch('load_balance_checks.generate',return_value=[case]), patch('load_balance_checks.subprocess.run',return_value=SimpleNamespace(stdout=output)) as execute:
            result=run_checks('/public/classes')
        self.assertEqual(result['passed'],1)
        self.assertEqual(len(execute.call_args_list),4)
        java,exact,wrapping,binary=execute.call_args_list
        self.assertEqual(java.args[0],['java','-cp','/public/classes','LoadBalanceOracle'])
        self.assertEqual(len(exact.args[0]),1)
        self.assertEqual(wrapping.args[0],exact.args[0]+['--wrapping64-moments'])
        self.assertEqual(binary.args[0],exact.args[0]+['--wrapping64-moments-binary64-root'])
        expected='x\t0,2,0;1,1,0\tc:0:0;c:1:1;t:2:1\n'
        self.assertEqual([call.kwargs['input'] for call in execute.call_args_list],[expected]*4)

if __name__=='__main__':unittest.main()
