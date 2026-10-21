"""Regression checks for the denominator and result accounting, not solver claims."""
import unittest
import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch
from differential_scores import source_digest
from compat_report import exclusion_matches, counts, declarations, family, read_test_run, resolve_provenance_target

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

    def test_exact_exclusion_does_not_hide_other_methods_in_the_same_class(self):
        rule = {'method_id': 'Example.java#jvmOnly:9', 'reason': 'JVM only'}
        for identity, expected in [('Example.java#jvmOnly:9', True),
                                   ('Example.java#nativeBehavior:20', False),
                                   ('Other.java#jvmOnly:9', False),
                                   ('Example.java#jvmOnly:10', False)]:
            with self.subTest(identity=identity):
                self.assertEqual(exclusion_matches({'id': identity, 'path': identity.split('#')[0]}, rule), expected)
        self.assertTrue(exclusion_matches({'path': 'quarkus/example.java'}, {'path_prefix': 'quarkus'}))

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

class ProvenanceTargetTests(unittest.TestCase):
    def test_nested_names_require_exact_pinned_declaration_line(self):
        methods = declarations("class A { @Test void same() {} }\nclass B { @Test void same() {} }")
        lookup = {('Example.java', 'same'): methods}
        case = dict(id='example', upstream_path='Example.java', upstream_test='same')
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            resolve_provenance_target(lookup, case)
        self.assertIs(resolve_provenance_target(lookup, dict(case, upstream_line=1)), methods[0])
        self.assertIs(resolve_provenance_target(lookup, dict(case, upstream_line=2)), methods[1])
        for line in [3, 0, -1, True, '2', None]:
            with self.subTest(line=line), self.assertRaises(ValueError):
                resolve_provenance_target(lookup, dict(case, upstream_line=line))
        for changed in [dict(case, upstream_path='Other.java'), dict(case, upstream_test='other')]:
            with self.assertRaises(ValueError):
                resolve_provenance_target(lookup, dict(changed, upstream_line=1))
        unique = {('Example.java', 'same'): [methods[0]]}
        self.assertIs(resolve_provenance_target(unique, case), methods[0])

class CapturedResultTests(unittest.TestCase):
    def test_focused_or_unknown_log_producer_cannot_claim_a_full_native_gate(self):
        with TemporaryDirectory() as directory:
            path = Path(directory) / 'focused.log'
            path.write_text('Running tests/one.rs (target/debug/one)\ntest real_case ... ok\n')
            metadata = {'source_before': 'a', 'source_after': 'a', 'exit_code': 0,
                        'log_sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
            for command, expected in [(None, False), (['cargo', 'test', '--test', 'one'], False),
                                      ('cargo test --test one', False),
                                      ('per-target cargo test (matching successful targets reused)', True)]:
                with self.subTest(command=command):
                    metadata['command'] = command
                    path.with_suffix('.log.json').write_text(json.dumps(metadata))
                    results, runs = read_test_run(path, 'a')
                    self.assertEqual(results[('crates/solver/tests/one.rs', 'real_case')], 'passed')
                    self.assertIs(runs[0]['full_native_gate'], expected)
                    self.assertEqual(runs[0]['native_source_digest'], 'a')
                    self.assertEqual(runs[0]['log_sha256'], metadata['log_sha256'])
                    if isinstance(command, list):
                        self.assertEqual(runs[0]['command'], command)
            metadata['exit_code'] = 101
            path.with_suffix('.log.json').write_text(json.dumps(metadata))
            self.assertFalse(read_test_run(path, 'a')[1][0]['full_native_gate'])

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
