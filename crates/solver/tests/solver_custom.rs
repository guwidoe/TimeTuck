//! Original cross-backend ordered custom-phase regressions.
use std::cell::Cell;
use timetuck::*;
#[derive(Debug, PartialEq)]
struct Outcome {
    values: Vec<Option<i64>>,
    score: Option<SimpleScore>,
    complete: bool,
    work: SolverWork,
    phases: Vec<PhaseReport<SimpleScore>>,
    reason: SolverTermination,
}
#[derive(Debug, PartialEq)]
enum Event {
    Best(usize, SolverPhaseKind, SimpleScore, bool),
    Initialized(SolverInitialization, Option<SimpleScore>, bool),
}
#[derive(Debug, PartialEq)]
enum Failure {
    Custom(usize),
    Scoring(usize),
    Unbound(usize),
    Token(usize),
    Other,
    Incomplete(usize),
}
fn failure<E>(e: SolverError<E>) -> Failure {
    match e {
        SolverError::Custom {
            phase,
            error: CustomPhaseError::Scoring(_),
        } => Failure::Scoring(phase),
        SolverError::Custom { phase, .. } => Failure::Custom(phase),
        SolverError::UnboundCustomPhase { phase } => Failure::Unbound(phase),
        SolverError::ConflictingCancellation { phase } => Failure::Token(phase),
        SolverError::IncompleteLocalSearch { phase } => Failure::Incomplete(phase),
        _ => Failure::Other,
    }
}
fn command<A: Clone, M, E>(
    index: usize,
    c: &mut CustomPhaseContext<'_, A, M, SimpleScore, E>,
    make: impl Fn(&A, i64) -> M,
    operations: &[Vec<i64>],
    entered: &mut Vec<(usize, SimpleScore)>,
) -> Result<(), CustomPhaseError<E>> {
    entered.push((index, c.score()));
    for &value in &operations[index] {
        if value == 999 {
            return Err(CustomPhaseError::Command("requested failure".into()));
        }
        if value < 0 {
            c.evaluate(&make(c.assignment(), -value - 1), |_, s| s)?;
        } else {
            c.execute(&make(c.assignment(), value))?;
        }
    }
    Ok(())
}
fn custom() -> SolverPhase<'static> {
    SolverPhase::Custom(CustomPhaseConfig::default())
}
fn local() -> SolverPhase<'static> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 0,
        ..Default::default()
    })
}
fn sequence(phases: Vec<SolverPhase<'_>>) -> SolverSequence<'_> {
    SolverSequence {
        phases,
        limits: SolverLimits::default(),
    }
}
#[allow(clippy::too_many_arguments)]
fn run(
    mode: usize,
    initialized: bool,
    config: SolverSequence<'_>,
    operations: &[Vec<i64>],
    entered: &mut Vec<(usize, SimpleScore)>,
    events: &mut Vec<Event>,
    observed: bool,
    hook: impl Fn(),
) -> Result<Outcome, Failure> {
    macro_rules! invoke {
        ($plain:path, $observed:path; $($args:expr),*) => {{
            if observed { $observed($($args,)* |e| match e {
                SolverEvent::Best(u) => events.push(Event::Best(u.phase, u.kind, u.score, u.complete)),
                SolverEvent::FirstInitialized { initialization, score, complete, .. } => events.push(Event::Initialized(initialization, score, complete)),
            }) } else { $plain($($args),*) }
        }};
    }
    macro_rules! done {
        ($r:expr,$values:expr) => {{
            let r = $r.map_err(failure)?;
            let values = $values(&r.assignment);
            Ok(Outcome {
                values,
                score: r.score,
                complete: r.complete,
                work: r.work,
                phases: r.phases,
                reason: r.termination,
            })
        }};
    }
    if mode < 2 {
        let v = Variable::new(vec![0, 1, 2]);
        let p = FiniteProblem::new(vec![if initialized { v.with_initial(0) } else { v }]).unwrap();
        let make = |_: &Assignment<i64>, v| Move::Change {
            variable: 0,
            value: Some(v),
        };
        let values = |a: &Assignment<i64>| a.values().to_vec();
        if mode == 0 {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| {
                hook();
                SimpleScore::new(v[0].copied().unwrap_or(0))
            })
            .unwrap();
            return done!(
                invoke!(solve_with_commands, solve_with_commands_and_events;
                    &p,
                    &n,
                    config,
                    |index, c| command(index, c, make, operations, entered)
                ),
                values
            );
        }
        let graph = || {
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<i64>)>();
            b.score("value", s.port(), |(_, v)| {
                hook();
                SimpleScore::new(v.unwrap_or(0))
            })
            .unwrap();
            Ok((b.build(), s))
        };
        return done!(
            invoke!(solve_stream_with_commands, solve_stream_with_commands_and_events;
                &p,
                graph,
                config,
                |index, c| command(index, c, make, operations, entered)
            ),
            values
        );
    }
    let list = || {
        ListProblem::new(
            vec![7i64],
            (0..3)
                .map(|i| {
                    ListEntity::new(if initialized && i == 0 {
                        vec![7]
                    } else {
                        vec![]
                    })
                })
                .collect(),
        )
        .unwrap()
    };
    let list_move = |a: &ListAssignment<i64>, v: i64| {
        let destination = ListPosition {
            entity: v as usize,
            index: 0,
        };
        match a.location(&7) {
            Some(source) => ListMove::Change {
                source,
                destination,
            },
            None => ListMove::Assign {
                value: 7,
                destination,
            },
        }
    };
    if mode < 4 {
        let p = list();
        let values = |a: &ListAssignment<i64>| vec![a.location(&7).map(|p| p.entity as i64)];
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(3, 1);
            n.add_constraint("value", vec![ListScope::Value(0)], |v| {
                hook();
                let ListFact::Value { element, .. } = &v[0] else {
                    panic!()
                };
                SimpleScore::new(element.as_ref().map_or(0, |e| e.position.entity as i64))
            })
            .unwrap();
            return done!(
                invoke!(solve_list_with_commands, solve_list_with_commands_and_events;
                    &p,
                    &n,
                    config,
                    |index, c| command(index, c, list_move, operations, entered)
                ),
                values
            );
        }
        let graph = || {
            let mut b = StreamGraphBuilder::new();
            let s = ListStreamSources::new(&mut b);
            b.score("value", s.values.port(), |v| {
                hook();
                SimpleScore::new(v.position.map_or(0, |p| p.entity as i64))
            })
            .unwrap();
            Ok((b.build(), s))
        };
        return done!(
            invoke!(solve_list_stream_with_commands, solve_list_stream_with_commands_and_events;
                &p,
                graph,
                config,
                |index, c| command(index, c, list_move, operations, entered)
            ),
            values
        );
    }
    let config = MixedSolverSequence {
        limits: config.limits,
        phases: config
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
                SolverPhase::Construction(c) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction: c,
                        ..Default::default()
                    })
                }
                SolverPhase::LocalSearch(c) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search: c,
                        ..Default::default()
                    })
                }
                SolverPhase::RuinRecreate(search) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search,
                        ..Default::default()
                    })
                }
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
            })
            .collect(),
    };
    let v = Variable::new(vec![0, 1, 2]);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![if initialized { v.with_initial(0) } else { v }]).unwrap(),
        list(),
    );
    let make = |a: &MixedAssignment<i64, i64>, v| MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(v),
        },
        lists: list_move(a.lists(), v),
    };
    let values = |a: &MixedAssignment<i64, i64>| {
        vec![
            a.basic().values()[0],
            a.lists().location(&7).map(|p| p.entity as i64),
        ]
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 3, 1);
        n.add_constraint(
            "value",
            vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
            |v| {
                hook();
                let (MixedFact::Basic(a), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
                else {
                    panic!()
                };
                SimpleScore::new(
                    a.copied().unwrap_or(0)
                        + element.as_ref().map_or(0, |e| e.position.entity as i64),
                )
            },
        )
        .unwrap();
        return done!(
            invoke!(solve_mixed_with_commands, solve_mixed_with_commands_and_events;
                &p,
                &n,
                config,
                |index, c| command(index, c, make, operations, entered)
            ),
            values
        );
    }
    let graph = || {
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let j = b
            .join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())
            .unwrap();
        b.score("value", &j, |((_, a), v)| {
            hook();
            SimpleScore::new(a.unwrap_or(0) + v.position.map_or(0, |p| p.entity as i64))
        })
        .unwrap();
        Ok((b.build(), s))
    };
    done!(
        invoke!(solve_mixed_stream_with_commands, solve_mixed_stream_with_commands_and_events;
            &p,
            graph,
            config,
            |index, c| command(index, c, make, operations, entered)
        ),
        values
    )
}

