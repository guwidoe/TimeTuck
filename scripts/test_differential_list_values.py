"""Evidence checks reject wrong identities and keep Java limitations visible."""
import base64
from copy import deepcopy
import unittest
from differential_lists import expected
from differential_list_values import assess, expected_values, parse


class ListValueEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.case = dict(id='case', count=3, initial=[[0, 1], [2]],
                         moves=[['s', 0, 0, 1, 0], ['c', 0, 0, 1, 1]])

    def line(self, values='0,2', committed=None):
        fields = expected(self.case).split('\t')
        return '\t'.join([fields[0], 'ok', *fields[1:], values,
                          values if committed is None else committed])

    def assess(self, java, rust=None):
        return assess([self.case], dict(java=[java], rust=[self.line() if rust is None else rust]))

    def test_capture_uses_each_child_input_and_does_not_mutate(self):
        before = deepcopy(self.case)
        self.assertEqual(expected_values(self.case), [0, 2])
        self.assertEqual(self.case, before)
        self.case['moves'] = [['u', 0, 0, 2], ['a', 1, 1, 1], ['a', 0, 0, 0]]
        self.assertEqual(expected_values(self.case), [0, 1])

    def test_reversed_spans_keep_original_identity_membership(self):
        self.case = dict(id='span', count=5, initial=[[0, 1, 2], [3, 4]],
                         moves=[['x', 0, 0, 2, 1, 0, 1, 1], ['r', 1, 0, 2, 0, 0, 1]])
        self.assertEqual(expected_values(self.case), [0, 1, 3])

    def test_java_order_and_duplicates_are_retained_but_membership_compares_as_set(self):
        result = self.assess(self.line('2,0,2'))
        self.assertEqual(result['passed'], 1)
        self.assertEqual(result['duplicate_observations'], 2)
        self.assertEqual(result['order_differences'], 2)
        self.assertEqual(parse(self.case, self.line('2,0,2'), 'java')['values'][0], [2, 0, 2])

    def test_missing_extra_or_wrong_commit_value_does_not_pass(self):
        for line in [self.line('0'), self.line('0,1,2'), self.line(committed='1,2')]:
            result = self.assess(line)
            self.assertEqual(result['passed'], 0)
            self.assertEqual(result['divergent_cases'], 1)
        for bad in ['2,0', '0,2,0']:
            result = self.assess(self.line(), self.line(bad))
            self.assertTrue(result['failures'])
            self.assertEqual(result['native_reference_passed'], 0)
            self.assertEqual(result['passed'], 0)

    def test_protocol_and_state_corruption_fail(self):
        good = self.line()
        fields = good.split('\t')
        wrong_state = fields.copy()
        wrong_state[3] = fields[2]
        for bad in [good.replace('case\t', 'other\t'), good + '\textra',
                    good.rsplit('\t', 1)[0], '\t'.join(wrong_state),
                    self.line('-1'), self.line('3'), self.line('x'), self.line('٠'),
                    'case\terror\t%%%']:
            with self.subTest(bad=bad):
                self.assertTrue(self.assess(bad)['failures'])

    def test_errors_remain_distinct_from_agreement(self):
        encoded = base64.b64encode(b'java.lang.IndexOutOfBoundsException: index 1').decode()
        result = self.assess('case\terror\t' + encoded)
        self.assertEqual(result['passed'], 0)
        self.assertEqual(result['java_error_cases'], 1)
        self.assertEqual(result['native_reference_passed'], 1)
        self.assertIn('IndexOutOfBounds', result['differences'][0]['error'])

    def test_empty_java_composition_is_unsupported_not_passing(self):
        self.case['moves'] = []
        result = self.assess('case\tunsupported_empty', self.line(''))
        self.assertEqual(result['unsupported_cases'], 1)
        self.assertEqual(result['passed'], 0)
        self.assertEqual(result['native_reference_passed'], 1)
        self.case['moves'] = [['c', 0, 0, 1, 0]]
        self.assertTrue(self.assess('case\tunsupported_empty')['failures'])
        with self.assertRaises(ValueError):
            parse(self.case, 'case\tunsupported_empty', 'rust')


if __name__ == '__main__':
    unittest.main()
