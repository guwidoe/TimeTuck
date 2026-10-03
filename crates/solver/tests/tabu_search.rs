// Original tabu-memory, move-selection and integrated-search regressions.
use std::cell::Cell;
use timetuck::{
    Acceptance, AcceptanceError, ConstraintNetwork, FiniteProblem, LocalSearchConfig,
    LocalSearchError, LocalSearchTermination as Stop, Move, MoveOrder, MoveSelection, Neighborhood,
    SimpleScore, TabuMemory, Variable, local_search,
};

fn config() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_evaluations: 100,
        max_accepted_moves: 3,
        neighborhood: Neighborhood::Change,
        selection: MoveSelection::BestAccepted,
        assert_scores: true,
        ..LocalSearchConfig::default()
    }
}
fn valley(v: &[Option<&i32>]) -> SimpleScore {
    SimpleScore::new(match v {
        [Some(0), Some(0), Some(0)] => 0,
        [Some(1), Some(0), Some(0)] => 3,
        [Some(1), Some(1), Some(0)] => 2,
        [Some(1), Some(1), Some(1)] => 5,
        _ => -10,
    })
}

#[test]
fn best_accepted_tabu_crosses_a_valley_and_preserves_best_after_worsening() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 3]).unwrap();
    let mut network = ConstraintNetwork::new(3);
    network
        .add_constraint("valley", vec![0, 1, 2], valley)
        .unwrap();
    let strict = local_search(&problem, &network, config()).unwrap();
    assert_eq!(strict.best.unwrap().score, SimpleScore::new(3));
    assert_eq!(strict.accepted_moves, 1);
    assert_eq!(strict.termination, Stop::NoAcceptedMove);
    let cfg = LocalSearchConfig {
        acceptance: Acceptance::VariableTabu {
            tenure: 1,
            aspiration: true,
        },
        ..config()
    };
    let tabu = local_search(&problem, &network, cfg).unwrap();
    assert_eq!(
        tabu.best.as_ref().unwrap().assignment.values(),
        &[Some(1); 3]
    );
    assert_eq!(tabu.best.as_ref().unwrap().score, SimpleScore::new(5));
    assert_eq!(tabu.evaluations, 10);
    assert_eq!(tabu.accepted_moves, 3);
    assert_eq!(tabu.termination, Stop::AcceptedMoveLimit);
    let longer = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            max_accepted_moves: 4,
            ..cfg
        },
    )
    .unwrap();
    assert_eq!(longer.accepted_moves, 4); // The fourth selected step scores -10.
    assert_eq!(longer.best, tabu.best);
    assert_eq!(tabu, local_search(&problem, &network, cfg).unwrap());
}

#[test]
fn best_selection_budget_prefixes_keep_cached_winner_without_extra_callbacks() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 3, 2])]).unwrap();
    let calls = Cell::new(0);
    let mut network = ConstraintNetwork::<i64, SimpleScore>::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(*v[0].unwrap())
        })
        .unwrap();
    for (budget, best, accepted, stop) in [
        (0, None, 0, Stop::EvaluationLimit),
        (1, Some(0), 0, Stop::EvaluationLimit),
        (2, Some(1), 1, Stop::EvaluationLimit),
        (3, Some(3), 1, Stop::EvaluationLimit),
        (4, Some(3), 1, Stop::AcceptedMoveLimit),
    ] {
        calls.set(0);
        let result = local_search(
            &problem,
            &network,
            LocalSearchConfig {
                max_evaluations: budget,
                max_accepted_moves: 1,
                assert_scores: false,
                ..config()
            },
        )
        .unwrap();
        assert_eq!(result.best.map(|s| s.score), best.map(SimpleScore::new));
        assert_eq!(result.evaluations, budget);
        assert_eq!(result.constraint_evaluations, u128::from(budget));
        assert_eq!(calls.get(), budget);
        assert_eq!(result.accepted_moves, accepted);
        assert_eq!(result.termination, stop);
    }
}

#[test]
fn first_and_best_selection_differ_and_ties_keep_first_candidate() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3])]).unwrap();
    let mut network = ConstraintNetwork::<i64, SimpleScore>::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            SimpleScore::new((*v[0].unwrap()).min(2))
        })
        .unwrap();
    for (selection, value, evaluations) in [
        (MoveSelection::FirstAccepted, 1, 2),
        (MoveSelection::BestAccepted, 2, 4),
    ] {
        let result = local_search(
            &problem,
            &network,
            LocalSearchConfig {
                selection,
                max_accepted_moves: 1,
                ..config()
            },
        )
        .unwrap();
        assert_eq!(result.best.unwrap().assignment.values(), &[Some(value)]);
        assert_eq!(result.evaluations, evaluations);
    }
}

