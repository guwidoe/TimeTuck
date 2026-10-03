import unittest
from conditional_collector_checks import expected, generate, validate


class ConditionalCollectorChecksTest(unittest.TestCase):
    def test_rejected_group_presence_replacement_and_trial_undo(self):
        case = {'id': 'test', 'operations': [
            ('c', 0, (1, -3)), ('c', 1, (1, 0)), ('c', 2, (2, 7)),
            ('t', 0, (3, 4)), ('c', 0, None), ('c', 1, None), ('c', 2, None),
        ]}
        scores = [(0, 0), (0, 101), (0, 101), (1, 308), (2, 612),
                  (1, 308), (1, 308), (1, 208), (0, 0)]
        self.assertEqual(expected(case), [f'test\t{i}\t{h}\t{s}' for i, (h,s) in enumerate(scores)])

    def test_validation_rejects_missing_extra_wrong_scores_and_identities(self):
        cases = [generate()[1]]
        correct = expected(cases[0])
        self.assertEqual(validate(cases, correct), len(correct))
        for bad in [[], correct[:-1], correct+['extra'], ['other\t0\t0\t0']+correct[1:], correct[:-1]+['conditional-01\t11\t0\t1']]:
            with self.assertRaises(ValueError):
                validate(cases, bad)
