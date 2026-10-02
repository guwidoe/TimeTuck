//! Original native regressions; no upstream test-method coverage is claimed.
use std::{
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

fn position(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn problem() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1]),
            Variable::new(vec![9]).with_initial(9).with_pinned(true),
        ])
        .unwrap(),
        ListProblem::new(
            vec![10, 20],
            vec![ListEntity::new(vec![10]), ListEntity::new(vec![20])],
        )
        .unwrap(),
    )
}
fn joint_move() -> MixedMove<i32, i32> {
    MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: ListMove::Change {
            source: position(0, 0),
            destination: position(1, 1),
        },
    }
}

#[test]
fn joint_moves_commit_undo_and_panic_restore_both_components() {
    let problem = problem();
    let mut state = problem.initial_assignment();
    let original = state.clone();
    {
        let trial = problem.apply(&mut state, &joint_move()).unwrap();
        assert_eq!(trial.original(), &original);
        assert_eq!(trial.basic().values(), &[Some(1), Some(9)]);
        assert_eq!(trial.lists().lists(), &[vec![], vec![20, 10]]);
        assert!(problem.is_complete(&trial));
    }
    assert_eq!(state, original);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = problem.apply(&mut state, &joint_move()).unwrap();
            panic!("scoring failed");
        }))
        .is_err()
    );
    assert_eq!(state, original);
    problem.apply(&mut state, &joint_move()).unwrap().commit();
    assert_eq!(state.basic().values()[0], Some(1));
    assert_eq!(state.lists().lists(), &[vec![], vec![20, 10]]);
}

#[test]
fn failed_component_or_invalid_start_cannot_leak_other_component_writes() {
    let problem = problem();
    let mut state = problem.initial_assignment();
    let original = state.clone();
    let mut movement = joint_move();
    movement.lists = ListMove::Composite(vec![
        movement.lists,
        ListMove::Unassign {
            entity: 99,
            start: 0,
            end: 1,
        },
    ]);
    assert!(matches!(
        problem.apply(&mut state, &movement),
        Err(MixedError::Lists(_))
    ));
    assert_eq!(state, original);
    movement = joint_move();
    movement.basic = Move::Change {
        variable: 1,
        value: None,
    };
    assert!(matches!(
        problem.apply(&mut state, &movement),
        Err(MixedError::Basic(ModelError::PinnedValueChanged {
            variable: 1
        }))
    ));
    assert_eq!(state, original);
    state = MixedAssignment::new(
        Assignment::new(vec![None, Some(9)]),
        ListAssignment::new(vec![vec![10], vec![10]]),
    );
    let invalid = state.clone();
    assert!(matches!(
        problem.apply(&mut state, &joint_move()),
        Err(MixedError::Lists(ListError::DuplicateAssignedValue))
    ));
    assert_eq!(state, invalid);
}

#[test]
fn nested_component_composites_validate_final_pins_and_ranges() {
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![1]).with_initial(1).with_pinned(true),
        ])
        .unwrap(),
        ListProblem::new(
            vec![10, 20],
            vec![
                ListEntity::new(vec![10])
                    .with_pinned_prefix(1)
                    .with_value_range(vec![10]),
                ListEntity::new(vec![20]).with_value_range(vec![20]),
            ],
        )
        .unwrap(),
    );
    let mut state = problem.initial_assignment();
    let original = state.clone();
    let swap = ListMove::Swap {
        left: position(0, 0),
        right: position(1, 0),
    };
    let movement = MixedMove {
        basic: Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(99),
            },
            Move::Composite(vec![Move::Change {
                variable: 0,
                value: Some(1),
            }]),
        ]),
        lists: ListMove::Composite(vec![swap.clone(), ListMove::Composite(vec![swap])]),
    };
    problem.apply(&mut state, &movement).unwrap().commit();
    assert_eq!(state, original);
    let movement = MixedMove {
        basic: Move::Composite(vec![]),
        lists: ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
    };
    assert!(matches!(
        problem.apply(&mut state, &movement),
        Err(MixedError::Lists(ListError::PinChanged { entity: 0 }))
    ));
    assert_eq!(state, original);
}

