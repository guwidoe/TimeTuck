import copy
import json
from pathlib import Path
import tempfile
import unittest

from real_world_reference import initial_schedule, scaled, score_schedule, validate_reported_score

ROOT = Path(__file__).resolve().parents[1]


def small():
    return {'scenario': {'people': [{'id': 'a', 'attributes': {'role': 'x'}},
                                    {'id': 'b', 'attributes': {'role': 'y'}},
                                    {'id': 'c', 'attributes': {'role': 'x'}}],
                         'groups': [{'id': 'g0', 'capacity': 2}, {'id': 'g1', 'capacity': 2}],
                         'session_count': 2},
            'objectives': [{'type': 'maximize_unique_contacts', 'weight': 1},
                           {'type': 'avoid_repeats', 'weight': 2, 'penalty_curve': 'squared',
                            'max_encounters_per_pair': 1}], 'constraints': []}


class RealWorldReferenceTests(unittest.TestCase):
    def test_initial_last_tie_and_fixed_first(self):
        problem = small()
        problem['constraints'] = [{'type': 'fixed_placement', 'person_ids': ['a'], 'group_id': 'g0'}]
        start = initial_schedule(problem)
        self.assertEqual(start['session_0'], {'g0': ['a'], 'g1': ['b', 'c']})
        self.assertTrue(score_schedule(problem, start)['feasible'])

    def test_contacts_and_repeated_pair_exact_scale(self):
        result = score_schedule(small(), {f'session_{s}': {'g0': ['a', 'b'], 'g1': ['c']} for s in range(2)})
        self.assertEqual(result['score'], [0, -100])  # one +100 contact, one -200 repeat
        self.assertEqual(result['breakdown']['repeat_penalty_units'], 1)

    def test_hard_and_soft_pair_counts_capacity_and_infeasible_validation(self):
        problem = small()
        problem['constraints'] = [{'type': 'keep_apart', 'person_ids': ['a', 'b']},
                                  {'type': 'prefer_apart', 'person_ids': ['a', 'c'], 'penalty_weight': 3}]
        schedule = {f'session_{s}': {'g0': ['a', 'b', 'c'], 'g1': []} for s in range(2)}
        result = validate_reported_score(problem, schedule, [-4, -900])
        self.assertEqual(result['score'], [-4, -900])
        self.assertTrue(result['valid'])
        self.assertFalse(result['feasible'])
        mismatch = validate_reported_score(problem, schedule, [-4, -800])
        self.assertFalse(mismatch['valid'])  # Infeasibility must never hide a soft-score error.
        self.assertFalse(validate_reported_score(problem, schedule, [-3, -900])['valid'])

    def test_balance_exact_atleast_scope_and_empty_group_policy(self):
        problem = small()
        problem['constraints'] = [
            {'type': 'attribute_balance', 'attribute_key': 'role', 'target_counts': {'x': 2},
             'target_mode': 'at_least', 'penalty_weight': 3, 'group_ids': ['g0'], 'session_indices': [0]},
            {'type': 'attribute_balance', 'attribute_key': 'role', 'target_counts': {'y': 0},
             'target_mode': 'exact', 'penalty_weight': 5, 'group_ids': ['g0'], 'session_indices': [0]},
            {'type': 'attribute_balance', 'attribute_key': 'role', 'target_counts': {'x': 9},
             'target_mode': 'at_least', 'penalty_weight': 100, 'group_ids': ['g1'], 'session_indices': [1]}]
        schedule = {'session_0': {'g0': ['a', 'b'], 'g1': ['c']},
                    'session_1': {'g0': ['a', 'b', 'c'], 'g1': []}}
        self.assertEqual(score_schedule(problem, schedule)['breakdown']['balance_penalty'], 800)

    def test_attendance_missing_duplicates_unknown_and_pins(self):
        problem = small()
        problem['scenario']['people'][2]['session_indices'] = [0]
        problem['constraints'] = [{'type': 'fixed_placement', 'person_ids': ['a'], 'group_id': 'g0'}]
        schedule = initial_schedule(problem)
        self.assertNotIn('c', sum(schedule['session_1'].values(), []))
        wrong = copy.deepcopy(schedule)
        wrong['session_0']['g0'].remove('a')
        wrong['session_0']['g1'].append('a')
        self.assertTrue(score_schedule(problem, wrong)['pin_violations'])
        wrong['session_1']['g1'] += ['a', 'c', 'unknown']
        result = score_schedule(problem, wrong)
        self.assertTrue(any('duplicate' in e for e in result['structural_errors']))
        self.assertTrue(any('absent' in e for e in result['structural_errors']))
        self.assertTrue(any('unknown' in e for e in result['structural_errors']))
        wrong['session_0']['g1'].remove('b')
        self.assertTrue(any('missing' in e for e in score_schedule(problem, wrong)['structural_errors']))

    def test_weights_reject_nonintegral_scaling(self):
        self.assertEqual(scaled(0.01), 1)
        self.assertEqual(scaled(None), 100000)
        with self.assertRaises(ValueError):
            scaled(0.005)

    def test_public_fixtures_flattened_start_and_hard_score_not_elided(self):
        from benchmark_real_world import CASES, FIXTURES, java_schedule, normalized_schedule, prep_module
        with tempfile.TemporaryDirectory() as directory:
            for case in CASES:
                problem_path = FIXTURES / case / 'problem.json'
                problem = json.loads(problem_path.read_text())
                path = Path(directory) / 'model.txt'
                prep_module().model(str(problem_path), str(path))
                lines = path.read_text().splitlines()
                offset = 2 + problem['scenario']['session_count']
                count = int(lines[offset])
                rows = Path(directory) / 'assignments.txt'
                rows.write_text('\n'.join(' '.join(row.split()[:3]) for row in lines[offset+1:offset+1+count]))
                start = initial_schedule(problem)
                self.assertEqual(normalized_schedule(java_schedule(problem, rows)), normalized_schedule(start))
                result = score_schedule(problem, start)
                self.assertFalse(result['structural_errors'])
                self.assertFalse(result['pin_violations'])
                if case == 'sailing-trip-v1':
                    self.assertLess(result['score'][0], 0)

    def test_runner_summary_never_turns_validation_into_speed_target(self):
        from benchmark_real_world import summarize
        runs = [{'case': 'sailing-trip-v1', 'runtime': runtime, 'valid': True,
                 'reported': {'elapsed_ns': 10**9}, 'evaluations_per_second': throughput,
                 'validation': {'score': [-1, 100], 'feasible': False}}
                for runtime, throughput in [('rust-streams', 50), ('java-streams', 100)]]
        result = summarize(runs, ['sailing-trip-v1'])[0]
        self.assertFalse(result['throughput_target_met'])
        self.assertEqual(result['rust_over_java_streams_evaluations_per_second'], .5)
        self.assertFalse(result['quality_equal_gated'])


if __name__ == '__main__':
    unittest.main()
