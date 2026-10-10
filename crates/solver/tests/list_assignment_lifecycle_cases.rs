// Adapted from pinned Community SelectorBasedListAssignMoveTest and
// SelectorBasedListUnassignMoveTest. Apache-2.0; see compat/provenance.json
// and THIRD_PARTY_NOTICES.md. Native event counters replace Mockito verification.
use std::cell::RefCell;
use timetuck::*;
#[derive(Debug, PartialEq, Eq)]
enum Event {
    Assigned(i32),
    Unassigned(i32),
    Range(ListRange),
    Flush,
}
fn event(e: ListLifecycleEvent<'_, i32>) -> (ListChangePhase, Event) {
    let kind = match e.change {
        ListLifecycleChange::Assigned(v) => Event::Assigned(*v),
        ListLifecycleChange::Unassigned(v) => Event::Unassigned(*v),
        ListLifecycleChange::Changed(r) => Event::Range(r),
        ListLifecycleChange::Flush => Event::Flush,
    };
    (e.phase, kind)
}
#[test]
fn assign_execution_and_temporary_notifications() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![])]).unwrap();
    let mut a = p.initial_assignment();
    let log = RefCell::new(vec![]);
    let mut listener = |e: ListLifecycleEvent<'_, i32>| log.borrow_mut().push(event(e));
    let plan = ListLifecyclePlan::prepare(
        &p,
        &a,
        ListMove::Assign {
            value: 1,
            destination: ListPosition {
                entity: 0,
                index: 0,
            },
        },
    )
    .unwrap();
    {
        let edit = plan.apply(&mut a, &mut listener).unwrap();
        assert_eq!(edit.assignment().lists(), &[vec![1]]);
        for expected in [
            (
                ListChangePhase::Before,
                Event::Range(ListRange {
                    entity: 0,
                    start: 0,
                    end: 0,
                }),
            ),
            (ListChangePhase::Before, Event::Assigned(1)),
            (ListChangePhase::After, Event::Assigned(1)),
            (
                ListChangePhase::After,
                Event::Range(ListRange {
                    entity: 0,
                    start: 0,
                    end: 1,
                }),
            ),
        ] {
            assert_eq!(log.borrow().iter().filter(|e| **e == expected).count(), 1);
        }
        assert!(
            log.borrow()
                .iter()
                .filter(|(_, e)| *e == Event::Flush)
                .count()
                >= 1
        );
    }
    for (value, index) in [(2, 0), (3, 1), (1, 0)] {
        let plan = ListLifecyclePlan::prepare(
            &p,
            &a,
            ListMove::Assign {
                value,
                destination: ListPosition { entity: 0, index },
            },
        )
        .unwrap();
        plan.apply(&mut a, &mut listener).unwrap().commit();
    }
    assert_eq!(a.lists(), &[vec![1, 2, 3]]);
}
#[test]
fn assignment_entity_ranges() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![1, 3]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let checker = p.move_checker(&a).unwrap();
    assert!(
        checker
            .is_doable(&ListMove::Assign {
                value: 1,
                destination: ListPosition {
                    entity: 0,
                    index: 0
                }
            })
            .unwrap()
    );
    assert!(
        checker
            .is_doable(&ListMove::Assign {
                value: 1,
                destination: ListPosition {
                    entity: 1,
                    index: 0
                }
            })
            .unwrap()
    );
}
#[test]
fn unassign_last_reports_element_removal() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])]).unwrap();
    let mut a = p.initial_assignment();
    let log = RefCell::new(vec![]);
    let mut listener = |e: ListLifecycleEvent<'_, i32>| log.borrow_mut().push(event(e));
    let plan = ListLifecyclePlan::unassign(
        &p,
        &a,
        ListRange {
            entity: 0,
            start: 2,
            end: 3,
        },
    )
    .unwrap();
    plan.apply(&mut a, &mut listener).unwrap().commit();
    assert_eq!(a.lists()[0].len(), 2);
    for phase in [ListChangePhase::Before, ListChangePhase::After] {
        assert_eq!(
            log.borrow()
                .iter()
                .filter(|e| **e == (phase, Event::Unassigned(3)))
                .count(),
            1
        );
    }
}
