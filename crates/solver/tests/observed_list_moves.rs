//! Original observed transaction lifecycle and panic guarantees.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn movement() -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 1,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])],
    )
    .unwrap()
}
#[test]
fn events_expose_whole_snapshots_and_commit_has_no_undo_notifications() {
    let p = problem();
    let mut a = p.initial_assignment();
    let original = a.clone();
    let mut events = vec![];
    let mut observer = |e: ListChangeEvent<'_, i32>| {
        events.push((e.phase, e.undo, e.assignment.clone(), e.ranges.to_vec()))
    };
    p.apply_observed(&mut a, &movement(), &mut observer)
        .unwrap()
        .commit();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].2, original);
    assert_eq!(events[1].2, a);
    assert_eq!(
        events[0].3,
        vec![
            ListRange {
                entity: 0,
                start: 1,
                end: 2
            },
            ListRange {
                entity: 1,
                start: 0,
                end: 0
            }
        ]
    );
    assert_eq!(
        events[1].3,
        vec![
            ListRange {
                entity: 0,
                start: 1,
                end: 1
            },
            ListRange {
                entity: 1,
                start: 0,
                end: 1
            }
        ]
    );
    let mut called = 0;
    let mut observer = |_: ListChangeEvent<'_, i32>| called += 1;
    assert!(
        p.apply_observed(
            &mut a,
            &ListMove::Assign {
                value: 0,
                destination: ListPosition {
                    entity: 0,
                    index: 0
                }
            },
            &mut observer
        )
        .is_err()
    );
    p.apply_observed(&mut a, &ListMove::Composite(vec![]), &mut observer)
        .unwrap()
        .rollback();
    assert_eq!(called, 0);
}
#[test]
fn observer_panics_at_each_boundary_restore_assignments() {
    for fail_at in 0..4 {
        let p = problem();
        let mut a = p.initial_assignment();
        let original = a.clone();
        let calls = Cell::new(0);
        let mut observer = |_: ListChangeEvent<'_, i32>| {
            let n = calls.get();
            calls.set(n + 1);
            assert_ne!(n, fail_at, "observer panic");
        };
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _edit = p
                    .apply_observed(&mut a, &movement(), &mut observer)
                    .unwrap();
            }))
            .is_err()
        );
        assert_eq!(a, original);
        assert_eq!(calls.get(), fail_at + 1);
    }
    let p = problem();
    let mut a = p.initial_assignment();
    let original = a.clone();
    let calls = Cell::new(0);
    let mut observer = |_: ListChangeEvent<'_, i32>| calls.set(calls.get() + 1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _edit = p
                .apply_observed(&mut a, &movement(), &mut observer)
                .unwrap();
            panic!("unrelated panic");
        }))
        .is_err()
    );
    assert_eq!(a, original);
    assert_eq!(calls.get(), 2);
}
#[test]
fn pin_and_dependency_failures_publish_no_events() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0, 1]).with_pinned_prefix(1)],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let mut calls = 0;
    let mut observer = |_: ListChangeEvent<'_, i32>| calls += 1;
    assert!(matches!(
        p.relocate_observed(
            &mut a,
            ListPosition {
                entity: 0,
                index: 0
            },
            ListPosition {
                entity: 0,
                index: 1
            },
            &mut observer
        ),
        Err(ListError::PinChanged { .. })
    ));
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    let mut a = p.initial_assignment();
    let original = a.clone();
    assert!(matches!(
        p.relocate_observed(
            &mut a,
            ListPosition {
                entity: 0,
                index: 0
            },
            ListPosition {
                entity: 0,
                index: 1
            },
            &mut observer
        ),
        Err(ListError::DependencyCycle { .. })
    ));
    assert_eq!(calls, 0);
    assert_eq!(a, original);
}
