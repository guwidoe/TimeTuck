"""Selective invalidation and actual subprocess reuse, without running solvers."""
import json
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from verification_cache import run_job, run_measurement
from verification_inputs import digest, measurement_digest, rust_test_inputs, report_digest


class InputTests(unittest.TestCase):
    def setUp(self):
        self.directory = TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for name, text in {
            'crates/solver/src/lib.rs': 'pub mod score; pub mod search;',
            'crates/solver/src/score.rs': 'pub struct Score;',
            'crates/solver/src/search.rs': 'pub struct Search;',
            'crates/solver/tests/a.rs': 'include_str!("fixtures/a.txt");',
            'crates/solver/tests/b.rs': 'test b',
            'crates/solver/tests/fixtures/a.txt': 'fixture a',
            'crates/solver/examples/score_benchmark.rs': 'use timetuck::score::Score;',
            'crates/solver/examples/k_opt_benchmark.rs': 'use timetuck::search::Search; mod support;',
            'crates/solver/examples/support/mod.rs': 'support',
            'scripts/benchmark_scores.py': 'from helper import value\n',
            'scripts/helper.py': 'value = 1',
            'scripts/benchmark_kopt.py': 'kopt = 1',
            'compat/oracle/ScoreBenchmark.java': 'class ScoreBenchmark {}',
            'compat/oracle/KOptBenchmark.java': 'class KOptBenchmark { ListSolverBenchmark shared; }',
            'compat/oracle/ListSolverBenchmark.java': 'class ListSolverBenchmark {}',
            'compat/provenance.json': '{}',
        }.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)

    def key(self, entry):
        return digest(rust_test_inputs(entry, self.root), self.root)

    def test_test_fixture_edit_only_invalidates_its_target(self):
        a, b = 'crates/solver/tests/a.rs', 'crates/solver/tests/b.rs'
        before = self.key(a), self.key(b)
        fixture = self.root / 'crates/solver/tests/fixtures/a.txt'
        fixture.write_text('changed')
        self.assertNotEqual(before[0], self.key(a))
        self.assertEqual(before[1], self.key(b))
        fixture.unlink()
        self.assertNotEqual(before[0], self.key(a))

    def test_reporting_and_java_edits_do_not_invalidate_rust_tests(self):
        entry = 'crates/solver/tests/a.rs'
        before, report = self.key(entry), report_digest(self.root)
        (self.root / 'compat/provenance.json').write_text('{"new": true}')
        (self.root / 'compat/oracle/ScoreBenchmark.java').write_text('changed')
        self.assertEqual(before, self.key(entry))
        self.assertNotEqual(report, report_digest(self.root))

    def test_rust_module_change_only_invalidates_dependent_targets(self):
        a, b = 'crates/solver/tests/a.rs', 'crates/solver/tests/b.rs'
        (self.root / a).write_text('use timetuck::score::Score;')
        (self.root / b).write_text('use timetuck::search::Search;')
        before = self.key(a), self.key(b)
        (self.root / 'crates/solver/src/score.rs').write_text('pub struct Score { value: i64 }')
        self.assertNotEqual(before[0], self.key(a))
        self.assertEqual(before[1], self.key(b))

    def test_benchmark_dependencies_are_separate_from_tests_and_other_adapters(self):
        score = measurement_digest('performance', self.root)
        kopt = measurement_digest('kopt-performance', self.root)
        (self.root / 'crates/solver/tests/a.rs').write_text('changed')
        self.assertEqual(score, measurement_digest('performance', self.root))
        self.assertEqual(kopt, measurement_digest('kopt-performance', self.root))
        (self.root / 'compat/oracle/ListSolverBenchmark.java').write_text('changed')
        self.assertEqual(score, measurement_digest('performance', self.root))
        self.assertNotEqual(kopt, measurement_digest('kopt-performance', self.root))
        (self.root / 'scripts/helper.py').write_text('value = 2')
        self.assertNotEqual(score, measurement_digest('performance', self.root))

    def test_search_change_spares_score_arithmetic_and_example_support_is_tracked(self):
        score = measurement_digest('performance', self.root)
        kopt = measurement_digest('kopt-performance', self.root)
        (self.root / 'crates/solver/src/search.rs').write_text('changed')
        self.assertEqual(score, measurement_digest('performance', self.root))
        self.assertNotEqual(kopt, measurement_digest('kopt-performance', self.root))
        kopt = measurement_digest('kopt-performance', self.root)
        (self.root / 'crates/solver/examples/support/mod.rs').write_text('changed')
        self.assertNotEqual(kopt, measurement_digest('kopt-performance', self.root))
        (self.root / 'crates/solver/src/score.rs').write_text('changed')
        self.assertNotEqual(score, measurement_digest('performance', self.root))

    def test_unrelated_facade_exports_spare_existing_tests_and_benchmarks(self):
        facade = self.root / 'crates/solver/src/lib.rs'
        facade.write_text('pub mod score; pub mod search; pub use score::{Score};')
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::*; fn probe(_: Score) {}')
        before = self.key(entry), measurement_digest('performance', self.root)
        library = self.key('crates/solver/src/lib.rs')
        (self.root / 'crates/solver/src/extra.rs').write_text('pub struct Extra;')
        facade.write_text('pub mod score; pub mod search; mod extra; pub use extra::Extra; pub use score::{Score, Score as Unused};')
        self.assertEqual(before, (self.key(entry), measurement_digest('performance', self.root)))
        self.assertNotEqual(library, self.key('crates/solver/src/lib.rs'))

    def test_used_export_retargeting_invalidates_only_its_consumers(self):
        facade = self.root / 'crates/solver/src/lib.rs'
        facade.write_text('pub mod score; pub mod search; pub use score::Score as Chosen;')
        a, b = 'crates/solver/tests/a.rs', 'crates/solver/tests/b.rs'
        (self.root / a).write_text('use timetuck::Chosen;')
        (self.root / b).write_text('use timetuck::search::Search;')
        before = self.key(a), self.key(b)
        facade.write_text('pub mod score; pub mod search; pub use search::Search as Chosen;')
        self.assertNotEqual(before[0], self.key(a))
        self.assertEqual(before[1], self.key(b))

    def test_facade_keeps_crate_attributes_and_root_code_but_ignores_docs(self):
        facade = self.root / 'crates/solver/src/lib.rs'
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::score::Score;')
        original = facade.read_text()
        before = self.key(entry)
        facade.write_text('//! Added documentation.\n' + original)
        self.assertEqual(before, self.key(entry))
        facade.write_text('#![forbid(unsafe_code)]\n' + original)
        self.assertNotEqual(before, self.key(entry))
        facade.write_text(original + ' pub const LABEL: &str = "a  b";')
        code = self.key(entry)
        self.assertNotEqual(before, code)
        facade.write_text(original + ' pub const LABEL: &str = "a b";')
        self.assertNotEqual(code, self.key(entry))



