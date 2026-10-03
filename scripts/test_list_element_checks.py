"""Independent reference boundaries and malformed-result checks."""
from copy import deepcopy
import unittest
from list_element_checks import apply, encode, expected, generate, states, validate_outputs

class ListElementChecksTest(unittest.TestCase):
    def test_ordered_prefix_times_follow_list_order_not_value_id(self):
        c=generate()[0]
        s=next(states(c))
        self.assertEqual(expected(c,0,s).split('\t')[4:],['0,0,0','2,0,-','6,9,4','9,16,6','16,11','-27'])

    def test_empty_owner_fallback_and_unassigned_penalty(self):
        c=generate()[0];s=list(states(c))[1]
        self.assertEqual(expected(c,1,s).split('\t')[4:],['1,-,-','-,-,-','11,-,-','14,-,-','4,14','-218'])

    def test_nullable_mixed_total_is_not_an_empty_total(self):
        c=generate()[1];rows=list(states(c))
        self.assertEqual(expected(c,0,rows[0]).split('\t')[7:],['-,4,3','-,3','0'])
        self.assertEqual(expected(c,1,rows[1]).split('\t')[7:],['6,4,3','6,7','0'])
        self.assertEqual(expected(c,3,rows[3]).split('\t')[8],'4,-')
        c=generate()[3]
        self.assertEqual(expected(c,0,next(states(c))).split('\t')[7:],['_','0','0'])

    def test_composites_are_sequential_and_undo_restores_without_aliases(self):
        c=generate()[0];original=deepcopy(c);rows=list(states(c))
        self.assertEqual(rows[0],rows[2]);self.assertEqual(rows[3]['lists'],[[0,2],[1]])
        rows[0]['lists'][0].clear();self.assertEqual(rows[2]['lists'],original['lists']);self.assertEqual(c,original)
        c=generate()[1];rows=list(states(c))
        self.assertIsNone(rows[4]['durations'][0]);self.assertEqual(rows[3],rows[5])

    def test_invalid_moves_and_duplicate_members_are_rejected(self):
        c=generate()[0];s=next(states(c))
        for op in [['a',0,1,0],['a',3,1,0],['c',0,-1,1,0],['c',0,0,1,1],['u',0,1,0],['b',0,2]]:
            with self.subTest(op=op),self.assertRaises(ValueError):apply(c,s,[op])
        c=generate()[1];s=next(states(c))
        for op in [['b',-1,2],['b',0,10],['u',0,0,1]]:
            with self.subTest(op=op),self.assertRaises(ValueError):apply(c,s,[op])

    def test_output_integrity_covers_every_field_and_order(self):
        cs=generate()[:2];rows=[expected(c,i,s) for c in cs for i,s in enumerate(states(c))]
        self.assertEqual(len(validate_outputs(cs,{'java':rows,'rust':rows})),len(rows))
        for bad in [rows[:-1],rows+[rows[-1]],rows[::-1]]:
            with self.assertRaises(ValueError):validate_outputs(cs,{'java':bad,'rust':rows})
        for field in range(10):
            bad=rows[:];bits=bad[0].split('\t');bits[field]='corrupt';bad[0]='\t'.join(bits)
            with self.subTest(field=field),self.assertRaises(ValueError):validate_outputs(cs,{'java':rows,'rust':bad})
        with self.assertRaises(ValueError):validate_outputs(cs,{'rust':rows})

    def test_seeded_generation_is_valid_and_reproducible(self):
        cs=generate();self.assertEqual(cs,generate());self.assertEqual(len(cs),128)
        self.assertEqual(sum(c['model']=='o' for c in cs),64)
        for c in cs:
            self.assertEqual(len(encode(c).split('\t')),6)
            for i,s in enumerate(states(c)):self.assertEqual(len(expected(c,i,s).split('\t')),10)

if __name__=='__main__':unittest.main()
