//! Original executable XML termination controls; no upstream-method credit.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
macro_rules! extensions {
    ($clock:expr,$provider:expr,$bindings:expr) => {{
        let mut ext = SolverExtensions {
            clock: Some($clock),
            phase_terminations: $bindings,
            ..Default::default()
        };
        ext.move_providers.bind(1, $provider);
        ext
    }};
}
fn project(body: &str) -> XmlTerminationPlan {
    SolverXmlConfiguration::parse(&format!("<solver>{body}</solver>"))
        .unwrap()
        .native_termination_plan()
        .unwrap()
}
fn local() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 24,
        max_evaluations: 70,
        acceptance: Acceptance::HillClimbing,
        move_order: MoveOrder::ShuffledVariables { seed: 162 },
        ..Default::default()
    }
}
const MODES: [&str; 7] = [
    "single",
    "and",
    "or-rates",
    "nested",
    "override",
    "phase-first",
    "identity",
];
fn dim(window: u64) -> String {
    format!(
        "<diminishedReturns><slidingWindowSeconds>{window}</slidingWindowSeconds></diminishedReturns>"
    )
}
fn body(mode: &str) -> String {
    let short = dim(44);
    let long = dim(88);
    let root = match mode {
        "and" | "phase-first" => format!(
            "<terminationCompositionStyle>AND</terminationCompositionStyle><secondsSpentLimit>1000</secondsSpentLimit>{short}"
        ),
        "or-rates" => {
            format!("<termination>{short}</termination><termination>{long}</termination>")
        }
        "nested" => format!(
            "<termination><terminationCompositionStyle>AND</terminationCompositionStyle><secondsSpentLimit>1000</secondsSpentLimit>{short}</termination><termination>{long}</termination>"
        ),
        "identity" => format!(
            "<terminationCompositionStyle>AND</terminationCompositionStyle><termination>{short}</termination><termination>{long}</termination>"
        ),
        _ => short,
    };
    let phase = match mode {
        "override" => format!("<termination>{}</termination>", dim(1000)),
        "phase-first" => format!("<termination>{long}</termination>"),
        _ => String::new(),
    };
    format!(
        "<termination>{root}</termination><constructionHeuristic/><localSearch>{phase}</localSearch>"
    )
}
fn objective(v: i32, mode: &str) -> SimpleScore {
    SimpleScore::new(if mode.ends_with("unimproved") {
        if v == 0 { 0 } else { 700 }
    } else if v == 0 {
        0
    } else {
        700 + i64::from(v.min(4))
    })
}
fn ticks() -> (Arc<AtomicU64>, SolverClock) {
    let t = Arc::new(AtomicU64::new(0));
    let c = t.clone();
    (
        t,
        SolverClock::new(move || Duration::from_secs(c.load(Ordering::SeqCst))),
    )
}
fn check_phase<S>(
    mode: &str,
    termination: SolverTermination,
    phase: PhaseTermination,
    score: Option<S>,
) {
    assert!(score.is_some());
    if mode == "identity" {
        assert_eq!(termination, SolverTermination::Condition);
    } else if mode == "and" {
        assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit)
        );
    } else {
        assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
        );
    }
}
#[test]
fn inherited_rate_tree_drives_real_basic_dense_and_stream_solves() {
    let p = FiniteProblem::new(vec![Variable::new((0..64).collect()).with_initial(0)]).unwrap();
    let mut measured = std::collections::BTreeMap::new();
    for mode in MODES {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(&body(mode));
            let seq = SolverSequence {
                phases: vec![
                    match mode {
                        "custom-target" => SolverPhase::Custom(Default::default()),
                        "exact-target" => SolverPhase::Exact(Default::default()),
                        "ruin-target" => SolverPhase::RuinRecreate(Default::default()),
                        _ => SolverPhase::Construction(Default::default()),
                    },
                    SolverPhase::LocalSearch(local()),
                ],
                ..Default::default()
            };
            let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(a.values()[0].unwrap() + 1),
                }]
            };
            let r = if stream {
                xml.with_sequence_extensions(seq, &[], |seq, bindings| {
                    let mut command = |_, _: &mut CustomPhaseContext<'_, _, _, _, _>| {
                        panic!("a satisfied custom phase target must skip the command")
                    };
                    let mut ext = extensions!(&clock, &mut provider, bindings);
                    ext.command = Some(&mut command);
                    solve_stream_with_extensions(
                        &p,
                        || {
                            let mut b = StreamGraphBuilder::new();
                            let source = b.source::<usize, (usize, Option<i32>)>();
                            let c = t.clone();
                            b.score("progress", source.port(), move |(_, v)| {
                                c.fetch_add(11, Ordering::SeqCst);
                                objective(v.unwrap(), mode)
                            })?;
                            Ok((b.build(), source))
                        },
                        seq,
                        ext,
                    )
                })
                .unwrap()
                .unwrap()
            } else {
                let mut n = ConstraintNetwork::new(1);
                let c = t.clone();
                n.add_constraint("progress", vec![0], move |v| {
                    c.fetch_add(11, Ordering::SeqCst);
                    objective(*v[0].unwrap(), mode)
                })
                .unwrap();
                xml.with_sequence_extensions(seq, &[], |seq, bindings| {
                    let mut command = |_, _: &mut CustomPhaseContext<'_, _, _, _, _>| {
                        panic!("a satisfied custom phase target must skip the command")
                    };
                    let mut ext = extensions!(&clock, &mut provider, bindings);
                    ext.command = Some(&mut command);
                    solve_with_extensions(&p, &n, seq, ext)
                })
                .unwrap()
                .unwrap()
            };
            measured.insert((mode, stream), r.work.completed_steps);
            if mode == "identity" {
                assert_eq!(r.termination, SolverTermination::Condition);
                assert_eq!(r.work.move_evaluations, 0);
                assert_eq!(r.work.completed_steps, 0);
                assert_eq!(r.score, None);
                continue;
            }
            assert!(
                mode == "identity" || r.assignment.values()[0].unwrap() > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            assert_eq!(
                r.score,
                Some(objective(r.assignment.values()[0].unwrap(), mode))
            );

            if mode == "identity" {
                assert_eq!(r.work.move_evaluations, 0);
            } else {
                assert!(r.work.move_evaluations > 0, "{mode} {r:?}");
                if mode == "and" {
                    assert_eq!(r.work.completed_steps, 24);
                } else {
                    assert!(r.work.completed_steps < 24, "{mode} {r:?}");
                }
            }
            let phase = r.phases.last().unwrap();
            check_phase(mode, r.termination, phase.termination, r.score);
        }
    }
    for stream in [false, true] {
        assert_eq!(
            measured[&("single", stream)],
            measured[&("override", stream)]
        );
        assert_eq!(
            measured[&("single", stream)],
            measured[&("or-rates", stream)]
        );
        assert!(
            measured[&("nested", stream)] > measured[&("or-rates", stream)],
            "{measured:?}"
        );
        assert_eq!(
            measured[&("nested", stream)],
            measured[&("phase-first", stream)]
        );
    }
}
#[test]
fn inherited_rate_tree_drives_real_list_dense_and_stream_solves() {
    let p = ListProblem::new((0..64).collect(), vec![ListEntity::new((0..64).collect())]).unwrap();
    let mut measured = std::collections::BTreeMap::new();
    for mode in MODES {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(&body(mode));
            let seq = SolverSequence {
                phases: vec![
                    match mode {
                        "custom-target" => SolverPhase::Custom(Default::default()),
                        "exact-target" => SolverPhase::Exact(Default::default()),
                        "ruin-target" => SolverPhase::RuinRecreate(Default::default()),
                        _ => SolverPhase::Construction(Default::default()),
                    },
                    SolverPhase::LocalSearch(local()),
                ],
                ..Default::default()
            };
            let mut provider = |_: &ListProblem<i32>, _: &ListAssignment<i32>, step: u64| {
                vec![ListMove::Swap {
                    left: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                    right: ListPosition {
                        entity: 0,
                        index: step as usize + 1,
                    },
                }]
            };
            let r = if stream {
                xml.with_sequence_extensions(seq, &[], |seq, bindings| {
                    let mut command = |_, _: &mut CustomPhaseContext<'_, _, _, _, _>| {
                        panic!("a satisfied custom phase target must skip the command")
                    };
                    let mut ext = extensions!(&clock, &mut provider, bindings);
                    ext.command = Some(&mut command);
                    solve_list_stream_with_extensions(
                        &p,
                        || {
                            let mut b = StreamGraphBuilder::new();
                            let sources = ListStreamSources::<i32>::new(&mut b);
                            let c = t.clone();
                            b.score("progress", sources.entities.port(), move |(_, row)| {
                                c.fetch_add(11, Ordering::SeqCst);
                                objective(row[0], mode)
                            })?;
                            Ok((b.build(), sources))
                        },
                        seq,
                        ext,
                    )
                })
                .unwrap()
                .unwrap()
            } else {
                let mut n = ListConstraintNetwork::new(1, 64);
                let c = t.clone();
                n.add_constraint("progress", vec![ListScope::Entity(0)], move |facts| {
                    c.fetch_add(11, Ordering::SeqCst);
                    let ListFact::Entity(row) = facts[0] else {
                        panic!()
                    };
                    objective(row[0], mode)
                })
                .unwrap();
                xml.with_sequence_extensions(seq, &[], |seq, bindings| {
                    let mut command = |_, _: &mut CustomPhaseContext<'_, _, _, _, _>| {
                        panic!("a satisfied custom phase target must skip the command")
                    };
                    let mut ext = extensions!(&clock, &mut provider, bindings);
                    ext.command = Some(&mut command);
                    solve_list_with_extensions(&p, &n, seq, ext)
                })
                .unwrap()
                .unwrap()
            };
            measured.insert((mode, stream), r.work.completed_steps);
            if mode == "identity" {
                assert_eq!(r.termination, SolverTermination::Condition);
                assert_eq!(r.work.move_evaluations, 0);
                assert_eq!(r.work.completed_steps, 0);
                assert_eq!(r.score, None);
                continue;
            }
            assert!(
                mode == "identity" || r.assignment.lists()[0][0] > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            p.validate(&r.assignment).unwrap();
            assert_eq!(r.score, Some(objective(r.assignment.lists()[0][0], mode)));

            if mode == "identity" {
                assert_eq!(r.work.move_evaluations, 0);
            } else {
                assert!(r.work.move_evaluations > 0, "{mode} {r:?}");
                if mode == "and" {
                    assert_eq!(r.work.completed_steps, 24);
                } else {
                    assert!(r.work.completed_steps < 24, "{mode} {r:?}");
                }
            }
            check_phase(
                mode,
                r.termination,
                r.phases.last().unwrap().termination,
                r.score,
            );
        }
    }
    for stream in [false, true] {
        assert_eq!(
            measured[&("single", stream)],
            measured[&("override", stream)]
        );
        assert_eq!(
            measured[&("single", stream)],
            measured[&("or-rates", stream)]
        );
        assert!(
            measured[&("nested", stream)] > measured[&("or-rates", stream)],
            "{measured:?}"
        );
        assert_eq!(
            measured[&("nested", stream)],
            measured[&("phase-first", stream)]
        );
    }
}
#[test]
fn inherited_rate_tree_drives_real_mixed_dense_and_stream_solves() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..64).collect()).with_initial(0)]).unwrap(),
        ListProblem::new(vec![9, 8], vec![ListEntity::new(vec![9, 8])]).unwrap(),
    );
    let mut measured = std::collections::BTreeMap::new();
    for mode in MODES {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(&body(mode));
            let seq = MixedSolverSequence {
                phases: vec![
                    match mode {
                        "custom-target" => MixedSolverPhase::Custom(Default::default()),
                        "exact-target" => MixedSolverPhase::Exact(Default::default()),
                        "ruin-target" => MixedSolverPhase::RuinRecreate(Default::default()),
                        _ => MixedSolverPhase::Construction(Default::default()),
                    },
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search: local(),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            };
            let mut provider =
                |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| {
                    vec![MixedMove {
                        basic: Move::Change {
                            variable: 0,
                            value: Some(a.basic().values()[0].unwrap() + 1),
                        },
                        lists: ListMove::Composite(vec![]),
                    }]
                };
            let r = if stream {
                xml.with_mixed_sequence_extensions(seq, &[], |seq, bindings| {
                    let mut command = |_, _: &mut CustomPhaseContext<'_, _, _, _, _>| {
                        panic!("a satisfied custom phase target must skip the command")
                    };
                    let mut ext = extensions!(&clock, &mut provider, bindings);
                    ext.command = Some(&mut command);
                    solve_mixed_stream_with_extensions(
                        &p,
                        || {
                            let mut b = StreamGraphBuilder::new();
                            let sources = MixedStreamSources::<i32, i32>::new(&mut b);
                            let c = t.clone();
                            b.score("progress", sources.basic.port(), move |(_, v)| {
                                c.fetch_add(11, Ordering::SeqCst);
                                objective(v.unwrap(), mode)
                            })?;
                            Ok((b.build(), sources))
                        },
                        seq,
                        ext,
                    )
                })
                .unwrap()
                .unwrap()
            } else {
                let mut n = MixedConstraintNetwork::new(1, 1, 2);
                let c = t.clone();
                n.add_constraint("progress", vec![MixedScope::Basic(0)], move |facts| {
                    c.fetch_add(11, Ordering::SeqCst);
                    let MixedFact::Basic(v) = facts[0] else {
                        panic!()
                    };
                    objective(*v.unwrap(), mode)
                })
                .unwrap();
                xml.with_mixed_sequence_extensions(seq, &[], |seq, bindings| {
                    let mut command = |_, _: &mut CustomPhaseContext<'_, _, _, _, _>| {
                        panic!("a satisfied custom phase target must skip the command")
                    };
                    let mut ext = extensions!(&clock, &mut provider, bindings);
                    ext.command = Some(&mut command);
                    solve_mixed_with_extensions(&p, &n, seq, ext)
                })
                .unwrap()
                .unwrap()
            };
            measured.insert((mode, stream), r.work.completed_steps);
            if mode == "identity" {
                assert_eq!(r.termination, SolverTermination::Condition);
                assert_eq!(r.work.move_evaluations, 0);
                assert_eq!(r.work.completed_steps, 0);
                assert_eq!(r.score, None);
                continue;
            }
            assert!(
                mode == "identity" || r.assignment.basic().values()[0].unwrap() > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            p.validate(&r.assignment).unwrap();
            let mut retained_values = r.assignment.lists().lists()[0].clone();
            retained_values.sort();
            assert_eq!(retained_values, vec![8, 9]);
            assert_eq!(
                r.score,
                Some(objective(r.assignment.basic().values()[0].unwrap(), mode))
            );

            if mode == "identity" {
                assert_eq!(r.work.move_evaluations, 0);
            } else {
                assert!(r.work.move_evaluations > 0, "{mode} {r:?}");
                if mode == "and" {
                    assert_eq!(r.work.completed_steps, 24);
                } else {
                    assert!(r.work.completed_steps < 24, "{mode} {r:?}");
                }
            }
            check_phase(
                mode,
                r.termination,
                r.phases.last().unwrap().termination,
                r.score,
            );
        }
    }
    for stream in [false, true] {
        assert_eq!(
            measured[&("single", stream)],
            measured[&("override", stream)]
        );
        assert_eq!(
            measured[&("single", stream)],
            measured[&("or-rates", stream)]
        );
        assert!(
            measured[&("nested", stream)] > measured[&("or-rates", stream)],
            "{measured:?}"
        );
        assert_eq!(
            measured[&("nested", stream)],
            measured[&("phase-first", stream)]
        );
    }
}

