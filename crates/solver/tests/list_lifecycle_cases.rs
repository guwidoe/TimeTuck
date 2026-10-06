// Adapted from Timefold Community MoveDirectorTest, Apache-2.0.
// Source methods and list fixtures are recorded in compat/provenance.json.
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
enum Item {
    Assign(u8),
    Unassign(u8),
    Range(ListRange),
    Flush,
}
use Item::*;
use ListChangePhase::{After, Before};
fn r(entity: usize, start: usize, end: usize) -> Item {
    Range(ListRange { entity, start, end })
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn problem(lists: Vec<Vec<u8>>) -> ListProblem<u8> {
    ListProblem::new(
        lists.iter().flatten().copied().collect(),
        lists.into_iter().map(ListEntity::new).collect(),
    )
    .unwrap()
}
fn item(e: &ListLifecycleEvent<'_, u8>) -> Item {
    match e.change {
        ListLifecycleChange::Assigned(v) => Assign(*v),
        ListLifecycleChange::Unassigned(v) => Unassign(*v),
        ListLifecycleChange::Changed(range) => Range(range),
        ListLifecycleChange::Flush => Flush,
    }
}
fn run(
    p: &ListProblem<u8>,
    plan: ListLifecyclePlan<'_, u8>,
    returned: &[u8],
    expected: Vec<Vec<u8>>,
    forward: Vec<(ListChangePhase, Item)>,
    undo: Option<Vec<(ListChangePhase, Item)>>,
) {
    let mut a = p.initial_assignment();
    let original = a.clone();
    let mut events = vec![];
    let mut projections = vec![];
    let mut observer = |e: ListLifecycleEvent<'_, u8>| {
        events.push((e.undo, e.phase, item(&e)));
        if matches!(e.change, ListLifecycleChange::Flush) {
            // Actual derived owner/index projection; it runs only at flush boundaries.
            projections.push(
                p.values()
                    .iter()
                    .map(|v| e.assignment.location(v))
                    .collect::<Vec<_>>(),
            );
        }
    };
    assert_eq!(plan.returned_values(), returned);
    let edit = plan.apply(&mut a, &mut observer).unwrap();
    assert_eq!(edit.assignment().lists(), expected);
    edit.rollback();
    assert_eq!(a, original);
    let actual: Vec<_> = events
        .iter()
        .filter(|(undo, _, _)| !undo)
        .map(|(_, phase, item)| (*phase, item.clone()))
        .collect();
    assert_eq!(actual, forward);
    if let Some(undo) = undo {
        let actual: Vec<_> = events
            .iter()
            .filter(|(undo, _, _)| *undo)
            .map(|(_, phase, item)| (*phase, item.clone()))
            .collect();
        assert_eq!(actual, undo);
    }
    assert_eq!(projections.len(), 2);
    let expected = ListAssignment::new(expected);
    assert_eq!(
        projections[0],
        p.values()
            .iter()
            .map(|v| expected.location(v))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        projections[1],
        p.values()
            .iter()
            .map(|v| original.location(v))
            .collect::<Vec<_>>()
    );
}
#[test]
fn replace_value_in_list() {
    for (source, replacement, removed, expected) in [(0, 2, 3, vec![2, 1]), (2, 0, 1, vec![3, 2])] {
        let p = problem(vec![vec![1, 2, 3]]);
        let plan = ListLifecyclePlan::replace(
            &p,
            &p.initial_assignment(),
            pos(0, source),
            pos(0, replacement),
        )
        .unwrap();
        run(
            &p,
            plan,
            &[removed],
            vec![expected],
            vec![
                (Before, Unassign(removed)),
                (Before, r(0, 0, 3)),
                (After, r(0, 0, 2)),
                (After, Unassign(removed)),
                (After, Flush),
            ],
            Some(vec![
                (Before, Assign(removed)),
                (Before, r(0, 0, 2)),
                (After, r(0, 0, 3)),
                (After, Assign(removed)),
                (After, Flush),
            ]),
        );
    }
}
#[test]
fn replace_value_in_list_throws_on_same_index() {
    let p = problem(vec![vec![1, 2]]);
    assert!(
        matches!(ListLifecyclePlan::replace(&p,&p.initial_assignment(),pos(0,1),pos(0,1)),Err(ListEditError::SamePosition(p)) if p==pos(0,1))
    );
}
#[test]
fn replace_value_between_lists() {
    let p = problem(vec![vec![1, 2, 3], vec![4, 5, 6]]);
    let plan =
        ListLifecyclePlan::replace(&p, &p.initial_assignment(), pos(1, 2), pos(0, 1)).unwrap();
    run(
        &p,
        plan,
        &[2],
        vec![vec![1, 6, 3], vec![4, 5]],
        vec![
            (Before, Unassign(2)),
            (Before, r(0, 1, 2)),
            (Before, r(1, 2, 3)),
            (After, r(1, 2, 2)),
            (After, r(0, 1, 2)),
            (After, Unassign(2)),
            (After, Flush),
        ],
        Some(vec![
            (Before, Assign(2)),
            (Before, r(0, 1, 2)),
            (Before, r(1, 2, 2)),
            (After, r(1, 2, 3)),
            (After, r(0, 1, 2)),
            (After, Assign(2)),
            (After, Flush),
        ]),
    );
}
#[test]
fn unassign_values_removes_span() {
    let p = problem(vec![vec![1, 2, 3, 4, 5]]);
    let plan = ListLifecyclePlan::unassign(
        &p,
        &p.initial_assignment(),
        ListRange {
            entity: 0,
            start: 1,
            end: 4,
        },
    )
    .unwrap();
    run(
        &p,
        plan,
        &[2, 3, 4],
        vec![vec![1, 5]],
        vec![
            (Before, Unassign(2)),
            (Before, Unassign(3)),
            (Before, Unassign(4)),
            (Before, r(0, 1, 4)),
            (After, r(0, 1, 1)),
            (After, Unassign(2)),
            (After, Unassign(3)),
            (After, Unassign(4)),
            (After, Flush),
        ],
        None,
    );
}
#[test]
fn unassign_values_throws_on_empty_span() {
    let p = problem(vec![vec![1, 2]]);
    assert!(matches!(
        ListLifecyclePlan::unassign(
            &p,
            &p.initial_assignment(),
            ListRange {
                entity: 0,
                start: 1,
                end: 1
            }
        ),
        Err(ListEditError::Model(ListError::InvalidRange {
            entity: 0,
            start: 1,
            end: 1
        }))
    ));
}
#[test]
fn mass_move_values_gathers_from_multiple_entities_to_one_destination_in_one_flush() {
    let p = problem(vec![vec![1, 2], vec![3, 4], vec![5]]);
    let plan =
        ListLifecyclePlan::gather(&p, &p.initial_assignment(), &[2, 3], Some(pos(2, 0))).unwrap();
    run(
        &p,
        plan,
        &[2, 3],
        vec![vec![1], vec![4], vec![2, 3, 5]],
        vec![
            (Before, Unassign(2)),
            (Before, r(0, 1, 2)),
            (After, r(0, 1, 1)),
            (After, Unassign(2)),
            (Before, Unassign(3)),
            (Before, r(1, 0, 1)),
            (After, r(1, 0, 0)),
            (After, Unassign(3)),
            (Before, Assign(2)),
            (Before, Assign(3)),
            (Before, r(2, 0, 0)),
            (After, r(2, 0, 2)),
            (After, Assign(2)),
            (After, Assign(3)),
            (After, Flush),
        ],
        None,
    );
}
#[test]
fn mass_move_values_batches_multiple_members_of_the_same_entity_into_one_bracket() {
    let p = problem(vec![vec![1, 2, 3, 4]]);
    let plan = ListLifecyclePlan::gather(&p, &p.initial_assignment(), &[2, 4], None).unwrap();
    run(
        &p,
        plan,
        &[2, 4],
        vec![vec![1, 3]],
        vec![
            (Before, Unassign(2)),
            (Before, Unassign(4)),
            (Before, r(0, 1, 4)),
            (After, r(0, 1, 2)),
            (After, Unassign(2)),
            (After, Unassign(4)),
            (After, Flush),
        ],
        None,
    );
}
