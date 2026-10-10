// Adapted from pinned Community selector-based list change/swap execution tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cell::{Cell, RefCell};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn range(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn problem(rows: Vec<Vec<i32>>) -> ListProblem<i32> {
    ListProblem::new(
        rows.iter().flatten().copied().collect(),
        rows.into_iter().map(ListEntity::new).collect(),
    )
    .unwrap()
}
// Mockito verifies individual range calls. Flatten batched native notifications;
// compare exact multiplicities, not an order the source never asserts.
fn assert_ranges(
    actual: &[(ListChangePhase, ListRange)],
    expected: &[(ListChangePhase, ListRange)],
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "unexpected extra range callbacks"
    );
    for item in expected {
        assert_eq!(actual.iter().filter(|e| *e == item).count(), 1, "{item:?}");
    }
}
fn observed(
    rows: Vec<Vec<i32>>,
    movement: ListMove<i32>,
    expected: Vec<Vec<i32>>,
    ranges: &[(ListChangePhase, ListRange)],
) {
    let p = problem(rows);
    let mut a = p.initial_assignment();
    let log = RefCell::new(vec![]);
    let mut listener = |e: ListChangeEvent<'_, i32>| {
        log.borrow_mut()
            .extend(e.ranges.iter().map(|r| (e.phase, *r)));
    };
    p.apply_observed(&mut a, &movement, &mut listener)
        .unwrap()
        .commit();
    assert_eq!(a.lists(), expected);
    assert_ranges(&log.borrow(), ranges);
}
use ListChangePhase::{After, Before};
#[test]
fn change_between_entities() {
    observed(
        vec![vec![1, 2], vec![3]],
        ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 1),
        },
        vec![vec![1], vec![3, 2]],
        &[
            (Before, range(0, 1, 2)),
            (After, range(0, 1, 1)),
            (Before, range(1, 1, 1)),
            (After, range(1, 1, 2)),
        ],
    );
}
// Adapted doAndUndoMoveOnTheSameEntity() MethodSource fixture; -1 sentinels retained.
fn same_entity_cases() -> Vec<(usize, Option<Vec<i32>>, i32, i32)> {
    vec![
        (0, Some(vec![2, 0, 1, 3, 4]), 0, 3),
        (1, Some(vec![0, 2, 1, 3, 4]), 1, 3),
        (2, None, -1, -1),
        (3, Some(vec![0, 1, 3, 2, 4]), 2, 4),
        (4, Some(vec![0, 1, 3, 4, 2]), 2, 5),
        (5, None, -1, -1),
    ]
}
#[test]
fn same_entity_change_positions_and_notification_intervals() {
    for (destination, expected, from, to) in same_entity_cases() {
        let p = problem(vec![vec![0, 1, 2, 3, 4]]);
        let mut a = p.initial_assignment();
        let m = ListMove::Change {
            source: pos(0, 2),
            destination: pos(0, destination),
        };
        let doable = p.is_move_doable(&a, &m) == Ok(true);
        let Some(expected) = expected else {
            assert!(!doable);
            continue;
        };
        assert!(doable);
        let log = RefCell::new(vec![]);
        let mut listener = |e: ListChangeEvent<'_, i32>| {
            log.borrow_mut()
                .extend(e.ranges.iter().map(|r| (e.phase, *r)))
        };
        p.apply_observed(&mut a, &m, &mut listener)
            .unwrap()
            .commit();
        assert_eq!(a.lists()[0].iter().position(|v| *v == 2), Some(destination));
        assert_eq!(a.lists()[0].get(destination), Some(&2));
        assert_eq!(
            a.lists()[0].iter().map(i32::to_string).collect::<Vec<_>>(),
            expected.iter().map(i32::to_string).collect::<Vec<_>>()
        );
        let r = range(0, from as usize, to as usize);
        assert_ranges(&log.borrow(), &[(Before, r), (After, r)]);
    }
}
#[test]
fn temporary_swap_then_same_entity_swap() {
    let p = problem(vec![vec![1, 2], vec![3]]);
    let mut a = p.initial_assignment();
    let log = RefCell::new(vec![]);
    let flushes = Cell::new(0);
    let mut listener = |e: ListLifecycleEvent<'_, i32>| match e.change {
        ListLifecycleChange::Changed(r) => log.borrow_mut().push((e.phase, r)),
        ListLifecycleChange::Flush => flushes.set(flushes.get() + 1),
        _ => {}
    };
    let plan = ListLifecyclePlan::prepare(
        &p,
        &a,
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        },
    )
    .unwrap();
    {
        let edit = plan.apply(&mut a, &mut listener).unwrap();
        assert_eq!(edit.assignment().lists(), &[vec![3, 2], vec![1]]);
        assert_ranges(
            &log.borrow(),
            &[
                (Before, range(0, 0, 1)),
                (After, range(0, 0, 1)),
                (Before, range(1, 0, 1)),
                (After, range(1, 0, 1)),
            ],
        );
        assert!(flushes.get() >= 1);
    }
    let plan = ListLifecyclePlan::prepare(
        &p,
        &a,
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(0, 1),
        },
    )
    .unwrap();
    plan.apply(&mut a, &mut listener).unwrap().commit();
    assert_eq!(a.lists()[0], vec![2, 1]);
}
#[test]
fn sublist_change_between_entities() {
    observed(
        vec![vec![1, 2, 3, 4], vec![5]],
        ListMove::SubListChange {
            entity: 0,
            start: 1,
            end: 3,
            destination: pos(1, 0),
            reverse: false,
        },
        vec![vec![1, 4], vec![2, 3, 5]],
        &[
            (Before, range(0, 1, 3)),
            (After, range(0, 1, 1)),
            (Before, range(1, 0, 0)),
            (After, range(1, 0, 2)),
        ],
    );
}
#[test]
fn sublist_change_reversed() {
    observed(
        vec![vec![1, 2, 3, 4], vec![5]],
        ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 3,
            destination: pos(1, 1),
            reverse: true,
        },
        vec![vec![4], vec![5, 3, 2, 1]],
        &[
            (Before, range(0, 0, 3)),
            (After, range(0, 0, 0)),
            (Before, range(1, 1, 1)),
            (After, range(1, 1, 4)),
        ],
    );
}
#[test]
fn sublist_change_same_entity() {
    observed(
        vec![vec![1, 2, 3, 4, 5, 6]],
        ListMove::SubListChange {
            entity: 0,
            start: 3,
            end: 5,
            destination: pos(0, 0),
            reverse: false,
        },
        vec![vec![4, 5, 1, 2, 3, 6]],
        &[(Before, range(0, 0, 5)), (After, range(0, 0, 5))],
    );
}
#[test]
fn sublist_swap_unequal_lengths() {
    observed(
        vec![vec![1, 2, 3, 4], vec![5]],
        ListMove::SubListSwap {
            left: range(0, 1, 3),
            right: range(1, 0, 1),
            reverse: false,
        },
        vec![vec![1, 5, 4], vec![2, 3]],
        &[
            (Before, range(0, 1, 3)),
            (After, range(0, 1, 2)),
            (Before, range(1, 0, 1)),
            (After, range(1, 0, 2)),
        ],
    );
}
#[test]
fn sublist_swap_reversed() {
    observed(
        vec![vec![1, 2, 3, 4], vec![5, 6]],
        ListMove::SubListSwap {
            left: range(0, 0, 3),
            right: range(1, 0, 2),
            reverse: true,
        },
        vec![vec![6, 5, 4], vec![3, 2, 1]],
        &[
            (Before, range(0, 0, 3)),
            (After, range(0, 0, 2)),
            (Before, range(1, 0, 2)),
            (After, range(1, 0, 3)),
        ],
    );
}
#[test]
fn sublist_swap_same_entity() {
    observed(
        vec![vec![1, 2, 3, 4, 5, 6, 7]],
        ListMove::SubListSwap {
            left: range(0, 0, 1),
            right: range(0, 4, 7),
            reverse: false,
        },
        vec![vec![5, 6, 7, 2, 3, 4, 1]],
        &[(Before, range(0, 0, 7)), (After, range(0, 0, 7))],
    );
}
