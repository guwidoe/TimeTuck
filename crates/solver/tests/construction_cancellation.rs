//! Original cancellation/prefix checks, independent of upstream method coverage.
use std::{cell::Cell, sync::mpsc, time::Duration};
use timetuck::*;

#[derive(Debug, PartialEq)]
struct Outcome {
    values: Vec<Option<usize>>,
    score: Option<SimpleScore>,
    complete: bool,
    evaluations: u64,
    callbacks: u128,
    assigned: usize,
    reason: ConstructionTermination,
}
fn objective(v: &[Option<usize>]) -> SimpleScore {
    SimpleScore::new(v.iter().flatten().map(|&v| v as i64).sum())
}
fn run(
    mode: usize,
    order: MixedConstructionOrder,
    config: ConstructionConfig<'_>,
    hook: impl Fn(),
) -> Outcome {
    let score = |v: &[Option<usize>]| {
        hook();
        objective(v)
    };
    macro_rules! outcome {
        ($r:expr, $p:expr, $values:expr, $assigned:expr) => {{
            let r = $r.unwrap();
            assert_eq!(r.move_evaluations(), r.evaluations.saturating_sub(1));
            $p.validate(&r.assignment).unwrap();
            let values: Vec<Option<usize>> = $values(&r.assignment);
            assert_eq!(r.complete, values.iter().all(Option::is_some));
            assert_eq!(r.solution().is_some(), r.complete && r.score.is_some());
            if let Some(s) = r.score {
                assert_eq!(
                    s,
                    objective(&values)
                        .with_structural(-(values.iter().filter(|v| v.is_none()).count() as i64))
                );
            }
            Outcome {
                values,
                score: r.score,
                complete: r.complete,
                evaluations: r.evaluations,
                callbacks: r.constraint_evaluations,
                assigned: $assigned(&r),
                reason: r.termination,
            }
        }};
    }
    if mode < 2 {
        let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]); 2]).unwrap();
        let values = |a: &Assignment<usize>| a.values().to_vec();
        let assigned = |r: &ConstructionResult<usize, SimpleScore>| r.assigned_variables;
        if mode == 0 {
            let mut n = ConstraintNetwork::new(2);
            n.add_constraint("value", vec![0, 1], |v| {
                score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>())
            })
            .unwrap();
            return outcome!(greedy_construction(&p, &n, config), p, values, assigned);
        }
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        let a = b.filter(s.port(), |(i, _)| *i == 0).unwrap();
        let z = b.filter(s.port(), |(i, _)| *i == 1).unwrap();
        let j = b.join_equal(&a, &z, |_| (), |_| ()).unwrap();
        b.score("value", &j, |((_, a), (_, z))| score(&[*a, *z]))
            .unwrap();
        return outcome!(
            greedy_stream_construction(&p, b.build(), s, config),
            p,
            values,
            assigned
        );
    }
    let lists = |values| {
        ListProblem::new(
            values,
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap()
    };
    if mode < 4 {
        let p = lists(vec![7, 8]);
        let values = |a: &ListAssignment<usize>| {
            [7, 8]
                .iter()
                .map(|v| a.location(v).map(|pos| pos.entity))
                .collect::<Vec<_>>()
        };
        let assigned = |r: &ListConstructionResult<usize, SimpleScore>| r.assigned_values;
        let evaluate_lists = |a: &[usize], z: &[usize]| {
            score(
                &[7, 8]
                    .iter()
                    .map(|v| {
                        if a.contains(v) {
                            Some(0)
                        } else if z.contains(v) {
                            Some(1)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(2, 2);
            n.add_constraint(
                "value",
                vec![ListScope::Entity(0), ListScope::Entity(1)],
                |v| {
                    let (ListFact::Entity(a), ListFact::Entity(z)) = (&v[0], &v[1]) else {
                        panic!()
                    };
                    evaluate_lists(a, z)
                },
            )
            .unwrap();
            return outcome!(
                greedy_list_construction(&p, &n, config),
                p,
                values,
                assigned
            );
        }
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::<usize>::new(&mut b);
        let a = b.filter(s.entities.port(), |(i, _)| *i == 0).unwrap();
        let z = b.filter(s.entities.port(), |(i, _)| *i == 1).unwrap();
        let j = b.join_equal(&a, &z, |_| (), |_| ()).unwrap();
        b.score("value", &j, |((_, a), (_, z))| evaluate_lists(a, z))
            .unwrap();
        return outcome!(
            greedy_list_stream_construction(&p, b.build(), s, config),
            p,
            values,
            assigned
        );
    }
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0usize, 1])]).unwrap(),
        lists(vec![7]),
    );
    let values = |a: &MixedAssignment<usize, usize>| {
        vec![
            a.basic().values()[0],
            a.lists().location(&7).map(|p| p.entity),
        ]
    };
    let assigned = |r: &MixedConstructionResult<usize, usize, SimpleScore>| {
        r.assigned_basic_variables + r.assigned_list_values
    };
    let config = MixedConstructionConfig {
        construction: config,
        order,
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 2, 1);
        n.add_constraint(
            "value",
            vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
            |v| {
                let (MixedFact::Basic(a), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
                else {
                    panic!()
                };
                score(&[a.copied(), element.as_ref().map(|v| v.position.entity)])
            },
        )
        .unwrap();
        return outcome!(
            greedy_mixed_construction(&p, &n, config),
            p,
            values,
            assigned
        );
    }
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<usize, usize>::new(&mut b);
    let j = b
        .join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())
        .unwrap();
    b.score("value", &j, |((_, a), v)| {
        score(&[*a, v.position.map(|p| p.entity)])
    })
    .unwrap();
    outcome!(
        greedy_mixed_stream_construction(&p, b.build(), s, config),
        p,
        values,
        assigned
    )
}
const ORDERS: [MixedConstructionOrder; 2] = [
    MixedConstructionOrder::BasicThenLists,
    MixedConstructionOrder::ListsThenBasic,
];

