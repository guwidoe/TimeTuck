//! Original reference, budget, model and failure tests; no upstream-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};
use timetuck::*;
#[path = "support/owned_score.rs"]
mod owned_score;
use owned_score::{OwnedScore, fail_clone_at};
const ORDERS: [NodeExploration; 4] = [
    NodeExploration::DepthFirst,
    NodeExploration::BreadthFirst,
    NodeExploration::ScoreFirst,
    NodeExploration::OptimisticBoundFirst,
];
fn config(order: NodeExploration) -> BranchAndBoundConfig<'static> {
    BranchAndBoundConfig {
        exploration: order,
        ..Default::default()
    }
}
fn sum(a: &Assignment<i64>) -> i64 {
    a.values().iter().flatten().sum()
}
#[test]
fn bounded_tree_matches_exhaustive_optima_and_prunes() {
    for order in ORDERS {
        for count in 1..7 {
            let p = FiniteProblem::new(vec![Variable::new(vec![2, 1, 0]); count]).unwrap();
            let exact = exhaustive_search(&p, |a| SimpleScore::new(sum(a)), u64::MAX);
            let r = branch_and_bound(
                &p,
                |a, left| {
                    BranchEvaluation::bounded(
                        SimpleScore::new(sum(a)),
                        SimpleScore::new(sum(a) + 2 * left.len() as i64),
                    )
                },
                config(order),
            )
            .unwrap();
            assert!(r.is_optimal());
            assert_eq!(r.best, exact.best);
            assert_eq!(r.complete_evaluations, 1);
            assert!(r.pruned_branches > 0);
            assert!(r.evaluations < 1 + (3usize.pow(count as u32 + 1) - 1) as u64 / 2);
        }
    }
}
#[test]
fn unknown_bounds_match_every_complete_assignment_and_mixed_sign_scores() {
    for seed in 0..32 {
        for order in ORDERS {
            let p = FiniteProblem::new(vec![Variable::new(vec![-2, 0, 3]); 4]).unwrap();
            let value = |a: &Assignment<i64>| {
                let v = a.values();
                let x = v[0].unwrap_or(0);
                let y = v[1].unwrap_or(0);
                let z = v[2].unwrap_or(0);
                let w = v[3].unwrap_or(0);
                HardSoftScore::new(-((x + y + seed) % 7).abs(), x * z - y * w + seed)
            };
            let expected = exhaustive_search(&p, value, u64::MAX);
            let mut leaves = Vec::new();
            let r = branch_and_bound(
                &p,
                |a, left| {
                    if left.is_empty() {
                        leaves.push(a.clone());
                    }
                    BranchEvaluation::unbounded(value(a))
                },
                config(order),
            )
            .unwrap();
            assert!(r.is_optimal());
            assert_eq!(r.best.unwrap().score, expected.best.unwrap().score);
            assert_eq!(r.evaluations, 121);
            assert_eq!(r.complete_evaluations, 81);
            assert_eq!(r.pruned_branches, 0);
            leaves.sort_by(|a, b| a.values().cmp(b.values()));
            leaves.dedup();
            assert_eq!(leaves.len(), 81);
        }
    }
}
#[test]
fn optional_none_pins_empty_ranges_and_initial_values() {
    for order in ORDERS {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![8]).with_initial(8).with_pinned(true),
            Variable::new(vec![1, 2]).with_initial(1),
            Variable::new(vec![3]).allowing_unassigned(),
            Variable::new(vec![]).allowing_unassigned(),
            Variable::new(vec![7])
                .allowing_unassigned()
                .with_pinned(true),
        ])
        .unwrap();
        let mut roots = 0;
        let r = branch_and_bound(
            &p,
            |a, left| {
                assert_eq!(a.values()[0], Some(8));
                assert_eq!(a.values()[4], None);
                if left == [1, 2, 3] {
                    roots += 1;
                    assert_eq!(a.values()[1], None);
                }
                for &i in left {
                    assert!(a.values()[i].is_none());
                }
                BranchEvaluation::unbounded(SimpleScore::new(sum(a)))
            },
            config(order),
        )
        .unwrap();
        assert_eq!(roots, 1);
        assert_eq!(r.complete_evaluations, 4);
        assert!(r.is_optimal());
        assert_eq!(
            r.best.unwrap().assignment.values(),
            &[Some(8), Some(2), Some(3), None, None]
        );
        assert_eq!(p.initial_assignment().values()[1], Some(1));
    }
    let p = FiniteProblem::<i64>::new(vec![Variable::new(vec![])]).unwrap();
    let r = branch_and_bound(
        &p,
        |_, _| -> BranchEvaluation<SimpleScore> { panic!("empty space") },
        BranchAndBoundConfig {
            max_evaluations: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, BranchTermination::Exhausted);
    assert!(!r.is_optimal());
    assert_eq!(r.evaluations, 0);
    let empty = FiniteProblem::<i64>::new(vec![]).unwrap();
    let r = branch_and_bound(
        &empty,
        |_, left| {
            assert!(left.is_empty());
            BranchEvaluation::unbounded(SimpleScore::ZERO)
        },
        BranchAndBoundConfig {
            max_frontier: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.is_optimal());
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.complete_evaluations, 1);
}
#[test]
fn all_evaluation_prefixes_preserve_best_and_do_not_claim_optimality() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 3]).unwrap();
    for order in ORDERS {
        let mut full = Vec::new();
        let complete = branch_and_bound(
            &p,
            |a, left| {
                full.push((a.clone(), left.is_empty()));
                BranchEvaluation::unbounded(SimpleScore::new(sum(a)))
            },
            config(order),
        )
        .unwrap();
        for budget in 0..=complete.evaluations {
            let mut prefix = Vec::new();
            let r = branch_and_bound(
                &p,
                |a, left| {
                    prefix.push((a.clone(), left.is_empty()));
                    BranchEvaluation::unbounded(SimpleScore::new(sum(a)))
                },
                BranchAndBoundConfig {
                    max_evaluations: budget,
                    ..config(order)
                },
            )
            .unwrap();
            assert_eq!(prefix, &full[..budget as usize]);
            assert_eq!(r.evaluations, budget);
            let expected = prefix
                .iter()
                .filter(|(_, leaf)| *leaf)
                .map(|(a, _)| sum(a))
                .max();
            assert_eq!(r.best.as_ref().map(|s| s.score.score()), expected);
            assert_eq!(r.is_optimal(), budget == complete.evaluations);
            assert_eq!(
                r.termination,
                if budget == complete.evaluations {
                    BranchTermination::Exhausted
                } else {
                    BranchTermination::EvaluationLimit
                }
            );
        }
    }
}
#[test]
fn frontier_limit_never_discards_work_and_claims_completion() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    for limit in 0..4 {
        let r = branch_and_bound(
            &p,
            |a, _| BranchEvaluation::unbounded(SimpleScore::new(sum(a))),
            BranchAndBoundConfig {
                max_frontier: limit,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.termination, BranchTermination::FrontierLimit);
        assert!(!r.is_optimal());
        assert!(r.peak_frontier <= limit);
    }
    let r = branch_and_bound(
        &p,
        |a, _| BranchEvaluation::unbounded(SimpleScore::new(sum(a))),
        BranchAndBoundConfig {
            max_frontier: 8,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.is_optimal());
}
#[test]
fn structural_rejection_is_only_for_leaves_and_positive_state_uses_full_order() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let r = branch_and_bound(
        &p,
        |a, left| {
            let structural = if !left.is_empty() || a.values()[1] == Some(2) {
                -1
            } else if a.values()[0] == Some(0) {
                1
            } else {
                0
            };
            BranchEvaluation::unbounded(SimpleScore::new(sum(a)).with_structural(structural))
        },
        Default::default(),
    )
    .unwrap();
    assert!(r.is_optimal());
    assert_eq!(r.best.unwrap().assignment.values(), &[Some(0), Some(1)]);
    let r = branch_and_bound(
        &p,
        |_, _| BranchEvaluation::unbounded(SimpleScore::ZERO.with_structural(-1)),
        Default::default(),
    )
    .unwrap();
    assert_eq!(r.termination, BranchTermination::Exhausted);
    assert!(r.best.is_none());
    assert!(!r.is_optimal());
}
#[test]
fn visited_leaf_checks_its_own_bound_and_all_ancestor_bounds() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 3]).unwrap();
    for bad_depth in 0..4 {
        let error = branch_and_bound(
            &p,
            |a, left| BranchEvaluation {
                score: SimpleScore::new(sum(a)),
                optimistic_bound: Some(SimpleScore::new(if 3 - left.len() == bad_depth {
                    2
                } else {
                    10
                })),
            },
            Default::default(),
        )
        .unwrap_err();
        assert_eq!(error.score, SimpleScore::new(3));
        assert_eq!(error.bound, SimpleScore::new(2));
    }
}
#[test]
fn cancellation_observation_and_final_leaf_exhaustion_precedence() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let token = CancellationToken::new();
    let calls = Cell::new(0);
    let r = branch_and_bound(
        &p,
        |a, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                token.cancel();
            }
            BranchEvaluation::unbounded(SimpleScore::new(sum(a)))
        },
        BranchAndBoundConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, BranchTermination::Cancelled);
    assert_eq!(r.evaluations, 3);
    assert_eq!(r.expanded_nodes, 1);
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let token = CancellationToken::new();
    let mut events = Vec::new();
    let r = branch_and_bound_with_observer(
        &p,
        |a, _| BranchEvaluation::unbounded(SimpleScore::new(sum(a))),
        BranchAndBoundConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
        |s| {
            events.push(s.clone());
            token.cancel();
        },
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(r.best, events.pop());
    assert_eq!(r.termination, BranchTermination::Cancelled);
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let token = CancellationToken::new();
    let r = branch_and_bound_with_observer(
        &p,
        |a, _| BranchEvaluation::unbounded(SimpleScore::new(sum(a))),
        BranchAndBoundConfig {
            cancellation: Some(&token),
            max_evaluations: 2,
            ..Default::default()
        },
        |_| {
            token.cancel();
        },
    )
    .unwrap();
    assert!(r.is_optimal());
    assert_eq!(r.termination, BranchTermination::Exhausted);
}
#[test]
fn zero_limits_do_not_invoke_callbacks() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let token = CancellationToken::new();
    token.cancel();
    for (settings, expected) in [
        (
            BranchAndBoundConfig {
                max_evaluations: 0,
                cancellation: Some(&token),
                max_time: Some(Duration::ZERO),
                ..Default::default()
            },
            BranchTermination::EvaluationLimit,
        ),
        (
            BranchAndBoundConfig {
                cancellation: Some(&token),
                max_time: Some(Duration::ZERO),
                ..Default::default()
            },
            BranchTermination::Cancelled,
        ),
        (
            BranchAndBoundConfig {
                max_time: Some(Duration::ZERO),
                ..Default::default()
            },
            BranchTermination::TimeLimit,
        ),
    ] {
        let r = branch_and_bound(
            &p,
            |_, _| -> BranchEvaluation<SimpleScore> { panic!("zero budget") },
            settings,
        )
        .unwrap();
        assert_eq!(r.termination, expected);
        assert_eq!(r.evaluations, 0);
    }
}
#[test]
fn owned_scores_and_panics_leave_the_problem_reusable() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 3]).unwrap();
    for order in ORDERS {
        let r = branch_and_bound(
            &p,
            |a, left| {
                BranchEvaluation::bounded(
                    OwnedScore::new(0, sum(a)),
                    OwnedScore::new(0, sum(a) + left.len() as i64),
                )
            },
            config(order),
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!(r.best.unwrap().score, OwnedScore::new(0, 3));
    }
    for at in 1..8 {
        fail_clone_at(at);
        let result = catch_unwind(AssertUnwindSafe(|| {
            branch_and_bound(
                &p,
                |a, _| BranchEvaluation::bounded(OwnedScore::new(0, sum(a)), OwnedScore::new(0, 3)),
                Default::default(),
            )
        }));
        fail_clone_at(0);
        assert!(result.is_err());
        assert_eq!(p.initial_assignment().values(), &[Some(0); 3]);
    }
    assert!(
        catch_unwind(AssertUnwindSafe(|| branch_and_bound_with_observer(
            &p,
            |a, _| BranchEvaluation::unbounded(SimpleScore::new(sum(a))),
            Default::default(),
            |_| panic!("observer")
        )))
        .is_err()
    );
    assert!(
        branch_and_bound(
            &p,
            |a, _| BranchEvaluation::unbounded(SimpleScore::new(sum(a))),
            Default::default()
        )
        .unwrap()
        .is_optimal()
    );
}

