// Adapted from pinned Timefold Community ChangeMoveTest and SwapMoveTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Native indices replace entities/value descriptors; RAII guards replace MoveTester.
use timetuck::{Assignment, Move};

// Inline fixture states follow TestdataSolution.generateSolution: value i % count.
fn change(variable: usize, value: Option<i32>) -> Move<i32> {
    Move::Change { variable, value }
}

#[test]
fn change_move_executes_permanently() {
    let mut state = Assignment::new(vec![Some(0), Some(1)]);
    state.apply(&change(0, Some(2))).unwrap().commit();
    assert_eq!(state.get(0), Some(&Some(2)));
}

#[test]
fn change_move_to_null() {
    let mut state = Assignment::new(vec![Some(0)]);
    state.apply(&change(0, None)).unwrap().commit();
    assert_eq!(state.get(0), Some(&None));
}

#[test]
fn change_move_multiple_entities() {
    let mut state = Assignment::new(vec![Some(0), Some(1), Some(2)]);
    for movement in [change(0, Some(1)), change(1, Some(0)), change(2, Some(1))] {
        state.apply(&movement).unwrap().commit();
    }
    assert_eq!(state.values(), &[Some(1), Some(0), Some(1)]);
}

#[test]
fn change_move_executes_temporarily_with_undo() {
    let mut state = Assignment::new(vec![Some(0)]);
    {
        let candidate = state.apply(&change(0, Some(2))).unwrap();
        assert_eq!(candidate.get(0), Some(&Some(2)));
    }
    assert_eq!(state.get(0), Some(&Some(0)));
}

#[test]
fn change_move_temporary_multiple_entities() {
    let mut state = Assignment::new(vec![Some(0), Some(1), Some(2)]);
    let movement = Move::Composite(vec![
        change(0, Some(1)),
        change(1, Some(0)),
        change(2, Some(1)),
    ]);
    {
        let candidate = state.apply(&movement).unwrap();
        assert_eq!(candidate.values(), &[Some(1), Some(0), Some(1)]);
    }
    assert_eq!(state.values(), &[Some(0), Some(1), Some(2)]);
}

#[test]
fn swap_move_executes_permanently() {
    let mut state = Assignment::new(vec![Some(0), Some(1)]);
    state
        .apply(&Move::Swap { left: 0, right: 1 })
        .unwrap()
        .commit();
    assert_eq!(state.values(), &[Some(1), Some(0)]);
}

#[test]
fn same_value_swap_writes_both_variables() {
    let mut state = Assignment::new(vec![Some(0), Some(0)]);
    let candidate = state.apply(&Move::Swap { left: 0, right: 1 }).unwrap();
    // Upstream observes two changeVariable calls through a mock. The native
    // transaction's journal exposes the corresponding two writes directly.
    assert_eq!(candidate.prior_values().len(), 2);
    assert_eq!(candidate.prior_values()[0].variable, 0);
    assert_eq!(candidate.prior_values()[1].variable, 1);
    assert_eq!(candidate.values(), &[Some(0), Some(0)]);
}

#[test]
fn multiple_swap_moves() {
    let mut state = Assignment::new(vec![Some(0), Some(1), Some(2)]);
    state
        .apply(&Move::Swap { left: 0, right: 1 })
        .unwrap()
        .commit();
    state
        .apply(&Move::Swap { left: 1, right: 2 })
        .unwrap()
        .commit();
    assert_eq!(state.values(), &[Some(1), Some(2), Some(0)]);
}

#[test]
fn swap_move_executes_temporarily_with_undo() {
    let mut state = Assignment::new(vec![Some(0), Some(1)]);
    {
        let candidate = state.apply(&Move::Swap { left: 0, right: 1 }).unwrap();
        assert_eq!(candidate.values(), &[Some(1), Some(0)]);
    }
    assert_eq!(state.values(), &[Some(0), Some(1)]);
}

#[test]
fn multiple_swap_moves_temporary() {
    let mut state = Assignment::new(vec![Some(0), Some(1), Some(2)]);
    {
        let candidate = state.apply(&Move::Swap { left: 0, right: 1 }).unwrap();
        assert_eq!(candidate.values(), &[Some(1), Some(0), Some(2)]);
    }
    assert_eq!(state.values(), &[Some(0), Some(1), Some(2)]);
    {
        let candidate = state.apply(&Move::Swap { left: 1, right: 2 }).unwrap();
        assert_eq!(candidate.values(), &[Some(0), Some(2), Some(1)]);
    }
    assert_eq!(state.values(), &[Some(0), Some(1), Some(2)]);
}