#[test]
fn cancelled_construction_retains_the_same_prefix_as_a_matching_budget() {
    for mode in 0..6 {
        for order in ORDERS {
            for pick_early in [
                ConstructionPickEarly::Never,
                ConstructionPickEarly::FirstFeasibleScore,
            ] {
                let base = ConstructionConfig {
                    pick_early,
                    ..Default::default()
                };
                let full = run(mode, order, base, || {});
                for budget in 0..=full.evaluations + 1 {
                    for cancel_at in 0..=full.evaluations + 1 {
                        let token = CancellationToken::new();
                        if cancel_at == 0 {
                            token.cancel();
                        }
                        let calls = Cell::new(0);
                        let actual = run(
                            mode,
                            order,
                            ConstructionConfig {
                                cancellation: Some(&token),
                                max_evaluations: budget,
                                ..base
                            },
                            || {
                                calls.set(calls.get() + 1);
                                if calls.get() == cancel_at {
                                    token.cancel();
                                }
                            },
                        );
                        let mut expected = run(
                            mode,
                            order,
                            ConstructionConfig {
                                max_evaluations: budget.min(cancel_at),
                                ..base
                            },
                            || {},
                        );
                        if budget > 0 && cancel_at <= budget.min(full.evaluations) {
                            expected.reason = ConstructionTermination::Cancelled;
                        }
                        assert_eq!(
                            actual, expected,
                            "mode {mode}, {order:?}, {pick_early:?}, budget {budget}, cancel {cancel_at}"
                        );
                        assert_eq!(actual.evaluations, calls.get());
                        assert_eq!(actual.callbacks, calls.get() as u128);
                    }
                }
            }
        }
    }
}
#[test]
fn construction_can_be_cancelled_by_another_thread_during_a_candidate() {
    for mode in 0..6 {
        for order in ORDERS {
            let token = CancellationToken::new();
            let worker_token = token.clone();
            let (entered_tx, entered_rx) = mpsc::sync_channel(0);
            let (release_tx, release_rx) = mpsc::sync_channel(0);
            let worker = std::thread::spawn(move || {
                let calls = Cell::new(0);
                run(
                    mode,
                    order,
                    ConstructionConfig {
                        cancellation: Some(&worker_token),
                        ..Default::default()
                    },
                    || {
                        calls.set(calls.get() + 1);
                        if calls.get() == 2 {
                            entered_tx.send(()).unwrap();
                            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                        }
                    },
                )
            });
            entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            token.cancel();
            release_tx.send(()).unwrap();
            let r = worker.join().unwrap();
            assert_eq!(r.reason, ConstructionTermination::Cancelled);
            assert_eq!(r.evaluations, 2);
            assert_eq!(r.assigned, 1);
            assert!(!r.complete);
            assert_eq!(r.values.iter().filter(|v| v.is_some()).count(), 1);
        }
    }
}
#[test]
fn cancellation_during_diagnostics_retains_initial_score_and_can_be_reused() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        let calls = Cell::new(0);
        let c = ConstructionConfig {
            cancellation: Some(&token),
            assert_scores: true,
            ..Default::default()
        };
        let r = run(mode, ORDERS[0], c, || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                token.cancel();
            }
        });
        assert_eq!(r.reason, ConstructionTermination::Cancelled);
        assert_eq!(r.evaluations, 1);
        assert_eq!(r.callbacks, 1);
        assert_eq!(r.assigned, 0);
        let r = run(mode, ORDERS[0], c, || {
            panic!("reused cancelled token must not score")
        });
        assert_eq!(r.reason, ConstructionTermination::Cancelled);
        assert_eq!(r.evaluations, 0);
        assert_eq!(r.score, None);
    }
}

