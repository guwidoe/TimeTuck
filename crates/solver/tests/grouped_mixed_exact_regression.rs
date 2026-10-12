//! Original complete-space, limit, identity and shared-engine comparison checks.
#[path = "fixtures/grouped_mixed_exact_fixture.rs"]
mod fixture;
use fixture::*;
use std::{collections::BTreeSet, time::Duration};
use timetuck::*;
fn empty_lists() -> ListProblem<i32> {
    ListProblem::new(vec![], vec![]).unwrap()
}
fn coupled_network(
    b: usize,
    e: usize,
    v: usize,
) -> MixedConstraintNetwork<'static, i32, i32, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(b, e, v);
    let mut scopes: Vec<_> = (0..b).map(MixedScope::Basic).collect();
    scopes.extend((0..e).map(MixedScope::ListEntity));
    n.add_constraint("coupled", scopes, |facts| {
        let basics: i64 = facts
            .iter()
            .filter_map(|f| {
                if let MixedFact::Basic(v) = f {
                    Some(i64::from(v.copied().unwrap_or(-2)))
                } else {
                    None
                }
            })
            .sum();
        let lists: i64 = facts
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                if let MixedFact::ListEntity(row) = f {
                    Some(
                        row.iter()
                            .enumerate()
                            .map(|(j, v)| i64::from(*v) * (j as i64 + 1) + (i as i64) * 3)
                            .sum::<i64>(),
                    )
                } else {
                    None
                }
            })
            .sum();
        SimpleScore::new(basics * lists - lists * lists + basics)
    })
    .unwrap();
    n
}
fn assignment_key(a: &MixedAssignment<i32, i32>) -> (Vec<Option<i32>>, Vec<Vec<i32>>) {
    (a.basic().values().to_vec(), a.lists().lists().to_vec())
}
#[test]
fn grouped_search_matches_complete_independent_enumeration_across_pins_ranges_and_optionality() {
    for optional in [false, true] {
        for pinned in [false, true] {
            for restricted in [false, true] {
                let variables = vec![
                    Variable::new(vec![0, 1])
                        .with_initial(1)
                        .with_pinned(pinned),
                    if optional {
                        Variable::new(vec![0, 1]).allowing_unassigned()
                    } else {
                        Variable::new(vec![0, 1])
                    },
                ];
                let entities = vec![
                    ListEntity::new(if pinned { vec![7] } else { vec![] })
                        .with_pinned_prefix(usize::from(pinned)),
                    if restricted {
                        ListEntity::new(vec![]).with_value_range(vec![8])
                    } else {
                        ListEntity::new(vec![])
                    },
                ];
                let lists = ListProblem::new(vec![7, 8], entities).unwrap();
                let lists = if optional {
                    lists.allowing_unassigned()
                } else {
                    lists
                };
                let p = MixedProblem::new(FiniteProblem::new(variables).unwrap(), lists);
                let n = coupled_network(2, 2, 2);
                let mut expected = BTreeSet::new();
                let reference = exhaustive_mixed_search(
                    &p,
                    |a| {
                        assert!(expected.insert(assignment_key(a)));
                        n.calculate(&p, a).unwrap()
                    },
                    10_000,
                );
                assert!(reference.is_optimal());
                let mut actual = BTreeSet::new();
                let result = GroupedMixedExactSolver::default()
                    .solve_with_leaf_observer(&p, &n, None, |a, _| {
                        assert!(actual.insert(assignment_key(a)));
                    })
                    .unwrap();
                assert_eq!(actual, expected);
                assert!(result.is_optimal());
                assert_eq!(result.pruned_branches, 0);
                assert_eq!(result.complete_evaluations, expected.len() as u64);
                assert_eq!(
                    result.best.as_ref().unwrap().score,
                    reference.best.as_ref().unwrap().score
                );
                let best = &result.best.as_ref().unwrap().assignment;
                assert!(p.validate(best).is_ok());
                assert!(p.is_complete(best));
                assert!(expected.contains(&assignment_key(best)));
            }
        }
    }
}
#[test]
fn cartesian_group_reduces_actual_expansions_and_score_calls_without_pruning() {
    let p = problem();
    let n = network();
    let old = branch_and_bound_mixed(
        &p,
        |a, _, _| BranchEvaluation::unbounded(n.calculate(&p, a).unwrap()),
        BranchAndBoundConfig::default(),
    )
    .unwrap();
    let grouped = GroupedMixedExactSolver::default()
        .solve(&p, &n, None)
        .unwrap();
    assert!(old.is_optimal() && grouped.is_optimal());
    assert_eq!(old.complete_evaluations, 1944);
    assert_eq!(grouped.complete_evaluations, old.complete_evaluations);
    assert_eq!(old.best.unwrap().score, grouped.best.unwrap().score);
    assert_eq!((old.expanded_nodes, grouped.expanded_nodes), (969, 33));
    assert_eq!((old.evaluations, grouped.evaluations), (2913, 1977));
    assert_eq!((old.pruned_branches, grouped.pruned_branches), (0, 0));
}
#[test]
fn interruptions_close_attempts_and_never_claim_exhaustion() {
    let p = problem();
    let n = network();
    let mut events = vec![];
    let limited = GroupedMixedExactSolver {
        config: BranchAndBoundConfig {
            max_evaluations: 2,
            ..Default::default()
        },
    }
    .solve(&p, &n, Some(&mut |e| events.push(e)))
    .unwrap();
    assert_eq!(limited.termination, BranchTermination::EvaluationLimit);
    assert_eq!(limited.evaluations, 2);
    assert!(!limited.is_optimal());
    assert!(events.iter().any(|e| matches!(
        e,
        SolverLifecycleEvent::StepEnded {
            completed: false,
            ..
        }
    )));
    assert_eq!(events.last(), Some(&SolverLifecycleEvent::SolveEnded));
    let token = CancellationToken::new();
    token.cancel();
    for config in [
        BranchAndBoundConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        BranchAndBoundConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
        BranchAndBoundConfig {
            max_time: Some(Duration::ZERO),
            ..Default::default()
        },
        BranchAndBoundConfig {
            max_frontier: 0,
            ..Default::default()
        },
    ] {
        let r = GroupedMixedExactSolver { config }
            .solve(&p, &n, None)
            .unwrap();
        assert!(!r.is_optimal());
        assert!(r.best.is_none());
    }
}
#[test]
fn impossible_spaces_empty_products_and_overflows_are_distinguished_before_lifecycle() {
    let impossible = MixedProblem::new(
        FiniteProblem::new(vec![Variable::<i32>::new(vec![])]).unwrap(),
        empty_lists(),
    );
    let r = GroupedMixedExactSolver::default()
        .solve(&impossible, &coupled_network(1, 0, 0), None)
        .unwrap();
    assert_eq!(r.termination, BranchTermination::Exhausted);
    assert_eq!(r.evaluations, 0);
    let empty = MixedProblem::new(FiniteProblem::<i32>::new(vec![]).unwrap(), empty_lists());
    let r = GroupedMixedExactSolver::default()
        .solve(&empty, &coupled_network(0, 0, 0), None)
        .unwrap();
    assert!(r.is_optimal());
    assert_eq!(r.complete_evaluations, 1);
    assert_eq!(r.expanded_nodes, 0);
    let huge = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]); usize::BITS as usize + 1]).unwrap(),
        empty_lists(),
    );
    let mut events = vec![];
    assert!(matches!(
        GroupedMixedExactSolver::default().solve(
            &huge,
            &coupled_network(usize::BITS as usize + 1, 0, 0),
            Some(&mut |e| events.push(e))
        ),
        Err(GroupedMixedExactError::CartesianProductOverflow)
    ));
    assert!(events.is_empty());
    assert!(matches!(
        GroupedMixedExactSolver::default().solve(
            &problem(),
            &coupled_network(0, 0, 0),
            Some(&mut |e| events.push(e))
        ),
        Err(GroupedMixedExactError::Scoring(
            MixedScoringError::ModelShape { .. }
        ))
    ));
    assert!(events.is_empty());
}
#[test]
fn all_frontier_orders_cover_every_leaf_with_optional_basic_only_or_list_only_models() {
    for basic_only in [false, true] {
        let p = if basic_only {
            MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 2])
                    .unwrap(),
                empty_lists(),
            )
        } else {
            MixedProblem::new(
                FiniteProblem::new(vec![]).unwrap(),
                ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![]); 2]).unwrap(),
            )
        };
        let n = coupled_network(
            p.basic().variables().len(),
            p.lists().entities().len(),
            p.lists().values().len(),
        );
        let reference = exhaustive_mixed_search(&p, |a| n.calculate(&p, a).unwrap(), 10_000);
        for exploration in [
            NodeExploration::DepthFirst,
            NodeExploration::InputOrderDepthFirst,
            NodeExploration::BreadthFirst,
            NodeExploration::ScoreFirst,
            NodeExploration::OptimisticBoundFirst,
        ] {
            let r = GroupedMixedExactSolver {
                config: BranchAndBoundConfig {
                    exploration,
                    ..Default::default()
                },
            }
            .solve(&p, &n, None)
            .unwrap();
            assert!(r.is_optimal());
            assert_eq!(r.complete_evaluations, reference.evaluations);
            assert_eq!(
                r.best.unwrap().score,
                reference.best.as_ref().unwrap().score
            );
        }
    }
}
