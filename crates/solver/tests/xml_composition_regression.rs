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
fn body(mode: &str) -> &'static str {
    match mode {
        "root-or" => {
            "<termination><terminationCompositionStyle>OR</terminationCompositionStyle><spentLimit>PT100S</spentLimit><stepCountLimit>3</stepCountLimit><bestScoreLimit>701</bestScoreLimit></termination>"
        }
        "root-and" => {
            "<termination><terminationCompositionStyle>AND</terminationCompositionStyle><spentLimit>PT100S</spentLimit><stepCountLimit>3</stepCountLimit><bestScoreLimit>701</bestScoreLimit></termination>"
        }
        "phase-and" => {
            "<constructionHeuristic/><localSearch><termination><terminationCompositionStyle>AND</terminationCompositionStyle><spentLimit>PT100S</spentLimit><stepCountLimit>3</stepCountLimit><bestScoreLimit>701</bestScoreLimit></termination></localSearch>"
        }
        "custom-target" => {
            "<customPhase><termination><bestScoreLimit>0</bestScoreLimit></termination></customPhase><localSearch/>"
        }
        "exact-target" => {
            "<exhaustiveSearch><termination><bestScoreLimit>0</bestScoreLimit></termination></exhaustiveSearch><localSearch/>"
        }
        "ruin-target" => {
            "<ruinRecreate><termination><bestScoreLimit>0</bestScoreLimit></termination></ruinRecreate><localSearch/>"
        }
        "construction-target" => {
            "<constructionHeuristic><termination><bestScoreLimit>0</bestScoreLimit></termination></constructionHeuristic><localSearch/>"
        }
        "phase-or" => {
            "<constructionHeuristic/><localSearch><termination><terminationCompositionStyle>OR</terminationCompositionStyle><stepCountLimit>3</stepCountLimit><bestScoreLimit>701</bestScoreLimit></termination></localSearch>"
        }
        _ => {
            "<constructionHeuristic/><localSearch><termination><terminationCompositionStyle>AND</terminationCompositionStyle><stepCountLimit>3</stepCountLimit><diminishedReturns/></termination></localSearch>"
        }
    }
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
    if mode.ends_with("-target") {
        assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit)
        );
    } else if mode.starts_with("root-") {
        assert_eq!(termination, SolverTermination::Condition);
    } else {
        assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
        );
    }
}
#[test]
fn xml_composition_drives_real_basic_dense_and_stream_solves() {
    let p = FiniteProblem::new(vec![Variable::new((0..64).collect()).with_initial(0)]).unwrap();
    for mode in [
        "root-and",
        "root-or",
        "phase-and",
        "phase-or",
        "phase-dim-and",
        "construction-target",
        "exact-target",
        "ruin-target",
        "custom-target",
    ] {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(body(mode));
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
            assert!(
                r.assignment.values()[0].unwrap() > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            assert_eq!(
                r.score,
                Some(objective(r.assignment.values()[0].unwrap(), mode))
            );
            if mode == "construction-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Construction(ConstructionTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            if mode == "exact-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Exact(ExactPhaseTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 1);
            }
            if mode == "ruin-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::RuinRecreate(RuinRecreateTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            if mode == "custom-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Custom(CustomPhaseTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            assert!(r.work.move_evaluations > 0);
            if mode.ends_with("-or") {
                assert_eq!(r.assignment.values()[0], Some(1));
            } else {
                assert!(r.assignment.values()[0].unwrap() >= 3);
            }
            let phase = r.phases.last().unwrap();
            check_phase(mode, r.termination, phase.termination, r.score);
        }
    }
}
#[test]
fn xml_composition_drives_real_list_dense_and_stream_solves() {
    let p = ListProblem::new((0..64).collect(), vec![ListEntity::new((0..64).collect())]).unwrap();
    for mode in [
        "root-and",
        "root-or",
        "phase-and",
        "phase-or",
        "phase-dim-and",
        "construction-target",
        "exact-target",
        "ruin-target",
        "custom-target",
    ] {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(body(mode));
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
            assert!(
                r.assignment.lists()[0][0] > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            if mode.ends_with("-or") {
                assert_eq!(r.assignment.lists()[0][0], 1);
            } else {
                assert!(r.assignment.lists()[0][0] >= 3);
            }
            p.validate(&r.assignment).unwrap();
            assert_eq!(r.score, Some(objective(r.assignment.lists()[0][0], mode)));
            if mode == "construction-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Construction(ConstructionTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            if mode == "exact-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Exact(ExactPhaseTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 1);
            }
            if mode == "ruin-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::RuinRecreate(RuinRecreateTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            if mode == "custom-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Custom(CustomPhaseTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            assert!(r.work.move_evaluations > 0);
            check_phase(
                mode,
                r.termination,
                r.phases.last().unwrap().termination,
                r.score,
            );
        }
    }
}
#[test]
fn xml_composition_drives_real_mixed_dense_and_stream_solves() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..64).collect()).with_initial(0)]).unwrap(),
        ListProblem::new(vec![9, 8], vec![ListEntity::new(vec![9, 8])]).unwrap(),
    );
    for mode in [
        "root-and",
        "root-or",
        "phase-and",
        "phase-or",
        "phase-dim-and",
        "construction-target",
        "exact-target",
        "ruin-target",
        "custom-target",
    ] {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(body(mode));
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
            assert!(
                r.assignment.basic().values()[0].unwrap() > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            if mode.ends_with("-or") {
                assert_eq!(r.assignment.basic().values()[0], Some(1));
            } else {
                assert!(r.assignment.basic().values()[0].unwrap() >= 3);
            }
            p.validate(&r.assignment).unwrap();
            let mut retained_values = r.assignment.lists().lists()[0].clone();
            retained_values.sort();
            assert_eq!(retained_values, vec![8, 9]);
            assert_eq!(
                r.score,
                Some(objective(r.assignment.basic().values()[0].unwrap(), mode))
            );
            if mode == "construction-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Construction(ConstructionTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            if mode == "exact-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Exact(ExactPhaseTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 1);
            }
            if mode == "ruin-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::RuinRecreate(RuinRecreateTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            if mode == "custom-target" {
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Custom(CustomPhaseTermination::Condition)
                );
                assert_eq!(r.phases[0].work.move_evaluations, 0);
            }
            assert!(r.work.move_evaluations > 0);
            check_phase(
                mode,
                r.termination,
                r.phases.last().unwrap().termination,
                r.score,
            );
        }
    }
}

