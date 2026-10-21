//! Original overflow/publication boundary regression for private boxed values.
use super::*;
use crate::primitive_rows::{Exact, group_collectors::GroupCollectors};
use std::panic::{AssertUnwindSafe, catch_unwind};
#[test]
fn owned_collector_box_count_overflow_preserves_value_owner_and_rollback() {
    let mut rows = GroupCollectors::<usize>::new();
    assert!(rows.bind_primitive());
    rows.enable_consumed_owned();
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
    let next = (usize::MAX, 7);
    assert!(
        catch_unwind(AssertUnwindSafe(|| rows.apply_owned::<Exact>(
            None,
            Some(&next),
            Some(&usize::MAX),
            false
        )))
        .is_err()
    );
    assert_eq!(rows.get(&usize::MAX).unwrap().summary(), before);
    assert_eq!(rows.get(&usize::MAX).unwrap() as *const _, owner);
    rows.rollback_journal(outer);
    assert_eq!(rows.get(&usize::MAX).unwrap().summary(), before);
    assert_eq!(rows.get(&usize::MAX).unwrap() as *const _, owner);
}