#[test]
fn custom_phases_carry_best_states_and_attribute_real_initialization_boundaries() {
    for mode in 0..6 {
        let factor = if mode < 4 { 1 } else { 2 };
        let mut entered = vec![];
        let mut events = vec![];
        let calls = Cell::new(0);
        let r = run(
            mode,
            true,
            sequence(vec![custom(), custom(), local()]),
            &[vec![-3, 1, 0], vec![2, 0], vec![]],
            &mut entered,
            &mut events,
            true,
            || calls.set(calls.get() + 1),
        )
        .unwrap();
        assert_eq!(r.values, vec![Some(2); factor]);
        assert_eq!(r.score, Some(SimpleScore::new(2 * factor as i64)));
        assert_eq!(r.work.evaluations, 8);
        assert_eq!(r.work.move_evaluations, 5);
        assert_eq!(r.work.completed_steps, 0);
        assert_eq!(r.work.accepted_moves, 0);
        assert_eq!(r.work.constraint_evaluations, calls.get());
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        assert_eq!(
            entered,
            vec![(0, SimpleScore::ZERO), (1, SimpleScore::new(factor as i64))]
        );
        assert_eq!(
            events,
            vec![
                Event::Best(0, SolverPhaseKind::Custom, SimpleScore::ZERO, true),
                Event::Best(
                    0,
                    SolverPhaseKind::Custom,
                    SimpleScore::new(factor as i64),
                    true
                ),
                Event::Best(
                    1,
                    SolverPhaseKind::Custom,
                    SimpleScore::new(2 * factor as i64),
                    true
                ),
                Event::Initialized(
                    SolverInitialization {
                        phase: 1,
                        kind: SolverPhaseKind::Custom,
                        terminated_early: false
                    },
                    r.score,
                    true
                ),
            ]
        );
        assert_eq!(
            r.phases.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }
}

#[test]
fn custom_phases_accept_partial_handoffs_and_do_not_initialize_without_local_search() {
    for mode in 0..6 {
        for phases in [
            vec![
                custom(),
                SolverPhase::Construction(ConstructionConfig::default()),
                local(),
            ],
            vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                custom(),
                local(),
            ],
            vec![custom(), custom()],
        ] {
            let mut entered = vec![];
            let mut events = vec![];
            let no_local = phases.len() == 2;
            let expected_kind = if matches!(phases[1], SolverPhase::Custom(_)) {
                SolverPhaseKind::Custom
            } else {
                SolverPhaseKind::Construction
            };
            let r = run(
                mode,
                false,
                sequence(phases),
                &[vec![], vec![1], vec![]],
                &mut entered,
                &mut events,
                true,
                || {},
            )
            .unwrap();
            assert!(r.complete);
            let initializations: Vec<_> = events
                .iter()
                .filter(|e| matches!(e, Event::Initialized(..)))
                .collect();
            if no_local {
                assert!(initializations.is_empty());
            } else {
                assert_eq!(
                    initializations,
                    vec![&Event::Initialized(
                        SolverInitialization {
                            phase: 1,
                            kind: expected_kind,
                            terminated_early: false,
                        },
                        r.score,
                        true
                    )]
                );
            }
        }
    }
}