#[test]
fn rate_grace_arms_only_at_first_actual_step_and_resets_for_a_new_phase() {
    let config = PhaseDiminishedReturnsConfig::new(
        DiminishedReturnsConfig::new(Duration::from_secs(10), 1, 2).unwrap(),
        &SimpleScore::ZERO,
    )
    .unwrap();
    let expression =
        StatefulSearchTermination::Condition(StatefulSearchLimit::DiminishedReturns(config));
    let mut session = StatefulTerminationSession::new(&expression);
    let progress = |seconds| SearchProgress {
        elapsed: Duration::from_secs(seconds),
        ..Default::default()
    };
    for origin in [0, 100_000] {
        session.reset();
        session.start_search(Duration::from_secs(origin));
        session.observe_best(Duration::from_secs(origin), &SimpleScore::ZERO);
        assert!(
            !session
                .status(progress(origin + 50_000), None)
                .unwrap()
                .reached
        );
        session.start_step(Duration::from_secs(origin + 50_000));
        assert!(
            !session
                .status(progress(origin + 50_009), None)
                .unwrap()
                .reached
        );
        session.start_step(Duration::from_secs(origin + 50_009));
        assert!(
            session
                .status(progress(origin + 50_010), None)
                .unwrap()
                .reached
        );
    }
}

