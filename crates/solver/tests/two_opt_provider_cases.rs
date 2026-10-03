// Adapted from Community ListTailSwapMoveProviderTest/TwoOptListMoveProviderTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn problem(rows: Vec<Vec<i32>>) -> ListProblem<i32> {
    ListProblem::new(
        rows.iter().flatten().copied().collect(),
        rows.into_iter().map(ListEntity::new).collect(),
    )
    .unwrap()
}
fn swap(l: (usize, usize, usize), r: (usize, usize, usize), reverse: bool) -> ListMove<i32> {
    ListMove::SubListSwap {
        left: ListRange {
            entity: l.0,
            start: l.1,
            end: l.2,
        },
        right: ListRange {
            entity: r.0,
            start: r.1,
            end: r.2,
        },
        reverse,
    }
}
fn reversed(m: &ListMove<i32>) -> bool {
    matches!(m, ListMove::SubListSwap { reverse: true, .. })
}
fn ranged() -> ListProblem<i32> {
    // TestdataListEntityProvidingSolution.generateSolution(), then source setup.
    ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![1, 3]).with_value_range(vec![1, 3]),
        ],
    )
    .unwrap()
}
#[test]
fn produces_tail_swaps_across_entities() {
    let p = problem(vec![vec![0, 1, 2], vec![3, 4]]);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_tail_swap_moves(&p, &a, true).unwrap().collect();
    assert!(moves.contains(&swap((0, 0, 3), (1, 0, 2), false)));
    assert!(moves.contains(&swap((0, 2, 3), (1, 1, 2), false)));
}
#[test]
fn reversing_appears_by_default_and_can_be_disabled() {
    let p = problem(vec![vec![0, 1, 2], vec![3, 4, 5]]);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_tail_swap_moves(&p, &a, true)
        .unwrap()
        .take(300)
        .collect();
    assert!(moves.iter().any(reversed));
    let moves: Vec<_> = list_tail_swap_moves(&p, &a, false)
        .unwrap()
        .take(300)
        .collect();
    assert!(!moves.is_empty());
    assert!(!moves.iter().any(reversed));
}
#[test]
fn reversing_never_happens_when_both_tails_are_single_element() {
    let p = problem(vec![vec![0], vec![1]]);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_tail_swap_moves(&p, &a, true)
        .unwrap()
        .take(300)
        .collect();
    assert!(!moves.is_empty());
    assert!(!moves.iter().any(reversed));
}
#[test]
fn same_entity_never_produces_a_move() {
    let p = problem(vec![vec![0, 1, 2]]);
    let a = p.initial_assignment();
    assert!(list_tail_swap_moves(&p, &a, true).unwrap().next().is_none());
}
#[test]
fn value_range_on_entity_rejects_invalid_swap() {
    let p = ranged();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_tail_swap_moves(&p, &a, true).unwrap().collect();
    for reverse in [false, true] {
        for (l, r) in [((0, 0, 1), (1, 0, 2)), ((1, 0, 2), (0, 0, 1))] {
            assert!(!moves.contains(&swap(l, r, reverse)));
        }
    }
}
#[test]
fn same_entity_always_produces_reversal() {
    let p = problem(vec![(0..10).collect()]);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_two_opt_moves(&p, &a, true)
        .unwrap()
        .take(300)
        .collect();
    assert!(!moves.is_empty());
    for m in moves {
        let ListMove::SubListChange {
            entity,
            start,
            end,
            destination,
            reverse,
        } = m
        else {
            panic!("expected reversal")
        };
        assert_eq!(
            destination,
            ListPosition {
                entity,
                index: start
            }
        );
        assert!(reverse);
        assert!(end - start >= 2);
    }
}
fn two_entity() -> ListProblem<i32> {
    problem(vec![(0..5).collect(), (5..10).collect()])
}
#[test]
fn crossing_entity_false_never_produces_tail_swap() {
    let p = two_entity();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_two_opt_moves(&p, &a, false)
        .unwrap()
        .take(500)
        .collect();
    assert!(!moves.is_empty());
    assert!(
        !moves
            .iter()
            .any(|m| matches!(m, ListMove::SubListSwap { .. }))
    );
}
#[test]
fn crossing_entity_true_produces_non_reversing_tail_swap_too() {
    let p = two_entity();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_two_opt_moves(&p, &a, true)
        .unwrap()
        .filter(|m| matches!(m, ListMove::SubListSwap { .. }))
        .take(300)
        .collect();
    assert!(!moves.is_empty());
    assert!(!moves.iter().any(reversed));
}
#[test]
fn pinned_prefix_never_touched() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![ListEntity::new((0..8).collect()).with_pinned_prefix(3)],
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_two_opt_moves(&p, &a, true)
        .unwrap()
        .take(300)
        .collect();
    assert!(!moves.is_empty());
    for m in moves {
        let ListMove::SubListChange { start, .. } = m else {
            panic!("expected reversal")
        };
        assert!(start >= 3);
    }
}
#[test]
fn fully_pinned_entity_never_involved() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new((0..3).collect()).with_pinned(true),
            ListEntity::new((3..8).collect()),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_two_opt_moves(&p, &a, true)
        .unwrap()
        .take(300)
        .collect();
    assert!(!moves.is_empty());
    for m in moves {
        match m {
            ListMove::SubListChange { entity, .. } => assert_ne!(entity, 0),
            ListMove::SubListSwap { left, right, .. } => {
                assert_ne!(left.entity, 0);
                assert_ne!(right.entity, 0);
            }
            _ => panic!("unexpected move"),
        }
    }
}
#[test]
fn value_range_on_entity_rejects_tail_swap() {
    let p = ranged();
    let a = p.initial_assignment();
    let moves: Vec<_> = list_two_opt_moves(&p, &a, true).unwrap().collect();
    for (l, r) in [((0, 0, 1), (1, 0, 2)), ((1, 0, 2), (0, 0, 1))] {
        assert!(!moves.contains(&swap(l, r, false)));
    }
}
#[test]
fn executing_reversal_pins_the_index_math() {
    let p = problem(vec![(0..5).collect()]);
    let mut a = p.initial_assignment();
    p.apply(
        &mut a,
        &ListMove::SubListChange {
            entity: 0,
            start: 1,
            end: 4,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
            reverse: true,
        },
    )
    .unwrap()
    .commit();
    assert_eq!(a.list(0).unwrap(), &[0, 3, 2, 1, 4]);
}