#[test]
fn tabu_aspiration_is_strict_tracks_global_best_and_refreshes_keys() {
    let mut state = Acceptance::VariableTabu {
        tenure: 2,
        aspiration: true,
    }
    .start(SimpleScore::ZERO)
    .unwrap();
    state.step_ended(SimpleScore::new(1), &[0]);
    for _ in 0..10 {
        assert!(!state.accepts(SimpleScore::new(-10), SimpleScore::new(1), &[0]));
        assert!(state.accepts(SimpleScore::new(1), SimpleScore::new(2), &[0]));
    }
    state.step_ended(SimpleScore::new(2), &[0, 0]);
    state.step_ended(SimpleScore::new(-10), &[1]);
    assert!(!state.accepts(SimpleScore::new(-10), SimpleScore::new(1), &[0]));
    assert!(!state.accepts(SimpleScore::new(-10), SimpleScore::new(2), &[0, 1]));
    assert!(state.accepts(SimpleScore::new(-10), SimpleScore::new(3), &[0, 1]));
    state.step_ended(SimpleScore::new(-20), &[2]);
    assert!(state.accepts(SimpleScore::new(-20), SimpleScore::new(-30), &[0]));
    let mut disabled = TabuMemory::new(1, false).unwrap();
    disabled.step_ended(&[0]);
    assert!(!disabled.accepts(&[0], SimpleScore::new(100), SimpleScore::ZERO));
}

#[test]
fn memory_matches_independent_recent_step_log_including_duplicates_and_empty_steps() {
    for tenure in [1, 2, 7] {
        let mut memory = TabuMemory::new(tenure, false).unwrap();
        let mut steps: Vec<Vec<usize>> = Vec::new();
        for step in 0..1000 {
            for key in 0..11 {
                let expected = steps
                    .iter()
                    .rev()
                    .take(tenure)
                    .any(|keys| keys.contains(&key));
                assert_eq!(
                    memory.is_tabu(&key),
                    expected,
                    "tenure {tenure}, step {step}, key {key}"
                );
            }
            let keys = if step % 5 == 0 {
                vec![]
            } else {
                vec![step % 11, (step * 7) % 11, step % 11]
            };
            memory.step_ended(&keys);
            steps.push(keys);
        }
    }
    let mut huge = TabuMemory::new(usize::MAX, true).unwrap();
    huge.step_ended(&[0]);
    for _ in 0..10 {
        huge.step_ended(&[]);
    }
    assert!(huge.is_tabu(&0));
}

#[test]
fn touched_variables_include_swaps_nested_composites_and_duplicate_writes() {
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 3,
            value: Some(0),
        },
        Move::Swap { left: 1, right: 3 },
        Move::Composite(vec![
            Move::Swap { left: 2, right: 2 },
            Move::Composite(vec![]),
        ]),
    ]);
    assert_eq!(movement.touched_variables(), vec![1, 2, 3]);
    assert!(
        Move::<i32>::Composite(vec![])
            .touched_variables()
            .is_empty()
    );
}

#[test]
fn tabu_zero_is_rejected_before_scoring_and_exhausted_tabu_neighborhood_stops() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let calls = Cell::new(0);
    let mut network = ConstraintNetwork::<i64, SimpleScore>::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(*v[0].unwrap())
        })
        .unwrap();
    let invalid = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            max_evaluations: 0,
            acceptance: Acceptance::VariableTabu {
                tenure: 0,
                aspiration: true,
            },
            ..config()
        },
    );
    assert!(matches!(
        invalid,
        Err(LocalSearchError::Acceptance(AcceptanceError::EmptyTabu))
    ));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        TabuMemory::<usize>::new(0, false),
        Err(AcceptanceError::EmptyTabu)
    ));
    let stopped = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 2,
                aspiration: true,
            },
            ..config()
        },
    )
    .unwrap();
    assert_eq!(stopped.accepted_moves, 1);
    assert_eq!(stopped.evaluations, 3);
    assert_eq!(stopped.termination, Stop::NoAcceptedMove);
    assert_eq!(stopped.best.unwrap().score, SimpleScore::new(1));
}