#[test]
fn shared_custom_budgets_bound_every_prefix_with_identical_observed_and_plain_results() {
    for mode in 0..6 {
        for scores in 0..=9 {
            for moves in 0..=8 {
                let c = SolverSequence {
                    limits: SolverLimits {
                        max_evaluations: scores,
                        max_move_evaluations: Some(moves),
                        ..Default::default()
                    },
                    phases: vec![custom(), custom(), local()],
                };
                let mut events = vec![];
                let mut observed_entries = vec![];
                let r = run(
                    mode,
                    true,
                    c.clone(),
                    &[vec![1, -3, 0], vec![2, 0], vec![]],
                    &mut observed_entries,
                    &mut events,
                    true,
                    || {},
                )
                .unwrap();
                let mut plain_entries = vec![];
                let plain = run(
                    mode,
                    true,
                    c,
                    &[vec![1, -3, 0], vec![2, 0], vec![]],
                    &mut plain_entries,
                    &mut vec![],
                    false,
                    || {},
                )
                .unwrap();
                assert_eq!(r, plain);
                assert_eq!(observed_entries, plain_entries);
                assert!(r.work.evaluations <= scores);
                assert!(r.work.move_evaluations <= moves);
                assert_eq!(
                    r.work.evaluations,
                    r.phases.iter().map(|p| p.work.evaluations).sum::<u64>()
                );
                let bests: Vec<_> = events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Best(_, _, s, _) => Some(*s),
                        _ => None,
                    })
                    .collect();
                assert!(bests.windows(2).all(|w| w[0] < w[1]));
                assert_eq!(bests.last().copied(), r.score);
            }
        }
    }
}