#[test]
fn invalid_composition_score_and_binding_conflicts_reject_before_run() {
    for style in ["and", "X", ""] {
        let xml=SolverXmlConfiguration::parse(&format!("<solver><termination><terminationCompositionStyle>{style}</terminationCompositionStyle></termination></solver>")).unwrap();
        assert!(xml.native_termination_plan().is_err());
    }
    for body in [
        "<constructionHeuristic><termination><bestScoreLimit>wrong</bestScoreLimit></termination></constructionHeuristic>",
        "<constructionHeuristic><termination><diminishedReturns/></termination></constructionHeuristic>",
        "<constructionHeuristic><termination><unimprovedStepCountLimit>2</unimprovedStepCountLimit></termination></constructionHeuristic>",
    ] {
        let mut called = false;
        assert!(
            project(body)
                .with_sequence_extensions(
                    SolverSequence::<SimpleScore> {
                        phases: vec![SolverPhase::Construction(Default::default())],
                        ..Default::default()
                    },
                    &[],
                    |_, _| {
                        called = true;
                    }
                )
                .is_err()
        );
        assert!(!called);
    }
    let condition = SearchTermination::Condition(SearchLimit::Time(Duration::from_secs(9)));
    let binding = PhaseTerminationBinding::<SimpleScore>::search(0, &condition);
    let mut called = false;
    assert!(project("<constructionHeuristic><termination><bestScoreLimit>2</bestScoreLimit></termination></constructionHeuristic>").with_sequence_extensions(SolverSequence { phases:vec![SolverPhase::Construction(Default::default())], ..Default::default() }, &[binding], |_,_| {called=true;}).is_err());
    assert!(!called);
}

