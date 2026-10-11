// Original regression cases; no upstream compatibility credit.
use std::cell::Cell;
use timetuck::{
    ConstraintNetwork, ConstructionConfig, ConstructionPickEarly as Pick,
    ConstructionTermination as Stop, FiniteProblem, HardSoftScore, ScoringError, SimpleScore,
    Variable, greedy_construction,
};
fn config(budget: u64) -> ConstructionConfig<'static> {
    ConstructionConfig {
        max_evaluations: budget,
        assert_scores: true,
        ..ConstructionConfig::default()
    }
}

#[test]
fn best_choices_use_one_base_and_install_cached_scores_without_replay() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![2, 3, 1]); 2]).unwrap();
    let calls = Cell::new(0);
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("sum", vec![0, 1], |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(v.iter().filter_map(|v| v.copied()).sum())
        })
        .unwrap();
    let result = greedy_construction(
        &problem,
        &network,
        ConstructionConfig {
            assert_scores: false,
            ..config(7)
        },
    )
    .unwrap();
    assert_eq!(result.assignment.values(), &[Some(3), Some(3)]);
    assert_eq!(result.score, Some(SimpleScore::new(6)));
    assert_eq!(result.evaluations, 7);
    assert_eq!(result.constraint_evaluations, 7);
    assert_eq!(calls.get(), 7); // No callback replay for either winning choice.
    assert_eq!(result.assigned_variables, 2);
    assert_eq!(result.termination, Stop::Completed);
    assert!(result.complete);
    assert_eq!(
        result.solution().unwrap().score,
        network.calculate(&problem, &result.assignment).unwrap()
    );
}

#[test]
fn every_budget_boundary_reports_partial_and_complete_progress_honestly() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    for i in 0..2 {
        network
            .add_constraint(format!("v{i}"), vec![i], |v| {
                SimpleScore::new(v[0].copied().unwrap_or(0))
            })
            .unwrap();
    }
    let expected = [
        (vec![None, None], None, false, 0, Stop::EvaluationLimit),
        (
            vec![None, None],
            Some(SimpleScore::new(0).with_structural(-2)),
            false,
            0,
            Stop::EvaluationLimit,
        ),
        (
            vec![Some(0), None],
            Some(SimpleScore::new(0).with_structural(-1)),
            false,
            1,
            Stop::EvaluationLimit,
        ),
        (
            vec![Some(1), None],
            Some(SimpleScore::new(1).with_structural(-1)),
            false,
            1,
            Stop::EvaluationLimit,
        ),
        (
            vec![Some(1), Some(0)],
            Some(SimpleScore::new(1)),
            true,
            2,
            Stop::EvaluationLimit,
        ),
        (
            vec![Some(1), Some(1)],
            Some(SimpleScore::new(2)),
            true,
            2,
            Stop::Completed,
        ),
    ];
    for (budget, (values, score, complete, assigned, termination)) in
        expected.into_iter().enumerate()
    {
        let result = greedy_construction(&problem, &network, config(budget as u64)).unwrap();
        assert_eq!(result.assignment.values(), values, "budget {budget}");
        assert_eq!(result.score, score);
        assert_eq!(result.complete, complete);
        assert_eq!(result.assigned_variables, assigned);
        assert_eq!(result.evaluations, budget as u64);
        assert_eq!(
            result.constraint_evaluations,
            if budget == 0 { 0 } else { budget as u128 + 1 }
        );
        assert_eq!(result.termination, termination);
        assert_eq!(result.solution().is_some(), complete);
    }
}

#[test]
fn ties_preserve_domain_order_and_assigned_or_optional_values_are_preserved() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(2),
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![1, 2]).allowing_unassigned(),
        Variable::new(vec![])
            .allowing_unassigned()
            .with_pinned(true),
        Variable::new(vec![9, 8]),
    ])
    .unwrap();
    let network = ConstraintNetwork::<_, SimpleScore>::new(5);
    let result = greedy_construction(&problem, &network, config(3)).unwrap();
    assert_eq!(
        result.assignment.values(),
        &[Some(2), Some(1), None, None, Some(9)]
    );
    assert_eq!(result.assigned_variables, 1);
    assert_eq!(result.score, Some(SimpleScore::ZERO));
    assert_eq!(result.termination, Stop::Completed);
}