#[test]
fn generated_admissible_bounds_match_independent_complete_tables() {
    for seed in 0..24_i64 {
        let p = FiniteProblem::new(vec![Variable::new(vec![-2, 0, 3]).allowing_unassigned(); 3])
            .unwrap();
        let score = |a: &Assignment<i64>| {
            let x = a.values()[0].unwrap_or(-1);
            let y = a.values()[1].unwrap_or(1);
            let z = a.values()[2].unwrap_or(2);
            HardSoftScore::new(-((x + y + seed) % 5).abs(), x * z - y * y + seed)
                .with_structural(if x == 3 && y == 3 { -1 } else { 0 })
        };
        let mut table = Vec::new();
        let expected = exhaustive_search(
            &p,
            |a| {
                let s = score(a);
                table.push((a.clone(), s));
                s
            },
            u64::MAX,
        );
        for order in ORDERS {
            for loose in [false, true] {
                let r = branch_and_bound(
                    &p,
                    |a, left| {
                        let actual = score(a);
                        let bound = if loose && left.len() == 2 {
                            None
                        } else {
                            table
                                .iter()
                                .filter(|(full, s)| {
                                    s.structural() >= 0
                                        && (0..3).all(|i| {
                                            left.contains(&i) || full.values()[i] == a.values()[i]
                                        })
                                })
                                .map(|(_, s)| {
                                    if loose {
                                        HardSoftScore::new(s.hard() + 1, s.soft() + 19)
                                    } else {
                                        *s
                                    }
                                })
                                .max()
                        };
                        BranchEvaluation {
                            score: actual,
                            optimistic_bound: bound,
                        }
                    },
                    config(order),
                )
                .unwrap();
                assert!(r.is_optimal());
                assert_eq!(r.best.unwrap().score, expected.best.as_ref().unwrap().score);
            }
        }
    }
}
#[test]
fn partial_constraint_network_scores_and_bounds_drive_real_search() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2, 3]); 4]).unwrap();
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("reward", vec![0, 1, 2, 3], |v| {
        HardSoftScore::new(0, v.iter().filter_map(|v| v.copied()).sum())
    })
    .unwrap();
    for i in 0..3 {
        n.add_constraint(format!("collision {i}"), vec![i, i + 1], |v| {
            HardSoftScore::new(-i64::from(v[0].is_some() && v[0] == v[1]), 0)
        })
        .unwrap();
    }
    let exact = exhaustive_search(&p, |a| n.calculate(&p, a).unwrap(), u64::MAX);
    for order in ORDERS {
        let r = branch_and_bound(
            &p,
            |a, left| {
                let s = n.calculate(&p, a).unwrap();
                BranchEvaluation::bounded(
                    s,
                    HardSoftScore::new(s.hard(), s.soft() + 3 * left.len() as i64),
                )
            },
            config(order),
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!(
            r.best.as_ref().unwrap().score,
            exact.best.as_ref().unwrap().score
        );
        assert_eq!(
            n.calculate(&p, &r.best.as_ref().unwrap().assignment)
                .unwrap(),
            r.best.as_ref().unwrap().score
        );
        assert!(r.complete_evaluations < exact.evaluations);
    }
}
#[test]
fn score_order_is_exact_for_owned_decimal_levels_and_unknown_bounds() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let low: SimpleDecimalScore = "9007199254740992.000000000000000001".parse().unwrap();
    let high: SimpleDecimalScore = "9007199254740992.000000000000000002".parse().unwrap();
    for order in ORDERS {
        let r = branch_and_bound(
            &p,
            |a, _| {
                BranchEvaluation::bounded(
                    if a.values()[0] == Some(1) && a.values()[1] == Some(1) {
                        high.clone()
                    } else {
                        low.clone()
                    },
                    high.clone(),
                )
            },
            config(order),
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!(r.best.unwrap().score, high);
    }
    let a = SearchNodeKey {
        depth: 1,
        score: low.clone(),
        optimistic_bound: Some(low.clone()),
        parent_breadth: 0,
        breadth: 0,
    };
    let b = SearchNodeKey {
        score: high.clone(),
        ..a.clone()
    };
    for order in ORDERS {
        assert!(order.compare(&a, &b).is_lt());
    }
    let unknown = SearchNodeKey {
        optimistic_bound: None,
        ..a.clone()
    };
    assert!(
        NodeExploration::OptimisticBoundFirst
            .compare(&unknown, &b)
            .is_gt()
    );
}