class CacheTests(unittest.TestCase):
    def test_actual_command_runs_once_until_inputs_force_log_or_environment_change(self):
        with TemporaryDirectory() as directory, patch('verification_cache.environment', return_value={'version': 1}) as env:
            root = Path(directory)
            source = root / 'input'
            source.write_text('a')
            command = [sys.executable, '-c', 'from pathlib import Path; p=Path("count"); p.write_text(str(int(p.read_text())+1) if p.exists() else "1"); print("passed")']
            def run(**kw): return run_job('sample', command, lambda: {'input'}, root=root, kind='python', **kw)
            self.assertEqual(run()[0], 0)
            log = root / 'work/verification/sample.log'
            stamp = log.stat().st_mtime_ns
            self.assertEqual(run()[0], 0)
            self.assertEqual(log.stat().st_mtime_ns, stamp)
            self.assertEqual((root / 'count').read_text(), '1')
            source.write_text('b')
            run()
            run(force=True)
            log.write_text('tampered')
            run()
            env.return_value = {'version': 2}
            run()
            self.assertEqual((root / 'count').read_text(), '5')

    def test_failure_and_mid_run_changes_are_not_cached(self):
        with TemporaryDirectory() as directory, patch('verification_cache.environment', return_value={}):
            root = Path(directory)
            (root / 'input').write_text('a')
            failing = [sys.executable, '-c', 'raise SystemExit(3)']
            for _ in range(2):
                self.assertEqual(run_job('failed', failing, lambda: {'input'}, root=root)[0], 3)
            changed = [sys.executable, '-c', 'from pathlib import Path; Path("input").write_text("b")']
            self.assertEqual(run_job('changed', changed, lambda: {'input'}, root=root)[0], 1)
            self.assertFalse((root / 'work/verification/failed.json').exists())
            self.assertFalse((root / 'work/verification/changed.json').exists())

    def test_measurement_reuses_output_and_honors_arguments_and_force(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'work').mkdir()
            report = root / 'work/performance.json'
            calls = []
            def main():
                calls.append(1)
                report.write_text(json.dumps({'source_digest': 'inputs', 'generated_at': 'original'}))
            with patch('verification_cache.ROOT', root), patch('verification_cache.measurement_digest', return_value='inputs'), patch('verification_cache.environment', return_value={}), patch('sys.argv', ['benchmark_scores.py']):
                run_measurement('performance', main)
                run_measurement('performance', main)
                self.assertEqual(len(calls), 1)
                self.assertEqual(json.loads(report.read_text())['generated_at'], 'original')
                sys.argv.append('--samples=2')
                run_measurement('performance', main)
                sys.argv.append('--force')
                run_measurement('performance', main)
                self.assertNotIn('--force', sys.argv)
                self.assertEqual(len(calls), 3)
                report.write_text('{}')
                run_measurement('performance', main)
                self.assertEqual(len(calls), 4)


if __name__ == '__main__':
    unittest.main()
