// Original regression tests; not counted as adapted upstream cases.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::{Assignment, FiniteProblem, ModelError, Move, MoveError, Variable};

#[test]
fn nested_repeated_writes_undo_in_reverse_order() {
    let original = Assignment::new(vec![Some(String::from("a")), None]);
    let mut state = original.clone();
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some("b".into()),
        },
        Move::Composite(vec![
            Move::Swap { left: 0, right: 1 },
            Move::Change {
                variable: 1,
                value: Some("c".into()),
            },
            Move::Swap { left: 1, right: 1 },
        ]),
    ]);
    {
        let candidate = state.apply(&movement).unwrap();
        assert_eq!(candidate.values(), &[None, Some("c".into())]);
        assert_eq!(candidate.prior_values().len(), 6);
    }
    assert_eq!(state, original);
    state.apply(&movement).unwrap().commit();
    assert_eq!(state.values(), &[None, Some("c".into())]);
}

#[test]
fn invalid_later_moves_restore_all_earlier_writes() {
    for invalid in [
        Move::Change {
            variable: usize::MAX,
            value: Some(9),
        },
        Move::Swap {
            left: 0,
            right: usize::MAX,
        },
        Move::Swap {
            left: usize::MAX,
            right: 0,
        },
    ] {
        let mut state = Assignment::new(vec![Some(1), None]);
        let movement = Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Swap { left: 0, right: 1 },
            invalid,
        ]);
        assert!(matches!(
            state.apply(&movement),
            Err(MoveError {
                variable: usize::MAX,
                variable_count: 2
            })
        ));
        assert_eq!(state.values(), &[Some(1), None]);
    }
}

#[test]
fn unwinding_candidate_evaluation_restores_assignment() {
    let mut state = Assignment::new(vec![Some(1)]);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let candidate = state
            .apply(&Move::Change {
                variable: 0,
                value: None,
            })
            .unwrap();
        assert_eq!(candidate.values(), &[None]);
        panic!("scorer failure");
    }));
    assert!(outcome.is_err());
    assert_eq!(state.values(), &[Some(1)]);
}

#[test]
fn panicking_clone_restores_already_applied_steps() {
    #[derive(Debug, Eq, PartialEq)]
    struct Value(i32);
    impl Clone for Value {
        fn clone(&self) -> Self {
            assert_ne!(self.0, 99, "deliberate clone failure");
            Self(self.0)
        }
    }
    let mut state = Assignment::new(vec![Some(Value(1))]);
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(Value(2)),
        },
        Move::Change {
            variable: 0,
            value: Some(Value(99)),
        },
    ]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = state.apply(&movement);
        }))
        .is_err()
    );
    assert_eq!(state.values(), &[Some(Value(1))]);
}

#[test]
fn invalid_domain_metadata_is_rejected() {
    assert!(matches!(
        FiniteProblem::new(vec![Variable::new(vec![1, 2, 1])]),
        Err(ModelError::DuplicateValue { variable: 0 })
    ));
    assert!(matches!(
        FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(2)]),
        Err(ModelError::ValueOutsideDomain { variable: 0 })
    ));
    assert!(matches!(
        FiniteProblem::new(vec![Variable::new(vec![1]).with_pinned(true)]),
        Err(ModelError::PinnedUnassigned { variable: 0 })
    ));
}

#[test]
fn model_rejects_invalid_candidates_and_preserves_state() {
    let model = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![2, 3]).with_initial(2),
    ])
    .unwrap();
    let mut state = model.initial_assignment();
    let original = state.clone();
    for (movement, expected) in [
        (
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            ModelError::PinnedValueChanged { variable: 0 },
        ),
        (
            Move::Change {
                variable: 1,
                value: Some(1),
            },
            ModelError::ValueOutsideDomain { variable: 1 },
        ),
        (
            Move::Composite(vec![
                Move::Change {
                    variable: 1,
                    value: Some(3),
                },
                Move::Change {
                    variable: 0,
                    value: None,
                },
            ]),
            ModelError::PinnedValueChanged { variable: 0 },
        ),
    ] {
        assert!(matches!(model.apply(&mut state, &movement), Err(error) if error == expected));
        assert_eq!(state, original);
    }
    let mut wrong_length = Assignment::new(vec![Some(1)]);
    assert!(matches!(
        model.apply(&mut wrong_length, &Move::Composite(vec![])),
        Err(ModelError::AssignmentLength {
            expected: 2,
            actual: 1
        })
    ));
    assert_eq!(wrong_length.values(), &[Some(1)]);
}

#[test]
fn partial_assignments_are_valid_but_not_complete() {
    let model = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let mut state = model.initial_assignment();
    assert!(model.validate(&state).is_ok());
    assert!(!model.is_complete(&state));
    model
        .apply(
            &mut state,
            &Move::Change {
                variable: 0,
                value: Some(1),
            },
        )
        .unwrap()
        .commit();
    assert!(model.is_complete(&state));
    model
        .apply(
            &mut state,
            &Move::Change {
                variable: 0,
                value: None,
            },
        )
        .unwrap()
        .commit();
    assert!(!model.is_complete(&state));
    let optional = FiniteProblem::new(vec![
        Variable::<i32>::new(vec![])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    assert!(optional.is_complete(&optional.initial_assignment()));
    assert_eq!(optional.search_space_size(), Some(1));
}

#[test]
fn all_small_two_step_transactions_match_an_independent_interpreter() {
    // Exhaust all assignments, including unassigned states, and pairs of moves.
    let values = [None, Some(0), Some(1)];
    let mut moves: Vec<_> = (0..2)
        .flat_map(|variable| values.map(|value| Move::Change { variable, value }))
        .collect();
    moves.extend([
        Move::Swap { left: 0, right: 1 },
        Move::Swap { left: 0, right: 0 },
    ]);
    for a in values {
        for b in values {
            for first in &moves {
                for second in &moves {
                    let original = vec![a, b];
                    let mut expected = original.clone();
                    for movement in [first, second] {
                        match movement {
                            Move::Change { variable, value } => expected[*variable] = *value,
                            Move::Swap { left, right } => expected.swap(*left, *right),
                            Move::Composite(_) => unreachable!(),
                        }
                    }
                    let mut state = Assignment::new(original.clone());
                    let combined = Move::Composite(vec![first.clone(), second.clone()]);
                    {
                        let candidate = state.apply(&combined).unwrap();
                        assert_eq!(candidate.values(), expected);
                    }
                    assert_eq!(state.values(), original);
                    state.apply(&combined).unwrap().commit();
                    assert_eq!(state.values(), expected);
                }
            }
        }
    }
}