#[test]
fn construction_validation_and_impossibility_precede_cancellation() {
    let token = CancellationToken::new();
    token.cancel();
    for max_evaluations in [0, 100] {
        let c = ConstructionConfig {
            cancellation: Some(&token),
            max_evaluations,
            ..Default::default()
        };
        let basic = FiniteProblem::<usize>::new(vec![Variable::new(vec![])]).unwrap();
        let lists = ListProblem::<usize>::new(vec![7], vec![]).unwrap();
        let n = ConstraintNetwork::<usize, SimpleScore>::new(1);
        let b = greedy_construction(&basic, &n, c).unwrap();
        assert_eq!(b.termination, ConstructionTermination::NoCompleteAssignment);
        let n = ListConstraintNetwork::<usize, SimpleScore>::new(0, 1);
        let l = greedy_list_construction(&lists, &n, c).unwrap();
        assert_eq!(l.termination, ConstructionTermination::NoCompleteAssignment);
        let p = MixedProblem::new(basic, lists);
        let n = MixedConstraintNetwork::<usize, usize, SimpleScore>::new(1, 0, 1);
        let m = greedy_mixed_construction(
            &p,
            &n,
            MixedConstructionConfig {
                construction: c,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(m.termination, ConstructionTermination::NoCompleteAssignment);
        let n = ConstraintNetwork::<usize, SimpleScore>::new(0);
        assert!(greedy_construction(p.basic(), &n, c).is_err());
        let n = ListConstraintNetwork::<usize, SimpleScore>::new(1, 1);
        assert!(greedy_list_construction(p.lists(), &n, c).is_err());
        let n = MixedConstraintNetwork::<usize, usize, SimpleScore>::new(2, 0, 1);
        assert!(
            greedy_mixed_construction(
                &p,
                &n,
                MixedConstructionConfig {
                    construction: c,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let s = builder.source::<usize, (usize, Option<usize>)>();
        let mut g = builder.build();
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(0, (0, None))).unwrap();
        tx.commit().unwrap();
        assert!(greedy_stream_construction(p.basic(), g, s, c).is_err());
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::<usize>::new(&mut builder);
        let mut g = builder.build();
        let mut tx = g.transaction();
        tx.apply(&s.entities, TupleChange::put(0, (0, vec![])))
            .unwrap();
        tx.commit().unwrap();
        assert!(greedy_list_stream_construction(p.lists(), g, s, c).is_err());
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::<usize, usize>::new(&mut builder);
        let mut g = builder.build();
        let mut tx = g.transaction();
        tx.apply(&s.basic, TupleChange::put(0, (0, None))).unwrap();
        tx.commit().unwrap();
        assert!(
            greedy_mixed_stream_construction(
                &p,
                g,
                s,
                MixedConstructionConfig {
                    construction: c,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn initial_callback_cancellation_is_observed_even_without_construction_work() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        let c = ConstructionConfig {
            cancellation: Some(&token),
            ..Default::default()
        };
        let basic = FiniteProblem::<usize>::new(vec![]).unwrap();
        let lists = ListProblem::<usize>::new(vec![], vec![]).unwrap();
        macro_rules! check {
            ($r:expr) => {{
                let r = $r.unwrap();
                assert_eq!(r.move_evaluations(), r.evaluations.saturating_sub(1));
                assert_eq!(r.termination, ConstructionTermination::Cancelled);
                assert_eq!(r.evaluations, 1);
                assert_eq!(r.constraint_evaluations, 1);
                assert!(r.complete);
                assert!(r.solution().is_some());
                assert_eq!(r.score, Some(SimpleScore::ZERO));
            }};
        }
        if mode == 0 {
            let mut n = ConstraintNetwork::new(0);
            n.add_constraint("constant", vec![], |_| {
                token.cancel();
                SimpleScore::ZERO
            })
            .unwrap();
            check!(greedy_construction(&basic, &n, c));
        } else if mode == 2 {
            let mut n = ListConstraintNetwork::new(0, 0);
            n.add_constraint("constant", vec![], |_| {
                token.cancel();
                SimpleScore::ZERO
            })
            .unwrap();
            check!(greedy_list_construction(&lists, &n, c));
        } else if mode == 4 {
            let p = MixedProblem::new(basic, lists);
            let mut n = MixedConstraintNetwork::new(0, 0, 0);
            n.add_constraint("constant", vec![], |_| {
                token.cancel();
                SimpleScore::ZERO
            })
            .unwrap();
            check!(greedy_mixed_construction(
                &p,
                &n,
                MixedConstructionConfig {
                    construction: c,
                    ..Default::default()
                }
            ));
        } else {
            // Fully initialized source rows require no construction candidates.
            let mut b = StreamGraphBuilder::new();
            if mode == 1 {
                let basic =
                    FiniteProblem::new(vec![Variable::new(vec![7usize]).with_initial(7)]).unwrap();
                let s = b.source::<usize, (usize, Option<usize>)>();
                b.score("constant", s.port(), |_| {
                    token.cancel();
                    SimpleScore::ZERO
                })
                .unwrap();
                check!(greedy_stream_construction(&basic, b.build(), s, c));
            } else if mode == 3 {
                let lists = ListProblem::new(vec![7usize], vec![ListEntity::new(vec![7])]).unwrap();
                let s = ListStreamSources::<usize>::new(&mut b);
                b.score("constant", s.values.port(), |_| {
                    token.cancel();
                    SimpleScore::ZERO
                })
                .unwrap();
                check!(greedy_list_stream_construction(&lists, b.build(), s, c));
            } else {
                let basic =
                    FiniteProblem::new(vec![Variable::new(vec![7usize]).with_initial(7)]).unwrap();
                let p = MixedProblem::new(basic, lists);
                let s = MixedStreamSources::<usize, usize>::new(&mut b);
                b.score("constant", s.basic.port(), |_| {
                    token.cancel();
                    SimpleScore::ZERO
                })
                .unwrap();
                check!(greedy_mixed_stream_construction(
                    &p,
                    b.build(),
                    s,
                    MixedConstructionConfig {
                        construction: c,
                        ..Default::default()
                    }
                ));
            }
        }
    }
}

#[test]
fn construction_candidate_budgets_exclude_initialization_and_diagnostics() {
    for mode in 0..6 {
        for order in ORDERS {
            for pick_early in [
                ConstructionPickEarly::Never,
                ConstructionPickEarly::FirstFeasibleScore,
            ] {
                for max_evaluations in 0..=8 {
                    for max_moves in 0..=7 {
                        let c = ConstructionConfig {
                            max_evaluations,
                            pick_early,
                            assert_scores: true,
                            ..Default::default()
                        };
                        let mut expected = run(
                            mode,
                            order,
                            ConstructionConfig {
                                max_evaluations: max_evaluations.min(max_moves + 1),
                                ..c
                            },
                            || {},
                        );
                        let actual = run(
                            mode,
                            order,
                            ConstructionConfig {
                                max_move_evaluations: Some(max_moves),
                                ..c
                            },
                            || {},
                        );
                        if max_moves + 1 < max_evaluations
                            && expected.reason == ConstructionTermination::EvaluationLimit
                        {
                            expected.reason = ConstructionTermination::MoveEvaluationLimit;
                        }
                        assert_eq!(
                            actual, expected,
                            "mode {mode}, order {order:?}, score budget {max_evaluations}, move budget {max_moves}"
                        );
                        assert!(actual.evaluations.saturating_sub(1) <= max_moves);
                    }
                }
            }
        }
    }
}

#[test]
fn construction_move_limit_keeps_precedence_if_diagnostics_cancel_after_observation() {
    for mode in 0..6 {
        for order in ORDERS {
            // Initial score + diagnostic; first candidate + diagnostic; installed
            // prefix diagnostic. Request four closes enumeration before the cap
            // is observed. Request five arrives after the cap was observed.
            for at in [4, 5] {
                let token = CancellationToken::new();
                let calls = Cell::new(0);
                let r = run(
                    mode,
                    order,
                    ConstructionConfig {
                        cancellation: Some(&token),
                        max_move_evaluations: Some(1),
                        assert_scores: true,
                        ..Default::default()
                    },
                    || {
                        calls.set(calls.get() + 1);
                        if calls.get() == at {
                            token.cancel();
                        }
                    },
                );
                assert_eq!(
                    r.reason,
                    if at == 4 {
                        ConstructionTermination::Cancelled
                    } else {
                        ConstructionTermination::MoveEvaluationLimit
                    }
                );
                assert_eq!(r.evaluations, 2);
                assert_eq!(r.callbacks, 2);
                assert_eq!(r.assigned, 1);
                assert!(!r.complete);
                assert_eq!(calls.get(), 5);
            }
        }
    }
}

#[path = "support/construction_step_cases.rs"]
mod step_cases;
