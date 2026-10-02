//! Adapted Timefold Community provider tests; Apache-2.0.
//! See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn assign(value: i32, entity: usize, index: usize) -> ListMove<i32> {
    ListMove::Assign {
        value,
        destination: pos(entity, index),
    }
}
fn change(a: usize, i: usize, b: usize, j: usize) -> ListMove<i32> {
    ListMove::Change {
        source: pos(a, i),
        destination: pos(b, j),
    }
}
fn swap(a: usize, i: usize, b: usize, j: usize) -> ListMove<i32> {
    ListMove::Swap {
        left: pos(a, i),
        right: pos(b, j),
    }
}
fn unassign(entity: usize, index: usize) -> ListMove<i32> {
    ListMove::Unassign {
        entity,
        start: index,
        end: index + 1,
    }
}
fn problem(n: i32, lists: Vec<Vec<i32>>, optional: bool, pin: bool) -> ListProblem<i32> {
    let p = ListProblem::new(
        (0..n).collect(),
        lists
            .into_iter()
            .enumerate()
            .map(|(e, v)| ListEntity::new(v).with_pinned(pin && e == 0))
            .collect(),
    )
    .unwrap();
    if optional { p.allowing_unassigned() } else { p }
}
#[test]
fn assign_pinned_entity_skipped() {
    let p = problem(2, vec![vec![], vec![]], true, true);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_assign_moves(&p, &a).unwrap().collect();
    for v in 0..2 {
        assert!(moves.contains(&assign(v, 1, 0)));
        assert!(!moves.contains(&assign(v, 0, 0)));
    }
}
#[test]
fn assign_from_solution() {
    let p = problem(2, vec![vec![], vec![0]], true, false);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_assign_moves(&p, &a).unwrap().collect();
    for m in [assign(1, 0, 0), assign(1, 1, 0), assign(1, 1, 1)] {
        assert!(moves.contains(&m));
    }
}
#[test]
fn change_pinned_entity_skipped() {
    let p = problem(2, vec![vec![0], vec![1]], false, true);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_change_moves(&p, &a).unwrap().collect();
    for m in [change(1, 0, 0, 0), change(0, 0, 1, 0)] {
        assert!(!moves.contains(&m));
    }
}
#[test]
fn change_from_solution() {
    let p = problem(2, vec![vec![], vec![1]], false, false);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_change_moves(&p, &a).unwrap().collect();
    assert!(moves.contains(&change(1, 0, 0, 0)));
    for m in [change(1, 0, 1, 0), change(1, 0, 1, 1)] {
        assert!(!moves.contains(&m));
    }
}
#[test]
fn change_from_solution_allows_unassigned() {
    let p = problem(2, vec![vec![], vec![0]], true, false);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_change_moves(&p, &a).unwrap().collect();
    assert!(moves.contains(&change(1, 0, 0, 0)));
    for m in [change(1, 0, 1, 0), change(1, 0, 1, 1)] {
        assert!(!moves.contains(&m));
    }
}
#[test]
fn change_crossing_null_true_produces_assign_and_unassign() {
    let p = problem(2, vec![vec![0], vec![]], true, false);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_moves(&p, &a, Neighborhood::Change).unwrap().collect();
    for m in [unassign(0, 0), assign(1, 1, 0)] {
        assert!(moves.contains(&m));
    }
}
#[test]
fn change_crossing_null_false_produces_neither_assign_nor_unassign() {
    let p = problem(2, vec![vec![0], vec![]], true, false);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_change_moves(&p, &a).unwrap().collect();
    for m in [unassign(0, 0), assign(1, 1, 0)] {
        assert!(!moves.contains(&m));
    }
}
#[test]
fn swap_pinned_entity_skipped() {
    let p = problem(2, vec![vec![0], vec![1]], false, true);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_swap_moves(&p, &a).unwrap().collect();
    for m in [swap(1, 0, 0, 0), swap(0, 0, 1, 0)] {
        assert!(!moves.contains(&m));
    }
}
#[test]
fn swap_from_solution() {
    let p = problem(4, vec![vec![1], vec![2, 3]], false, false);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_swap_moves(&p, &a).unwrap().collect();
    for m in [
        swap(0, 0, 1, 0),
        swap(1, 0, 0, 0),
        swap(0, 0, 1, 1),
        swap(1, 1, 0, 0),
        swap(1, 0, 1, 1),
        swap(1, 1, 1, 0),
    ] {
        assert!(moves.contains(&m));
    }
    for (e, i) in [(0, 0), (1, 0), (1, 1)] {
        assert!(!moves.contains(&ListMove::Composite(vec![unassign(e, i), assign(0, e, i)])));
    }
}
#[test]
fn swap_both_unassigned_produces_no_moves() {
    let p = problem(2, vec![vec![], vec![]], false, false);
    let a = p.initial_assignment();
    assert!(list_swap_moves(&p, &a).unwrap().next().is_none());
}
#[test]
fn unassign_pinned_entity_skipped() {
    let p = problem(2, vec![vec![0], vec![1]], true, true);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_unassign_moves(&p, &a).unwrap().collect();
    assert!(moves.contains(&unassign(1, 0)));
    assert!(!moves.contains(&unassign(0, 0)));
}
#[test]
fn unassign_from_solution() {
    let p = problem(4, vec![vec![0, 1], vec![2]], true, false);
    let a = p.initial_assignment();
    let moves: Vec<_> = list_unassign_moves(&p, &a).unwrap().collect();
    for m in [unassign(0, 0), unassign(0, 1), unassign(1, 0)] {
        assert!(moves.contains(&m));
    }
}
#[test]
fn unassign_no_assigned_values() {
    let p = problem(2, vec![vec![], vec![]], true, false);
    let a = p.initial_assignment();
    assert!(list_unassign_moves(&p, &a).unwrap().next().is_none());
}
