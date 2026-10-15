"""Actual shard membership and log attribution; no solver or Cargo subprocesses."""
import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import compat_report
from rust_test_layout import generated, integration_targets, shard_origins, validate, write
from verification_inputs import digest, rust_test_inputs


class LayoutTests(unittest.TestCase):
    def setUp(self):
        directory = TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.tests = self.root / 'crates/solver/tests'
        self.tests.mkdir(parents=True)
        (self.tests.parent / 'Cargo.toml').write_text('[package]\nname = "example"\nversion = "0.1.0"\n')
        for name in ['a', 'b', 'ordered_destination_regression', 'owned_roster_cli_regression',
                     'solution_persistence_cases']:
            (self.tests / f'{name}.rs').write_text('#[test] fn original() {}\n')

    def test_partition_is_complete_unique_stable_and_keeps_exceptions(self):
        write(self.root)
        before = generated(self.root)
        write(self.root)
        self.assertEqual(before, generated(self.root))
        targets = validate(self.root)
        self.assertEqual({t['name'] for t in targets if not t.get('test', True)}, {'a', 'b'})
        active = dict(integration_targets(self.root))
        self.assertIn('ordered_destination_regression', active)
        self.assertIn('owned_roster_cli_regression', active)
        self.assertIn('solution_persistence_cases', active)
        origins = shard_origins(self.root)
        members = [p for paths in origins.values() for p in paths.values()]
        self.assertEqual(sorted(members), [f'crates/solver/tests/{n}.rs' for n in ['a', 'b']])
        (self.tests / 'later.rs').write_text('#[test] fn later() {}\n')
        with self.assertRaisesRegex(ValueError, 'stale'):
            integration_targets(self.root)
        write(self.root)
        old_members = {p: shard for shard, paths in origins.items() for p in paths.values()}
        new_members = {p: shard for shard, paths in shard_origins(self.root).items() for p in paths.values()}
        for p, shard in old_members.items():
            self.assertEqual(new_members[p], shard)

    def test_wrapper_edits_and_missing_originals_cannot_silently_drop_tests(self):
        write(self.root)
        wrapper = self.root / next(iter(shard_origins(self.root)))
        wrapper.write_text(wrapper.read_text().replace('mod a;', 'mod invented;'))
        # Picked wrapper may contain b instead of a.
        wrapper.write_text(wrapper.read_text() + '\n// unrecorded edit\n')
        with self.assertRaisesRegex(ValueError, 'stale'):
            validate(self.root)
        write(self.root)
        (self.tests / 'a.rs').unlink()
        with self.assertRaisesRegex(ValueError, 'stale'):
            validate(self.root)

    def test_only_the_owning_shard_inputs_change_for_a_fixture_edit(self):
        (self.tests / 'a.rs').write_text('include_str!("a.txt");')
        fixture = self.tests / 'a.txt'
        fixture.write_text('before')
        write(self.root)
        origins = shard_origins(self.root)
        owner = next(path for path, members in origins.items() if 'a' in members)
        standalone = 'crates/solver/tests/ordered_destination_regression.rs'
        key = lambda p: digest(rust_test_inputs(p, self.root), self.root)
        before = key(owner), key(standalone)
        fixture.write_text('after')
        self.assertNotEqual(before[0], key(owner))
        self.assertEqual(before[1], key(standalone))

    def read_log(self, text):
        log = self.root / 'test.log'
        log.write_text(text)
        log.with_suffix('.log.json').write_text(json.dumps(dict(
            source_before='verified', source_after='verified', exit_code=0,
            log_sha256=hashlib.sha256(log.read_bytes()).hexdigest())))
        with patch.object(compat_report, 'ROOT', self.root):
            return compat_report.read_test_run(log, 'verified')[0]

    def test_raw_logs_attribute_nested_names_without_inventing_success(self):
        write(self.root)
        owner = next(path for path, members in shard_origins(self.root).items() if 'a' in members)
        target = owner.removeprefix('crates/solver/')
        states = self.read_log(f'Running {target} (target/debug/deps/suite-deadbeef)\n'
                               'test a::nested::same_name ... ok\n'
                               'test a::failure ... FAILED\n'
                               'test a::skip ... ignored\n'
                               'Running unittests examples/other.rs (target/debug/examples/other)\n'
                               'test a::not_a_test ... ok\n'
                               'Running tests/ordered_destination_regression.rs (target/debug/deps/old)\n'
                               'test original ... ok\n')
        self.assertEqual(states, {
            ('crates/solver/tests/a.rs', 'nested::same_name'): 'passed',
            ('crates/solver/tests/a.rs', 'failure'): 'failed',
            ('crates/solver/tests/a.rs', 'skip'): 'unverified',
            ('crates/solver/tests/ordered_destination_regression.rs', 'original'): 'passed',
        })
        with self.assertRaisesRegex(ValueError, 'Unknown test identity'):
            self.read_log(f'Running {target} (target/debug/deps/suite)\ntest unregistered::original ... ok\n')

    def test_original_target_logs_still_resolve_in_managed_and_legacy_layouts(self):
        text = 'Running tests/a.rs (target/debug/deps/a)\ntest nested::original ... ok\n'
        expected = {('crates/solver/tests/a.rs', 'nested::original'): 'passed'}
        self.assertEqual(self.read_log(text), expected)
        write(self.root)
        self.assertEqual(self.read_log(text), expected)


if __name__ == '__main__':
    unittest.main()
