// Original boundary checks, separate from upstream adaptations.
use timetuck::{Assignment, FiniteProblem, ModelError, Move, Variable, change_moves, swap_moves};
#[test]
fn swaps_transfer_unassigned_values_only_into_optional_variables() {
    let model = FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0),
        Variable::new(vec![0]).allowing_unassigned(),
        Variable::new(vec![0]).with_initial(0).allowing_unassigned(),
    ])
    .unwrap();
    let mut state = model.initial_assignment();
    let moves: Vec<_> = swap_moves(&model, &state).unwrap().collect();
    assert_eq!(
        moves,
        vec![
            Move::Swap { left: 1, right: 2 },
            Move::Swap { left: 2, right: 1 }
        ]
    );
    for movement in moves {
        let candidate = model.apply(&mut state, &movement).unwrap();
        assert!(model.is_complete(&candidate));
    }
    assert_eq!(state, model.initial_assignment());
}
#[test]
fn invalid_assignments_cannot_be_enumerated() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap();
    let short = Assignment::<i32>::new(vec![]);
    assert!(matches!(
        change_moves(&model, &short),
        Err(ModelError::AssignmentLength { .. })
    ));
    let foreign = Assignment::new(vec![Some(1)]);
    assert!(matches!(
        swap_moves(&model, &foreign),
        Err(ModelError::ValueOutsideDomain { variable: 0 })
    ));
}
#[test]
fn every_generated_move_preserves_heterogeneous_domains_and_pins() {
    for a in [0, 1] {
        for b in [1, 2] {
            for c in [None, Some(0), Some(2)] {
                let first = Variable::new(vec![0, 1]).with_initial(a).with_pinned(true);
                let third = match c {
                    None => Variable::new(vec![0, 2]).allowing_unassigned(),
                    Some(v) => Variable::new(vec![0, 2])
                        .allowing_unassigned()
                        .with_initial(v),
                };
                let model = FiniteProblem::new(vec![
                    first,
                    Variable::new(vec![1, 2]).with_initial(b),
                    third,
                ])
                .unwrap();
                let original = model.initial_assignment();
                let mut state = original.clone();
                let moves: Vec<_> = change_moves(&model, &original)
                    .unwrap()
                    .chain(swap_moves(&model, &original).unwrap())
                    .collect();
                for (i, movement) in moves.iter().enumerate() {
                    assert!(!moves[..i].contains(movement));
                    let candidate = model.apply(&mut state, movement).unwrap();
                    assert!(model.is_complete(&candidate));
                    assert_eq!(candidate.values()[0], Some(a));
                    assert_ne!(*candidate, original);
                }
                assert_eq!(state, original);
            }
        }
    }
}
