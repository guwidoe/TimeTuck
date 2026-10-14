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
    ($clock:expr,$provider:expr) => {{
        let mut ext = SolverExtensions {
            clock: Some($clock),
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
#[test]
fn preserves_raw_absence_and_resolves_exact_binary_and_integer_defaults() {
    let raw = SolverXmlConfiguration::parse(
        "<solver><termination><diminishedReturns/></termination></solver>",
    )
    .unwrap();
    let p = raw.native_termination_plan().unwrap();
    let d = p.solver.unwrap().diminished_returns.unwrap();
    assert_eq!(d.minimum_improvement_ratio, None);
    assert_eq!(d.window.duration, None);
    assert_eq!(d.window.units, TimeLimitUnits::default());
    assert_eq!(d.resolve().unwrap().window(), Duration::from_secs(30));
    assert_eq!(d.resolve().unwrap().ratio(), (1.into(), 10000.into()));
    assert!(!raw.to_xml().contains("minimumImprovementRatio"));
    for value in ["0.0001", "0.1", "5e-324", "1.7976931348623157e308"] {
        let p = project(&format!(
            "<termination><diminishedReturns><minimumImprovementRatio>{value}</minimumImprovementRatio></diminishedReturns></termination>"
        ));
        let d = p.solver.unwrap().diminished_returns.unwrap();
        let number = value.parse::<f64>().unwrap();
        assert_eq!(d.minimum_improvement_ratio, Some(number));
        assert_eq!(
            d.resolve().unwrap(),
            DiminishedReturnsConfig::from_f64(Duration::from_secs(30), number).unwrap()
        );
    }
    let p = project(
        "<termination><millisecondsSpentLimit>5</millisecondsSpentLimit><secondsSpentLimit>4</secondsSpentLimit><minutesSpentLimit>3</minutesSpentLimit><hoursSpentLimit>2</hoursSpentLimit><daysSpentLimit>1</daysSpentLimit><unimprovedSpentLimit>PT2.5S</unimprovedSpentLimit><diminishedReturns><slidingWindowMilliseconds>1</slidingWindowMilliseconds><slidingWindowSeconds>2</slidingWindowSeconds><slidingWindowMinutes>3</slidingWindowMinutes><slidingWindowHours>4</slidingWindowHours><slidingWindowDays>5</slidingWindowDays></diminishedReturns></termination>",
    );
    let alias_only=SolverXmlConfiguration::parse("<solver><termination><bestScoreLimit>0</bestScoreLimit><secondsSpentLimit>2</secondsSpentLimit></termination></solver>").unwrap();
    assert_eq!(
        alias_only.native_spent_limit().unwrap(),
        Some(Duration::from_secs(2))
    );
    assert_eq!(
        alias_only
            .native_termination_plan()
            .unwrap()
            .solver
            .unwrap()
            .best_score_limit
            .as_deref(),
        Some("0")
    ); // getter ignores unrelated policies
    let s = p.solver.unwrap();
    assert_eq!(
        s.spent.resolve().unwrap(),
        Some(Duration::from_millis(93784005))
    );
    assert_eq!(
        s.unimproved.resolve().unwrap(),
        Some(Duration::from_millis(2500))
    );
    assert_eq!(
        s.diminished_returns.unwrap().resolve().unwrap().window(),
        Duration::from_millis(446582001)
    );
}
#[test]
fn invalid_declarations_and_inapplicable_phase_bindings_reject_before_run() {
    for inner in [
        "<spentLimit>-PT1S</spentLimit>",
        "<secondsSpentLimit>-1</secondsSpentLimit>",
        "<daysSpentLimit>18446744073709551615</daysSpentLimit>",
        "<spentLimit>PT1S</spentLimit><secondsSpentLimit>1</secondsSpentLimit>",
        "<stepCountLimit>1</stepCountLimit><stepCountLimit>2</stepCountLimit>",
        "<diminishedReturns><slidingWindowDuration>PT0S</slidingWindowDuration></diminishedReturns>",
        "<diminishedReturns><minimumImprovementRatio>NaN</minimumImprovementRatio></diminishedReturns>",
        "<diminishedReturns><minimumImprovementRatio>0</minimumImprovementRatio></diminishedReturns>",
        "<terminationCompositionStyle>AND</terminationCompositionStyle>",
    ] {
        let raw = SolverXmlConfiguration::parse(&format!(
            "<solver><termination>{inner}</termination></solver>"
        ))
        .unwrap();
        assert!(raw.native_termination_plan().is_err(), "{inner}");
    }
    assert!(SolverXmlConfiguration::parse("<solver><localSearch><minimumImprovementRatio>1</minimumImprovementRatio></localSearch></solver>").is_err());
    let p = project(
        "<constructionHeuristic><termination><unimprovedSecondsSpentLimit>1</unimprovedSecondsSpentLimit></termination></constructionHeuristic>",
    );
    let mut called = false;
    let seq = SolverSequence::<SimpleScore> {
        phases: vec![SolverPhase::Construction(Default::default())],
        ..Default::default()
    };
    assert!(p.with_sequence(seq, |_| { called = true }).is_err());
    assert!(!called);
    let p = project("<localSearch/><constructionHeuristic/>");
    assert!(
        p.with_sequence(SolverSequence::<SimpleScore>::default(), |_| {
            called = true
        })
        .is_err()
    );
    assert!(!called);
    let mut p = project("<termination><diminishedReturns/></termination>");
    p.solver
        .as_mut()
        .unwrap()
        .diminished_returns
        .as_mut()
        .unwrap()
        .minimum_improvement_ratio = Some(f64::INFINITY);
    assert!(
        p.with_sequence(
            SolverSequence::<SimpleScore> {
                phases: vec![SolverPhase::Construction(Default::default())],
                ..Default::default()
            },
            |_| { called = true }
        )
        .is_err()
    );
    assert!(!called);
}
#[test]
fn phase_precedence_and_unspecified_native_policies_survive_binding() {
    let explicit = DiminishedReturnsConfig::new(Duration::from_secs(9), 3, 4).unwrap();
    let seq = SolverSequence::<SimpleScore> {
        phases: vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_time: Some(Duration::from_secs(17)),
                diminished_returns: Some(explicit),
                ..Default::default()
            }),
            SolverPhase::LocalSearch(Default::default()),
        ],
        ..Default::default()
    };
    project("")
        .with_sequence(seq.clone(), |s| assert_eq!(s.phases, seq.phases))
        .unwrap();
    let p = project(
        "<termination><diminishedReturns><slidingWindowSeconds>60</slidingWindowSeconds></diminishedReturns></termination><localSearch/><localSearch><termination><diminishedReturns><slidingWindowSeconds>7</slidingWindowSeconds><minimumImprovementRatio>0.5</minimumImprovementRatio></diminishedReturns><unimprovedStepCountLimit>2</unimprovedStepCountLimit></termination></localSearch>",
    );
    p.with_sequence(seq, |s| {
        let SolverPhase::LocalSearch(first) = &s.phases[0] else {
            panic!()
        };
        assert_eq!(first.diminished_returns, Some(explicit));
        assert_eq!(first.max_time, Some(Duration::from_secs(17)));
        let SolverPhase::LocalSearch(second) = &s.phases[1] else {
            panic!()
        };
        assert_eq!(
            second.diminished_returns.unwrap().window(),
            Duration::from_secs(7)
        );
        assert_eq!(
            second.diminished_returns.unwrap().ratio(),
            (1.into(), 2.into())
        );
        assert_eq!(second.max_unimproved_steps, Some(2));
    })
    .unwrap();
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
        "target" => "<termination><bestScoreLimit>701</bestScoreLimit></termination>",
        "phase-target" => {
            "<constructionHeuristic/><localSearch><termination><bestScoreLimit>701</bestScoreLimit></termination></localSearch>"
        }
        "dim" => "<termination><diminishedReturns/></termination>",
        "spent" => "<termination><spentLimit>PT100S</spentLimit></termination>",
        "phase-spent" => {
            "<constructionHeuristic/><localSearch><termination><spentLimit>PT100S</spentLimit></termination></localSearch>"
        }
        "phase-unimproved" => {
            "<constructionHeuristic/><localSearch><termination><unimprovedSpentLimit>PT80S</unimprovedSpentLimit></termination></localSearch>"
        }
        _ => "<termination><unimprovedSpentLimit>PT80S</unimprovedSpentLimit></termination>",
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
    match mode {
        "target" => assert_eq!(termination, SolverTermination::BestScoreLimit),
        "phase-target" => assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::BestScoreLimit)
        ),
        "dim" => assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::DiminishedReturns)
        ),
        "spent" => assert_eq!(termination, SolverTermination::TimeLimit),
        "phase-spent" => assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::TimeLimit)
        ),
        "phase-unimproved" => assert_eq!(
            phase,
            PhaseTermination::LocalSearch(LocalSearchTermination::UnimprovedTimeLimit)
        ),
        _ => assert_eq!(termination, SolverTermination::UnimprovedTimeLimit),
    }
}
#[test]
fn xml_controls_drive_real_basic_dense_and_stream_solves() {
    let p = FiniteProblem::new(vec![Variable::new((0..64).collect()).with_initial(0)]).unwrap();
    for mode in [
        "target",
        "phase-target",
        "dim",
        "spent",
        "unimproved",
        "phase-spent",
        "phase-unimproved",
    ] {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(body(mode));
            let seq = SolverSequence {
                phases: vec![
                    SolverPhase::Construction(Default::default()),
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
                xml.with_sequence(seq, |seq| {
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
                        extensions!(&clock, &mut provider),
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
                xml.with_sequence(seq, |seq| {
                    solve_with_extensions(&p, &n, seq, extensions!(&clock, &mut provider))
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
            assert!(r.work.move_evaluations > 0);
            let phase = r.phases.last().unwrap();
            check_phase(mode, r.termination, phase.termination, r.score);
        }
    }
}
#[test]
fn xml_controls_drive_real_list_dense_and_stream_solves() {
    let p = ListProblem::new((0..64).collect(), vec![ListEntity::new((0..64).collect())]).unwrap();
    for mode in [
        "target",
        "phase-target",
        "dim",
        "spent",
        "unimproved",
        "phase-spent",
        "phase-unimproved",
    ] {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(body(mode));
            let seq = SolverSequence {
                phases: vec![
                    SolverPhase::Construction(Default::default()),
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
                xml.with_sequence(seq, |seq| {
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
                        extensions!(&clock, &mut provider),
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
                xml.with_sequence(seq, |seq| {
                    solve_list_with_extensions(&p, &n, seq, extensions!(&clock, &mut provider))
                })
                .unwrap()
                .unwrap()
            };
            assert!(
                r.assignment.lists()[0][0] > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            p.validate(&r.assignment).unwrap();
            assert_eq!(r.score, Some(objective(r.assignment.lists()[0][0], mode)));
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
fn xml_controls_drive_real_mixed_dense_and_stream_solves() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..64).collect()).with_initial(0)]).unwrap(),
        ListProblem::new(vec![9, 8], vec![ListEntity::new(vec![9, 8])]).unwrap(),
    );
    for mode in [
        "target",
        "phase-target",
        "dim",
        "spent",
        "unimproved",
        "phase-spent",
        "phase-unimproved",
    ] {
        for stream in [false, true] {
            let (t, clock) = ticks();
            let xml = project(body(mode));
            let seq = MixedSolverSequence {
                phases: vec![
                    MixedSolverPhase::Construction(Default::default()),
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
                xml.with_mixed_sequence(seq, |seq| {
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
                        extensions!(&clock, &mut provider),
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
                xml.with_mixed_sequence(seq, |seq| {
                    solve_mixed_with_extensions(&p, &n, seq, extensions!(&clock, &mut provider))
                })
                .unwrap()
                .unwrap()
            };
            assert!(
                r.assignment.basic().values()[0].unwrap() > 0,
                "mode={mode} stream={stream} result={r:?}"
            );
            assert_eq!(r.assignment.lists().lists(), &[vec![9, 8]]);
            assert_eq!(
                r.score,
                Some(objective(r.assignment.basic().values()[0].unwrap(), mode))
            );
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
fn solver_deadline_survives_handoffs_while_phase_deadlines_restart() {
    let p = FiniteProblem::new(vec![Variable::new((0..64).collect()).with_initial(0)]).unwrap();
    for stream in [false, true] {
        let mut counts = vec![];
        for global in [true, false] {
            let root = if global {
                "<termination><spentLimit>PT100S</spentLimit></termination>"
            } else {
                ""
            };
            let phase = if global {
                ""
            } else {
                "<termination><spentLimit>PT100S</spentLimit></termination>"
            };
            let raw = format!(
                "{root}<constructionHeuristic/><localSearch><termination><stepCountLimit>1</stepCountLimit></termination></localSearch><localSearch>{phase}</localSearch>"
            );
            let xml = project(&raw);
            let (t, clock) = ticks();
            let seq = SolverSequence {
                phases: vec![
                    SolverPhase::Construction(Default::default()),
                    SolverPhase::LocalSearch(local()),
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
            // Providers cannot be mutably registered twice; independent closures
            // preserve the two real phase roles without retained cursor state.
            let mut first = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(a.values()[0].unwrap() + 1),
                }]
            };
            macro_rules! ext {
                () => {{
                    let mut e = SolverExtensions {
                        clock: Some(&clock),
                        ..Default::default()
                    };
                    e.move_providers.bind(1, &mut first);
                    e.move_providers.bind(2, &mut provider);
                    e
                }};
            }
            let result = if stream {
                xml.with_sequence(seq, |seq| {
                    solve_stream_with_extensions(
                        &p,
                        || {
                            let mut b = StreamGraphBuilder::new();
                            let source = b.source::<usize, (usize, Option<i32>)>();
                            let c = t.clone();
                            b.score("steps", source.port(), move |(_, v)| {
                                c.fetch_add(11, Ordering::SeqCst);
                                SimpleScore::new(i64::from(v.unwrap()))
                            })?;
                            Ok((b.build(), source))
                        },
                        seq,
                        ext!(),
                    )
                })
                .unwrap()
                .unwrap()
            } else {
                let mut n = ConstraintNetwork::new(1);
                let c = t.clone();
                n.add_constraint("steps", vec![0], move |v| {
                    c.fetch_add(11, Ordering::SeqCst);
                    SimpleScore::new(i64::from(*v[0].unwrap()))
                })
                .unwrap();
                xml.with_sequence(seq, |seq| solve_with_extensions(&p, &n, seq, ext!()))
                    .unwrap()
                    .unwrap()
            };
            assert_eq!(
                result.score,
                Some(SimpleScore::new(i64::from(
                    result.assignment.values()[0].unwrap()
                )))
            );
            assert_eq!(result.phases[1].work.completed_steps, 1);
            assert_eq!(
                result.termination,
                if global {
                    SolverTermination::TimeLimit
                } else {
                    SolverTermination::PlanCompleted
                }
            );
            counts.push(result.work.completed_steps);
        }
        assert!(counts[1] > counts[0], "phase={counts:?} stream={stream}");
    }
}

#[test]
fn typed_score_limits_reject_malformed_before_callbacks_and_preserve_absence() {
    for body in [
        "<termination><bestScoreLimit>bogus</bestScoreLimit></termination>",
        "<constructionHeuristic/><localSearch><termination><bestScoreLimit>1hard/2soft</bestScoreLimit></termination></localSearch>",
    ] {
        let plan = project(body);
        let mut called = false;
        let result = plan.with_sequence(
            SolverSequence::<SimpleScore> {
                phases: vec![
                    SolverPhase::Construction(Default::default()),
                    SolverPhase::LocalSearch(local()),
                ],
                ..Default::default()
            },
            |_| {
                called = true;
            },
        );
        assert!(result.unwrap_err().to_string().contains("bestScoreLimit"));
        assert!(!called);
    }
    project("")
        .with_sequence(
            SolverSequence::<SimpleScore> {
                limits: SolverLimits {
                    best_score_limit: Some(SimpleScore::new(3)),
                    ..Default::default()
                },
                ..Default::default()
            },
            |s| assert_eq!(s.limits.best_score_limit, Some(SimpleScore::new(3))),
        )
        .unwrap();
}
