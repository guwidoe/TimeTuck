//! Original XML weighted goal integration controls; no source-method credit.
use timetuck::*;

fn plan(body: &str) -> XmlTerminationPlan {
    SolverXmlConfiguration::parse(&format!("<solver>{body}</solver>"))
        .unwrap()
        .native_termination_plan()
        .unwrap()
}
fn objective(value: i32) -> HardSoftScore {
    HardSoftScore::new(i64::from((value - 3).min(0)), i64::from(value))
}
fn local(phase: bool) -> LocalSearchConfig<'static, HardSoftScore> {
    LocalSearchConfig {
        max_steps: 8,
        acceptance: Acceptance::SimulatedAnnealingTermination {
            starting_temperature: HardSoftScore::new(20, 20),
            scope: if phase {
                TerminationProgressScope::Phase
            } else {
                TerminationProgressScope::Solve
            },
            seed: 11,
        },
        selection: MoveSelection::FirstAccepted,
        ..Default::default()
    }
}
#[derive(Debug)]
struct ResultRow {
    value: i32,
    score: Option<HardSoftScore>,
    complete: bool,
    work: SolverWork,
}
macro_rules! extensions {
    ($provider:expr, $bindings:expr, $solver:expr) => {{
        let mut ext = SolverExtensions {
            phase_terminations: $bindings,
            stateful_termination: $solver,
            ..Default::default()
        };
        ext.move_providers.bind(0, $provider);
        ext
    }};
}
fn run(
    model: usize,
    stream: bool,
    xml: &XmlTerminationPlan,
    cap: Option<u64>,
    phase: bool,
) -> ResultRow {
    let basic = FiniteProblem::new(vec![Variable::new((0..16).collect()).with_initial(0)]).unwrap();
    let lists =
        ListProblem::new((0..16).collect(), vec![ListEntity::new((0..16).collect())]).unwrap();
    let limits = SolverLimits {
        max_move_evaluations: cap,
        ..Default::default()
    };
    let seq = SolverSequence {
        limits,
        phases: vec![SolverPhase::LocalSearch(local(phase))],
    };
    match model {
        0 => {
            let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(a.values()[0].unwrap() + 1),
                }]
            };
            let r = xml
                .with_sequence_controls(seq, &[], None, |seq, bindings, solver| {
                    if stream {
                        solve_stream_with_extensions(
                            &basic,
                            || {
                                let mut b = StreamGraphBuilder::new();
                                let source = b.source::<usize, (usize, Option<i32>)>();
                                b.score("objective", source.port(), |(_, v)| {
                                    objective(v.unwrap())
                                })?;
                                Ok((b.build(), source))
                            },
                            seq,
                            extensions!(&mut provider, bindings, solver),
                        )
                    } else {
                        let mut n = ConstraintNetwork::new(1);
                        n.add_constraint("objective", vec![0], |v| objective(*v[0].unwrap()))
                            .unwrap();
                        solve_with_extensions(
                            &basic,
                            &n,
                            seq,
                            extensions!(&mut provider, bindings, solver),
                        )
                    }
                })
                .unwrap()
                .unwrap();
            basic.validate(&r.assignment).unwrap();
            ResultRow {
                value: r.assignment.values()[0].unwrap(),
                score: r.score,
                complete: r.complete,
                work: r.work,
            }
        }
        1 => {
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
            let r = xml
                .with_sequence_controls(seq, &[], None, |seq, bindings, solver| {
                    if stream {
                        solve_list_stream_with_extensions(
                            &lists,
                            || {
                                let mut b = StreamGraphBuilder::new();
                                let sources = ListStreamSources::<i32>::new(&mut b);
                                b.score("objective", sources.entities.port(), |(_, row)| {
                                    objective(row[0])
                                })?;
                                Ok((b.build(), sources))
                            },
                            seq,
                            extensions!(&mut provider, bindings, solver),
                        )
                    } else {
                        let mut n = ListConstraintNetwork::new(1, 16);
                        n.add_constraint("objective", vec![ListScope::Entity(0)], |facts| {
                            let ListFact::Entity(row) = facts[0] else {
                                panic!()
                            };
                            objective(row[0])
                        })
                        .unwrap();
                        solve_list_with_extensions(
                            &lists,
                            &n,
                            seq,
                            extensions!(&mut provider, bindings, solver),
                        )
                    }
                })
                .unwrap()
                .unwrap();
            lists.validate(&r.assignment).unwrap();
            let mut values = r.assignment.lists()[0].clone();
            values.sort();
            assert_eq!(values, (0..16).collect::<Vec<_>>());
            ResultRow {
                value: r.assignment.lists()[0][0],
                score: r.score,
                complete: r.complete,
                work: r.work,
            }
        }
        _ => {
            let p = MixedProblem::new(basic, lists);
            let seq = MixedSolverSequence {
                limits,
                phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search: local(phase),
                    ..Default::default()
                })],
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
            let r = xml
                .with_mixed_sequence_controls(seq, &[], None, |seq, bindings, solver| {
                    if stream {
                        solve_mixed_stream_with_extensions(
                            &p,
                            || {
                                let mut b = StreamGraphBuilder::new();
                                let sources = MixedStreamSources::<i32, i32>::new(&mut b);
                                b.score("objective", sources.basic.port(), |(_, v)| {
                                    objective(v.unwrap())
                                })?;
                                Ok((b.build(), sources))
                            },
                            seq,
                            extensions!(&mut provider, bindings, solver),
                        )
                    } else {
                        let mut n = MixedConstraintNetwork::new(1, 1, 16);
                        n.add_constraint("objective", vec![MixedScope::Basic(0)], |facts| {
                            let MixedFact::Basic(v) = facts[0] else {
                                panic!()
                            };
                            objective(*v.unwrap())
                        })
                        .unwrap();
                        solve_mixed_with_extensions(
                            &p,
                            &n,
                            seq,
                            extensions!(&mut provider, bindings, solver),
                        )
                    }
                })
                .unwrap()
                .unwrap();
            p.validate(&r.assignment).unwrap();
            assert_eq!(r.assignment.lists().lists()[0], (0..16).collect::<Vec<_>>());
            ResultRow {
                value: r.assignment.basic().values()[0].unwrap(),
                score: r.score,
                complete: r.complete,
                work: r.work,
            }
        }
    }
}