#[test]
fn no_callbacks_for_zero_budget_or_impossible_space_and_empty_model_is_complete() {
    let calls = Cell::new(0);
    let mut network = ConstraintNetwork::<i32, SimpleScore>::new(1);
    network
        .add_constraint("constant", vec![], |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
    for domain in [vec![], vec![1]] {
        let model = FiniteProblem::new(vec![Variable::new(domain.clone())]).unwrap();
        let result = greedy_construction(&model, &network, config(0)).unwrap();
        assert_eq!(
            result.termination,
            if domain.is_empty() {
                Stop::NoCompleteAssignment
            } else {
                Stop::EvaluationLimit
            }
        );
        assert!(result.solution().is_none());
        if domain.is_empty() {
            assert_eq!(
                greedy_construction(&model, &network, config(100))
                    .unwrap()
                    .evaluations,
                0
            );
        }
    }
    assert_eq!(calls.get(), 0);
    let empty = FiniteProblem::<i32>::new(vec![]).unwrap();
    let network = ConstraintNetwork::<_, SimpleScore>::new(0);
    let zero = greedy_construction(&empty, &network, config(0)).unwrap();
    assert!(zero.complete);
    assert_eq!(zero.score, None);
    assert!(zero.solution().is_none());
    let scored = greedy_construction(&empty, &network, config(1)).unwrap();
    assert_eq!(scored.termination, Stop::Completed);
    assert_eq!(scored.solution().unwrap().score, SimpleScore::ZERO);
}

#[test]
fn early_pick_respects_hard_levels_and_reduces_actual_callback_work() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    for i in 0..2 {
        network
            .add_constraint(format!("v{i}"), vec![i], |v| match v[0] {
                None => HardSoftScore::new(0, 0),
                Some(0) => HardSoftScore::new(-1, 100),
                Some(1) => HardSoftScore::new(0, -10),
                Some(_) => HardSoftScore::new(0, 0),
            })
            .unwrap();
    }
    for pick in [
        Pick::FirstFeasibleScore,
        Pick::FirstFeasibleScoreOrNonDeterioratingHard,
    ] {
        let result = greedy_construction(
            &problem,
            &network,
            ConstructionConfig {
                pick_early: pick,
                ..config(5)
            },
        )
        .unwrap();
        assert_eq!(result.assignment.values(), &[Some(1), Some(1)]);
        assert_eq!(result.evaluations, 5);
        assert_eq!(result.constraint_evaluations, 6);
        assert_eq!(result.termination, Stop::Completed);
    }
    let result = greedy_construction(
        &problem,
        &network,
        ConstructionConfig {
            pick_early: Pick::FirstNonDeterioratingScore,
            ..config(7)
        },
    )
    .unwrap();
    assert_eq!(result.assignment.values(), &[Some(2), Some(2)]);
    assert_eq!(result.termination, Stop::Completed);
}

#[test]
fn infeasible_complete_solution_is_retained_and_network_mismatch_is_an_error() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("hard", vec![0], |v| {
            HardSoftScore::new(v[0].copied().unwrap_or(0) - 2, 0)
        })
        .unwrap();
    let result = greedy_construction(&problem, &network, config(3)).unwrap();
    assert_eq!(result.solution().unwrap().score, HardSoftScore::new(-1, 0));
    assert!(!result.score.unwrap().is_feasible());
    let wrong = ConstraintNetwork::<_, SimpleScore>::new(0);
    assert!(matches!(
        greedy_construction(&problem, &wrong, config(0)),
        Err(ScoringError::VariableCount { .. })
    ));
}

