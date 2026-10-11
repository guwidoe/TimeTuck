//! Original regressions; these do not receive upstream test coverage credit.
use std::{cell::Cell, collections::BTreeSet};
use timetuck::*;
fn route<'a>(f: &ListFact<'a, i32>) -> &'a [i32] {
    match f {
        ListFact::Entity(v) => v,
        _ => panic!("entity"),
    }
}
fn empty(n: i32, m: usize) -> ListProblem<i32> {
    ListProblem::new(
        (0..n).collect(),
        (0..m).map(|_| ListEntity::new(vec![])).collect(),
    )
    .unwrap()
}
fn zero(p: &ListProblem<i32>) -> ListConstraintNetwork<'static, i32, SimpleScore> {
    ListConstraintNetwork::new(p.entities().len(), p.values().len())
}
#[test]
fn every_small_neighborhood_matches_independent_state_edits_and_undo() {
    for optional in [false, true] {
        let p = empty(3, 2);
        let p = if optional { p.allowing_unassigned() } else { p };
        let enumeration = exhaustive_list_search(
            &p,
            |a| {
                let lists = a.lists();
                let mut expected = BTreeSet::new();
                // Independent vector operations specify resulting states, not move objects.
                for e in 0..2 {
                    for i in 0..lists[e].len() {
                        let mut removed = lists.to_vec();
                        let v = removed[e].remove(i);
                        for d in 0..2 {
                            for j in 0..=removed[d].len() {
                                let mut b = removed.clone();
                                b[d].insert(j, v);
                                if b != lists {
                                    expected.insert(b);
                                }
                            }
                        }
                        if optional {
                            expected.insert(removed.clone());
                            for w in 0..3 {
                                if !lists.iter().flatten().any(|&x| x == w) {
                                    let mut b = removed.clone();
                                    b[e].insert(i, w);
                                    expected.insert(b);
                                }
                            }
                        }
                        for d in 0..2 {
                            for j in 0..lists[d].len() {
                                if (e, i) == (d, j) {
                                    continue;
                                }
                                let mut b = lists.to_vec();
                                b[e][i] = lists[d][j];
                                b[d][j] = lists[e][i];
                                expected.insert(b);
                            }
                        }
                    }
                }
                if optional {
                    for v in 0..3 {
                        if !lists.iter().flatten().any(|&x| x == v) {
                            for e in 0..2 {
                                for i in 0..=lists[e].len() {
                                    let mut b = lists.to_vec();
                                    b[e].insert(i, v);
                                    expected.insert(b);
                                }
                            }
                        }
                    }
                }
                let mut actual = BTreeSet::new();
                let mut working = a.clone();
                for m in list_moves(&p, a, Neighborhood::ChangeAndSwap).unwrap() {
                    {
                        let trial = p.apply(&mut working, &m).unwrap();
                        assert!(p.is_complete(&trial));
                        assert_ne!(&*trial, a);
                        actual.insert(trial.lists().to_vec());
                    }
                    assert_eq!(&working, a);
                }
                assert_eq!(actual, expected);
                SimpleScore::ZERO
            },
            10_000,
        );
        assert!(enumeration.is_optimal());
        assert_eq!(enumeration.evaluations, if optional { 49 } else { 24 });
    }
}
#[test]
fn prefixes_and_full_pins_survive_every_generated_move() {
    let p = ListProblem::new(
        (0..5).collect(),
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]).with_pinned_prefix(1),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let mut count = 0;
    for m in list_moves(&p, &a, Neighborhood::ChangeAndSwap).unwrap() {
        let mut b = a.clone();
        let t = p.apply(&mut b, &m).unwrap();
        assert_eq!(t.lists()[0], vec![0]);
        assert_eq!(t.lists()[1][0], 1);
        assert_eq!(t.lists()[2][0], 3);
        count += 1;
    }
    assert!(count > 0);
    assert!(list_moves(&p, &ListAssignment::new(vec![]), Neighborhood::Change).is_err());
}
#[test]
fn construction_budgets_keep_scanned_winner_without_callback_replay() {
    let p = empty(2, 2);
    let calls = Cell::new(0);
    let mut net = zero(&p);
    net.add_constraint(
        "count",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        },
    )
    .unwrap();
    for (budget, assigned, complete, termination) in [
        (0, 0, false, ConstructionTermination::EvaluationLimit),
        (1, 0, false, ConstructionTermination::EvaluationLimit),
        (2, 1, false, ConstructionTermination::EvaluationLimit),
        (3, 1, false, ConstructionTermination::EvaluationLimit),
        (4, 2, true, ConstructionTermination::EvaluationLimit),
        (6, 2, true, ConstructionTermination::Completed),
    ] {
        calls.set(0);
        let r = greedy_list_construction(
            &p,
            &net,
            ConstructionConfig {
                max_evaluations: budget,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.evaluations, budget);
        assert_eq!(r.assigned_values, assigned);
        assert_eq!(r.complete, complete);
        assert_eq!(r.termination, termination);
        assert_eq!(u128::from(calls.get()), r.constraint_evaluations);
        assert_eq!(calls.get(), budget);
        assert_eq!(r.solution().is_some(), complete);
        if complete {
            assert_eq!(r.assignment.lists(), &[vec![1, 0], vec![]]);
        }
    }
}
#[test]
fn early_policies_ignore_missing_required_structure_and_preserve_pins() {
    let p = ListProblem::new(
        (0..4).collect(),
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![1]).with_pinned_prefix(1),
        ],
    )
    .unwrap();
    for policy in [
        ConstructionPickEarly::Never,
        ConstructionPickEarly::FirstNonDeterioratingScore,
        ConstructionPickEarly::FirstFeasibleScore,
        ConstructionPickEarly::FirstFeasibleScoreOrNonDeterioratingHard,
    ] {
        let r = greedy_list_construction(
            &p,
            &zero(&p),
            ConstructionConfig {
                pick_early: policy,
                assert_scores: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.complete);
        assert_eq!(r.assignment.lists(), &[vec![0], vec![1, 3, 2]]);
        assert_eq!(
            r.evaluations,
            if policy == ConstructionPickEarly::Never {
                4
            } else {
                3
            }
        );
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
}
#[test]
fn optional_construction_and_impossible_required_models_do_no_insertions() {
    let p = empty(3, 2).allowing_unassigned();
    let r = greedy_list_construction(&p, &zero(&p), Default::default()).unwrap();
    assert!(r.complete);
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.assigned_values, 0);
    for p in [
        empty(1, 0),
        ListProblem::new(vec![0], vec![ListEntity::new(vec![]).with_pinned(true)]).unwrap(),
    ] {
        let r = greedy_list_construction(&p, &zero(&p), Default::default()).unwrap();
        assert_eq!(r.termination, ConstructionTermination::NoCompleteAssignment);
        assert_eq!(r.evaluations, 0);
        assert!(!r.complete);
    }
    let p = empty(0, 0);
    let r = greedy_list_construction(&p, &zero(&p), Default::default()).unwrap();
    assert!(r.complete);
    assert_eq!(r.evaluations, 1);
}
fn replacement_net() -> ListConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("one valuable item", vec![ListScope::Entity(0)], |f| {
        SimpleScore::new(match route(&f[0]) {
            [0] => 0,
            [1] => 10,
            _ => -100,
        })
    })
    .unwrap();
    n
}
#[test]
fn optional_replacement_escapes_a_single_move_valley() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0])])
        .unwrap()
        .allowing_unassigned();
    let n = replacement_net();
    for neighborhood in [Neighborhood::Change, Neighborhood::ChangeAndSwap] {
        let r = list_local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                neighborhood: neighborhood.clone(),
                assert_scores: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            r.best.unwrap().score,
            SimpleScore::new(if neighborhood == Neighborhood::Change {
                0
            } else {
                10
            })
        );
        assert_eq!(r.termination, LocalSearchTermination::NoAcceptedMove);
    }
}
#[test]
fn selection_budget_boundary_retains_best_prefix_and_counts_callbacks() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0])])
        .unwrap()
        .allowing_unassigned();
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(1, 3);
    n.add_constraint("reward", vec![ListScope::Entity(0)], |f| {
        calls.set(calls.get() + 1);
        SimpleScore::new(route(&f[0]).iter().map(|&x| i64::from(x)).sum())
    })
    .unwrap();
    for (selection, budget, score, steps) in [
        (MoveSelection::BestAccepted, 1, 0, 0),
        (MoveSelection::BestAccepted, 4, 2, 1),
        (MoveSelection::FirstAccepted, 2, 1, 1),
    ] {
        calls.set(0);
        let r = list_local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                max_evaluations: budget,
                selection,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.evaluations, budget);
        assert_eq!(r.best.unwrap().score, SimpleScore::new(score));
        assert_eq!(r.completed_steps, steps);
        assert_eq!(r.termination, LocalSearchTermination::EvaluationLimit);
        assert_eq!(calls.get(), budget);
        assert_eq!(r.constraint_evaluations, u128::from(budget));
    }
}
#[test]
fn fallback_counts_steps_separately_and_preserves_best() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let n = zero(&p);
    let r = list_local_search_from(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 7,
            max_accepted_moves: 1,
            rejected_moves: RejectedMovePolicy::BestScore,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.completed_steps, 7);
    assert_eq!(r.accepted_moves, 0);
    assert_eq!(r.termination, LocalSearchTermination::StepLimit);
    assert_eq!(r.best.unwrap().assignment, p.initial_assignment());
    let r = list_local_search_from(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 0,
            max_accepted_moves: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.termination, LocalSearchTermination::AcceptedMoveLimit);
}
#[test]
fn exhausted_neighborhood_at_exact_budget_is_not_a_limit() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let r = list_local_search_from(
        &p,
        &zero(&p),
        p.initial_assignment(),
        LocalSearchConfig {
            max_evaluations: 3,
            neighborhood: Neighborhood::Change,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.evaluations, 3);
    assert_eq!(r.termination, LocalSearchTermination::NoAcceptedMove);
}
#[test]
fn zero_budget_still_validates_model_and_acceptance_without_scoring() {
    let p = empty(1, 1);
    let n = zero(&p);
    let config = LocalSearchConfig {
        max_evaluations: 0,
        ..Default::default()
    };
    assert!(matches!(
        list_local_search_from(&p, &n, p.initial_assignment(), config.clone()),
        Err(ListLocalSearchError::IncompleteInitialAssignment)
    ));
    let p = p.allowing_unassigned();
    let mut n = zero(&p);
    n.add_constraint("must not call", vec![], |_| panic!("zero budget"))
        .unwrap();
    assert_eq!(
        list_local_search_from(&p, &n, p.initial_assignment(), config.clone())
            .unwrap()
            .evaluations,
        0
    );
    assert!(matches!(
        list_local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                acceptance: Acceptance::VariableTabu {
                    tenure: 0,
                    aspiration: false
                },
                ..config.clone()
            }
        ),
        Err(ListLocalSearchError::Acceptance(AcceptanceError::EmptyTabu))
    ));
    assert!(list_local_search_from(&p, &n, ListAssignment::new(vec![vec![2]]), config).is_err());
    let bad = ListConstraintNetwork::<i32, SimpleScore>::new(2, 1);
    assert!(
        greedy_list_construction(
            &p,
            &bad,
            ConstructionConfig {
                max_evaluations: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
}
// Independent complete formula for two routes on a line, including capacity.
fn reference(a: &ListAssignment<i32>) -> HardSoftScore {
    let mut distance = 0;
    let mut hard = 0;
    for (e, r) in a.lists().iter().enumerate() {
        let depot = if e == 0 { 0 } else { 10 };
        hard -= r.len().saturating_sub(3) as i64;
        let points: Vec<_> = std::iter::once(depot)
            .chain(r.iter().copied())
            .chain(std::iter::once(depot))
            .collect();
        distance += points
            .windows(2)
            .map(|w| i64::from((w[1] - w[0]).abs()))
            .sum::<i64>();
    }
    HardSoftScore::new(hard, -distance)
}
#[test]
fn all_acceptors_reproduce_seeded_results_and_match_small_routing_optimum() {
    let p = ListProblem::new(
        vec![1, 2, 3, 7, 8, 9],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 6);
    for e in 0..2 {
        n.add_constraint(format!("route {e}"), vec![ListScope::Entity(e)], move |f| {
            let r = route(&f[0]);
            let depot = if e == 0 { 0 } else { 10 };
            let mut cost = 0;
            let mut previous = depot;
            for &v in r {
                cost += i64::from((v - previous).abs());
                previous = v;
            }
            cost += i64::from((previous - depot).abs());
            HardSoftScore::new(-(r.len().saturating_sub(3) as i64), -cost)
        })
        .unwrap();
    }
    let exact = exhaustive_list_search(&p, reference, 10_000);
    assert!(exact.is_optimal());
    assert_eq!(
        exact.best.as_ref().unwrap().score,
        HardSoftScore::new(0, -12)
    );
    let construction = greedy_list_construction(&p, &n, Default::default()).unwrap();
    assert!(construction.complete);
    for seed in [1, 7, 20261005] {
        for acceptance in [
            Acceptance::StrictImprovement,
            Acceptance::HillClimbing,
            Acceptance::LateAcceptance {
                history_size: 5,
                hill_climbing: true,
            },
            Acceptance::VariableTabu {
                tenure: 1,
                aspiration: true,
            },
            Acceptance::SimulatedAnnealing {
                starting_temperature: HardSoftScore::new(0, 5),
                cooling_steps: 12,
                seed,
            },
        ] {
            let config = LocalSearchConfig {
                max_evaluations: 2_000,
                max_steps: 20,
                selection: MoveSelection::BestAccepted,
                acceptance,
                move_order: MoveOrder::ShuffledVariables { seed },
                assert_scores: true,
                ..Default::default()
            };
            let one =
                list_local_search_from(&p, &n, construction.assignment.clone(), config.clone())
                    .unwrap();
            let two =
                list_local_search_from(&p, &n, construction.assignment.clone(), config).unwrap();
            assert_eq!(one, two);
            assert!(one.evaluations <= 2000);
            assert!(one.completed_steps <= 20);
            let best = one.best.unwrap();
            assert!(p.is_complete(&best.assignment));
            assert_eq!(best.score, reference(&best.assignment));
            assert_eq!(best.score, exact.best.as_ref().unwrap().score);
        }
    }
}
#[test]
fn entity_tabu_blocks_both_lists_touched_by_a_relocation() {
    let p = ListProblem::new(
        vec![0],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![])],
    )
    .unwrap();
    let r = list_local_search_from(
        &p,
        &zero(&p),
        p.initial_assignment(),
        LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 1,
                aspiration: false,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.accepted_moves, 1);
    assert_eq!(r.completed_steps, 1);
    assert_eq!(r.termination, LocalSearchTermination::NoAcceptedMove);
}

