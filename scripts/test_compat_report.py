"""Regression checks for the denominator and result accounting, not solver claims."""
import unittest
import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch
from differential_scores import source_digest
from compat_report import counts, declarations, family, read_test_run

class InventoryTests(unittest.TestCase):
    def test_constraint_engine_and_api_cases_share_a_subsystem(self):
        root = 'core/src/test/java/ai/timefold/solver/core/'
        for package in ['impl/bavet/common/joiner/JoinersTest.java',
                        'impl/bavet/common/index/ComparisonIndexerTest.java',
                        'impl/score/stream/uni/UniConstraintStreamTest.java']:
            self.assertEqual(family(root + package), 'Constraint streams')
        self.assertEqual(family(root + 'api/score/SimpleScoreTest.java'), 'Score APIs & types')
        self.assertEqual(family(root + 'impl/util/ListTest.java'), 'Core utilities & configuration')
        self.assertEqual(family('quarkus-integration/' + root), 'JVM integrations')

    def test_ignores_comments_and_literals(self):
        text = '''
// @Test void imaginary() {}
/* @Test void alsoImaginary() {} */
String source = "@Test void stillImaginary() {}";
@Test void actual() { String value = "@Test void nestedFake() {}"; }
'''
        self.assertEqual([m['method'] for m in declarations(text)], ['actual'])

    def test_annotation_arguments_modifiers_generics_and_templates(self):
        text = '''
@ParameterizedTest(name = "{0}")
@MethodSource("arguments")
public void parameters(int a) {}
@Test <Solution_> void generic() {}
@TestTemplate @Disabled("pending") protected void template() {}
@RepeatedTest(value=10) void repeated() {}
'''
        methods = declarations(text)
        self.assertEqual([m['method'] for m in methods], ['parameters', 'generic', 'template', 'repeated'])
        self.assertEqual([m['annotation'] for m in methods], ['ParameterizedTest', 'Test', 'TestTemplate', 'RepeatedTest'])
        self.assertEqual(len(methods), 4)  # Never multiplies by repetition/parameter count.

    def test_unknown_declaration_fails_closed(self):
        with self.assertRaises(ValueError): declarations('@Test this is not a method;')

    def test_pending_excluded_and_unverified_do_not_count_as_passes(self):
        result = counts([{'status': status} for status in ['passed', 'failed', 'pending', 'excluded', 'unverified']])
        self.assertEqual(result['total'], 5)
        self.assertEqual(result['percent'], 20)
        self.assertEqual(result['passed'], 1)
        self.assertIsNone(counts([])['percent'])

class CapturedResultTests(unittest.TestCase):
    def test_real_results_are_scoped_to_the_correct_target(self):
        with TemporaryDirectory() as directory:
            path = Path(directory) / 'run.log'
            path.write_text("""Running tests/one.rs (target/debug/one)
test example ... ok
test skipped ... ignored
Running tests/two.rs (target/debug/two)
test example ... FAILED
Running unittests src/lib.rs (target/debug/deps/timetuck-abc123)
test diversified_late::cases::acceptance_criterion ... ok
test unit_skip ... ignored
test unit_failure ... FAILED
Running unittests src/lib.rs (target/debug/deps/another-abc123)
test not_timetuck ... ok
Running unittests examples/oracle.rs (target/debug/oracle)
test unrelated ... ok
""")
            metadata = {'source_before': 'a', 'source_after': 'a', 'exit_code': 101,
                        'log_sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
            path.with_suffix('.log.json').write_text(json.dumps(metadata))
            results, runs = read_test_run(path, 'a')
            self.assertEqual(results[('crates/solver/tests/one.rs', 'example')], 'passed')
            self.assertEqual(results[('crates/solver/tests/one.rs', 'skipped')], 'unverified')
            self.assertEqual(results[('crates/solver/tests/two.rs', 'example')], 'failed')
            self.assertEqual(results[('crates/solver/src/lib.rs', 'diversified_late::cases::acceptance_criterion')], 'passed')
            self.assertEqual(results[('crates/solver/src/lib.rs', 'unit_skip')], 'unverified')
            self.assertEqual(results[('crates/solver/src/lib.rs', 'unit_failure')], 'failed')
            self.assertEqual(len(results), 6)
            self.assertEqual(runs[0]['exit_code'], 101)
            with self.assertRaisesRegex(ValueError, 'stale'): read_test_run(path, 'b')
            path.write_text(path.read_text().replace('FAILED', 'ok'))
            with self.assertRaisesRegex(ValueError, 'checksum'): read_test_run(path, 'a')

class SourceFingerprintTests(unittest.TestCase):
    def test_fixture_content_names_and_removal_invalidate_evidence(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ['Cargo.lock', 'Cargo.toml', 'crates/solver/Cargo.toml',
                         'compat/oracle/pom.xml']:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('placeholder')
            fixture = root / 'crates/solver/tests/fixtures/nested/data.txt'
            fixture.parent.mkdir(parents=True)
            with patch('differential_scores.ROOT', root):
                empty = source_digest()
                fixture.write_text('expected A')
                before = source_digest()
                self.assertNotEqual(empty, before)
                self.assertEqual(before, source_digest())
                fixture.write_text('expected B')
                self.assertNotEqual(before, source_digest())
                fixture.write_text('expected A')
                renamed = fixture.with_name('renamed.txt')
                fixture.rename(renamed)
                self.assertNotEqual(before, source_digest())
                renamed.unlink()
                self.assertEqual(empty, source_digest())

if __name__ == '__main__': unittest.main()
