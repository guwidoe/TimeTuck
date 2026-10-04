//! Original independent enumeration and control checks; no upstream-method credit.
use std::{
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};
use timetuck::*;

const ORDERS: [NodeExploration; 4] = [
    NodeExploration::DepthFirst,
    NodeExploration::BreadthFirst,
    NodeExploration::ScoreFirst,
    NodeExploration::OptimisticBoundFirst,
];
fn config(order: NodeExploration) -> BranchAndBoundConfig<'static> {
    BranchAndBoundConfig {
        exploration: order,
        max_evaluations: u64::MAX,
        max_frontier: 100_000,
        ..Default::default()
    }
}
fn permutations(values: &[i32]) -> Vec<Vec<i32>> {
    if values.is_empty() {
        return vec![vec![]];
    }
    let mut out = Vec::new();
    for i in 0..values.len() {
        let mut rest = values.to_vec();
        let value = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, value);
            out.push(tail);
        }
    }
    out
}
// Independent subset/permutation/two-way partition construction, never insertion.
fn reference(p: &ListProblem<i32>) -> BTreeSet<Vec<Vec<i32>>> {
    assert_eq!(p.entities().len(), 2);
    let mut out = BTreeSet::new();
    for mask in 0..1usize << p.values().len() {
        if !p.allows_unassigned() && mask + 1 != 1 << p.values().len() {
            continue;
        }
        let selected: Vec<_> = p
            .values()
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (mask & (1 << i) != 0).then_some(*v))
            .collect();
        for order in permutations(&selected) {
            for cut in 0..=order.len() {
                let rows = vec![order[..cut].to_vec(), order[cut..].to_vec()];
                if rows.iter().zip(p.entities()).all(|(row, meta)| {
                    (!meta.is_pinned() || row == meta.initial())
                        && row.starts_with(&meta.initial()[..meta.pinned_prefix()])
                        && meta
                            .value_range()
                            .is_none_or(|range| row.iter().all(|v| range.contains(v)))
                }) {
                    out.insert(rows);
                }
            }
        }
    }
    out
}
fn score(a: &ListAssignment<i32>, seed: i64) -> HardSoftScore {
    let rows = a.lists();
    let assigned = rows.iter().map(Vec::len).sum::<usize>() as i64;
    let order: i64 = rows
        .iter()
        .enumerate()
        .map(|(e, r)| {
            r.iter()
                .enumerate()
                .map(|(i, v)| (e as i64 + 1) * (i as i64 + 1) * i64::from(*v))
                .sum::<i64>()
        })
        .sum();
    HardSoftScore::new(
        -((order + seed) % 5).abs(),
        order * (seed % 7 - 3) - assigned,
    )
}
fn range(mask: usize) -> Vec<i32> {
    [10, 20, 30]
        .into_iter()
        .enumerate()
        .filter_map(|(i, v)| (mask & (1 << i) != 0).then_some(v))
        .collect()
}
#[test]
fn every_range_pair_and_optionality_visits_exactly_the_independent_set() {
    for optional in [false, true] {
        for a in 0..8 {
            for b in 0..8 {
                let mut p = ListProblem::new(
                    vec![10, 20, 30],
                    vec![
                        ListEntity::new(vec![]).with_value_range(range(a)),
                        ListEntity::new(vec![]).with_value_range(range(b)),
                    ],
                )
                .unwrap();
                if optional {
                    p = p.allowing_unassigned();
                }
                let expected = reference(&p);
                let seed = (a * 8 + b) as i64;
                let best = expected
                    .iter()
                    .map(|r| score(&ListAssignment::new(r.clone()), seed))
                    .max();
                for order in ORDERS {
                    let mut seen = BTreeSet::new();
                    let r = branch_and_bound_lists(
                        &p,
                        |state, left| {
                            p.validate(state).unwrap();
                            for &i in left {
                                assert!(state.location(&p.values()[i]).is_none());
                            }
                            if left.is_empty() {
                                assert!(seen.insert(state.lists().to_vec()), "duplicate leaf");
                            }
                            BranchEvaluation::unbounded(score(state, seed))
                        },
                        config(order),
                    )
                    .unwrap();
                    assert_eq!(seen, expected);
                    assert_eq!(r.complete_evaluations as usize, expected.len());
                    assert_eq!(r.best.map(|b| b.score), best);
                    assert_eq!(r.termination, BranchTermination::Exhausted);
                }
            }
        }
    }
}
#[test]
fn fixed_lists_prefixes_and_movable_initial_values_do_not_restrict_the_tree() {
    for frozen in [false, true] {
        for optional in [false, true] {
            let first = if frozen {
                ListEntity::new(vec![10]).with_pinned(true)
            } else {
                ListEntity::new(vec![10, 20]).with_pinned_prefix(1)
            };
            let mut p = ListProblem::new(
                vec![30, 10, 20],
                vec![
                    first,
                    ListEntity::new(vec![]).with_value_range(vec![20, 30]),
                ],
            )
            .unwrap();
            if optional {
                p = p.allowing_unassigned();
            }
            let initial = p.initial_assignment();
            let expected = reference(&p);
            for order in ORDERS {
                let mut actual = BTreeSet::new();
                let mut root = None;
                let r = branch_and_bound_lists(
                    &p,
                    |a, left| {
                        if root.is_none() {
                            root = Some((a.clone(), left.to_vec()));
                        }
                        if left.is_empty() {
                            assert!(actual.insert(a.lists().to_vec()));
                        }
                        BranchEvaluation::unbounded(score(a, 3))
                    },
                    config(order),
                )
                .unwrap();
                assert!(r.is_optimal());
                assert_eq!(actual, expected);
                let (root, left) = root.unwrap();
                assert_eq!(root.lists(), &[vec![10], vec![]]);
                assert_eq!(left, vec![0, 2]); // registry indices, not value IDs
            }
            assert_eq!(p.initial_assignment(), initial);
        }
    }
}
// A prefix can be extended by inserting remaining values; delete those values from
// a full state to recover the prefix. Already decided optional omissions stay absent.
fn extends(
    full: &[Vec<i32>],
    partial: &ListAssignment<i32>,
    left: &[usize],
    p: &ListProblem<i32>,
) -> bool {
    full.iter()
        .map(|row| {
            row.iter()
                .filter(|v| !left.iter().any(|i| p.values()[*i] == **v))
                .copied()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        == partial.lists()
}
#[test]
fn table_bounds_preserve_position_and_neighbor_dependent_optima() {
    let p = ListProblem::new(
        vec![10, 20, 30],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap()
    .allowing_unassigned();
    let leaves = reference(&p);
    for seed in 0..12 {
        for order in ORDERS {
            for loose in [false, true] {
                let optimum = leaves
                    .iter()
                    .map(|a| score(&ListAssignment::new(a.clone()), seed))
                    .max()
                    .unwrap();
                let mut omitted = false;
                let r = branch_and_bound_lists(
                    &p,
                    |a, left| {
                        let bound = if loose && left.len() == 1 {
                            omitted = true;
                            None
                        } else {
                            leaves
                                .iter()
                                .filter(|full| extends(full, a, left, &p))
                                .map(|full| {
                                    let s = score(&ListAssignment::new(full.clone()), seed);
                                    HardSoftScore::new(s.hard() + i64::from(loose), s.soft())
                                })
                                .max()
                        };
                        BranchEvaluation {
                            score: score(a, seed),
                            optimistic_bound: bound,
                        }
                    },
                    config(order),
                )
                .unwrap();
                assert!(r.is_optimal());
                assert_eq!(r.best.unwrap().score, optimum);
                if !loose && order == NodeExploration::DepthFirst {
                    assert!(r.pruned_branches > 0);
                    assert!(r.complete_evaluations < leaves.len() as u64);
                }
                if loose {
                    assert!(omitted);
                }
            }
        }
    }
}
fn mixed_problem(optional: bool) -> MixedProblem<i64, i32> {
    let mut list = ListProblem::new(
        vec![10, 20, 30],
        vec![
            ListEntity::new(vec![10]).with_pinned_prefix(1),
            ListEntity::new(vec![]).with_value_range(vec![20, 30]),
        ],
    )
    .unwrap();
    if optional {
        list = list.allowing_unassigned();
    }
    MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1])
                .with_initial(1)
                .allowing_unassigned(),
            Variable::new(vec![7]).with_initial(7).with_pinned(true),
        ])
        .unwrap(),
        list,
    )
}
fn mixed_score(a: &MixedAssignment<i64, i32>, seed: i64) -> HardSoftScore {
    let owner = a.lists().location(&20).map_or(3, |p| p.entity as i64);
    let basic = a.basic().values()[0].unwrap_or(2);
    let s = score(a.lists(), seed);
    HardSoftScore::new(-((owner - basic).abs()), s.soft() + basic * (seed - 5))
}
#[test]
fn mixed_bounds_optimize_the_full_product_with_coupled_scores() {
    for optional in [false, true] {
        let p = mixed_problem(optional);
        let table: Vec<_> = reference(p.lists())
            .into_iter()
            .flat_map(|rows| {
                [Some(0), Some(1), None].map(|v| {
                    MixedAssignment::new(
                        Assignment::new(vec![v, Some(7)]),
                        ListAssignment::new(rows.clone()),
                    )
                })
            })
            .collect();
        for seed in 0..12 {
            for order in ORDERS {
                for bounded in [false, true] {
                    let best = table.iter().map(|a| mixed_score(a, seed)).max().unwrap();
                    let mut seen = BTreeSet::new();
                    let r = branch_and_bound_mixed(
                        &p,
                        |a, basic, lists| {
                            p.validate(a).unwrap();
                            assert_eq!(a.basic().values()[1], Some(7));
                            if !lists.is_empty() {
                                assert_eq!(basic, &[0]);
                            }
                            if basic.is_empty() && lists.is_empty() {
                                assert!(seen.insert((
                                    a.basic().values().to_vec(),
                                    a.lists().lists().to_vec()
                                )));
                            }
                            let bound = bounded.then(|| {
                                table
                                    .iter()
                                    .filter(|full| {
                                        extends(full.lists().lists(), a.lists(), lists, p.lists())
                                            && (basic.contains(&0)
                                                || full.basic().values()[0]
                                                    == a.basic().values()[0])
                                    })
                                    .map(|full| mixed_score(full, seed))
                                    .max()
                                    .unwrap()
                            });
                            BranchEvaluation {
                                score: mixed_score(a, seed),
                                optimistic_bound: bound,
                            }
                        },
                        config(order),
                    )
                    .unwrap();
                    assert!(r.is_optimal());
                    assert_eq!(r.best.unwrap().score, best);
                    if bounded {
                        assert!(r.pruned_branches > 0);
                    } else {
                        assert_eq!(seen.len(), table.len());
                    }
                }
            }
        }
    }
}
#[test]
fn every_budget_prefix_preserves_leaf_best_and_observer_sequence() {
    let p = mixed_problem(true);
    for order in ORDERS {
        let mut full = Vec::new();
        let complete = branch_and_bound_mixed(
            &p,
            |a, b, l| {
                full.push((a.clone(), b.is_empty() && l.is_empty()));
                BranchEvaluation::unbounded(mixed_score(a, 9))
            },
            config(order),
        )
        .unwrap();
        for budget in 0..=complete.evaluations {
            let mut actual = Vec::new();
            let mut observed = Vec::new();
            let r = branch_and_bound_mixed_with_observer(
                &p,
                |a, b, l| {
                    actual.push((a.clone(), b.is_empty() && l.is_empty()));
                    BranchEvaluation::unbounded(mixed_score(a, 9))
                },
                BranchAndBoundConfig {
                    max_evaluations: budget,
                    ..config(order)
                },
                |best| observed.push(best.clone()),
            )
            .unwrap();
            assert_eq!(actual, full[..budget as usize]);
            let mut expected = Vec::new();
            for (a, leaf) in &actual {
                let score = mixed_score(a, 9);
                if *leaf
                    && expected.last().is_none_or(
                        |b: &ScoredMixedAssignment<i64, i32, HardSoftScore>| score > b.score,
                    )
                {
                    expected.push(ScoredMixedAssignment {
                        assignment: a.clone(),
                        score,
                    });
                }
            }
            assert_eq!(observed, expected);
            assert_eq!(r.best.as_ref(), expected.last());
            assert_eq!(r.is_optimal(), budget == complete.evaluations);
            assert_eq!(r.evaluations, budget);
        }
    }
}
#[test]
fn empty_impossible_frozen_and_optional_spaces_have_correct_root_semantics() {
    let impossible =
        ListProblem::new(vec![10], vec![ListEntity::new(vec![]).with_pinned(true)]).unwrap();
    let r = branch_and_bound_lists(
        &impossible,
        |_, _| -> BranchEvaluation<SimpleScore> { panic!("impossible") },
        BranchAndBoundConfig {
            max_evaluations: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, BranchTermination::Exhausted);
    assert_eq!(r.evaluations, 0);
    assert!(!r.is_optimal());
    let p = MixedProblem::new(FiniteProblem::<i64>::new(vec![]).unwrap(), impossible);
    assert_eq!(
        branch_and_bound_mixed(
            &p,
            |_, _, _| -> BranchEvaluation<SimpleScore> { panic!("impossible") },
            config(ORDERS[0])
        )
        .unwrap()
        .evaluations,
        0
    );
    let p = MixedProblem::new(
        FiniteProblem::<i64>::new(vec![Variable::new(vec![])]).unwrap(),
        ListProblem::new(vec![10], vec![ListEntity::new(vec![])]).unwrap(),
    );
    assert_eq!(
        branch_and_bound_mixed(
            &p,
            |_, _, _| -> BranchEvaluation<SimpleScore> { panic!("impossible") },
            config(ORDERS[0])
        )
        .unwrap()
        .evaluations,
        0
    );
    for rows in [
        vec![],
        vec![ListEntity::new(vec![])],
        vec![ListEntity::new(vec![10]).with_pinned(true)],
    ] {
        let values = if rows.first().is_some_and(|r| !r.initial().is_empty()) {
            vec![10]
        } else {
            vec![]
        };
        let lists = ListProblem::new(values, rows).unwrap();
        let p = MixedProblem::new(FiniteProblem::<i64>::new(vec![]).unwrap(), lists.clone());
        let cfg = BranchAndBoundConfig {
            max_frontier: 0,
            ..config(ORDERS[0])
        };
        let r = branch_and_bound_mixed(
            &p,
            |_, b, l| {
                assert!(b.is_empty() && l.is_empty());
                BranchEvaluation::unbounded(SimpleScore::ZERO)
            },
            cfg,
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!((r.evaluations, r.complete_evaluations), (1, 1));
        assert!(
            branch_and_bound_lists(
                &lists,
                |_, l| {
                    assert!(l.is_empty());
                    BranchEvaluation::unbounded(SimpleScore::ZERO)
                },
                cfg
            )
            .unwrap()
            .is_optimal()
        );
    }
    let optional = ListProblem::new(vec![10, 20], vec![])
        .unwrap()
        .allowing_unassigned();
    let r = branch_and_bound_lists(
        &optional,
        |_, left| BranchEvaluation::unbounded(SimpleScore::new(left.len() as i64)),
        config(ORDERS[0]),
    )
    .unwrap();
    assert!(r.is_optimal());
    assert_eq!((r.evaluations, r.complete_evaluations), (3, 1));
    assert_eq!(r.best.unwrap().score, SimpleScore::ZERO);
}
#[test]
fn limits_cancellation_and_frontier_never_claim_false_exhaustion() {
    let p = mixed_problem(true);
    for order in ORDERS {
        for max_frontier in 0..=2 {
            let r = branch_and_bound_mixed(
                &p,
                |a, _, _| BranchEvaluation::unbounded(mixed_score(a, 5)),
                BranchAndBoundConfig {
                    max_frontier,
                    ..config(order)
                },
            )
            .unwrap();
            assert_eq!(r.termination, BranchTermination::FrontierLimit);
            assert!(!r.is_optimal());
            assert!(r.peak_frontier <= max_frontier);
        }
        let cancel = CancellationToken::new();
        let r = branch_and_bound_mixed_with_observer(
            &p,
            |a, _, _| BranchEvaluation::unbounded(mixed_score(a, 5)),
            BranchAndBoundConfig {
                cancellation: Some(&cancel),
                ..config(order)
            },
            |_| {
                cancel.cancel();
            },
        )
        .unwrap();
        assert_eq!(r.termination, BranchTermination::Cancelled);
        assert_eq!(r.complete_evaluations, 1);
        assert!(r.best.is_some());
    }
    for cfg in [
        BranchAndBoundConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        BranchAndBoundConfig {
            max_time: Some(Duration::ZERO),
            ..Default::default()
        },
    ] {
        let r = branch_and_bound_mixed(
            &p,
            |_, _, _| -> BranchEvaluation<SimpleScore> { panic!("zero limit") },
            cfg,
        )
        .unwrap();
        assert_eq!(r.evaluations, 0);
        assert!(!r.is_optimal());
    }
}
#[test]
fn inherited_bounds_and_structural_leaf_eligibility_use_full_score_order() {
    let p = ListProblem::new(vec![10, 20], vec![ListEntity::new(vec![])]).unwrap();
    let bad = branch_and_bound_lists(
        &p,
        |_, left| {
            if left.len() == 2 {
                BranchEvaluation::bounded(SimpleScore::ZERO, SimpleScore::new(2))
            } else {
                BranchEvaluation::unbounded(SimpleScore::new(3))
            }
        },
        config(ORDERS[0]),
    )
    .unwrap_err();
    assert_eq!(bad.score, SimpleScore::new(3));
    assert_eq!(bad.bound, SimpleScore::new(2));
    for order in ORDERS {
        let r = branch_and_bound_lists(
            &p,
            |a, left| {
                let score = if !left.is_empty() {
                    SimpleScore::new(100).with_structural(-4)
                } else if a.lists()[0][0] == 10 {
                    SimpleScore::new(1).with_structural(1)
                } else {
                    SimpleScore::new(1000).with_structural(-1)
                };
                BranchEvaluation::unbounded(score)
            },
            config(order),
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!(
            r.best.unwrap().score,
            SimpleScore::new(1).with_structural(1)
        );
    }
}
#[test]
fn decimal_scores_and_observer_panics_preserve_caller_models() {
    let p = mixed_problem(false);
    let initial = p.initial_assignment();
    let low: SimpleDecimalScore = "9007199254740992.000000000000000001".parse().unwrap();
    let high: SimpleDecimalScore = "9007199254740992.000000000000000002".parse().unwrap();
    for order in ORDERS {
        let run = |panic_observer: bool| {
            branch_and_bound_mixed_with_observer(
                &p,
                |a, _, _| {
                    BranchEvaluation::bounded(
                        if a.basic().values()[0] == Some(1)
                            && a.lists()
                                .location(&20)
                                .unwrap_or(ListPosition {
                                    entity: 9,
                                    index: 9,
                                })
                                .entity
                                == 1
                        {
                            high.clone()
                        } else {
                            low.clone()
                        },
                        high.clone(),
                    )
                },
                config(order),
                |_| assert!(!panic_observer, "observer failure"),
            )
        };
        assert!(catch_unwind(AssertUnwindSafe(|| run(true))).is_err());
        assert_eq!(p.initial_assignment(), initial);
        let r = run(false).unwrap();
        assert!(r.is_optimal());
        assert_eq!(r.best.unwrap().score, high);
    }
}

#[path = "support/owned_score.rs"]
mod owned_score;
#[test]
fn score_clone_failures_and_scoring_panics_leave_joint_models_reusable() {
    use owned_score::{OwnedScore, fail_clone_at};
    let p = mixed_problem(true);
    let initial = p.initial_assignment();
    for failure in 1..=12 {
        fail_clone_at(failure);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                branch_and_bound_mixed(
                    &p,
                    |a, _, _| {
                        let s = mixed_score(a, 4);
                        BranchEvaluation::bounded(
                            OwnedScore::new(s.hard(), s.soft()),
                            OwnedScore::new(100, 100),
                        )
                    },
                    config(ORDERS[0]),
                )
            }))
            .is_err()
        );
        fail_clone_at(0);
        assert_eq!(p.initial_assignment(), initial);
    }
    let mut calls = 0;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            branch_and_bound_lists(
                p.lists(),
                |_, _| -> BranchEvaluation<SimpleScore> {
                    calls += 1;
                    assert!(calls < 7, "scorer failure");
                    BranchEvaluation::unbounded(SimpleScore::ZERO)
                },
                config(ORDERS[0]),
            )
        }))
        .is_err()
    );
    assert_eq!(p.initial_assignment(), initial);
    assert!(
        branch_and_bound_mixed(
            &p,
            |a, _, _| BranchEvaluation::unbounded(mixed_score(a, 4)),
            config(ORDERS[0])
        )
        .unwrap()
        .is_optimal()
    );
}
