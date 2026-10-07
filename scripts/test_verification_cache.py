"""Selective invalidation and actual subprocess reuse, without running solvers."""
import json
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from verification_cache import run_job, run_measurement
from verification_inputs import digest, measurement_digest, rust_test_inputs, report_digest, doctest_inputs
from verification_inputs import library_symbol_owners, dependency_words


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

    def test_dependency_parsing_is_shared_across_targets_without_hiding_edits(self):
        library_symbol_owners.cache_clear()
        dependency_words.cache_clear()
        a, b = 'crates/solver/tests/a.rs', 'crates/solver/tests/b.rs'
        for entry in [a, b]:
            (self.root / entry).write_text('use timetuck::score::Score;')
        before = self.key(a), self.key(b)
        self.assertEqual(library_symbol_owners.cache_info().misses, 1)
        self.assertEqual(library_symbol_owners.cache_info().hits, 1)
        self.assertGreater(dependency_words.cache_info().hits, 0)
        # Same-length edit with the original mtime: contents, not metadata, win.
        score = self.root / 'crates/solver/src/score.rs'
        stamp = score.stat()
        score.write_text('pub struct Other;')
        import os
        os.utime(score, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
        self.assertNotEqual(before, (self.key(a), self.key(b)))
        self.assertEqual(library_symbol_owners.cache_info().misses, 2)

    def test_cached_index_rediscovers_modules_and_out_of_line_implementations(self):
        a, b = 'crates/solver/tests/a.rs', 'crates/solver/tests/b.rs'
        (self.root / a).write_text('use timetuck::Score;')
        (self.root / b).write_text('use timetuck::search::Search;')
        (self.root / 'crates/solver/src/lib.rs').write_text(
            'pub mod score; pub mod search; mod extension; pub use score::Score;')
        (self.root / 'crates/solver/src/search.rs').write_text(
            'pub struct Search;\npub fn compute() {}')
        before = self.key(a), self.key(b)
        extension = self.root / 'crates/solver/src/extension.rs'
        extension.write_text('use crate::*;\nimpl Score { fn probe() { compute(); } }')
        self.assertNotEqual(before[0], self.key(a))
        self.assertEqual(before[1], self.key(b))
        self.assertIn('@rust-library:crates/solver/src/search.rs', rust_test_inputs(a, self.root))
        extension.write_text('use crate::*;\nimpl Other { fn probe() { compute(); } }')
        self.assertEqual(before[0], self.key(a))
        extension.unlink()
        self.assertEqual(before, (self.key(a), self.key(b)))

    def test_cached_dependency_words_follow_changed_target_and_library_calls(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::*; fn probe() { compute(); }')
        (self.root / 'crates/solver/src/search.rs').write_text('pub fn compute() {}')
        before = self.key(entry)
        (self.root / 'crates/solver/src/search.rs').write_text(
            'use crate::Score;\npub fn compute() { let _ = Score; }')
        self.assertNotEqual(before, self.key(entry))
        self.assertIn('@rust-library:crates/solver/src/score.rs', rust_test_inputs(entry, self.root))
        (self.root / entry).write_text('use timetuck::*; use std::error::Error; fn probe(_: Score) {}')
        self.assertNotIn('@rust-library:crates/solver/src/search.rs', rust_test_inputs(entry, self.root))

    def test_local_extension_impls_do_not_invalidate_plain_model_users(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::*; use std::error::Error; fn probe(_: Score) {}')
        facade = self.root / 'crates/solver/src/lib.rs'
        facade.write_text('pub mod score; pub mod search; pub use score::Score;')
        before = self.key(entry)
        facade.write_text(facade.read_text() +
                          ' mod editing; pub use editing::DraftModel;')
        extension = self.root / 'crates/solver/src/editing.rs'
        extension.write_text('use crate::Score;\npub trait DraftModel { fn draft(&self); type Error; }\n'
                             'impl DraftModel for Score { fn draft(&self) {} }')
        self.assertEqual(before, self.key(entry))
        selected = '@rust-library:crates/solver/src/editing.rs'
        self.assertNotIn(selected, rust_test_inputs(entry, self.root))
        for caller in ['use timetuck::DraftModel; fn run<T: DraftModel>() {}',
                       'use timetuck::*; fn run(s: Score) { s.draft(); }',
                       'use timetuck::*; fn run() { let f = Score::draft; }']:
            (self.root / entry).write_text(caller)
            self.assertIn(selected, rust_test_inputs(entry, self.root))
            key = self.key(entry)
            extension.write_text(extension.read_text().replace('fn draft(&self) {}',
                                                             'fn draft(&self) { panic!("changed"); }'))
            self.assertNotEqual(key, self.key(entry))
            extension.write_text(extension.read_text().replace('fn draft(&self) { panic!("changed"); }',
                                                             'fn draft(&self) {}'))

    def test_trait_defaults_associated_items_and_qualified_impls_are_dependencies(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / 'crates/solver/src/lib.rs').write_text(
            'pub mod score; mod extension; mod traits; pub use score::Score; '
            'pub use traits::DraftModel as Drafting;')
        trait = self.root / 'crates/solver/src/traits.rs'
        trait.write_text('pub trait DraftModel { fn draft(&self) { if true {} } '
                         'type Snapshot; const LIMIT: usize = 4; }')
        extension = self.root / 'crates/solver/src/extension.rs'
        extension.write_text('use crate::Score;\nimpl crate::traits::DraftModel for Score '
                             '{ type Snapshot = Score; }')
        selected = '@rust-library:crates/solver/src/extension.rs'
        for access in ['s.draft()', 'Score::draft(&s)', 'Score::LIMIT',
                       '<Score as Drafting>::Snapshot']:
            (self.root / entry).write_text('use timetuck::*; fn run(s: Score) { ' + access + '; }')
            inputs = rust_test_inputs(entry, self.root)
            self.assertIn(selected, inputs)
            self.assertIn('@rust-library:crates/solver/src/traits.rs', inputs)
            before = self.key(entry)
            trait.write_text(trait.read_text().replace('= 4;', '= 5;'))
            self.assertNotEqual(before, self.key(entry))
            trait.write_text(trait.read_text().replace('= 5;', '= 4;'))

    def test_foreign_traits_and_inherent_extensions_still_follow_the_type(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::score::Score;')
        (self.root / 'crates/solver/src/lib.rs').write_text(
            'pub mod score; pub mod search; mod extension;')
        extension = self.root / 'crates/solver/src/extension.rs'
        before = self.key(entry)
        for header in ['impl Clone for Score', 'impl std::fmt::Display for Score',
                       'impl<T: Clone> From<T> for Score', 'impl Score']:
            extension.write_text(header + ' {}')
            self.assertNotEqual(before, self.key(entry))
            self.assertIn('@rust-library:crates/solver/src/extension.rs',
                          rust_test_inputs(entry, self.root))

    def test_trait_member_names_are_scoped_to_trait_bodies_and_updated(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::*; fn probe(s: Score) { s.unrelated(); }')
        (self.root / 'crates/solver/src/lib.rs').write_text(
            'pub mod score; mod traits; mod extension;')
        trait = self.root / 'crates/solver/src/traits.rs'
        trait.write_text('pub trait First { fn draft(&self) { if true {} } }\n'
                         'pub trait Second { fn unrelated(&self); }')
        (self.root / 'crates/solver/src/extension.rs').write_text(
            'impl First for Score { fn draft(&self) {} }')
        selected = '@rust-library:crates/solver/src/extension.rs'
        self.assertNotIn(selected, rust_test_inputs(entry, self.root))
        trait.write_text('pub trait First { fn unrelated(&self); }')
        self.assertIn(selected, rust_test_inputs(entry, self.root))

    def test_trait_methods_require_receivers_and_follow_factory_dependencies(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::*; fn run(s: Search) { s.draft(); }')
        (self.root / 'crates/solver/src/lib.rs').write_text(
            'pub mod score; pub mod search; mod traits; mod extension;')
        (self.root / 'crates/solver/src/traits.rs').write_text(
            'pub trait DraftModel { fn draft(&self); }')
        (self.root / 'crates/solver/src/extension.rs').write_text(
            'impl DraftModel for Score { fn draft(&self) {} }')
        selected = '@rust-library:crates/solver/src/extension.rs'
        self.assertNotIn(selected, rust_test_inputs(entry, self.root))
        (self.root / 'crates/solver/src/search.rs').write_text(
            'pub struct Search;\npub fn make() -> crate::Score { todo!() }')
        (self.root / entry).write_text('use timetuck::*; fn run() { make().draft(); }')
        self.assertIn(selected, rust_test_inputs(entry, self.root))

    def test_blanket_traits_and_internal_traits_keep_required_implementations(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / 'crates/solver/src/lib.rs').write_text(
            'pub mod score; pub mod search; mod traits; mod extension;')
        traits = self.root / 'crates/solver/src/traits.rs'
        traits.write_text('pub trait DraftModel { fn draft(&self); }')
        extension = self.root / 'crates/solver/src/extension.rs'
        extension.write_text('impl<T: Clone> DraftModel for T { fn draft(&self) {} }')
        selected = '@rust-library:crates/solver/src/extension.rs'
        for caller in ['use timetuck::*; fn run(s: Search) { s.draft(); }',
                       'use timetuck::DraftModel; fn run<T: DraftModel>(_: T) {}']:
            (self.root / entry).write_text(caller)
            self.assertIn(selected, rust_test_inputs(entry, self.root))
        extension.write_text('impl<T, U> DraftModel<U> for T { fn draft(&self) {} }')
        traits.write_text('pub trait DraftModel<U=()> { fn draft(&self); }')
        self.assertIn(selected, rust_test_inputs(entry, self.root))
        traits.write_text('pub(crate) trait DraftModel { fn draft(&self); }')
        extension.write_text('impl DraftModel for Score { fn draft(&self) {} }')
        (self.root / entry).write_text('use timetuck::Score;')
        self.assertIn(selected, rust_test_inputs(entry, self.root))

    def test_generic_box_forwarders_do_not_connect_unrelated_method_names(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / 'crates/solver/src/lib.rs').write_text(
            'pub mod score; mod traits; mod extension; mod factory;')
        (self.root / 'crates/solver/src/score.rs').write_text(
            'pub struct Score; impl Score { pub fn size(&self) -> usize { 3 } }')
        (self.root / 'crates/solver/src/traits.rs').write_text(
            'pub trait DraftModel { fn size(&self) -> usize; }')
        extension = self.root / 'crates/solver/src/extension.rs'
        selected = '@rust-library:crates/solver/src/extension.rs'
        for wrapper in ['Box', 'std::boxed::Box', 'Rc', 'Arc']:
            extension.write_text('impl<T: DraftModel> DraftModel for ' + wrapper +
                                 '<T> { fn size(&self) -> usize { (**self).size() } }')
            (self.root / entry).write_text(
                'use timetuck::*; fn run(s: ' + wrapper + '<Score>) { s.size(); }')
            before = self.key(entry)
            self.assertNotIn(selected, rust_test_inputs(entry, self.root))
            extension.write_text(extension.read_text().replace('(**self).size()', '4'))
            self.assertEqual(before, self.key(entry))
            # Trait objects name the dependency even when invoked through a glob.
            (self.root / entry).write_text(
                'use timetuck::*; fn run(s: ' + wrapper + '<dyn DraftModel>) { s.size(); }')
            self.assertIn(selected, rust_test_inputs(entry, self.root))
            before = self.key(entry)
            extension.write_text(extension.read_text().replace('4', '5'))
            self.assertNotEqual(before, self.key(entry))
            # Inferred factory results also expose the trait dependency.
            (self.root / 'crates/solver/src/factory.rs').write_text(
                'pub fn make() -> ' + wrapper + '<dyn DraftModel> { todo!() }')
            (self.root / entry).write_text('use timetuck::*; fn run() { make().size(); }')
            self.assertIn(selected, rust_test_inputs(entry, self.root))
        # A concrete wrapper implementation is not a generic forwarder.
        extension.write_text('impl DraftModel for Box<Score> { fn size(&self) -> usize { 1 } }')
        (self.root / entry).write_text('use timetuck::*; fn run(s: Box<Score>) { s.size(); }')
        self.assertIn(selected, rust_test_inputs(entry, self.root))
        # A universal implementation adds methods to unrelated payloads too.
        extension.write_text('impl<T> DraftModel for Box<T> { fn size(&self) -> usize { 1 } }')
        (self.root / entry).write_text('use timetuck::*; fn run(s: Box<i32>) { s.size(); }')
        self.assertIn(selected, rust_test_inputs(entry, self.root))

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
        (self.root / entry).write_text('use timetuck::*; use std::error::Error; fn probe(_: Score) {}')
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

    def test_path_unit_modules_and_support_only_invalidate_library_tests(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::score::Score;')
        score = self.root / 'crates/solver/src/score.rs'
        score.write_text('pub struct Score; #[path="probe.rs"] #[cfg(test)] mod checks;')
        probe = self.root / 'crates/solver/src/probe.rs'
        probe.write_text('use crate::search::Search; use crate::score::Score; impl Score { fn check(_: Search) {} } #[path="support.rs"] mod support;')
        support = self.root / 'crates/solver/src/support.rs'
        support.write_text('pub fn fixture() {}')
        def keys():
            return (self.key(entry), measurement_digest('performance', self.root),
                    self.key('crates/solver/src/lib.rs'))
        before = keys()
        for path in [probe, support]:
            path.write_text(path.read_text() + ' // changed fixture')
            after = keys()
            self.assertEqual(before[:2], after[:2])
            self.assertNotEqual(before[2], after[2])
            before = after
        # Runtime dependencies still invalidate consumers.
        score.write_text('pub struct Score { pub field: i32 } #[path="probe.rs"] #[cfg(test)] mod checks;')
        self.assertNotEqual(before[0], self.key(entry))
        self.assertNotEqual(before[1], measurement_digest('performance', self.root))

    def test_inline_unit_items_and_helpers_do_not_change_production_keys(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::score::Score;')
        score = self.root / 'crates/solver/src/score.rs'
        before = self.key(entry)
        score.write_text(score.read_text() + '''
            #[cfg(test)] mod checks {
                use crate::search::Search;
                fn fixture() { let braces = r##"} { # [cfg(test)]"##; }
            }
            #[cfg(test)] fn helper<T>(f: impl Fn(T) -> T) -> T { panic!("}") }
        ''')
        self.assertEqual(before, self.key(entry))
        unit = self.key('crates/solver/src/lib.rs')
        (self.root / 'crates/solver/src/search.rs').write_text('pub struct Search { value: i64 }')
        self.assertEqual(before, self.key(entry))
        self.assertNotEqual(unit, self.key('crates/solver/src/lib.rs'))
        score.write_text('pub struct Score; pub const TEXT: &str = r#"a  b"#;')
        before = self.key(entry)
        score.write_text('pub struct Score; pub const TEXT: &str = r#"a b"#;')
        self.assertNotEqual(before, self.key(entry))

    def test_test_target_cfg_modules_and_shared_runtime_support_remain_inputs(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('#[cfg(test)] mod checks { use timetuck::search::Search; }')
        before = self.key(entry)
        (self.root / 'crates/solver/src/search.rs').write_text('pub struct Search { n: i32 }')
        self.assertNotEqual(before, self.key(entry))
        (self.root / entry).write_text('use timetuck::score::Score;')
        (self.root / 'crates/solver/src/score.rs').write_text(
            'pub struct Score; #[cfg(test)] #[path="support.rs"] mod checks; #[path="support.rs"] mod runtime;')
        support = self.root / 'crates/solver/src/support.rs'
        support.write_text('pub fn value() -> i32 { 1 }')
        before = self.key(entry)
        support.write_text('pub fn value() -> i32 { 2 }')
        self.assertNotEqual(before, self.key(entry))

    def test_production_include_files_and_non_test_conditions_remain_inputs(self):
        entry = 'crates/solver/tests/a.rs'
        (self.root / entry).write_text('use timetuck::score::Score;')
        score = self.root / 'crates/solver/src/score.rs'
        score.write_text('pub struct Score; const INPUT: &str = include_str!("data.txt"); #[cfg(feature="x")] #[path="support.rs"] mod support;')
        data = self.root / 'crates/solver/src/data.txt'
        data.write_text('a')
        support = self.root / 'crates/solver/src/support.rs'
        support.write_text('pub fn helper() {}')
        for path in [data, support]:
            before = self.key(entry)
            path.write_text(path.read_text() + ' changed')
            self.assertNotEqual(before, self.key(entry))

    def test_doctests_keep_documentation_but_ignore_library_unit_fixtures(self):
        score = self.root / 'crates/solver/src/score.rs'
        score.write_text('/// Score documentation.\npub struct Score; #[cfg(test)] #[path="probe.rs"] mod checks;')
        probe = self.root / 'crates/solver/src/probe.rs'
        probe.write_text('fn check() {}')
        def key(): return digest(doctest_inputs(self.root), self.root)
        before = key()
        probe.write_text('fn changed_check() {}')
        self.assertEqual(before, key())
        score.write_text(score.read_text().replace('Score documentation.', 'Updated documentation.'))
        self.assertNotEqual(before, key())



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