#[test]
fn factory_preflight_accepts_scoped_and_rates_and_rejects_invalid_descendants() {
    let template: SolverSequence<'static> = SolverSequence {
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(local()),
        ],
        ..Default::default()
    };
    let factory: Result<XmlSolverFactory<(), (), ()>, _> =
        XmlSolverFactory::new(template.clone(), project(&body("nested")), |_, _, _, _| {
            Ok(())
        });
    assert!(factory.is_ok());
    let invalid = project(
        "<termination><terminationCompositionStyle>AND</terminationCompositionStyle><termination><bestScoreLimit>invalid</bestScoreLimit><diminishedReturns><minimumImprovementRatio>0.5</minimumImprovementRatio></diminishedReturns></termination></termination>",
    );
    let factory: Result<XmlSolverFactory<(), (), ()>, _> =
        XmlSolverFactory::new(template, invalid, |_, _, _, _| {
            panic!("invalid config must preflight")
        });
    assert!(factory.is_err());
}

#[test]
fn inherited_xml_rate_restarts_after_long_setup_on_each_real_dense_and_stream_phase() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    for stream in [false, true] {
        let xml = project(
            "<termination><diminishedReturns><slidingWindowMilliseconds>5</slidingWindowMilliseconds></diminishedReturns></termination>",
        );
        let tick = Arc::new(AtomicU64::new(0));
        let read = tick.clone();
        let clock = SolverClock::new(move || Duration::from_millis(read.load(Ordering::SeqCst)));
        let mut lifecycle = |event| match event {
            SolverLifecycleEvent::PhaseStarted { .. } => {
                tick.fetch_add(1_000_000, Ordering::SeqCst);
            }
            SolverLifecycleEvent::StepStarted { .. } => {
                tick.fetch_add(3, Ordering::SeqCst);
            }
            _ => {}
        };
        let sequence = SolverSequence {
            phases: vec![
                SolverPhase::LocalSearch(local()),
                SolverPhase::LocalSearch(local()),
            ],
            ..Default::default()
        };
        let result = xml
            .with_sequence_extensions(sequence, &[], |sequence, bindings| {
                if stream {
                    let extensions = SolverExtensions {
                        clock: Some(&clock),
                        lifecycle: Some(&mut lifecycle),
                        phase_terminations: bindings,
                        ..Default::default()
                    };

                    solve_stream_with_extensions(
                        &problem,
                        || {
                            let mut graph = StreamGraphBuilder::<SimpleScore>::new();
                            let source = graph.source::<usize, (usize, Option<i64>)>();
                            Ok((graph.build(), source))
                        },
                        sequence,
                        extensions,
                    )
                    .map_err(|error| error.to_string())
                } else {
                    let extensions = SolverExtensions {
                        clock: Some(&clock),
                        lifecycle: Some(&mut lifecycle),
                        phase_terminations: bindings,
                        ..Default::default()
                    };

                    solve_with_extensions(
                        &problem,
                        &ConstraintNetwork::<_, SimpleScore>::new(1),
                        sequence,
                        extensions,
                    )
                    .map_err(|error| error.to_string())
                }
            })
            .unwrap()
            .unwrap();
        assert_eq!(result.termination, SolverTermination::PlanCompleted);
        assert_eq!(result.phases.len(), 2);
        assert_eq!(result.score, Some(SimpleScore::ZERO));
        problem.validate(&result.assignment).unwrap();
        for phase in &result.phases {
            assert_eq!(phase.work.completed_steps, 1, "stream={stream} {result:?}");
            assert_eq!(phase.work.move_evaluations, 1);
            assert_eq!(
                phase.termination,
                PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
            );
        }
    }
}

