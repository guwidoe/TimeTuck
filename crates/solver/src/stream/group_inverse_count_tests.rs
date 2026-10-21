use super::*;
use crate::primitive_rows::group_collectors::{GroupCollectors, ScalarEdit};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn inverse_collector_count_overflow_precedes_mutation_and_undo_publication() {
    let mut rows = GroupCollectors::new();
    assert!(rows.bind_primitive());
    rows.insert(
        0usize,
        IntCollector {
            values: IntCollectorCounts::Inline {
                entries: [(7, usize::MAX), (0, 0)],
                len: 1,
            },
            count: usize::MAX,
            sum: 7i64.wrapping_mul(usize::MAX as i64),
        },
    );
    let before = rows.get(&0).unwrap().summary();
    let owner = rows.get(&0).unwrap() as *const IntCollector;
    let journal = rows.begin_journal();
    assert!(catch_unwind(AssertUnwindSafe(|| rows.try_edit(&0, ScalarEdit::Insert(7)))).is_err());
    assert_eq!(rows.get(&0).unwrap().summary(), before);
    assert_eq!(rows.get(&0).unwrap() as *const _, owner);
    rows.rollback_journal(journal);
    assert_eq!(rows.get(&0).unwrap().summary(), before);
    assert_eq!(rows.get(&0).unwrap() as *const _, owner);
}

#[test]
fn inverse_collector_wrapping_sum_and_tree_inline_transition_match_full_versions() {
    let mut rows = GroupCollectors::new();
    assert!(rows.bind_primitive());
    let values = [i64::MAX, i64::MIN];
    rows.insert(usize::MAX, values.into_iter().collect());
    let outer = rows.begin_journal();
    rows.try_edit(&usize::MAX, ScalarEdit::Insert(7)).unwrap();
    let full = rows.clone();
    rows.try_edit(&usize::MAX, ScalarEdit::Replace(i64::MIN, i64::MAX))
        .unwrap();
    assert_eq!(
        rows.get(&usize::MAX).unwrap().summary(),
        [i64::MAX, i64::MAX, 7]
            .into_iter()
            .collect::<IntCollector>()
            .summary()
    );
    rows.rollback_journal(outer);
    assert_eq!(
        rows.get(&usize::MAX).unwrap().summary(),
        values.into_iter().collect::<IntCollector>().summary()
    );
    assert_eq!(
        full.get(&usize::MAX).unwrap().summary(),
        [i64::MAX, i64::MIN, 7]
            .into_iter()
            .collect::<IntCollector>()
            .summary()
    );
}

#[test]
fn group_static_mapper_keeps_count_overflow_and_shared_weak_inverse_fallback_phases() {
    use crate::primitive_rows::{Exact, LookupPolicy, Stored};
    use std::sync::Arc;
    fn run<P: LookupPolicy<usize>>() {
        let mut rows = GroupCollectors::new();
        assert!(rows.bind_primitive());
        rows.insert(
            usize::MAX,
            IntCollector {
                values: IntCollectorCounts::Inline {
                    entries: [(7, usize::MAX), (0, 0)],
                    len: 1,
                },
                count: usize::MAX,
                sum: 7i64.wrapping_mul(usize::MAX as i64),
            },
        );
        let before = rows.get(&usize::MAX).unwrap().summary();
        let owner = rows.get(&usize::MAX).unwrap() as *const IntCollector;
        let outer = rows.begin_journal();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || rows.try_edit_with_policy::<P>(&usize::MAX, ScalarEdit::Insert(7),)
            ))
            .is_err()
        );
        assert_eq!(rows.get(&usize::MAX).unwrap().summary(), before);
        assert_eq!(rows.get(&usize::MAX).unwrap() as *const _, owner);
        rows.rollback_journal(outer);
        assert_eq!(rows.get(&usize::MAX).unwrap().summary(), before);
        rows.insert(usize::MAX, [1, 2].into_iter().collect());
        let held = rows.shared_entries();
        let weak = Arc::downgrade(&held[0].1);
        let saved = rows.get(&usize::MAX).unwrap().summary();
        let outer = rows.begin_journal();
        assert!(
            rows.try_edit_with_policy::<P>(&usize::MAX, ScalarEdit::Insert(3))
                .is_none()
        );
        drop(held);
        assert!(
            rows.try_edit_with_policy::<P>(&usize::MAX, ScalarEdit::Insert(3))
                .is_none()
        );
        assert_eq!(rows.get(&usize::MAX).unwrap().summary(), saved);
        let owner = rows.get(&usize::MAX).unwrap() as *const IntCollector;
        drop(weak);
        assert!(
            rows.try_edit_with_policy::<P>(&usize::MAX, ScalarEdit::Insert(3))
                .is_some()
        );
        assert_eq!(rows.get(&usize::MAX).unwrap() as *const _, owner);
        rows.rollback_journal(outer);
        assert_eq!(rows.get(&usize::MAX).unwrap().summary(), saved);
    }
    run::<Exact>();
    run::<Stored>();
}
