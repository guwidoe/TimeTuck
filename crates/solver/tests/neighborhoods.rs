// Adapted from Timefold Community ChangeMoveProviderTest / SwapMoveProviderTest.
// Apache-2.0; individual methods and test-fixture provenance: compat/provenance.json.
use timetuck::{FiniteProblem, Move, Variable, change_moves, swap_moves};
fn ordinary() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap()
}
fn optional() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![0, 1])
            .with_initial(1)
            .allowing_unassigned(),
    ])
    .unwrap()
}
fn change(variable: usize, value: Option<i32>) -> Move<i32> {
    Move::Change { variable, value }
}

#[test]
fn change_from_solution() {
    let model = ordinary();
    let state = model.initial_assignment();
    let moves: Vec<_> = change_moves(&model, &state).unwrap().collect();
    assert!(moves.contains(&change(0, Some(1))));
    assert!(moves.contains(&change(1, Some(0))));
    assert!(!moves.contains(&change(0, Some(0))));
    assert!(!moves.contains(&change(1, Some(1))));
    assert_eq!(moves.len(), 2);
}
#[test]
fn change_pinned_entity_skipped() {
    let mut variables = ordinary().variables().to_vec();
    variables[0] = variables[0].clone().with_pinned(true);
    let model = FiniteProblem::new(variables).unwrap();
    let state = model.initial_assignment();
    let moves: Vec<_> = change_moves(&model, &state).unwrap().collect();
    assert!(moves.contains(&change(1, Some(0))));
    for excluded in [change(1, Some(1)), change(0, Some(0)), change(0, Some(1))] {
        assert!(!moves.contains(&excluded));
    }
    assert_eq!(moves.len(), 1);
}
#[test]
fn crossing_null_default_true_assigns_and_unassigns() {
    let model = optional();
    let state = model.initial_assignment();
    let moves: Vec<_> = change_moves(&model, &state).unwrap().collect();
    for expected in [
        change(1, Some(0)),
        change(0, Some(0)),
        change(0, Some(1)),
        change(1, None),
    ] {
        assert!(moves.contains(&expected));
    }
    for excluded in [change(1, Some(1)), change(0, None)] {
        assert!(!moves.contains(&excluded));
    }
    assert_eq!(moves.len(), 4);
}
#[test]
fn crossing_null_unassign_move_is_undoable() {
    let model = optional();
    let mut state = model.initial_assignment();
    let original = state.values()[1];
    assert!(original.is_some());
    let movement = change_moves(&model, &state)
        .unwrap()
        .find(|m| *m == change(1, None))
        .unwrap();
    {
        let candidate = model.apply(&mut state, &movement).unwrap();
        assert_eq!(candidate.values()[1], None);
    }
    assert_eq!(state.values()[1], original);
}
#[test]
fn crossing_null_assign_move_is_undoable() {
    let model = optional();
    let mut state = model.initial_assignment();
    assert_eq!(state.values()[0], None);
    let movement = change_moves(&model, &state)
        .unwrap()
        .find(|m| *m == change(0, Some(0)))
        .unwrap();
    {
        let candidate = model.apply(&mut state, &movement).unwrap();
        assert_eq!(candidate.values()[0], Some(0));
    }
    assert_eq!(state.values()[0], None);
}
#[test]
fn swap_univariate() {
    let model = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let state = model.initial_assignment();
    let moves: Vec<_> = swap_moves(&model, &state).unwrap().collect();
    for (left, right) in [(0, 1), (1, 0), (1, 2), (2, 1)] {
        assert!(moves.contains(&Move::Swap { left, right }));
    }
    for (left, right) in [(0, 2), (2, 0)] {
        assert!(!moves.contains(&Move::Swap { left, right }));
    }
    assert_eq!(moves.len(), 4);
}
#[test]
fn swap_pinned_entity_skipped() {
    let mut variables = ordinary().variables().to_vec();
    variables[0] = variables[0].clone().with_pinned(true);
    let model = FiniteProblem::new(variables).unwrap();
    let state = model.initial_assignment();
    let moves: Vec<_> = swap_moves(&model, &state).unwrap().collect();
    assert!(!moves.contains(&Move::Swap { left: 0, right: 1 }));
    assert!(!moves.contains(&Move::Swap { left: 1, right: 0 }));
    assert!(moves.is_empty());
}
#[test]
fn swap_from_entity() {
    let model = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![2]).with_initial(2),
    ])
    .unwrap();
    let state = model.initial_assignment();
    let moves: Vec<_> = swap_moves(&model, &state).unwrap().collect();
    for (left, right) in [(0, 1), (1, 0)] {
        assert!(moves.contains(&Move::Swap { left, right }));
    }
    for (left, right) in [(0, 2), (2, 0), (1, 2), (2, 1)] {
        assert!(!moves.contains(&Move::Swap { left, right }));
    }
    assert_eq!(moves.len(), 2);
}
