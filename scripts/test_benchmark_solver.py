"""Original checks for admissibility of the branch benchmark's stated bound."""
import itertools
import unittest
from benchmark_solver import ALGORITHMS, workload
from differential_solver import optimum

class ExactBenchmarkTests(unittest.TestCase):
    def test_branch_orders_share_inputs_and_every_prefix_bounds_completions(self):
        for size in [2, 4]:
            base = workload(size, 'depth')
            def score(values):
                hard = -sum(values[a] is not None and values[a] == values[b] for a,b in base['edges'])
                soft = sum(row[v] for row,v in zip(base['weights'], values) if v is not None)
                return hard,soft
            completions = list(itertools.product(range(3), repeat=size))
            self.assertEqual(max(map(score, completions)), optimum(base))
            for complete in completions:
                for depth in range(size + 1):
                    self.assertGreaterEqual(score(complete[:depth] + (None,)*(size-depth)), score(complete))
            for algorithm in ALGORITHMS:
                if algorithm == 'brute': continue
                case = workload(size, algorithm)
                self.assertTrue(all(v <= 0 for row in case['weights'] for v in row))
                case['id'] = base['id']
                self.assertEqual(case, base)
    def test_unknown_algorithm_is_rejected(self):
        with self.assertRaises(ValueError): workload(2, 'unknown')

if __name__ == '__main__': unittest.main()