#[test]
fn seeded_tabu_runs_respect_pins_optional_values_budgets_and_independent_score() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(1).with_pinned(true),
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![1, 2]),
        Variable::new(vec![0, 2]),
    ])
    .unwrap();
    let mut network = ConstraintNetwork::new(4);
    network
        .add_constraint("adjacent", vec![0, 1, 2, 3], |v| {
            SimpleScore::new(
                -(v.windows(2)
                    .filter(|pair| pair[0].is_some() && pair[0] == pair[1])
                    .count() as i64),
            )
        })
        .unwrap();
    for seed in 0..32 {
        let cfg = LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 1,
                aspiration: true,
            },
            neighborhood: Neighborhood::ChangeAndSwap,
            max_evaluations: 80,
            max_accepted_moves: 9,
            move_order: MoveOrder::ShuffledVariables { seed },
            ..config()
        };
        let result = local_search(&problem, &network, cfg).unwrap();
        assert_eq!(result, local_search(&problem, &network, cfg).unwrap());
        assert!(result.evaluations <= 80);
        assert!(result.accepted_moves <= 9);
        let best = result.best.unwrap();
        assert!(problem.is_complete(&best.assignment));
        assert_eq!(best.assignment.values()[0], Some(1));
        let independently_scored = -(best
            .assignment
            .values()
            .windows(2)
            .filter(|pair| pair[0].is_some() && pair[0] == pair[1])
            .count() as i64);
        assert_eq!(best.score, SimpleScore::new(independently_scored));
        assert!(best.score >= SimpleScore::new(-1));
    }
}

#[test]
fn selected_swap_makes_both_variables_tabu() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("swap preferred", vec![0, 1], |v| {
            SimpleScore::new(match v {
                [Some(0), Some(1)] => 0,
                [Some(1), Some(0)] => 2,
                _ => -100,
            })
        })
        .unwrap();
    let result = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 1,
                aspiration: true,
            },
            neighborhood: Neighborhood::ChangeAndSwap,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(
        result.best.unwrap().assignment.values(),
        &[Some(1), Some(0)]
    );
    assert_eq!(result.accepted_moves, 1);
    assert_eq!(result.evaluations, 9);
    assert_eq!(result.termination, Stop::NoAcceptedMove);
}

#[test]
fn best_selection_installs_only_winners_sparse_contributions() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    for (i, weight) in [10_i64, 1].into_iter().enumerate() {
        network
            .add_constraint(format!("weight{i}"), vec![i], move |v| {
                SimpleScore::new(weight * *v[0].unwrap())
            })
            .unwrap();
    }
    let result = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            max_accepted_moves: 1,
            ..config()
        },
    )
    .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(1), Some(0)]);
    assert_eq!(best.score, SimpleScore::new(10));
    assert_eq!(result.evaluations, 3);
    assert_eq!(result.constraint_evaluations, 4);
}

#[test]
fn rejected_fallback_is_explicit_and_completed_step_limits_bound_it() {
    use timetuck::RejectedMovePolicy;
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut network = ConstraintNetwork::<i64, SimpleScore>::new(1);
    network
        .add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
        .unwrap();
    let cfg = LocalSearchConfig {
        max_accepted_moves: u64::MAX,
        max_steps: 3,
        rejected_moves: RejectedMovePolicy::BestScore,
        ..config()
    };
    let result = local_search(&problem, &network, cfg).unwrap();
    assert_eq!(result.completed_steps, 3);
    assert_eq!(result.accepted_moves, 2);
    assert_eq!(result.evaluations, 4);
    assert_eq!(result.termination, Stop::StepLimit);
    assert_eq!(result.best.unwrap().score, SimpleScore::new(1));
    let zero = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            max_steps: 0,
            ..cfg
        },
    )
    .unwrap();
    assert_eq!(zero.evaluations, 1);
    assert_eq!(zero.completed_steps, 0);
    assert_eq!(zero.termination, Stop::StepLimit);
}

#[test]
fn accepted_candidate_outranks_better_rejected_fallback() {
    use timetuck::RejectedMovePolicy;
    // After the first step selects variable 0, changing it back is tabu but
    // scores higher than changing variable 1. The accepted worsening candidate
    // must still win; the next step can then reach the better solution.
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("rank acceptance first", vec![0, 1], |v| {
            SimpleScore::new(match v {
                [Some(0), Some(0)] => 0,
                [Some(1), Some(0)] => 3,
                [Some(1), Some(1)] => -1,
                _ => 5,
            })
        })
        .unwrap();
    let result = local_search(
        &problem,
        &network,
        LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 1,
                aspiration: true,
            },
            selection: MoveSelection::FirstAccepted,
            rejected_moves: RejectedMovePolicy::BestScore,
            max_accepted_moves: u64::MAX,
            max_steps: 3,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(result.best.unwrap().score, SimpleScore::new(5));
    assert_eq!(result.completed_steps, 3);
    assert_eq!(result.accepted_moves, 3);
}
