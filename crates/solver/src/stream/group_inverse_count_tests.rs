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
