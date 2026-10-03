//! Original combined stream lifecycle/search checks, with no upstream credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Problem = MixedProblem<i64, i32>;
type State = MixedAssignment<i64, i32>;
type Sources = MixedStreamSources<i64, i32>;
fn problem(optional: bool) -> Problem {
    let mut variables = vec![
        Variable::new(vec![-2, 0, 3]).with_initial(0),
        Variable::new(vec![-2, 0, 3]).with_initial(3),
    ];
    if optional {
        variables = variables
            .into_iter()
            .map(Variable::allowing_unassigned)
            .collect();
    }
    variables.push(Variable::new(vec![9]).with_initial(9).with_pinned(true));
    let mut lists = ListProblem::new(
        vec![11, 23, 47, 89],
        vec![
            ListEntity::new(vec![11, 23])
                .with_pinned_prefix(1)
                .with_value_range(vec![11, 23, 47]),
            ListEntity::new(vec![47, 89]),
        ],
    )
    .unwrap();
    if optional {
        lists = lists.allowing_unassigned();
    }
    MixedProblem::new(FiniteProblem::new(variables).unwrap(), lists)
}
fn graph() -> (StreamGraph<'static, HardSoftScore>, Sources) {
    let mut b = StreamGraphBuilder::new();
    let s = Sources::new(&mut b);
    let lists = b
        .join_equal(
            s.basic.port(),
            s.lists.entities.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )
        .unwrap();
    b.score("loads", &lists, |((_, v), (_, list))| {
        HardSoftScore::new(
            -(list.len().saturating_sub(2) as i64),
            v.unwrap_or(0) * list.len() as i64,
        )
    })
    .unwrap();
    let values = b
        .join_equal(
            s.basic.port(),
            s.lists.values.port(),
            |(i, _)| Some(*i),
            |v| v.position.map(|p| p.entity),
        )
        .unwrap();
    b.score("shadows", &values, |((_, v), row)| {
        HardSoftScore::new(
            0,
            v.unwrap_or(0) * i64::from(row.value)
                + row.position.unwrap().index as i64
                + i64::from(row.previous.unwrap_or(0))
                - i64::from(row.next.unwrap_or(0)),
        )
    })
    .unwrap();
    let fact = b.source::<usize, i64>();
    b.score("fact", fact.port(), |v| HardSoftScore::new(0, *v))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&fact, TupleChange::put(0, 7)).unwrap();
    tx.commit().unwrap();
    (g, s)
}
fn direct(p: &Problem, a: &State) -> HardSoftScore {
    let (mut hard, mut soft) = (0, 7);
    for (owner, list) in a.lists().lists().iter().enumerate() {
        let b = a.basic().values()[owner].unwrap_or(0);
        hard -= list.len().saturating_sub(2) as i64;
        soft += b * list.len() as i64;
        for (i, &v) in list.iter().enumerate() {
            soft += b * i64::from(v) + i as i64 + i64::from(if i > 0 { list[i - 1] } else { 0 })
                - i64::from(list.get(i + 1).copied().unwrap_or(0));
        }
    }
    let missing = p
        .basic()
        .variables()
        .iter()
        .zip(a.basic().values())
        .filter(|(v, x)| !v.allows_unassigned() && x.is_none())
        .count()
        + if p.lists().allows_unassigned() {
            0
        } else {
            p.lists().values().len() - a.lists().lists().iter().map(Vec::len).sum::<usize>()
        };
    HardSoftScore::new(hard, soft).with_structural(-(missing as i64))
}
fn rows(g: &StreamGraph<'_, HardSoftScore>, s: &Sources, p: &Problem, a: &State) {
    let basics = g.source_rows(&s.basic).unwrap();
    assert_eq!(basics.len(), a.basic().values().len());
    for (i, v) in a.basic().values().iter().enumerate() {
        assert_eq!(&basics[&i], &(i, *v));
    }
    let entities = g.source_rows(&s.lists.entities).unwrap();
    assert_eq!(entities.len(), a.lists().lists().len());
    for (i, v) in a.lists().lists().iter().enumerate() {
        assert_eq!(&entities[&i], &(i, v.clone()));
    }
    let values = g.source_rows(&s.lists.values).unwrap();
    assert_eq!(values.len(), p.lists().values().len());
    for (i, v) in p.lists().values().iter().enumerate() {
        let mut expected = ListValueRow {
            range_index: i,
            value: *v,
            position: None,
            previous: None,
            next: None,
        };
        for (e, list) in a.lists().lists().iter().enumerate() {
            for (j, x) in list.iter().enumerate() {
                if x == v {
                    expected.position = Some(ListPosition {
                        entity: e,
                        index: j,
                    });
                    expected.previous = j.checked_sub(1).map(|k| list[k]);
                    expected.next = list.get(j + 1).copied();
                }
            }
        }
        assert_eq!(values[&i], expected);
    }
}
#[test]
fn seeded_joint_edits_match_direct_scores_and_all_bound_rows_through_commit_and_undo() {
    let mut seed = 20261106_u64;
    for optional in [false, true] {
        let p = problem(optional);
        let (g, s) = graph();
        let inspect = s.clone();
        let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
        let retained = d
            .graph()
            .source_rows(&inspect.lists.values)
            .unwrap()
            .clone();
        for step in 0..400 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let before = d.assignment().clone();
            let old = d.score();
            let moves: Vec<_> = mixed_moves(&p, &before, Neighborhood::ChangeAndSwap, true)
                .unwrap()
                .collect();
            let movement = &moves[(seed as usize) % moves.len()];
            let trial = d.apply(movement).unwrap();
            assert_eq!(trial.score(), direct(&p, trial.assignment()));
            rows(trial.graph(), &inspect, &p, trial.assignment());
            assert!(trial.checkpoint_count() > 0);
            if step % 3 != 0 {
                trial.commit();
            } else {
                drop(trial);
                assert_eq!(d.assignment(), &before);
                assert_eq!(d.score(), old);
            }
            rows(d.graph(), &inspect, &p, d.assignment());
            d.assert_consistent().unwrap();
            if step % 40 == 0 {
                let (g, s) = graph();
                let fresh =
                    MixedStreamScoreDirector::new(&p, g, s, d.assignment().clone()).unwrap();
                assert_eq!(d.score(), fresh.score());
            }
        }
        assert_eq!(retained[&0].next, Some(23));
        assert_eq!(retained[&3].previous, Some(47));
    }
}
fn small(partial: bool) -> MixedProblem<i32, i32> {
    let v = Variable::new(vec![0, 1]);
    MixedProblem::new(
        FiniteProblem::new(vec![if partial { v } else { v.with_initial(0) }]).unwrap(),
        ListProblem::new(
            vec![7],
            vec![
                ListEntity::new(if partial { vec![] } else { vec![7] }),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap(),
    )
}
fn objective(b: Option<i32>, owner: Option<usize>) -> SimpleScore {
    SimpleScore::new(match (b, owner) {
        (Some(1), Some(1)) => 10,
        (Some(0), Some(0)) => 0,
        (None, _) | (_, None) => -2,
        _ => -5,
    })
}
fn small_graph(calls: &Cell<u128>) -> (StreamGraph<'_, SimpleScore>, MixedStreamSources<i32, i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    let j = b
        .join_equal(s.basic.port(), s.lists.values.port(), |_| 0, |_| 0)
        .unwrap();
    b.score("coupled", &j, |((_, v), row)| {
        calls.set(calls.get() + 1);
        objective(*v, row.position.map(|p| p.entity))
    })
    .unwrap();
    (b.build(), s)
}
fn small_network() -> MixedConstraintNetwork<'static, i32, i32, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            let MixedFact::Basic(b) = f[0] else { panic!() };
            let MixedFact::ListValue { ref element, .. } = f[1] else {
                panic!()
            };
            objective(b.copied(), element.as_ref().map(|e| e.position.entity))
        },
    )
    .unwrap();
    n
}
#[test]
fn construction_matches_scoped_selection_for_both_orders_policies_and_budget_prefixes() {
    let p = small(true);
    let n = small_network();
    let calls = Cell::new(0);
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        for pick_early in [
            ConstructionPickEarly::Never,
            ConstructionPickEarly::FirstNonDeterioratingScore,
            ConstructionPickEarly::FirstFeasibleScore,
            ConstructionPickEarly::FirstFeasibleScoreOrNonDeterioratingHard,
        ] {
            for max_evaluations in 0..=7 {
                let config = MixedConstructionConfig {
                    order,
                    construction: ConstructionConfig {
                        cancellation: None,
                        max_evaluations,
                        pick_early,
                        assert_scores: false,
                    },
                };
                let expected = greedy_mixed_construction(&p, &n, config).unwrap();
                calls.set(0);
                let (g, s) = small_graph(&calls);
                let actual = greedy_mixed_stream_construction(&p, g, s, config).unwrap();
                assert_eq!(calls.get(), actual.constraint_evaluations);
                assert_eq!(actual.assignment, expected.assignment);
                assert_eq!(actual.score, expected.score);
                assert_eq!(actual.complete, expected.complete);
                assert_eq!(actual.evaluations, expected.evaluations);
                assert_eq!(actual.termination, expected.termination);
                assert_eq!(
                    actual.assigned_basic_variables,
                    expected.assigned_basic_variables
                );
                assert_eq!(actual.assigned_list_values, expected.assigned_list_values);
            }
        }
    }
}
#[test]
fn local_search_matches_scoped_policies_and_exact_quality_with_joint_moves() {
    let p = small(false);
    let n = small_network();
    let calls = Cell::new(0);
    let exact = exhaustive_mixed_search(
        &p,
        |a| {
            objective(
                a.basic().values()[0],
                a.lists().location(&7).map(|p| p.entity),
            )
        },
        100,
    );
    assert!(exact.is_optimal());
    assert_eq!(exact.best.unwrap().score, SimpleScore::new(10));
    for acceptance in [
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: false,
        },
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: SimpleScore::new(5),
            cooling_steps: 5,
            seed: 20261107,
        },
    ] {
        for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
            for joint_moves in [false, true] {
                for budget in [0, 1, 2, 3, 4, 5, 12, 100] {
                    let settings = MixedLocalSearchConfig {
                        joint_moves,
                        search: LocalSearchConfig {
                            acceptance,
                            selection,
                            max_evaluations: budget,
                            max_steps: 5,
                            move_order: MoveOrder::ShuffledVariables { seed: 20261107 },
                            rejected_moves: RejectedMovePolicy::BestScore,
                            ..Default::default()
                        },
                    };
                    let expected =
                        mixed_local_search_from(&p, &n, p.initial_assignment(), settings).unwrap();
                    calls.set(0);
                    let (g, s) = small_graph(&calls);
                    let actual =
                        mixed_stream_local_search_from(&p, g, s, p.initial_assignment(), settings)
                            .unwrap();
                    assert_eq!(actual.constraint_evaluations, calls.get());
                    assert_eq!(actual.best, expected.best);
                    assert_eq!(actual.evaluations, expected.evaluations);
                    assert_eq!(actual.accepted_moves, expected.accepted_moves);
                    assert_eq!(actual.completed_steps, expected.completed_steps);
                    assert_eq!(actual.termination, expected.termination);
                    if joint_moves && budget == 100 {
                        assert_eq!(actual.best.unwrap().score, SimpleScore::new(10));
                    }
                }
            }
        }
    }
}
#[test]
fn later_graph_failure_and_caller_unwind_restore_both_components_and_cached_matches() {
    let p = small(false);
    let calls = Cell::new(0);
    for panic in [false, true] {
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let inspect = s.clone();
        b.score("basic", s.basic.port(), |(_, v)| {
            calls.set(calls.get() + 1);
            SimpleScore::new(i64::from(v.unwrap_or(0)))
        })
        .unwrap();
        b.score("list", s.lists.values.port(), |row| {
            if row.position.is_some_and(|p| p.entity == 1) {
                assert!(!panic, "injected scoring panic");
                return SimpleScore::ZERO.with_structural(1);
            }
            SimpleScore::ZERO
        })
        .unwrap();
        let mut d =
            MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let before = calls.get();
        let m = MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 0,
                },
                destination: ListPosition {
                    entity: 1,
                    index: 0,
                },
            },
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _trial = d.apply(&m)?;
            Ok::<(), MixedStreamScoringError>(())
        }));
        if panic {
            assert!(outcome.is_err());
        } else {
            assert!(outcome.unwrap().is_err());
        }
        assert!(calls.get() > before);
        assert_eq!(d.assignment(), &p.initial_assignment());
        assert_eq!(d.score(), SimpleScore::ZERO);
        assert_eq!(
            d.graph().source_rows(&inspect.basic).unwrap()[&0],
            (0, Some(0))
        );
        assert_eq!(
            d.graph().source_rows(&inspect.lists.entities).unwrap()[&0],
            (0, vec![7])
        );
        assert_eq!(
            d.graph().source_rows(&inspect.lists.values).unwrap()[&0].position,
            Some(ListPosition {
                entity: 0,
                index: 0
            })
        );
        d.assert_consistent().unwrap();
        let safe = MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(1),
            },
            lists: ListMove::Composite(vec![]),
        };
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _trial = d.apply(&safe).unwrap();
                panic!("caller unwind");
            }))
            .is_err()
        );
        assert_eq!(d.assignment(), &p.initial_assignment());
        d.apply(&safe).unwrap().commit();
        assert_eq!(d.score(), SimpleScore::new(1));
        let before = calls.get();
        let bad = MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(0),
            },
            lists: ListMove::Assign {
                value: 99,
                destination: ListPosition {
                    entity: 0,
                    index: 0,
                },
            },
        };
        assert!(d.apply(&bad).is_err());
        assert_eq!(calls.get(), before);
        assert_eq!(d.score(), SimpleScore::new(1));
        d.assert_consistent().unwrap();
    }
}
#[test]
fn source_ownership_emptiness_and_initial_validity_are_checked_at_zero_budget() {
    let p = small(false);
    let c = MixedConstructionConfig {
        construction: ConstructionConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    for index in 0..4 {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::<i32, i32>::new(&mut b);
        let mut g = b.build();
        if index < 3 {
            let mut tx = g.transaction();
            match index {
                0 => tx
                    .apply(&s.basic, TupleChange::put(0, (0, Some(0))))
                    .unwrap(),
                1 => tx
                    .apply(&s.lists.entities, TupleChange::put(0, (0, vec![])))
                    .unwrap(),
                _ => tx
                    .apply(
                        &s.lists.values,
                        TupleChange::put(
                            0,
                            ListValueRow {
                                range_index: 0,
                                value: 7,
                                position: None,
                                previous: None,
                                next: None,
                            },
                        ),
                    )
                    .unwrap(),
            }
            tx.commit().unwrap();
        }
        let s = if index == 3 {
            MixedStreamSources::new(&mut StreamGraphBuilder::<SimpleScore>::new())
        } else {
            s
        };
        assert!(greedy_mixed_stream_construction(&p, g, s, c).is_err());
    }
    let calls = Cell::new(0);
    let (g, s) = small_graph(&calls);
    let initial = MixedAssignment::new(
        Assignment::new(vec![Some(99)]),
        p.initial_assignment().lists().clone(),
    );
    let settings = MixedLocalSearchConfig {
        search: LocalSearchConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(mixed_stream_local_search_from(&p, g, s, initial, settings).is_err());
    assert_eq!(calls.get(), 0);
    let (g, s) = small_graph(&calls);
    let partial = small(true);
    assert!(matches!(
        mixed_stream_local_search_from(&partial, g, s, partial.initial_assignment(), settings),
        Err(MixedLocalSearchError::IncompleteInitialAssignment)
    ));
    assert_eq!(calls.get(), 0);
    let impossible = MixedProblem::new(
        FiniteProblem::new(vec![Variable::<i32>::new(vec![])]).unwrap(),
        ListProblem::new(vec![7], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let (g, s) = small_graph(&calls);
    let r = greedy_mixed_stream_construction(&impossible, g, s, MixedConstructionConfig::default())
        .unwrap();
    assert_eq!(r.termination, ConstructionTermination::NoCompleteAssignment);
    assert_eq!(calls.get(), 0);
}
#[test]
fn partial_joint_composites_deliver_final_rows_once_and_preserve_retained_matches() {
    let p = small(true);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    let j = b
        .join_equal(s.basic.port(), s.lists.values.port(), |_| 0, |_| 0)
        .unwrap();
    let terminal = b
        .score("coupled", &j, |((_, v), row)| {
            calls.set(calls.get() + 1);
            objective(*v, row.position.map(|p| p.entity))
        })
        .unwrap();
    let mut d = MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(-2).with_structural(-2));
    let retained = d.graph().constraint_matches(&terminal).unwrap().clone();
    let movement = MixedMove {
        basic: Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(1),
            },
            Move::Change {
                variable: 0,
                value: Some(0),
            },
        ]),
        lists: ListMove::Composite(vec![
            ListMove::Assign {
                value: 7,
                destination: ListPosition {
                    entity: 1,
                    index: 0,
                },
            },
            ListMove::Change {
                source: ListPosition {
                    entity: 1,
                    index: 0,
                },
                destination: ListPosition {
                    entity: 0,
                    index: 0,
                },
            },
        ]),
    };
    let before = calls.get();
    let trial = d.apply(&movement).unwrap();
    assert_eq!(trial.constraint_evaluations(), 2);
    assert_eq!(calls.get() - before, 2);
    assert_eq!(trial.score(), SimpleScore::ZERO);
    let matches = trial.graph().constraint_matches(&terminal).unwrap();
    assert_eq!(matches.len(), 1);
    let (tuple, contribution) = &matches[&(0, 0)];
    assert_eq!(tuple.0, (0, Some(0)));
    assert_eq!(
        tuple.1.position,
        Some(ListPosition {
            entity: 0,
            index: 0
        })
    );
    assert_eq!(*contribution, SimpleScore::ZERO);
    trial.commit();
    assert_eq!(calls.get() - before, 2);
    assert_eq!(retained[&(0, 0)].0.0, (0, None));
    assert!(retained[&(0, 0)].0.1.position.is_none());
    let noop = MixedMove {
        basic: Move::Composite(vec![]),
        lists: ListMove::Composite(vec![]),
    };
    let trial = d.apply(&noop).unwrap();
    assert_eq!(trial.constraint_evaluations(), 0);
    assert_eq!(trial.checkpoint_count(), 0);
    drop(trial);
    d.assert_consistent().unwrap();
}
#[test]
fn failed_rebuild_diagnostic_restores_cached_joint_score_and_remains_reusable() {
    let p = small(false);
    let corrupt = Cell::new(false);
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    let j = b
        .join_equal(s.basic.port(), s.lists.values.port(), |_| 0, |_| 0)
        .unwrap();
    b.score("coupled", &j, |((_, v), row)| {
        let ordinary = objective(*v, row.position.map(|p| p.entity));
        if corrupt.get() {
            ordinary + SimpleScore::new(1)
        } else {
            ordinary
        }
    })
    .unwrap();
    let mut d = MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    corrupt.set(true);
    assert!(d.assert_consistent().is_err());
    assert_eq!(d.score(), SimpleScore::ZERO);
    assert_eq!(d.assignment(), &p.initial_assignment());
    corrupt.set(false);
    d.assert_consistent().unwrap();
    d.apply(&MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 0,
            },
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    })
    .unwrap()
    .commit();
    assert_eq!(d.score(), SimpleScore::new(10));
    d.assert_consistent().unwrap();
}