#[test]
fn shared_targets_stop_committed_best_operations_but_ignore_temporary_trials() {
    for mode in 0..6 {
        let target = SimpleScore::new(if mode < 4 { 1 } else { 2 });
        for initialized in [false, true] {
            let c = SolverSequence {
                limits: SolverLimits {
                    best_score_limit: Some(target),
                    ..Default::default()
                },
                phases: vec![custom(), local()],
            };
            let mut events = vec![];
            let r = run(
                mode,
                initialized,
                c,
                &[vec![-3, 1, 2], vec![]],
                &mut vec![],
                &mut events,
                true,
                || {},
            )
            .unwrap();
            assert_eq!(r.score, Some(target));
            assert_eq!(r.reason, SolverTermination::BestScoreLimit);
            assert_eq!(r.phases.len(), 1);
            assert_eq!(r.work.evaluations, 3);
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::Custom(CustomPhaseTermination::BestScoreLimit)
            );
            assert!(matches!(
                events.last(),
                Some(Event::Initialized(
                    SolverInitialization {
                        phase: 0,
                        kind: SolverPhaseKind::Custom,
                        terminated_early: true
                    },
                    _,
                    true
                ))
            ));
        }
        for feasible in [false, true] {
            let c = SolverSequence {
                limits: SolverLimits {
                    stop_when_feasible: feasible,
                    max_unimproved_steps: (!feasible).then_some(0),
                    ..Default::default()
                },
                phases: vec![custom()],
            };
            let mut entered = vec![];
            let r = run(
                mode,
                true,
                c,
                &[vec![1, 2]],
                &mut entered,
                &mut vec![],
                false,
                || {},
            )
            .unwrap();
            assert_eq!(r.work.evaluations, 1);
            assert!(entered.is_empty());
            assert_eq!(
                r.reason,
                if feasible {
                    SolverTermination::FeasibleScore
                } else {
                    SolverTermination::UnimprovedStepLimit
                }
            );
        }
    }
}

#[test]
fn phase_limits_can_continue_while_shared_cancellation_and_conflicts_stop_the_solve() {
    for mode in 0..6 {
        let c = sequence(vec![
            SolverPhase::Custom(CustomPhaseConfig {
                max_evaluations: 2,
                ..Default::default()
            }),
            custom(),
            local(),
        ]);
        let mut entered = vec![];
        let r = run(
            mode,
            true,
            c,
            &[vec![1, 2], vec![2], vec![]],
            &mut entered,
            &mut vec![],
            true,
            || {},
        )
        .unwrap();
        assert_eq!(entered.len(), 2);
        assert_eq!(r.phases.len(), 3);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Custom(CustomPhaseTermination::EvaluationLimit)
        );
        assert_eq!(r.values, vec![Some(2); if mode < 4 { 1 } else { 2 }]);
        let token = CancellationToken::new();
        let other = CancellationToken::new();
        let c = SolverSequence {
            limits: SolverLimits {
                cancellation: Some(&token),
                ..Default::default()
            },
            phases: vec![
                custom(),
                SolverPhase::Custom(CustomPhaseConfig {
                    cancellation: Some(&other),
                    ..Default::default()
                }),
            ],
        };
        assert_eq!(
            run(
                mode,
                true,
                c,
                &[vec![], vec![]],
                &mut vec![],
                &mut vec![],
                false,
                || panic!("invalid plan scored")
            ),
            Err(Failure::Token(1))
        );
        let calls = Cell::new(0);
        let c = SolverSequence {
            limits: SolverLimits {
                cancellation: Some(&token),
                ..Default::default()
            },
            phases: vec![custom(), local()],
        };
        let r = run(
            mode,
            true,
            c,
            &[vec![1, 2], vec![]],
            &mut vec![],
            &mut vec![],
            true,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    token.cancel();
                }
            },
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(r.work.move_evaluations, 1);
        assert_eq!(r.values, vec![Some(1); if mode < 4 { 1 } else { 2 }]);
    }
}