#[test]
fn weighted_goals_execute_basic_list_mixed_with_both_backends_and_independent_caps() {
    for phase in [false, true] {
        for model in 0..3 {
            for stream in [false, true] {
                for (condition, value) in [
                    ("<bestScoreLimit>0hard/5soft</bestScoreLimit>", 5),
                    ("<bestScoreFeasible>true</bestScoreFeasible>", 3),
                    (
                        "<terminationCompositionStyle>AND</terminationCompositionStyle><bestScoreFeasible>true</bestScoreFeasible><termination><bestScoreLimit>0hard/5soft</bestScoreLimit></termination>",
                        5,
                    ),
                    (
                        "<terminationCompositionStyle>OR</terminationCompositionStyle><bestScoreFeasible>true</bestScoreFeasible><termination><bestScoreLimit>0hard/5soft</bestScoreLimit></termination>",
                        3,
                    ),
                ] {
                    let body = format!("<termination>{condition}</termination>");
                    let xml = plan(&if phase {
                        format!("<localSearch>{body}</localSearch>")
                    } else {
                        body
                    });
                    for cap in [None, Some(2)] {
                        let r = run(model, stream, &xml, cap, phase);
                        let expected = if cap.is_some() { 2 } else { value };
                        assert_eq!(
                            r.value, expected,
                            "model{model}/stream{stream}/phase{phase}/{condition}/cap{cap:?}"
                        );
                        assert_eq!(r.score, Some(objective(expected)));
                        assert!(r.complete);
                        assert_eq!(r.work.completed_steps, expected as u64);
                        assert_eq!(r.work.move_evaluations, expected as u64);
                    }
                }
            }
        }
    }
}
fn inspect_goal<S: AdditiveScore + std::str::FromStr + std::fmt::Debug>(
    condition: &str,
    start: S,
    rows: Vec<(S, f64, bool)>,
) {
    let xml = plan(&format!("<termination>{condition}</termination>"));
    xml.with_sequence_controls(
        SolverSequence::<S> {
            phases: vec![SolverPhase::LocalSearch(Default::default())],
            ..Default::default()
        },
        &[],
        None,
        |_, bindings, solver| {
            assert!(bindings.is_empty());
            let expression = solver.unwrap();
            let mut session = StatefulTerminationSession::new(expression);
            let before = session
                .status(SearchProgress::default(), Some(&start))
                .unwrap();
            assert!(!before.reached);
            assert_eq!(before.progress, Some(0.0)); // XML OR group before goal initialization.
            session.observe_initialized_score(&start);
            for (score, progress, reached) in &rows {
                let actual = session
                    .status(SearchProgress::default(), Some(score))
                    .unwrap();
                assert_eq!(
                    actual,
                    TerminationState {
                        progress: Some(*progress),
                        reached: *reached
                    },
                    "{condition}/{score:?}"
                );
            }
        },
    )
    .unwrap();
}

