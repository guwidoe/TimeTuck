// Adapted from pinned SelectorBasedKOptListMoveTest and its TestdataList fixtures.
// Apache-2.0; see compat/provenance.json. Native edge-order orientation is specified.
use timetuck::*;
fn check(index: usize) {
    let cases = [
        (
            6,
            0,
            vec![(6, 1), (2, 3), (4, 5)],
            vec![(1, 3), (2, 5), (4, 6)],
            vec![1, 2, 5, 6, 4, 3],
        ),
        (
            7,
            1,
            vec![(7, 2), (3, 4), (5, 6)],
            vec![(2, 4), (3, 6), (5, 7)],
            vec![1, 2, 3, 6, 7, 5, 4],
        ),
        (
            10,
            0,
            vec![(10, 1), (3, 4), (8, 9)],
            vec![(1, 4), (3, 9), (8, 10)],
            vec![1, 2, 3, 9, 10, 8, 7, 6, 5, 4],
        ),
        (
            8,
            0,
            vec![(1, 2), (4, 3), (7, 8), (6, 5)],
            vec![(2, 4), (3, 7), (8, 6), (5, 1)],
            vec![1, 5, 4, 2, 3, 7, 6, 8],
        ),
        (
            12,
            0,
            vec![(1, 12), (2, 3), (5, 6), (9, 10)],
            vec![(1, 3), (2, 6), (5, 10), (9, 12)],
            vec![1, 3, 4, 5, 10, 11, 12, 9, 8, 7, 6, 2],
        ),
        (
            12,
            0,
            vec![(2, 3), (6, 5), (10, 9), (12, 1)],
            vec![(2, 5), (6, 10), (9, 1), (12, 3)],
            vec![1, 2, 5, 4, 3, 12, 11, 10, 6, 7, 8, 9],
        ),
        (
            8,
            0,
            vec![(8, 1), (4, 5), (2, 3), (6, 7)],
            vec![(1, 4), (6, 3), (5, 8), (7, 2)],
            vec![1, 2, 7, 8, 5, 6, 3, 4],
        ),
        (
            12,
            0,
            vec![(12, 1), (5, 6), (2, 3), (8, 9)],
            vec![(1, 5), (8, 3), (6, 12), (9, 2)],
            vec![1, 2, 9, 10, 11, 12, 6, 7, 8, 3, 4, 5],
        ),
    ];
    let (n, prefix, removed, added, expected) = &cases[index];
    let p = ListProblem::new(
        (1..=*n).collect(),
        vec![ListEntity::new((1..=*n).collect()).with_pinned_prefix(*prefix)],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let edges = |items: &[(usize, usize)]| {
        items
            .iter()
            .map(|&(x, y)| (x - 1, y - 1))
            .collect::<Vec<_>>()
    };
    let movement = list_k_opt_edge_move(&p, &a, 0, &edges(removed), &edges(added)).unwrap();
    let plan = ListLifecyclePlan::prepare_with_scope(
        &p,
        &a,
        movement,
        ListNotificationScope::MovableSuffix,
    )
    .unwrap();
    let mut events = vec![];
    plan.apply(
        &mut a,
        &mut |event: ListLifecycleEvent<'_, usize>| match event.change {
            ListLifecycleChange::Changed(range) => {
                assert!(!event.undo);
                assert_eq!(
                    event.assignment.lists()[0],
                    if event.phase == ListChangePhase::Before {
                        (1..=*n).collect::<Vec<_>>()
                    } else {
                        expected.clone()
                    }
                );
                events.push((event.phase, range));
            }
            ListLifecycleChange::Flush => {}
            _ => panic!("reconnection preserves membership"),
        },
    )
    .unwrap()
    .commit();
    assert_eq!(&a.lists()[0], expected);
    let range = ListRange {
        entity: 0,
        start: *prefix,
        end: *n,
    };
    assert_eq!(
        events,
        [
            (ListChangePhase::Before, range),
            (ListChangePhase::After, range)
        ]
    );
}
#[test]
fn test3_opt() {
    check(0);
}
#[test]
fn test3_opt_pinned() {
    check(1);
}
#[test]
fn test3_opt_long() {
    check(2);
}
#[test]
fn test4_opt() {
    check(3);
}
#[test]
fn test4_opt_long() {
    check(4);
}
#[test]
fn test_inverted4_opt_long() {
    check(5);
}
#[test]
fn test_double_bridge4_opt() {
    check(6);
}
#[test]
fn test_double_bridge4_opt_long() {
    check(7);
}