#[test]
fn command_and_scoring_failures_keep_indices_and_never_emit_initialization() {
    for mode in 0..6 {
        for (value, expected) in [(999, Failure::Custom(1)), (99, Failure::Scoring(1))] {
            let mut events = vec![];
            assert_eq!(
                run(
                    mode,
                    true,
                    sequence(vec![custom(), custom(), local()]),
                    &[vec![1], vec![value], vec![]],
                    &mut vec![],
                    &mut events,
                    true,
                    || {}
                ),
                Err(expected)
            );
            assert!(!events.iter().any(|e| matches!(e, Event::Initialized(..))));
            assert_eq!(events.len(), 2);
        }
    }
}

#[test]
fn unbound_custom_phases_are_rejected_before_scoring_even_when_unreachable() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("never", vec![0], |_| -> SimpleScore {
        panic!("unbound plan scored")
    })
    .unwrap();
    let c = SolverSequence {
        limits: SolverLimits {
            max_evaluations: 0,
            ..Default::default()
        },
        phases: vec![local(), custom()],
    };
    assert!(matches!(
        solve(&p, &n, c.clone()),
        Err(SolverError::UnboundCustomPhase { phase: 1 })
    ));
    assert!(matches!(
        solve_stream(&p, || { panic!("unbound factory") }, c),
        Err(SolverError::UnboundCustomPhase { phase: 1 })
    ));
    let r = solve_with_commands(
        &p,
        &ConstraintNetwork::<_, SimpleScore>::new(2),
        sequence(vec![SolverPhase::Custom(CustomPhaseConfig {
            max_evaluations: 0,
            ..Default::default()
        })]),
        |_, _| panic!("invalid input command"),
    );
    assert!(matches!(
        r,
        Err(SolverError::Custom {
            phase: 0,
            error: CustomPhaseError::Scoring(_)
        })
    ));
}

#[test]
fn strict_handoff_policy_applies_to_custom_and_unscored_exact_producers() {
    for mode in 0..6 {
        for repeat in [1, 2] {
            let mut phases = vec![custom(); repeat];
            phases.push(local());
            let c = SolverSequence {
                phases,
                limits: SolverLimits {
                    incomplete_handoff: IncompleteHandoffPolicy::Error,
                    ..Default::default()
                },
            };
            let mut events = vec![];
            let mut entered = vec![];
            assert_eq!(
                run(
                    mode,
                    false,
                    c,
                    &[vec![], vec![], vec![]],
                    &mut entered,
                    &mut events,
                    true,
                    || {}
                ),
                Err(Failure::Incomplete(repeat))
            );
            assert_eq!(entered.len(), repeat);
            assert!(
                matches!(events.last(), Some(Event::Initialized(SolverInitialization { phase,kind:SolverPhaseKind::Custom,terminated_early:false },Some(_),false)) if *phase==repeat-1)
            );
        }
        let c = SolverSequence {
            phases: vec![
                SolverPhase::Exact(ExactPhaseConfig {
                    max_evaluations: 0,
                    ..Default::default()
                }),
                local(),
            ],
            limits: SolverLimits {
                incomplete_handoff: IncompleteHandoffPolicy::Error,
                ..Default::default()
            },
        };
        let mut events = vec![];
        assert_eq!(
            run(
                mode,
                false,
                c,
                &[vec![], vec![]],
                &mut vec![],
                &mut events,
                true,
                || panic!("zero exact scored")
            ),
            Err(Failure::Incomplete(1))
        );
        assert_eq!(
            events,
            vec![Event::Initialized(
                SolverInitialization {
                    phase: 0,
                    kind: SolverPhaseKind::Exact,
                    terminated_early: true
                },
                None,
                false
            )]
        );
    }
}