#[test]
fn swap_improves_capacity_constrained_lists_when_relocation_cannot() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![1])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 2);
    for e in 0..2 {
        n.add_constraint(
            format!("preference {e}"),
            vec![ListScope::Entity(e)],
            move |f| {
                let r = route(&f[0]);
                HardSoftScore::new(
                    -(r.len().saturating_sub(1) as i64),
                    if r.contains(&(e as i32)) { -1 } else { 0 },
                )
            },
        )
        .unwrap();
    }
    for neighborhood in [Neighborhood::Change, Neighborhood::ChangeAndSwap] {
        let r = list_local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                neighborhood: neighborhood.clone(),
                assert_scores: true,
                ..Default::default()
            },
        )
        .unwrap();
        let best = r.best.unwrap();
        assert_eq!(
            best.score,
            HardSoftScore::new(
                0,
                if neighborhood == Neighborhood::Change {
                    -2
                } else {
                    0
                }
            )
        );
        assert_eq!(
            r.accepted_moves,
            if neighborhood == Neighborhood::Change {
                0
            } else {
                1
            }
        );
    }
}

#[test]
fn taking_one_assignment_clones_only_one_value_not_the_neighborhood() {
    #[derive(Eq, PartialEq)]
    struct Value(i32);
    thread_local! { static CLONES: Cell<usize> = const { Cell::new(0) }; }
    impl Clone for Value {
        fn clone(&self) -> Self {
            CLONES.with(|n| n.set(n.get() + 1));
            Self(self.0)
        }
    }
    let p = ListProblem::new(
        (0..100).map(Value).collect(),
        (0..100).map(|_| ListEntity::new(vec![])).collect(),
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    CLONES.with(|n| n.set(0));
    let first = list_assign_moves(&p, &a).unwrap().next().unwrap();
    assert!(matches!(
        first,
        ListMove::Assign {
            value: Value(0),
            destination: ListPosition {
                entity: 0,
                index: 0
            }
        }
    ));
    CLONES.with(|n| assert_eq!(n.get(), 1));
}
