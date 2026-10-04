import copy
import unittest
from value_queue_construction_checks import generate_cases, reference


class ValueQueueReferenceTests(unittest.TestCase):
    def test_best_receiver_wrap_and_stable_ties(self):
        cases = generate_cases()
        self.assertEqual(reference(cases[-3]), ([1, 0, 0], (0, 15)))
        self.assertEqual(reference(cases[-2]), ([0, 1, 0, 1, 0], (0, 0)))
        self.assertEqual(reference(cases[-1]), ([1, 0], (0, 5)))

    def test_pins_preserved_and_movable_initial_values_reset(self):
        case = copy.deepcopy(generate_cases()[-3])
        case['initial'] = [0, 1, 1]
        self.assertEqual(reference(case)[0], [1, 0, 0])
        case['pins'] = [False, True, False]
        self.assertEqual(reference(case)[0], [0, 1, 1])

    def test_scope_errors_are_rejected(self):
        for update in [dict(domains=[[0]]*3), dict(optional=[True]*3),
                       dict(value_count=0), dict(pins=[True]*3)]:
            with self.subTest(update=update), self.assertRaises(ValueError):
                reference(dict(generate_cases()[-3], **update))
