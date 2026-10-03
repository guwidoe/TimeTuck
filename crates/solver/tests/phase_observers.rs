//! Original committed-state observation across every model and scoring backend.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
    time::Duration,
};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    values: Vec<Option<usize>>,
    score: SimpleScore,
    complete: bool,
}
#[derive(Debug, Eq, PartialEq)]
struct Run {
    last: Option<Snapshot>,
    evaluations: u64,
    constraint_evaluations: u128,
    steps: u64,
    reason: String,
    calls: usize,
}
#[allow(clippy::too_many_arguments)] // Shared six-backend test fixture.
fn run(
    mode: usize,
    construction: bool,
    observed: bool,
    c: ConstructionConfig<'_>,
    l: LocalSearchConfig<'_>,
    order: MixedConstructionOrder,
    objective: impl Fn(i64) -> SimpleScore,
    mut notify: impl FnMut(Snapshot),
) -> Result<Run, String> {
    let calls = Cell::new(0);
    let score = |v: &[Option<usize>]| {
        calls.set(calls.get() + 1);
        objective(v.iter().flatten().map(|&v| v as i64).sum())
    };
    let mut event = |values: Vec<Option<usize>>, s, complete| {
        assert_eq!(
            s,
            objective(values.iter().flatten().map(|&v| v as i64).sum())
                .with_structural(-(values.iter().filter(|v| v.is_none()).count() as i64))
        );
        assert_eq!(complete, values.iter().all(Option::is_some));
        notify(Snapshot {
            values,
            score: s,
            complete,
        });
    };
    macro_rules! invoke {
        ($plain:path,$obs:path,$values:expr; $($arg:expr),*) => {{
            if observed { $obs($($arg,)* |u| event($values(u.assignment),u.score,u.complete)) }
            else { $plain($($arg),*) }.map_err(|e|e.to_string())?
        }};
    }
    macro_rules! constructed {
        ($r:expr,$values:expr) => {{
            let r = $r;
            Ok(Run {
                last: r.score.map(|s| Snapshot {
                    values: $values(&r.assignment),
                    score: s,
                    complete: r.complete,
                }),
                evaluations: r.evaluations,
                constraint_evaluations: r.constraint_evaluations,
                steps: 0,
                reason: format!("{:?}", r.termination),
                calls: calls.get(),
            })
        }};
    }
    macro_rules! searched {
        ($r:expr,$values:expr) => {{
            let r = $r;
            Ok(Run {
                last: r.best.map(|b| Snapshot {
                    values: $values(&b.assignment),
                    score: b.score,
                    complete: true,
                }),
                evaluations: r.evaluations,
                constraint_evaluations: r.constraint_evaluations,
                steps: r.completed_steps,
                reason: format!("{:?}", r.termination),
                calls: calls.get(),
            })
        }};
    }
    let variable = Variable::new(vec![0usize, 1, 2]);
    let basic = FiniteProblem::new(vec![if construction {
        variable
    } else {
        variable.with_initial(0)
    }])
    .unwrap();
    if mode < 2 {
        let values = |a: &Assignment<usize>| a.values().to_vec();
        if mode == 0 {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("score", vec![0], |v| score(&[v[0].copied()]))
                .unwrap();
            return if construction {
                constructed!(
                    invoke!(greedy_construction,greedy_construction_with_observer,values; &basic,&n,c),
                    values
                )
            } else {
                searched!(
                    invoke!(local_search_from,local_search_from_with_observer,values; &basic,&n,basic.initial_assignment(),l),
                    values
                )
            };
        }
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        b.score("score", s.port(), |(_, v)| score(&[*v])).unwrap();
        return if construction {
            constructed!(
                invoke!(greedy_stream_construction,greedy_stream_construction_with_observer,values; &basic,b.build(),s,c),
                values
            )
        } else {
            searched!(
                invoke!(stream_local_search_from,stream_local_search_from_with_observer,values; &basic,b.build(),s,basic.initial_assignment(),l),
                values
            )
        };
    }
    let lists = ListProblem::new(
        vec![7usize],
        vec![
            ListEntity::new(if construction { vec![] } else { vec![7] }),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    if mode < 4 {
        let values = |a: &ListAssignment<usize>| vec![a.location(&7).map(|p| p.entity)];
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(3, 1);
            n.add_constraint("score", vec![ListScope::Value(0)], |v| {
                let ListFact::Value { element, .. } = &v[0] else {
                    panic!()
                };
                score(&[element.as_ref().map(|v| v.position.entity)])
            })
            .unwrap();
            return if construction {
                constructed!(
                    invoke!(greedy_list_construction,greedy_list_construction_with_observer,values; &lists,&n,c),
                    values
                )
            } else {
                searched!(
                    invoke!(list_local_search_from,list_local_search_from_with_observer,values; &lists,&n,lists.initial_assignment(),l),
                    values
                )
            };
        }
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::<usize>::new(&mut b);
        b.score("score", s.values.port(), |v| {
            score(&[v.position.map(|p| p.entity)])
        })
        .unwrap();
        return if construction {
            constructed!(
                invoke!(greedy_list_stream_construction,greedy_list_stream_construction_with_observer,values; &lists,b.build(),s,c),
                values
            )
        } else {
            searched!(
                invoke!(list_stream_local_search_from,list_stream_local_search_from_with_observer,values; &lists,b.build(),s,lists.initial_assignment(),l),
                values
            )
        };
    }
    let p = MixedProblem::new(basic, lists);
    let values = |a: &MixedAssignment<usize, usize>| {
        vec![
            a.basic().values()[0],
            a.lists().location(&7).map(|p| p.entity),
        ]
    };
    let c = MixedConstructionConfig {
        construction: c,
        order,
    };
    let l = MixedLocalSearchConfig {
        search: l,
        joint_moves: true,
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 3, 1);
        n.add_constraint(
            "score",
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
        return if construction {
            constructed!(
                invoke!(greedy_mixed_construction,greedy_mixed_construction_with_observer,values; &p,&n,c),
                values
            )
        } else {
            searched!(
                invoke!(mixed_local_search_from,mixed_local_search_from_with_observer,values; &p,&n,p.initial_assignment(),l),
                values
            )
        };
    }
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<usize, usize>::new(&mut b);
    let j = b
        .join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())
        .unwrap();
    b.score("score", &j, |((_, a), v)| {
        score(&[*a, v.position.map(|p| p.entity)])
    })
    .unwrap();
    if construction {
        constructed!(
            invoke!(greedy_mixed_stream_construction,greedy_mixed_stream_construction_with_observer,values; &p,b.build(),s,c),
            values
        )
    } else {
        searched!(
            invoke!(mixed_stream_local_search_from,mixed_stream_local_search_from_with_observer,values; &p,b.build(),s,p.initial_assignment(),l),
            values
        )
    }
}
const ORDER: MixedConstructionOrder = MixedConstructionOrder::BasicThenLists;
fn local() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 8,
        selection: MoveSelection::FirstAccepted,
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn observers_preserve_results_counts_and_emit_only_committed_states_all_backends() {
    for mode in 0..6 {
        for construction in [true, false] {
            for order in [ORDER, MixedConstructionOrder::ListsThenBasic] {
                let c = ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                };
                let plain = run(
                    mode,
                    construction,
                    false,
                    c,
                    local(),
                    order,
                    SimpleScore::new,
                    |_| panic!("not observed"),
                )
                .unwrap();
                let mut events = Vec::new();
                let observed = run(
                    mode,
                    construction,
                    true,
                    c,
                    local(),
                    order,
                    SimpleScore::new,
                    |e| events.push(e),
                )
                .unwrap();
                assert_eq!(observed, plain, "mode {mode}, construction {construction}");
                assert!(!events.is_empty());
                assert!(events.windows(2).all(|w| w[0].score < w[1].score));
                assert_eq!(events.last(), observed.last.as_ref());
                if construction {
                    assert!(!events[0].complete);
                    assert_eq!(events.len(), if mode < 4 { 2 } else { 3 });
                    if mode >= 4 {
                        assert_eq!(
                            events[1].values,
                            if order == ORDER {
                                vec![Some(2), None]
                            } else {
                                vec![None, Some(2)]
                            }
                        );
                    }
                } else {
                    assert!(events.iter().all(|e| e.complete));
                    assert!(events.len() > 1);
                }
            }
        }
    }
}
#[test]
fn budgets_cancellation_and_partial_construction_keep_last_committed_notification() {
    for mode in 0..6 {
        for construction in [true, false] {
            for budget in 0..=8 {
                let c = ConstructionConfig {
                    max_evaluations: budget,
                    ..Default::default()
                };
                let l = LocalSearchConfig {
                    max_evaluations: budget,
                    ..local()
                };
                let mut events = Vec::new();
                let r = run(
                    mode,
                    construction,
                    true,
                    c,
                    l,
                    ORDER,
                    SimpleScore::new,
                    |e| events.push(e),
                )
                .unwrap();
                assert_eq!(events.last(), r.last.as_ref());
                assert_eq!(events.is_empty(), budget == 0);
            }
            let token = CancellationToken::new();
            let c = ConstructionConfig {
                cancellation: Some(&token),
                ..Default::default()
            };
            let l = LocalSearchConfig {
                cancellation: Some(&token),
                ..local()
            };
            let mut events = Vec::new();
            let r = run(
                mode,
                construction,
                true,
                c,
                l,
                ORDER,
                SimpleScore::new,
                |e| {
                    events.push(e);
                    token.cancel();
                },
            )
            .unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!(r.evaluations, 1);
            assert_eq!(r.reason, "Cancelled");
            assert_eq!(events.last(), r.last.as_ref());
            let r = run(
                mode,
                construction,
                true,
                c,
                l,
                ORDER,
                SimpleScore::new,
                |_| panic!("pre-cancelled"),
            )
            .unwrap();
            assert_eq!(r.evaluations, 0);
        }
    }
}
#[test]
fn plateaus_and_accepted_worsening_moves_do_not_emit_extra_bests() {
    for mode in 0..6 {
        let mut plateau = Vec::new();
        let r = run(
            mode,
            false,
            true,
            ConstructionConfig::default(),
            LocalSearchConfig {
                acceptance: Acceptance::HillClimbing,
                ..local()
            },
            ORDER,
            |_| SimpleScore::ZERO,
            |e| plateau.push(e),
        )
        .unwrap();
        assert!(r.steps > 0);
        assert_eq!(plateau.len(), 1);
        let mut events = Vec::new();
        let r = run(
            mode,
            false,
            true,
            ConstructionConfig::default(),
            LocalSearchConfig {
                acceptance: Acceptance::VariableTabu {
                    tenure: 1,
                    aspiration: false,
                },
                ..local()
            },
            ORDER,
            |v| SimpleScore::new(-v),
            |e| events.push(e),
        )
        .unwrap();
        assert!(r.steps > 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events.last(), r.last.as_ref());
    }
}
#[test]
fn synchronous_observer_panics_propagate_and_managed_workers_remain_usable() {
    for mode in 0..6 {
        for construction in [true, false] {
            assert!(
                catch_unwind(AssertUnwindSafe(|| run(
                    mode,
                    construction,
                    true,
                    ConstructionConfig::default(),
                    local(),
                    ORDER,
                    SimpleScore::new,
                    |_| panic!("observer")
                )))
                .is_err()
            );
            let mut manager = SolverManager::<Run, String>::new(1).unwrap();
            let job = manager
                .submit(1, move |_| {
                    run(
                        mode,
                        construction,
                        true,
                        ConstructionConfig::default(),
                        local(),
                        ORDER,
                        SimpleScore::new,
                        |_| panic!("observer"),
                    )
                })
                .unwrap();
            assert!(
                matches!(job.wait_timeout(Duration::from_secs(10)).unwrap().as_ref(),Err(SolverJobError::Panicked(s)) if s=="observer")
            );
            assert!(
                manager
                    .submit(2, move |_| run(
                        mode,
                        construction,
                        true,
                        ConstructionConfig::default(),
                        local(),
                        ORDER,
                        SimpleScore::new,
                        |_| {}
                    ))
                    .unwrap()
                    .wait()
                    .is_ok()
            );
            manager.shutdown().unwrap();
        }
    }
}
#[test]
fn actual_phase_updates_reach_manager_consumers_before_final_for_all_backends() {
    let mut manager = SolverManager::<Snapshot, String>::new(2).unwrap();
    for mode in 0..6 {
        for construction in [true, false] {
            let snapshots = Arc::new(Mutex::new(Vec::new()));
            let seen = snapshots.clone();
            let final_seen = snapshots.clone();
            let job = manager
                .submit_with_progress(
                    mode,
                    move |token, p| {
                        let c = ConstructionConfig {
                            cancellation: Some(&token),
                            ..Default::default()
                        };
                        let l = LocalSearchConfig {
                            cancellation: Some(&token),
                            ..local()
                        };
                        run(
                            mode as usize,
                            construction,
                            true,
                            c,
                            l,
                            ORDER,
                            SimpleScore::new,
                            |e| {
                                assert!(p.publish(e));
                            },
                        )
                        .map(|r| r.last.unwrap())
                    },
                    SolverJobCallbacks::default()
                        .on_best(move |_, s: &Snapshot| {
                            seen.lock().unwrap().push(s.clone());
                        })
                        .on_final(move |_, s| {
                            let events = final_seen.lock().unwrap();
                            assert_eq!(events.last(), Some(s));
                            assert!(events.windows(2).all(|w| w[0].score < w[1].score));
                        }),
                )
                .unwrap();
            assert!(job.wait_timeout(Duration::from_secs(10)).unwrap().is_ok());
            assert!(job.callback_failures().is_empty());
            assert!(!snapshots.lock().unwrap().is_empty());
        }
    }
    manager.shutdown().unwrap();
}