#[test]
fn xml_default_weighted_goals_capture_initialized_multilevel_decimal_baselines() {
    inspect_goal(
        "<bestScoreLimit>10</bestScoreLimit>",
        SimpleScore::new(0),
        vec![
            (SimpleScore::new(0), 0., false),
            (SimpleScore::new(6), 0.6, false),
            (SimpleScore::new(10), 1., true),
        ],
    );
    inspect_goal(
        "<bestScoreLimit>0hard/0soft</bestScoreLimit>",
        HardSoftScore::new(-10, -100),
        vec![
            (HardSoftScore::new(-5, -100), 0.25, false),
            (HardSoftScore::new(0, -100), 0.5, false),
            (HardSoftScore::new(0, -50), 0.75, false),
            (HardSoftScore::ZERO, 1., true),
        ],
    );
    inspect_goal(
        "<bestScoreLimit>0hard/0medium/0soft</bestScoreLimit>",
        HardMediumSoftScore::new(-10, -10, -10),
        vec![
            (HardMediumSoftScore::new(0, -10, -10), 0.5, false),
            (HardMediumSoftScore::new(0, 0, -10), 0.75, false),
            (HardMediumSoftScore::ZERO, 1., true),
        ],
    );
    inspect_goal(
        "<bestScoreLimit>[0/0]hard/[0/0/0]soft</bestScoreLimit>",
        BendableScore::<2, 3>::new([-10, -10], [-10, -10, -10]),
        vec![
            (BendableScore::new([0, -10], [-10, -10, -10]), 0.5, false),
            (BendableScore::new([0, 0], [-10, -10, -10]), 0.75, false),
            (BendableScore::new([0, 0], [0, -10, -10]), 0.875, false),
            (BendableScore::new([0, 0], [0, 0, -10]), 0.9375, false),
            (BendableScore::ZERO, 1., true),
        ],
    );
    inspect_goal(
        "<bestScoreFeasible>true</bestScoreFeasible>",
        HardSoftScore::new(-10, -100),
        vec![
            (HardSoftScore::new(-5, 999), 0.5, false),
            (HardSoftScore::new(0, -10000), 1., true),
        ],
    );
    inspect_goal(
        "<bestScoreFeasible>true</bestScoreFeasible>",
        BendableScore::<2, 1>::new([-10, -10], [-10]),
        vec![
            (BendableScore::new([0, -10], [100]), 0.5, false),
            (BendableScore::new([0, 0], [-10000]), 1., true),
        ],
    );
    let decimal = |s: &str| s.parse::<HardSoftDecimalScore>().unwrap();
    inspect_goal(
        "<bestScoreLimit>0hard/0soft</bestScoreLimit>",
        decimal("-10hard/-100soft"),
        vec![
            (decimal("0hard/-100soft"), 0.5, false),
            (decimal("0hard/-50soft"), 0.75, false),
            (decimal("0hard/0soft"), 1., true),
        ],
    );
}
#[test]
fn full_xml_goals_preflight_recursively_and_false_feasibility_stays_empty() {
    for body in [
        "<termination><bestScoreFeasible>true</bestScoreFeasible></termination>",
        "<termination><termination><bestScoreFeasible>true</bestScoreFeasible></termination></termination>",
        "<localSearch><termination><bestScoreFeasible>true</bestScoreFeasible></termination></localSearch>",
        "<termination><bestScoreLimit>bad</bestScoreLimit></termination>",
    ] {
        let xml = plan(body);
        let mut called = false;
        let r = xml.with_sequence_controls(
            SolverSequence::<SimpleScore> {
                phases: vec![SolverPhase::LocalSearch(Default::default())],
                ..Default::default()
            },
            &[],
            None,
            |_, _, _| called = true,
        );
        assert!(r.is_err());
        assert!(!called);
    }
    let xml = plan(
        "<termination><bestScoreFeasible>false</bestScoreFeasible></termination><localSearch><termination><bestScoreFeasible>false</bestScoreFeasible></termination></localSearch>",
    );
    xml.with_sequence_controls(
        SolverSequence::<SimpleScore> {
            phases: vec![SolverPhase::LocalSearch(Default::default())],
            ..Default::default()
        },
        &[],
        None,
        |_, bindings, solver| {
            assert!(bindings.is_empty());
            assert!(solver.is_none());
        },
    )
    .unwrap();
    // The explicit legacy profile retains static stopping rather than weighted goals.
    let xml = plan("<termination><bestScoreLimit>10</bestScoreLimit></termination>");
    xml.with_sequence_extensions(
        SolverSequence::<SimpleScore> {
            phases: vec![SolverPhase::LocalSearch(Default::default())],
            ..Default::default()
        },
        &[],
        |seq, _| {
            assert!(seq.limits.scored_termination.is_some());
        },
    )
    .unwrap();
}
#[test]
fn nested_xml_goal_progress_keeps_and_or_groups_and_prior_histories_independent() {
    for and in [false, true] {
        let style = if and { "AND" } else { "OR" };
        let xml = plan(&format!(
            "<termination><terminationCompositionStyle>{style}</terminationCompositionStyle><stepCountLimit>4</stepCountLimit><termination><bestScoreLimit>0hard/0soft</bestScoreLimit></termination></termination>"
        ));
        let prior = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
            ScoreGoalProgressConfig::target(HardSoftScore::new(0, 100), vec![0.5]).unwrap(),
        ));
        for existing in [None, Some(&prior)] {
            xml.with_sequence_controls(
                SolverSequence::<HardSoftScore> {
                    phases: vec![SolverPhase::LocalSearch(Default::default())],
                    ..Default::default()
                },
                &[],
                existing,
                |_, _, solver| {
                    let mut session = StatefulTerminationSession::new(solver.unwrap());
                    session.observe_initialized_score(&HardSoftScore::new(-10, -100));
                    let progress = SearchProgress {
                        completed_steps: 2,
                        ..Default::default()
                    };
                    let s = session
                        .status(progress, Some(&HardSoftScore::new(0, -50)))
                        .unwrap();
                    let expected = if and {
                        if existing.is_some() { 0.625 } else { 0.5 }
                    } else {
                        0.75
                    };
                    assert_eq!(
                        s,
                        TerminationState {
                            progress: Some(expected),
                            reached: false
                        }
                    );
                    let s = session
                        .status(progress, Some(&HardSoftScore::ZERO))
                        .unwrap();
                    assert_eq!(s.reached, !and);
                },
            )
            .unwrap();
        }
    }
}
