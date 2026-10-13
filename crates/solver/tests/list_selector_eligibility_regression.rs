//! Original prefilter/execution distinction and borrowed lookup regressions.
use std::cell::Cell;
use timetuck::*;
fn span(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
#[test]
fn geometry_checks_are_independent_of_rows_and_strict_execution_stays_strict() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4, 5],
        vec![ListEntity::new(vec![1, 2, 3, 4]), ListEntity::new(vec![5])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let left = span(0, 0, 1);
    let right = span(0, 4, 5);
    assert!(left.selector_swap_is_doable(right));
    assert!(
        p.is_move_doable(
            &a,
            &ListMove::SubListSwap {
                left,
                right,
                reverse: false
            }
        )
        .is_err()
    );
    for (l, r) in [
        (span(0, 0, 0), span(1, 0, 1)),
        (span(0, 2, 1), span(1, 0, 1)),
        (span(0, 0, 2), span(0, 1, 3)),
    ] {
        assert!(!l.selector_swap_is_doable(r));
    }
    assert!(span(0, 0, 2).selector_swap_is_doable(span(0, 2, 3)));
    assert!(span(0, 2, 3).selector_swap_is_doable(span(0, 0, 2)));
    assert_eq!(a, p.initial_assignment());
}
struct NonClone(u32);
#[test]
fn range_queries_borrow_only_arrivals_and_keep_deferred_geometry_short_circuit() {
    let a = ListAssignment::new(vec![
        vec![NonClone(1), NonClone(4), NonClone(2)],
        vec![NonClone(3), NonClone(5)],
    ]);
    let calls = Cell::new(0);
    assert!(
        span(0, 0, 2)
            .selector_swap_is_doable_with_ranges(span(1, 0, 1), &a, |entity, value| {
                calls.set(calls.get() + 1);
                if entity == 0 {
                    [1, 2, 3].contains(&value.0)
                } else {
                    [1, 3, 4, 5].contains(&value.0)
                }
            })
            .unwrap()
    );
    assert_eq!(calls.get(), 3);
    assert!(
        !span(0, 0, 2)
            .selector_swap_is_doable_with_ranges(span(0, 1, 3), &a, |_, _| panic!(
                "overlap must not query ranges"
            ))
            .unwrap()
    );
    assert!(
        span(0, 0, 1)
            .selector_swap_is_doable_with_ranges(span(0, 99, 100), &a, |_, _| panic!(
                "same row has no arrivals"
            ))
            .unwrap()
    );
    assert!(matches!(
        span(0, 0, 4).selector_swap_is_doable_with_ranges(span(1, 0, 1), &a, |_, _| panic!(
            "bounds precede lookup"
        )),
        Err(ListError::InvalidRange { .. })
    ));
    assert!(matches!(
        span(0, 0, 1).selector_swap_is_doable_with_ranges(span(2, 0, 1), &a, |_, _| panic!(
            "entities precede lookup"
        )),
        Err(ListError::EntityIndex { .. })
    ));
}