#[test]
fn invalid_acceptance_and_input_shape_emit_nothing_even_at_zero_budget() {
    for mode in 0..6 {
        let l = LocalSearchConfig {
            max_evaluations: 0,
            acceptance: Acceptance::VariableTabu {
                tenure: 0,
                aspiration: false,
            },
            ..local()
        };
        assert!(
            run(
                mode,
                false,
                true,
                ConstructionConfig::default(),
                l,
                ORDER,
                SimpleScore::new,
                |_| panic!("invalid acceptance")
            )
            .is_err()
        );
    }
    let p = FiniteProblem::<usize>::new(vec![]).unwrap();
    let n = ConstraintNetwork::<usize, SimpleScore>::new(1);
    let c = ConstructionConfig {
        max_evaluations: 0,
        ..Default::default()
    };
    assert!(greedy_construction_with_observer(&p, &n, c, |_| panic!()).is_err());
    let list = ListProblem::<usize>::new(vec![], vec![]).unwrap();
    let n = ListConstraintNetwork::<usize, SimpleScore>::new(1, 0);
    assert!(greedy_list_construction_with_observer(&list, &n, c, |_| panic!()).is_err());
    let mixed = MixedProblem::new(p, list);
    let n = MixedConstraintNetwork::<usize, usize, SimpleScore>::new(1, 0, 0);
    assert!(
        greedy_mixed_construction_with_observer(
            &mixed,
            &n,
            MixedConstructionConfig {
                construction: c,
                ..Default::default()
            },
            |_| panic!()
        )
        .is_err()
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = b.source::<usize, (usize, Option<usize>)>();
    assert!(
        greedy_stream_construction_with_observer(
            mixed.basic(),
            StreamGraphBuilder::<SimpleScore>::new().build(),
            foreign,
            c,
            |_| panic!()
        )
        .is_err()
    );
    let foreign = ListStreamSources::new(&mut b);
    assert!(
        greedy_list_stream_construction_with_observer(
            mixed.lists(),
            StreamGraphBuilder::<SimpleScore>::new().build(),
            foreign,
            c,
            |_| panic!()
        )
        .is_err()
    );
    let foreign = MixedStreamSources::new(&mut b);
    assert!(
        greedy_mixed_stream_construction_with_observer(
            &mixed,
            StreamGraphBuilder::<SimpleScore>::new().build(),
            foreign,
            MixedConstructionConfig {
                construction: c,
                ..Default::default()
            },
            |_| panic!()
        )
        .is_err()
    );
}
#[test]
fn optional_unassigned_values_and_empty_models_can_emit_complete_initial_states() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]).allowing_unassigned()]).unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
    let mut count = 0;
    let r = greedy_construction_with_observer(&p, &n, ConstructionConfig::default(), |u| {
        count += 1;
        assert!(u.complete);
        assert_eq!(u.assignment.values(), &[None]);
        assert_eq!(u.score, SimpleScore::ZERO);
    })
    .unwrap();
    assert!(r.complete);
    assert_eq!(count, 1);
    let p = ListProblem::new(vec![7], vec![])
        .unwrap()
        .allowing_unassigned();
    let n = ListConstraintNetwork::<i32, SimpleScore>::new(0, 1);
    let mut count = 0;
    let r = greedy_list_construction_with_observer(&p, &n, ConstructionConfig::default(), |u| {
        count += 1;
        assert!(u.complete);
        assert!(u.assignment.location(&7).is_none());
        assert_eq!(u.score, SimpleScore::ZERO);
    })
    .unwrap();
    assert!(r.complete);
    assert_eq!(count, 1);
    let p = MixedProblem::new(
        FiniteProblem::<i32>::new(vec![]).unwrap(),
        ListProblem::<i32>::new(vec![], vec![]).unwrap(),
    );
    let n = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(0, 0, 0);
    let mut count = 0;
    let r =
        greedy_mixed_construction_with_observer(&p, &n, MixedConstructionConfig::default(), |u| {
            count += 1;
            assert!(u.complete);
            assert_eq!(u.score, SimpleScore::ZERO);
        })
        .unwrap();
    assert!(r.complete);
    assert_eq!(count, 1);
}
