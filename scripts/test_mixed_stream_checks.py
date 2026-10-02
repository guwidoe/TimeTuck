"""Guard mixed differential validation against plausible but incorrect results."""
import unittest
from copy import deepcopy
from mixed_stream_checks import apply, encode, expected, generate, score, states, validate


class MixedStreamChecksTest(unittest.TestCase):
    def test_hand_calculated_joint_trial_undo_and_commit(self):
        case = generate()[0]
        observed = list(states(case))
        self.assertEqual([score(s) for s in observed], [(0,0),(0,13),(0,0),(0,-48)])
        self.assertEqual(observed[0], observed[2])
        self.assertEqual(observed[1], {'basic':[3,None],'lists':[[],[23,11]]})
        self.assertEqual(expected(case,1,observed[1]), 'mixed-000\t1\t0\t0\t13\t3,-\t-;23,11\t1:1:23:-,1:0:-:11')
        self.assertEqual(expected(case,3,observed[3]), 'mixed-000\t3\t1\t0\t-48\t0,-2\t11;23\t0:0:-:-,1:0:-:-')

    def test_validator_rejects_corruption_in_every_observable_field(self):
        case = generate()[0]
        state = list(states(case))[1]
        line = expected(case,1,state)
        validate(case,1,state,line)
        for i, replacement in enumerate(['wrong','2','1','-1','12','3,0','23,11;-','-,-']):
            fields = line.split('\t'); fields[i] = replacement
            with self.subTest(field=i), self.assertRaises(ValueError):
                validate(case,1,state,'\t'.join(fields))
        for bad in [line+'\textra', '\t'.join(line.split('\t')[:-1])]:
            with self.assertRaises(ValueError): validate(case,1,state,bad)

    def test_generated_protocol_is_repeatable_and_exercises_joint_states_and_pins(self):
        cases = generate()
        self.assertEqual(cases, generate())
        self.assertEqual(len(cases),128)
        self.assertEqual(len({c['id'] for c in cases}),128)
        kinds, modes = set(), set()
        joint = 0
        for case in cases:
            self.assertEqual(len(encode(case).split('\t')),6)
            observed = list(states(case))
            self.assertEqual(len(observed),1+sum(2 if b['mode']=='t' else 1 for b in case['batches']))
            for state in observed:
                for i,pin in enumerate(case['pins']):
                    if pin: self.assertEqual(state['basic'][i],case['basic'][i])
            for batch in case['batches']:
                moves={op[0] for op in batch['moves']}
                kinds.update(moves); modes.add(batch['mode'])
                joint += 'b' in moves and len(moves)>1
        self.assertEqual(kinds,{'b','a','c','s','u'})
        self.assertEqual(modes,{'t','c'})
        self.assertGreater(joint,100)
        self.assertTrue(any(any(c['pins']) for c in cases))
        self.assertTrue(any(not c['values'] for c in cases))

    def test_invalid_bounds_domains_and_pins_do_not_mutate_input(self):
        case=generate()[2]
        original={'basic':case['basic'][:],'lists':deepcopy(case['lists'])}
        for op in [['b',0,0],['b',1,99],['b',-1,0],['a',999,1,0],['a',11,1,0],
                   ['a',23,1,9],['c',0,0,0,3],['s',0,-1,1,0],['u',0,2,1]]:
            candidate=deepcopy(original)
            with self.subTest(op=op), self.assertRaises(ValueError): apply(case,candidate,[op])
            self.assertEqual(candidate,original)


if __name__=='__main__':unittest.main()
