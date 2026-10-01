"""A failed adapter must not masquerade as agreement on valid score inputs."""
import unittest
from differential_scores import result_matches

class ScoreEvidenceTests(unittest.TestCase):
    def test_valid_cases_require_matching_nonerror_results(self):
        self.assertTrue(result_matches({}, '0\t0\ttrue\t0', '0\t0\ttrue\t0'))
        self.assertFalse(result_matches({}, 'ERROR', 'ERROR'))
        self.assertFalse(result_matches({}, '1', '0'))

    def test_expected_parse_errors_require_both_runtimes_to_reject(self):
        case={'expect_error':True}
        self.assertTrue(result_matches(case,'ERROR','ERROR'))
        self.assertFalse(result_matches(case,'ERROR','0'))
        self.assertFalse(result_matches(case,'0','0'))