// Independent enumeration: permute every selected subset, then split the sequence
// between two owners. This does not reuse the solver's insertion enumeration.
fn permutations(values: &[i32]) -> Vec<Vec<i32>> {
    if values.is_empty() {
        return vec![vec![]];
    }
    let mut result = vec![];
    for i in 0..values.len() {
        let mut rest = values.to_vec();
        let value = rest.remove(i);
        for mut permutation in permutations(&rest) {
            permutation.insert(0, value);
            result.push(permutation);
        }
    }
    result
}

#[test]
fn full_product_matches_independent_subset_permutation_enumeration() {
    for optional in [false, true] {
        for first_range in 0_u32..8 {
            for second_range in 0_u32..8 {
                let range = |bits: u32| {
                    (0..3)
                        .filter(|v| bits & (1 << v) != 0)
                        .collect::<Vec<i32>>()
                };
                let mut lists = ListProblem::new(
                    vec![0, 1, 2],
                    vec![
                        ListEntity::new(vec![]).with_value_range(range(first_range)),
                        ListEntity::new(vec![]).with_value_range(range(second_range)),
                    ],
                )
                .unwrap();
                if optional {
                    lists = lists.allowing_unassigned();
                }
                let problem = MixedProblem::new(
                    FiniteProblem::new(vec![
                        Variable::new(vec![0, 1]).allowing_unassigned(),
                        Variable::new(vec![9]).with_initial(9).with_pinned(true),
                    ])
                    .unwrap(),
                    lists,
                );
                let mut expected = BTreeSet::new();
                for subset in 0_u32..8 {
                    if !optional && subset != 7 {
                        continue;
                    }
                    for permutation in permutations(&range(subset)) {
                        for split in 0..=permutation.len() {
                            let left = permutation[..split].to_vec();
                            let right = permutation[split..].to_vec();
                            if left.iter().any(|v| first_range & (1 << v) == 0)
                                || right.iter().any(|v| second_range & (1 << v) == 0)
                            {
                                continue;
                            }
                            for value in [Some(0), Some(1), None] {
                                expected.insert((
                                    vec![value, Some(9)],
                                    vec![left.clone(), right.clone()],
                                ));
                            }
                        }
                    }
                }
                let mut actual = BTreeSet::new();
                let result = exhaustive_mixed_search(
                    &problem,
                    |state| {
                        assert!(problem.is_complete(state));
                        assert!(
                            actual.insert((
                                state.basic().values().to_vec(),
                                state.lists().lists().to_vec()
                            )),
                            "duplicate candidate"
                        );
                        SimpleScore::ZERO
                    },
                    u64::MAX,
                );
                assert_eq!(
                    actual, expected,
                    "optional={optional}, ranges={first_range}/{second_range}"
                );
                assert_eq!(result.evaluations as usize, expected.len());
                assert_eq!(result.termination, Termination::Exhausted);
                assert_eq!(result.is_optimal(), !expected.is_empty());
            }
        }
    }
}

#[test]
fn every_budget_boundary_preserves_prefix_ties_and_truthful_exhaustion() {
    let problem = problem();
    let mut full = vec![];
    let result = exhaustive_mixed_search(
        &problem,
        |state| {
            full.push(state.clone());
            SimpleScore::ZERO
        },
        u64::MAX,
    );
    assert_eq!(result.evaluations, 12);
    for budget in 0..=13 {
        let mut seen = vec![];
        let result = exhaustive_mixed_search(
            &problem,
            |state| {
                seen.push(state.clone());
                SimpleScore::ZERO
            },
            budget,
        );
        assert_eq!(seen, full[..budget.min(12) as usize]);
        assert_eq!(result.evaluations, budget.min(12));
        assert_eq!(
            result.termination,
            if budget < 12 {
                Termination::EvaluationLimit
            } else {
                Termination::Exhausted
            }
        );
        assert_eq!(
            result.best.as_ref().map(|b| &b.assignment),
            if budget == 0 { None } else { full.first() }
        );
    }
}