#[test]
fn inherited_xml_rates_preserve_prior_bindings_caps_and_cancellation() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]).with_initial(0)]).unwrap();
    let network = ConstraintNetwork::<_, SimpleScore>::new(1);
    let search = SearchTermination::Condition(SearchLimit::Steps(2));
    let scored =
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(2)));
    let stateful = StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&scored));
    for prior in [
        PhaseTerminationBinding::search(0, &search),
        PhaseTerminationBinding::scored(0, &scored),
        PhaseTerminationBinding::stateful(0, &stateful),
    ] {
        let xml = project("<termination><diminishedReturns/></termination>");
        let result = xml
            .with_sequence_extensions(
                SolverSequence {
                    phases: vec![SolverPhase::LocalSearch(local())],
                    ..Default::default()
                },
                &[prior],
                |sequence, bindings| {
                    assert_eq!(bindings.len(), 1);
                    solve_with_extensions(
                        &problem,
                        &network,
                        sequence,
                        SolverExtensions {
                            phase_terminations: bindings,
                            ..Default::default()
                        },
                    )
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(result.work.completed_steps, 2);
        assert_eq!(result.score, Some(SimpleScore::ZERO));
        problem.validate(&result.assignment).unwrap();
    }
    for cancel in [false, true] {
        let token = CancellationToken::new();
        if cancel {
            token.cancel();
        }
        let xml = project(
            "<termination><terminationCompositionStyle>AND</terminationCompositionStyle><secondsSpentLimit>1000</secondsSpentLimit><diminishedReturns/></termination>",
        );
        let result = xml
            .with_sequence_extensions(
                SolverSequence {
                    phases: vec![SolverPhase::LocalSearch(local())],
                    limits: SolverLimits {
                        cancellation: Some(&token),
                        max_time: (!cancel).then_some(Duration::ZERO),
                        ..Default::default()
                    },
                },
                &[],
                |sequence, bindings| {
                    solve_with_extensions(
                        &problem,
                        &network,
                        sequence,
                        SolverExtensions {
                            phase_terminations: bindings,
                            ..Default::default()
                        },
                    )
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            result.termination,
            if cancel {
                SolverTermination::Cancelled
            } else {
                SolverTermination::TimeLimit
            }
        );
        assert_eq!(result.work.move_evaluations, 0);
        assert_eq!(result.assignment, problem.initial_assignment());
    }
    let old = SearchTermination::Condition(SearchLimit::ConstructionSteps(0));
    let binding = [PhaseTerminationBinding::search(0, &old)];
    let xml = project(
        "<termination><diminishedReturns/></termination><constructionHeuristic/><localSearch/>",
    );
    xml.with_sequence_extensions(
        SolverSequence::<SimpleScore> {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::LocalSearch(local()),
            ],
            ..Default::default()
        },
        &binding,
        |_, bindings| assert_eq!(bindings.len(), 2),
    )
    .unwrap();
    let conflicting = project(
        "<termination><diminishedReturns/></termination><localSearch><termination><stepCountLimit>1</stepCountLimit></termination></localSearch>",
    );
    assert!(
        conflicting
            .with_sequence_extensions(
                SolverSequence::<SimpleScore> {
                    phases: vec![SolverPhase::LocalSearch(local())],
                    ..Default::default()
                },
                &[PhaseTerminationBinding::search(0, &search)],
                |_, _| panic!("explicit XML overlap must reject")
            )
            .is_err()
    );
}
