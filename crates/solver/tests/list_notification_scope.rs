use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        (0..7).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4, 5]),
            ListEntity::new(vec![6]).with_pinned(true),
        ],
    )
    .unwrap()
}
fn movement() -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 2,
        },
        destination: ListPosition {
            entity: 1,
            index: 1,
        },
    }
}
fn range(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
#[test]
fn suffix_brackets_track_old_and_new_lengths_and_reverse_on_undo() {
    let p = problem();
    let mut a = p.initial_assignment();
    let original = a.clone();
    let plan = ListLifecyclePlan::prepare_with_scope(
        &p,
        &a,
        movement(),
        ListNotificationScope::MovableSuffix,
    )
    .unwrap();
    let after = vec![vec![0, 1, 3], vec![4, 2, 5], vec![6]];
    let mut events = vec![];
    let mut flushes = vec![];
    {
        let mut listener = |e: ListLifecycleEvent<'_, i32>| match e.change {
            ListLifecycleChange::Changed(r) => {
                let at_old = (e.phase == ListChangePhase::Before) != e.undo;
                assert_eq!(
                    e.assignment.lists(),
                    if at_old { original.lists() } else { &after }
                );
                events.push((e.undo, e.phase, r));
            }
            ListLifecycleChange::Flush => flushes.push(e.undo),
            _ => panic!("no assignment membership change"),
        };
        let trial = plan.apply(&mut a, &mut listener).unwrap();
        assert_eq!(trial.assignment().lists(), after);
    }
    assert_eq!(a, original);
    use ListChangePhase::{After, Before};
    assert_eq!(
        events,
        [
            (false, Before, range(0, 1, 4)),
            (false, Before, range(1, 0, 2)),
            (false, After, range(1, 0, 3)),
            (false, After, range(0, 1, 3)),
            (true, Before, range(0, 1, 3)),
            (true, Before, range(1, 0, 3)),
            (true, After, range(1, 0, 2)),
            (true, After, range(0, 1, 4))
        ]
    );
    assert_eq!(flushes, [false, true]);
}
#[test]
fn default_keeps_minimal_ranges_and_noop_has_no_notifications() {
    let p = problem();
    let mut a = p.initial_assignment();
    let plan = ListLifecyclePlan::prepare(&p, &a, movement()).unwrap();
    let mut ranges = vec![];
    plan.apply(&mut a, &mut |e: ListLifecycleEvent<'_, i32>| {
        if let ListLifecycleChange::Changed(r) = e.change {
            ranges.push((e.phase, r));
        }
    })
    .unwrap()
    .commit();
    use ListChangePhase::{After, Before};
    assert_eq!(
        ranges,
        [
            (Before, range(0, 2, 3)),
            (Before, range(1, 1, 1)),
            (After, range(1, 1, 2)),
            (After, range(0, 2, 2))
        ]
    );
    let noop = ListLifecyclePlan::prepare_with_scope(
        &p,
        &a,
        ListMove::Composite(vec![]),
        ListNotificationScope::MovableSuffix,
    )
    .unwrap();
    noop.apply(&mut a, &mut |_| panic!("no-op must not notify"))
        .unwrap()
        .commit();
}
#[test]
fn suffix_scope_retains_stale_pin_and_observer_panic_protection() {
    let p = problem();
    let mut a = p.initial_assignment();
    let original = a.clone();
    let plan = ListLifecyclePlan::prepare_with_scope(
        &p,
        &a,
        movement(),
        ListNotificationScope::MovableSuffix,
    )
    .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut listener = |e: ListLifecycleEvent<'_, i32>| {
                if e.phase == ListChangePhase::After {
                    panic!("listener")
                }
            };
            let _trial = plan.apply(&mut a, &mut listener).unwrap();
        }))
        .is_err()
    );
    assert_eq!(a, original);
    p.apply(&mut a, &movement()).unwrap().commit();
    assert!(matches!(
        plan.apply(&mut a, &mut |_| panic!("stale must not notify")),
        Err(StaleListPlan)
    ));
    let invalid = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    assert!(
        ListLifecyclePlan::prepare_with_scope(
            &p,
            &a,
            invalid,
            ListNotificationScope::MovableSuffix
        )
        .is_err()
    );
}

#[test]
fn edge_order_selects_direction_without_an_expected_output_argument() {
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap();
    let original = p.initial_assignment();
    let added = [(0, 2), (1, 4), (3, 5)];
    for (removed, expected) in [
        ([(5, 0), (1, 2), (3, 4)], vec![0, 1, 4, 5, 3, 2]),
        ([(0, 5), (1, 2), (3, 4)], vec![0, 2, 3, 5, 4, 1]),
    ] {
        let mut a = original.clone();
        let m = list_k_opt_edge_move(&p, &a, 0, &removed, &added).unwrap();
        assert_eq!(p.apply(&mut a, &m).unwrap().lists()[0], expected);
        assert_eq!(a, original);
    }
    assert!(list_k_opt_edge_move(&p, &original, 0, &[], &[]).is_err());
    assert!(list_k_opt_edge_move(&p, &original, 0, &[(8, 0), (1, 2)], &[(8, 2), (0, 1)]).is_err());
    assert!(list_k_opt_edge_move(&p, &original, 0, &[(5, 0), (1, 2)], &[(1, 3), (0, 2)]).is_err());
    assert!(list_k_opt_edge_move(&p, &original, 99, &[(5, 0), (1, 2)], &added).is_err());
}
