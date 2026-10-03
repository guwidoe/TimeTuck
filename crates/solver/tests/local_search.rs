// Original local-search regressions; no additional upstream coverage credit.
use std::cell::Cell;
use timetuck::{
    Acceptance, ConstraintNetwork, FiniteProblem, HardSoftScore, LocalSearchConfig,
    LocalSearchTermination as Stop, Neighborhood, SimpleScore, Variable, exhaustive_search,
    local_search,
};
fn config<S>() -> LocalSearchConfig<'static, S> {
    LocalSearchConfig {
        assert_scores: true,
        ..LocalSearchConfig::default()
    }
}

#[test]
fn separable_problem_reaches_known_optimum_with_incremental_evaluations() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 20]).unwrap();
    let mut network = ConstraintNetwork::new(20);
    for i in 0..20 {
        network
            .add_constraint(format!("preference{i}"), vec![i], |v| {
                SimpleScore::new(i64::from(*v[0].unwrap()))
            })
            .unwrap();
    }
    let result = local_search(
        &model,
        &network,
        LocalSearchConfig {
            neighborhood: Neighborhood::Change,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(result.termination, Stop::NoAcceptedMove);
    assert_eq!(result.accepted_moves, 40);
    assert_eq!(result.best.as_ref().unwrap().score, SimpleScore::new(40));
    assert_eq!(result.best.unwrap().assignment.values(), &[Some(2); 20]);
    assert_eq!(
        result.constraint_evaluations,
        20 + u128::from(result.evaluations - 1)
    );
    assert!(result.constraint_evaluations < u128::from(result.evaluations) * 20);
    assert!(
        model
            .initial_assignment()
            .values()
            .iter()
            .all(Option::is_none)
    );
}
#[test]
fn swap_can_improve_when_every_single_change_breaks_a_hard_constraint() {
    let model = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("distinct", vec![0, 1], |v| {
            HardSoftScore::new(-i64::from(v[0] == v[1]), 0)
        })
        .unwrap();
    network
        .add_constraint("preference", vec![0, 1], |v| {
            HardSoftScore::new(0, i64::from(v[0] == Some(&1)) + i64::from(v[1] == Some(&0)))
        })
        .unwrap();
    let without = local_search(
        &model,
        &network,
        LocalSearchConfig {
            neighborhood: Neighborhood::Change,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(without.best.unwrap().score, HardSoftScore::ZERO);
    let with = local_search(&model, &network, config()).unwrap();
    assert_eq!(with.best.unwrap().score, HardSoftScore::new(0, 2));
    assert_eq!(with.accepted_moves, 1);
}
#[test]
fn local_termination_is_not_a_global_optimum_claim() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("trap", vec![0, 1], |v| {
            SimpleScore::new(if v == [Some(&1), Some(&1)] {
                100
            } else if v == [Some(&0), Some(&0)] {
                0
            } else {
                -1
            })
        })
        .unwrap();
    let result = local_search(&model, &network, config()).unwrap();
    assert_eq!(result.termination, Stop::NoAcceptedMove);
    assert_eq!(result.best.unwrap().score, SimpleScore::ZERO);
    let exact = exhaustive_search(&model, |s| network.calculate(&model, s).unwrap(), 4);
    assert_eq!(exact.best.unwrap().score, SimpleScore::new(100));
}
#[test]
fn budgets_include_initial_evaluation_and_preserve_best_so_far() {
    let calls = Cell::new(0);
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    let mut cfg = LocalSearchConfig {
        max_evaluations: 0,
        assert_scores: false,
        ..config()
    };
    let zero = local_search(&model, &network, cfg).unwrap();
    assert!(zero.best.is_none());
    assert_eq!(calls.get(), 0);
    cfg.max_evaluations = 1;
    let first = local_search(&model, &network, cfg).unwrap();
    assert_eq!(first.evaluations, 1);
    assert_eq!(first.termination, Stop::EvaluationLimit);
    cfg.max_evaluations = 2;
    let next = local_search(&model, &network, cfg).unwrap();
    assert_eq!(next.evaluations, 2);
    assert_eq!(next.best.unwrap().score, SimpleScore::new(1));
    assert_eq!(next.termination, Stop::EvaluationLimit);
    cfg.max_accepted_moves = 1;
    assert_eq!(
        local_search(&model, &network, cfg).unwrap().termination,
        Stop::AcceptedMoveLimit
    );
    cfg.max_accepted_moves = 0;
    let no_steps = local_search(&model, &network, cfg).unwrap();
    assert_eq!(no_steps.evaluations, 1);
    assert_eq!(no_steps.accepted_moves, 0);
}
#[test]
fn plateau_acceptance_is_bounded_and_keeps_earliest_best() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let network = ConstraintNetwork::<_, SimpleScore>::new(1);
    let result = local_search(
        &model,
        &network,
        LocalSearchConfig {
            acceptance: Acceptance::HillClimbing,
            max_accepted_moves: 3,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(result.termination, Stop::AcceptedMoveLimit);
    assert_eq!(result.evaluations, 4);
    assert_eq!(result.accepted_moves, 3);
    assert_eq!(result.best.unwrap().assignment.values(), &[Some(0)]);
    let strict = local_search(&model, &network, config()).unwrap();
    assert_eq!(strict.accepted_moves, 0);
    assert_eq!(strict.termination, Stop::NoAcceptedMove);
}
#[test]
fn pins_nullable_domains_and_heterogeneous_ranges_are_respected() {
    let model = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![1, 2]),
        Variable::new(vec![2]).allowing_unassigned(),
    ])
    .unwrap();
    let mut network = ConstraintNetwork::new(3);
    for i in 0..3 {
        network
            .add_constraint(format!("prefer{i}"), vec![i], |v| {
                SimpleScore::new(v[0].map_or(3, |&v| i64::from(v)))
            })
            .unwrap();
    }
    let result = local_search(&model, &network, config()).unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(0), Some(2), None]);
    assert_eq!(best.score, SimpleScore::new(5));
    assert!(model.is_complete(&best.assignment));
}
#[test]
fn exhausted_neighborhood_at_exact_budget_and_empty_spaces_are_truthful() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(1)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    let result = local_search(
        &model,
        &network,
        LocalSearchConfig {
            max_evaluations: 2,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(result.evaluations, 2);
    assert_eq!(result.termination, Stop::NoAcceptedMove);
    let impossible = FiniteProblem::new(vec![Variable::<i32>::new(vec![])]).unwrap();
    let result = local_search(&impossible, &network, config()).unwrap();
    assert!(result.best.is_none());
    assert_eq!(result.evaluations, 0);
    assert_eq!(result.termination, Stop::NoCompleteAssignment);
    let empty = FiniteProblem::<i32>::new(vec![]).unwrap();
    let empty_network = ConstraintNetwork::<_, SimpleScore>::new(0);
    let result = local_search(
        &empty,
        &empty_network,
        LocalSearchConfig {
            max_evaluations: 1,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(result.termination, Stop::NoAcceptedMove);
    assert!(result.best.unwrap().assignment.values().is_empty());
}

#[test]
fn generated_graph_results_obey_independent_quality_and_neighborhood_checks() {
    for seed in 0..32 {
        let edges: Vec<_> = (0..4)
            .flat_map(|a| {
                ((a + 1)..4)
                    .filter_map(move |b| ((a * 7 + b * 3 + seed) % 3 != 0).then_some((a, b)))
            })
            .collect();
        let model = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 4]).unwrap();
        let mut network = ConstraintNetwork::new(4);
        for &(a, b) in &edges {
            network
                .add_constraint(format!("edge{a}-{b}"), vec![a, b], |v| {
                    HardSoftScore::new(-i64::from(v[0] == v[1]), 0)
                })
                .unwrap();
        }
        for i in 0..4 {
            network
                .add_constraint(format!("preference{i}"), vec![i], move |v| {
                    HardSoftScore::new(0, i64::from(v[0] == Some(&((i + seed) % 3))))
                })
                .unwrap();
        }
        let score = |values: &[usize]| {
            HardSoftScore::new(
                -(edges
                    .iter()
                    .filter(|&&(a, b)| values[a] == values[b])
                    .count() as i64),
                values
                    .iter()
                    .enumerate()
                    .filter(|&(i, &v)| v == (i + seed) % 3)
                    .count() as i64,
            )
        };
        let optimum = (0..81)
            .map(|code| {
                let values: Vec<_> = (0..4).map(|i| (code / 3_usize.pow(i)) % 3).collect();
                score(&values)
            })
            .max()
            .unwrap();
        let result = local_search(&model, &network, config()).unwrap();
        assert_eq!(result.termination, Stop::NoAcceptedMove);
        let best = result.best.unwrap();
        let values: Vec<_> = best
            .assignment
            .values()
            .iter()
            .map(|v| v.unwrap())
            .collect();
        assert_eq!(score(&values), best.score);
        assert!(best.score >= score(&[0; 4]));
        assert!(best.score <= optimum);
        for i in 0..4 {
            for value in 0..3 {
                let mut changed = values.clone();
                changed[i] = value;
                assert!(score(&changed) <= best.score);
            }
            for j in 0..4 {
                let mut swapped = values.clone();
                swapped.swap(i, j);
                assert!(score(&swapped) <= best.score);
            }
        }
    }
}
