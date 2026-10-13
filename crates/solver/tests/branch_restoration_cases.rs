// Complete pinned Community DefaultExhaustiveSearchPhaseTest adaptations,
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn basic_record(
    p: &FiniteProblem<i32>,
    before: &Assignment<i32>,
    m: Move<i32>,
) -> RecordedMove<Assignment<i32>, Move<i32>> {
    let n = ConstraintNetwork::<i32, SimpleScore>::new(before.values().len());
    let mut d = ScoreDirector::new(p, &n, before.clone()).unwrap();
    let (_, record) = TemporaryMoves::new(&mut d)
        .recorded(&m, TemporaryMoveOptions::default(), |_, _| Ok::<_, ()>(()))
        .unwrap();
    record
}
fn list_record(
    p: &ListProblem<i32>,
    before: &ListAssignment<i32>,
    m: ListMove<i32>,
) -> RecordedMove<ListAssignment<i32>, ListMove<i32>> {
    let n = ListConstraintNetwork::<i32, SimpleScore>::new(p.entities().len(), p.values().len());
    let mut d = ListScoreDirector::new(p, &n, before.clone()).unwrap();
    let (_, record) = TemporaryMoves::new(&mut d)
        .recorded(&m, TemporaryMoveOptions::default(), |_, _| Ok::<_, ()>(()))
        .unwrap();
    record
}
fn counts(events: &[(usize, ReplayDirection)], nodes: usize) -> Vec<(usize, usize)> {
    (0..nodes)
        .map(|n| {
            (
                events
                    .iter()
                    .filter(|&&(i, d)| i == n && d == ReplayDirection::Forward)
                    .count(),
                events
                    .iter()
                    .filter(|&&(i, d)| i == n && d == ReplayDirection::Undo)
                    .count(),
            )
        })
        .collect()
}
#[test]
fn restore_working_solution_for_basic_variable() {
    let p = FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![10, 20, 21, 30, 31, 40]).allowing_unassigned())
            .collect(),
    )
    .unwrap();
    let mut working = p.initial_assignment();
    let mut tree = BranchRestorer::new(working.clone());
    // Source node0 has unused mock moves; native root has no incoming edge.
    let mut add = |parent, variable, value| {
        let record = basic_record(
            &p,
            tree.assignment_at(parent).unwrap(),
            Move::Change {
                variable,
                value: Some(value),
            },
        );
        tree.add_child(parent, record).unwrap()
    };
    let n1 = add(0, 0, 10);
    let n2a = add(n1, 1, 20);
    let n3a = add(n2a, 2, 30);
    let n2b = add(n1, 1, 21);
    let n3b = add(n2b, 2, 31);
    let n4b = add(n3b, 3, 40);
    tree.restore(
        n3a,
        BranchRestoreMode::CommonAncestor,
        &mut working,
        |a, m| {
            p.apply(a, m)?.commit();
            Ok::<_, ModelError>(())
        },
        |_, _| {},
    )
    .unwrap();
    let mut events = vec![];
    tree.restore(
        n4b,
        BranchRestoreMode::CommonAncestor,
        &mut working,
        |a, m| {
            p.apply(a, m)?.commit();
            Ok::<_, ModelError>(())
        },
        |i, d| events.push((i, d)),
    )
    .unwrap();
    assert_eq!(
        counts(&events, 7),
        vec![(0, 0), (0, 0), (0, 1), (0, 1), (1, 0), (1, 0), (1, 0)]
    );
    assert_eq!(
        events,
        vec![
            (n3a, ReplayDirection::Undo),
            (n2a, ReplayDirection::Undo),
            (n2b, ReplayDirection::Forward),
            (n3b, ReplayDirection::Forward),
            (n4b, ReplayDirection::Forward)
        ]
    );
    assert_eq!(working.values(), &[Some(10), Some(21), Some(31), Some(40)]);
    assert_eq!(tree.current_node(), n4b);
}
#[test]
fn restore_working_solution_for_list_variable() {
    let p = ListProblem::new((1..=6).collect(), vec![ListEntity::new(vec![])])
        .unwrap()
        .allowing_unassigned();
    let mut working = p.initial_assignment();
    let mut tree = BranchRestorer::new(working.clone());
    let mut add = |parent, value| {
        let before = tree.assignment_at(parent).unwrap();
        let index = before.lists()[0].len();
        let record = list_record(
            &p,
            before,
            ListMove::Assign {
                value,
                destination: ListPosition { entity: 0, index },
            },
        );
        tree.add_child(parent, record).unwrap()
    };
    let n1 = add(0, 1);
    let n2a = add(n1, 2);
    let n2b = add(n1, 3);
    let n3a = add(n2b, 4);
    let n3b = add(n3a, 5);
    let n4b = add(n3b, 6);
    tree.restore(
        n3b,
        BranchRestoreMode::RootReplay,
        &mut working,
        |a, m| {
            p.apply(a, m)?.commit();
            Ok::<_, ListError>(())
        },
        |_, _| {},
    )
    .unwrap();
    let mut events = vec![];
    tree.restore(
        n4b,
        BranchRestoreMode::RootReplay,
        &mut working,
        |a, m| {
            p.apply(a, m)?.commit();
            Ok::<_, ListError>(())
        },
        |i, d| events.push((i, d)),
    )
    .unwrap();
    assert_eq!(
        counts(&events, 7),
        vec![(0, 0), (1, 1), (0, 0), (1, 1), (1, 1), (1, 1), (1, 0)]
    );
    assert_eq!(
        events,
        vec![
            (n3b, ReplayDirection::Undo),
            (n3a, ReplayDirection::Undo),
            (n2b, ReplayDirection::Undo),
            (n1, ReplayDirection::Undo),
            (n1, ReplayDirection::Forward),
            (n2b, ReplayDirection::Forward),
            (n3a, ReplayDirection::Forward),
            (n3b, ReplayDirection::Forward),
            (n4b, ReplayDirection::Forward)
        ]
    );
    assert_eq!(working.lists(), &[vec![1, 3, 4, 5, 6]]);
    assert_eq!(tree.current_node(), n4b);
    assert_eq!(tree.assignment_at(n2a).unwrap().lists(), &[vec![1, 2]]);
}