#[test]
fn coupled_objective_searches_full_product_and_preserves_pins() {
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(1)]).unwrap(),
        ListProblem::new(
            vec![10, 20, 30],
            vec![
                ListEntity::new(vec![10, 20]).with_pinned_prefix(1),
                ListEntity::new(vec![]),
                ListEntity::new(vec![30]).with_pinned(true),
            ],
        )
        .unwrap(),
    );
    let original = problem.initial_assignment();
    let result = exhaustive_mixed_search(
        &problem,
        |state| {
            assert_eq!(state.lists().list(0).unwrap()[0], 10);
            assert_eq!(state.lists().list(2), Some([30].as_slice()));
            let basic = state.basic().values()[0].unwrap();
            let owner = state.lists().location(&20).unwrap().entity as i32;
            // Optimizing basic alone prefers 0, but the joint optimum requires 1.
            SimpleScore::new(if owner == 1 && basic == 1 {
                20
            } else if basic == 0 {
                5
            } else {
                0
            })
        },
        u64::MAX,
    );
    assert_eq!(result.evaluations, 4);
    assert!(result.is_optimal());
    let best = result.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(20));
    assert_eq!(best.assignment.basic().values(), &[Some(1)]);
    assert_eq!(best.assignment.lists().location(&20), Some(position(1, 0)));
    assert_eq!(problem.initial_assignment(), original);
}

#[test]
fn empty_impossible_and_overflowing_spaces_stop_without_unbounded_enumeration() {
    let empty_basic = || FiniteProblem::<i32>::new(vec![]).unwrap();
    let empty_lists = || ListProblem::<i32>::new(vec![], vec![]).unwrap();
    let problem = MixedProblem::new(empty_basic(), empty_lists());
    for budget in [0, 1] {
        let result = exhaustive_mixed_search(&problem, |_| SimpleScore::ZERO, budget);
        assert_eq!(result.evaluations, budget);
        assert_eq!(result.is_optimal(), budget == 1);
    }
    for problem in [
        MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![])]).unwrap(),
            empty_lists(),
        ),
        MixedProblem::new(empty_basic(), ListProblem::new(vec![1], vec![]).unwrap()),
        MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![])]).unwrap(),
            ListProblem::new((0..50).collect(), vec![ListEntity::new(vec![])]).unwrap(),
        ),
        MixedProblem::new(
            FiniteProblem::new((0..200).map(|_| Variable::new(vec![0, 1])).collect()).unwrap(),
            ListProblem::new(vec![1], vec![]).unwrap(),
        ),
    ] {
        let result = exhaustive_mixed_search(
            &problem,
            |_| -> SimpleScore { panic!("empty space evaluated") },
            0,
        );
        assert_eq!(result.termination, Termination::Exhausted);
        assert_eq!(result.evaluations, 0);
        assert!(!result.is_optimal());
    }
    let problem = MixedProblem::new(
        FiniteProblem::new((0..200).map(|_| Variable::new(vec![0, 1])).collect()).unwrap(),
        ListProblem::new((0..50).collect(), vec![ListEntity::new(vec![])]).unwrap(),
    );
    assert_eq!(problem.basic().search_space_size(), None);
    let result = exhaustive_mixed_search(&problem, |_| SimpleScore::ZERO, 1);
    assert_eq!(result.evaluations, 1);
    assert_eq!(result.termination, Termination::EvaluationLimit);
}

#[test]
fn structural_rejection_hard_infeasibility_and_scorer_panic_remain_distinct() {
    let problem = problem();
    let rejected = exhaustive_mixed_search(
        &problem,
        |_| HardSoftScore::ZERO.with_structural(-1),
        u64::MAX,
    );
    assert!(rejected.best.is_none());
    assert!(!rejected.is_optimal());
    assert_eq!(rejected.termination, Termination::Exhausted);
    let infeasible = exhaustive_mixed_search(&problem, |_| HardSoftScore::new(-1, 10), u64::MAX);
    assert!(infeasible.is_optimal());
    assert!(!infeasible.best.unwrap().score.is_feasible());
    let original = problem.initial_assignment();
    assert!(
        catch_unwind(AssertUnwindSafe(|| exhaustive_mixed_search(
            &problem,
            |_| -> SimpleScore { panic!("scoring") },
            1
        )))
        .is_err()
    );
    assert_eq!(problem.initial_assignment(), original);
}
