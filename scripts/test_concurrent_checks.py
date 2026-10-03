"""Independent concurrent boundaries and honest disagreement accounting."""
import base64
from copy import deepcopy
import unittest
from concurrent_checks import apply,encode,expected,generate,project,states,validate_outputs
class ConcurrentChecksTest(unittest.TestCase):
    def test_shared_start_and_distinct_ready_times(self):
        c=generate()[0];rows=list(states(c));p=project(c,rows[1])
        self.assertEqual(p['ready'],[60,0,0,120]);self.assertEqual(p['start'],[60,0,60,120]);self.assertEqual(p['finish'],[90,30,90,150])
        self.assertEqual((p['hard'],p['soft']),(0,-360))
        self.assertEqual(project(c,rows[0])['soft'],-240)
    def test_cycle_keeps_independent_ready_values_and_recovers_on_undo(self):
        c=generate()[0];rows=list(states(c));p=project(c,rows[3])
        self.assertEqual(p['inconsistent'],[1,1,1,0]);self.assertEqual(p['ready'],[0,None,None,0])
        self.assertEqual(p['start'],[None,None,None,0]);self.assertEqual((p['hard'],p['soft']),(-3,-30))
        self.assertEqual(project(c,rows[4])['inconsistent'],[0,0,0,0]);self.assertEqual(rows[3],rows[5])
    def test_unassigned_group_member_receives_group_times_and_zero_values_work(self):
        c=generate()[1];p=project(c,c['lists'])
        self.assertEqual(p['owner'],[None,0,0]);self.assertEqual(p['ready'],[None,60,0]);self.assertEqual(p['start'],[60,60,0]);self.assertEqual(p['finish'],[90,90,30])
        self.assertEqual(p['soft'],-120)
        c=generate()[2];self.assertEqual(expected(c,0,c['lists']).split('\t')[3:],['0','0',*['_']*9])
    def test_invalid_groups_moves_and_snapshot_aliasing(self):
        c=generate()[0]
        for groups in [[[]],[[0,0]],[[4]],[[0,1],[1,2]]]:
            with self.subTest(groups=groups),self.assertRaises(ValueError):list(states(dict(c,groups=groups)))
        for op in [['a',0,1,0],['a',4,1,0],['c',0,-1,1,0],['s',0,0,3,0]]:
            with self.subTest(op=op),self.assertRaises(ValueError):apply(c,c['lists'],[op])
        rows=list(states(c));original=deepcopy(c);rows[0][0].clear();self.assertEqual(rows[2],c['lists']);self.assertEqual(c,original)
    def test_java_runtime_errors_and_divergences_never_count_as_passes(self):
        c=generate()[0];rows=[expected(c,i,s) for i,s in enumerate(states(c))]
        error='ERROR\t'+c['id']+'\t'+base64.b64encode(b'known diagnostic').decode()
        r=validate_outputs([c],{'java':[rows[0],error],'rust':rows})
        self.assertEqual((r['passed'],r['native_reference_passed'],r['java_error_cases'],r['java_observations']),(0,1,1,1))
        self.assertEqual(r['failures'][0]['state'],1)
        bad=rows[:];bits=bad[0].split('\t');bits[11]='-,-,-,-';bad[0]='\t'.join(bits)
        r=validate_outputs([c],{'java':bad,'rust':rows});self.assertEqual(r['passed'],0);self.assertEqual(r['divergent_cases'],1)
        r=validate_outputs([c],{'java':rows,'rust':rows});self.assertEqual(r['passed'],1)
    def test_incomplete_reordered_and_corrupt_native_outputs_fail(self):
        c=generate()[0];rows=[expected(c,i,s) for i,s in enumerate(states(c))]
        for bad in [rows[:-1],rows+[rows[-1]],rows[::-1]]:
            with self.assertRaises(ValueError):validate_outputs([c],{'java':bad,'rust':rows})
        for field in range(14):
            bad=rows[:];bits=bad[0].split('\t');bits[field]='bad';bad[0]='\t'.join(bits)
            with self.subTest(field=field),self.assertRaises(ValueError):validate_outputs([c],{'java':rows,'rust':bad})
        with self.assertRaises(ValueError):validate_outputs([c],{'rust':rows})
    def test_generated_cases_are_reproducible_and_valid(self):
        cs=generate();self.assertEqual(cs,generate());self.assertEqual(len(cs),128)
        for c in cs:
            self.assertEqual(len(encode(c).split('\t')),5)
            for i,ls in enumerate(states(c)):self.assertEqual(len(expected(c,i,ls).split('\t')),14)
if __name__=='__main__':unittest.main()