#[test]
fn construction_target_consumes_real_retained_placement_before_optional_scan() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]),
        Variable::new(vec![5]).allowing_unassigned(),
    ])
    .unwrap();
    for stream in [false, true] {
        let plan = project(
            "<constructionHeuristic><termination><bestScoreLimit>2</bestScoreLimit></termination></constructionHeuristic>",
        );
        let seq = SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig {
                optional: OptionalConstruction::NonWorsening,
                ..Default::default()
            })],
            ..Default::default()
        };
        let r = if stream {
            plan.with_sequence_extensions(seq, &[], |seq, bindings| {
                solve_stream_with_extensions(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<i32>)>();
                        b.score("sum", source.port(), |(_, v)| {
                            SimpleScore::new(i64::from(v.unwrap_or(0)))
                        })?;
                        Ok((b.build(), source))
                    },
                    seq,
                    SolverExtensions {
                        phase_terminations: bindings,
                        ..Default::default()
                    },
                )
            })
            .unwrap()
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(2);
            n.add_constraint("sum", vec![0, 1], |v| {
                SimpleScore::new(v.iter().map(|v| i64::from(v.copied().unwrap_or(0))).sum())
            })
            .unwrap();
            plan.with_sequence_extensions(seq, &[], |seq, bindings| {
                solve_with_extensions(
                    &p,
                    &n,
                    seq,
                    SolverExtensions {
                        phase_terminations: bindings,
                        ..Default::default()
                    },
                )
            })
            .unwrap()
            .unwrap()
        };
        assert_eq!(r.assignment.values(), &[Some(2), None]);
        assert_eq!(r.score, Some(SimpleScore::new(2)));
        assert_eq!(r.work.construction_steps, 1);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Construction(ConstructionTermination::Condition)
        );
    }
}

#[test]
fn caller_deadline_and_cancellation_remain_independent_of_xml_and() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    for cancel in [false, true] {
        let token = CancellationToken::new();
        if cancel {
            token.cancel();
        }
        let plan = project(body("root-and"));
        let seq = SolverSequence {
            phases: vec![SolverPhase::LocalSearch(local())],
            limits: SolverLimits {
                cancellation: Some(&token),
                max_time: (!cancel).then_some(Duration::ZERO),
                ..Default::default()
            },
        };
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
        let r = plan
            .with_sequence_extensions(seq, &[], |seq, bindings| {
                solve_with_extensions(
                    &p,
                    &n,
                    seq,
                    SolverExtensions {
                        phase_terminations: bindings,
                        ..Default::default()
                    },
                )
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            r.termination,
            if cancel {
                SolverTermination::Cancelled
            } else {
                SolverTermination::TimeLimit
            }
        );
        assert_eq!(r.assignment.values(), &[Some(0)]);
        assert_eq!(
            r.score,
            if cancel {
                None
            } else {
                Some(SimpleScore::ZERO)
            },
            "cancel={cancel}"
        );
        assert_eq!(r.work.evaluations, u64::from(!cancel));
        assert_eq!(r.work.move_evaluations, 0);
    }
}

#[test]
fn existing_root_and_nonoverlapping_phase_expressions_are_retained() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let existing =
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::ZERO));
    let plan = project(body("root-and"));
    let r = plan
        .with_sequence_extensions(
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(local())],
                limits: SolverLimits {
                    scored_termination: Some(&existing),
                    ..Default::default()
                },
            },
            &[],
            |seq, bindings| {
                solve_with_extensions(
                    &p,
                    &n,
                    seq,
                    SolverExtensions {
                        phase_terminations: bindings,
                        ..Default::default()
                    },
                )
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.work.move_evaluations, 0);
    let old = SearchTermination::Condition(SearchLimit::ConstructionSteps(0));
    let bindings = [PhaseTerminationBinding::search(0, &old)];
    let plan = project(body("phase-and"));
    plan.with_sequence_extensions(
        SolverSequence::<SimpleScore> {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::LocalSearch(local()),
            ],
            ..Default::default()
        },
        &bindings,
        |seq, bindings| {
            assert_eq!(bindings.len(), 2);
            assert_eq!(bindings[0].phase(), 0);
            assert_eq!(bindings[1].phase(), 1);
            assert_eq!(seq.phases.len(), 2);
        },
    )
    .unwrap();
    let empty = project(
        "<termination><terminationCompositionStyle>AND</terminationCompositionStyle></termination>",
    );
    empty
        .with_sequence_extensions(
            SolverSequence::<SimpleScore>::default(),
            &[],
            |seq, bindings| {
                assert!(seq.limits.scored_termination.is_none());
                assert!(bindings.is_empty());
            },
        )
        .unwrap();
}