#[test]
fn constructed_solution_enters_local_search_with_explicit_combined_work() {
    use timetuck::{LocalSearchConfig, local_search_from};
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let calls = Cell::new(0);
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("coupled", vec![0, 1], |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(match (v[0].copied(), v[1].copied()) {
                (Some(0), None) => 1,
                (Some(0), Some(0)) => 2,
                (Some(0), Some(1)) => 3,
                (Some(1), Some(1)) => 4,
                _ => 0,
            })
        })
        .unwrap();
    let constructed = greedy_construction(
        &problem,
        &network,
        ConstructionConfig {
            assert_scores: false,
            ..config(5)
        },
    )
    .unwrap();
    assert_eq!(constructed.assignment.values(), &[Some(0), Some(1)]);
    assert_eq!(constructed.score, Some(SimpleScore::new(3)));
    let improved = local_search_from(
        &problem,
        &network,
        constructed.solution().unwrap().assignment,
        LocalSearchConfig::default(),
    )
    .unwrap();
    assert_eq!(improved.best.unwrap().score, SimpleScore::new(4));
    assert_eq!(improved.accepted_moves, 1);
    assert_eq!(constructed.evaluations + improved.evaluations, calls.get());
    assert_eq!(
        constructed.constraint_evaluations + improved.constraint_evaluations,
        u128::from(calls.get())
    );
    assert_eq!(problem.initial_assignment().values(), &[None, None]);
}

#[test]
fn explicit_local_search_rejects_incomplete_or_invalid_assignments_before_scoring() {
    use timetuck::{Assignment, LocalSearchConfig, LocalSearchError, local_search_from};
    let problem = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let mut network = ConstraintNetwork::<_, SimpleScore>::new(1);
    network
        .add_constraint("must not run", vec![], |_| panic!("unexpected scoring"))
        .unwrap();
    let config = LocalSearchConfig {
        max_evaluations: 0,
        ..LocalSearchConfig::default()
    };
    assert!(matches!(
        local_search_from(
            &problem,
            &network,
            problem.initial_assignment(),
            config.clone()
        ),
        Err(LocalSearchError::IncompleteInitialAssignment)
    ));
    assert!(matches!(
        local_search_from(
            &problem,
            &network,
            Assignment::new(vec![Some(2)]),
            config.clone()
        ),
        Err(LocalSearchError::Scoring(ScoringError::Model(_)))
    ));
    let valid =
        local_search_from(&problem, &network, Assignment::new(vec![Some(1)]), config).unwrap();
    assert_eq!(valid.evaluations, 0);
    assert!(valid.best.is_none());
}

#[test]
fn incremental_greedy_choices_match_independent_full_rescoring_with_interactions() {
    use timetuck::Assignment;
    for seed in 0..24_i64 {
        let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 4]).unwrap();
        let mut network = ConstraintNetwork::new(4);
        network
            .add_constraint("constant", vec![], |_| SimpleScore::new(13))
            .unwrap();
        for i in 0..4 {
            network
                .add_constraint(format!("pair{i}"), vec![i, (i + 1) % 4], move |v| {
                    let a = v[0].copied().unwrap_or(-1);
                    let b = v[1].copied().unwrap_or(-1);
                    SimpleScore::new(((a + 2) * (b + 3) * (seed + i as i64 + 1)) % 11 - 5)
                })
                .unwrap();
        }
        let mut expected = vec![None; 4];
        for i in 0..4 {
            let mut best = None;
            for value in 0..3 {
                expected[i] = Some(value);
                let score = network
                    .calculate(&problem, &Assignment::new(expected.clone()))
                    .unwrap();
                if best.is_none_or(|(_, prior)| score > prior) {
                    best = Some((value, score));
                }
            }
            expected[i] = Some(best.unwrap().0);
        }
        let result = greedy_construction(&problem, &network, config(13)).unwrap();
        assert_eq!(result.assignment.values(), expected, "seed {seed}");
        assert_eq!(
            result.score.unwrap(),
            network.calculate(&problem, &result.assignment).unwrap()
        );
        assert_eq!(result.evaluations, 13);
        assert_eq!(result.constraint_evaluations, 5 + 12 * 2);
        assert_eq!(result.termination, Stop::Completed);
    }
}
