//! Original executable XML score-difference controls; no source-method credit.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;

fn plan(body: &str) -> XmlTerminationPlan {
    SolverXmlConfiguration::parse(&format!("<solver>{body}</solver>"))
        .unwrap()
        .native_termination_plan()
        .unwrap()
}

fn threshold(minimum: &str) -> String {
    format!(
        "<unimprovedSecondsSpentLimit>3</unimprovedSecondsSpentLimit><unimprovedScoreDifferenceThreshold>{minimum}</unimprovedScoreDifferenceThreshold>"
    )
}
fn assert_row(row: &ResultRow, expected: i32) {
    assert_eq!(row.value, expected, "{row:?}");
    assert!(row.complete);
    assert_eq!(row.score, Some(objective(expected)));
    assert_eq!(row.work.move_evaluations, expected as u64);
    assert_eq!(row.work.completed_steps, expected as u64);
}

#[test]
fn significant_xml_uses_accumulated_multilevel_improvements_and_preserves_grouping() {
    // Every step improves soft score; hard score improves only twice. A minimum
    // of3 hard can never be met. A minimum of1 hard resets at the second step;
    // 3 soft accumulates and qualifies exactly at successive window boundaries.
    for (conditions, expected) in [
        (threshold("3hard/0soft"), 4),
        (threshold("1hard/0soft"), 5),
        (threshold("0hard/3soft"), 10),
        (threshold("0hard/0soft"), 10),
        (threshold("-1hard/0soft"), 10),
        (
            "<unimprovedSecondsSpentLimit>3</unimprovedSecondsSpentLimit>".into(),
            10,
        ),
        (
            format!(
                "<terminationCompositionStyle>AND</terminationCompositionStyle>{}<stepCountLimit>8</stepCountLimit>",
                threshold("3hard/0soft")
            ),
            8,
        ),
        (
            format!(
                "{}<stepCountLimit>2</stepCountLimit>",
                threshold("3hard/0soft")
            ),
            2,
        ),
        (
            format!(
                "<termination><terminationCompositionStyle>AND</terminationCompositionStyle>{}<stepCountLimit>8</stepCountLimit></termination><termination><stepCountLimit>6</stepCountLimit></termination>",
                threshold("3hard/0soft")
            ),
            6,
        ),
        (
            format!(
                "<terminationCompositionStyle>AND</terminationCompositionStyle>{}<bestScoreLimit>0hard/6soft</bestScoreLimit>",
                threshold("3hard/0soft")
            ),
            6,
        ),
    ] {
        for phase in [false, true] {
            let body = format!("<termination>{conditions}</termination>");
            let xml = plan(&if phase {
                format!("<localSearch>{body}</localSearch>")
            } else {
                body
            });
            for model in 0..3 {
                for stream in [false, true] {
                    let row = run(model, stream, &xml, false, Default::default(), None);
                    assert_row(&row, expected);
                    if expected < 10 {
                        if phase {
                            assert_eq!(
                                row.phase_reasons[0],
                                PhaseTermination::LocalSearch(
                                    LocalSearchTermination::StepCondition
                                )
                            );
                        } else {
                            assert_eq!(row.reason, SolverTermination::Condition);
                        }
                    }
                    let capped = run(
                        model,
                        stream,
                        &xml,
                        false,
                        SolverLimits {
                            max_move_evaluations: Some(2),
                            ..Default::default()
                        },
                        None,
                    );
                    assert_row(&capped, 2);
                }
            }
        }
    }
}

#[test]
fn solver_history_survives_handoff_while_explicit_phase_history_starts_fresh() {
    let root = plan(&format!(
        "<termination>{}</termination>",
        threshold("3hard/0soft")
    ));
    let phase = plan(&format!(
        "<localSearch/><localSearch><termination>{}</termination></localSearch>",
        threshold("3hard/0soft")
    ));
    for model in 0..3 {
        for stream in [false, true] {
            let global = run(model, stream, &root, true, Default::default(), None);
            assert_row(&global, 4);
            assert_eq!(global.phase_steps, [2, 2]);
            assert_eq!(global.reason, SolverTermination::Condition);
            let per_phase = run(model, stream, &phase, true, Default::default(), None);
            assert_row(&per_phase, 6);
            assert_eq!(per_phase.phase_steps, [2, 4]);
            assert_eq!(per_phase.reason, SolverTermination::PlanCompleted);
        }
    }
}

#[test]
fn prior_solver_histories_and_native_scored_limits_remain_independent() {
    let old_score =
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(2)));
    let old = StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&old_score));
    for conditions in [
        threshold("3hard/0soft"),
        "<stepCountLimit>4</stepCountLimit>".into(),
    ] {
        let xml = plan(&format!("<termination>{conditions}</termination>"));
        for model in 0..3 {
            for stream in [false, true] {
                assert_row(
                    &run(model, stream, &xml, false, Default::default(), Some(&old)),
                    2,
                );
                let goal =
                    ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(objective(1)));
                assert_row(
                    &run(
                        model,
                        stream,
                        &xml,
                        false,
                        SolverLimits {
                            scored_termination: Some(&goal),
                            ..Default::default()
                        },
                        None,
                    ),
                    1,
                );
            }
        }
    }
}

#[test]
fn invalid_thresholds_and_unsupported_binding_profiles_reject_before_callbacks() {
    for raw in [
        "<unimprovedScoreDifferenceThreshold>1hard/0soft</unimprovedScoreDifferenceThreshold>",
        "<unimprovedSecondsSpentLimit>-1</unimprovedSecondsSpentLimit><unimprovedScoreDifferenceThreshold>1hard/0soft</unimprovedScoreDifferenceThreshold>",
        "<unimprovedSecondsSpentLimit>3</unimprovedSecondsSpentLimit><unimprovedScoreDifferenceThreshold>1hard/0soft</unimprovedScoreDifferenceThreshold><unimprovedScoreDifferenceThreshold>2hard/0soft</unimprovedScoreDifferenceThreshold>",
    ] {
        let config = SolverXmlConfiguration::parse(&format!(
            "<solver><termination>{raw}</termination></solver>"
        ))
        .unwrap();
        assert!(config.native_termination_plan().is_err(), "{raw}");
    }
    let sequence = || SolverSequence {
        phases: vec![SolverPhase::LocalSearch(local(10))],
        ..Default::default()
    };
    for invalid in [
        "bad".to_owned(),
        HardSoftScore::new(1, 0).with_structural(-1).to_string(),
    ] {
        let xml = plan(&format!(
            "<termination>{}</termination>",
            threshold(&invalid)
        ));
        assert!(
            xml.with_sequence_controls(sequence(), &[], None, |_, _, _| panic!(
                "invalid score reached execution"
            ))
            .is_err()
        );
    }
    let xml = plan(&format!(
        "<termination>{}</termination>",
        threshold("1hard/0soft")
    ));
    assert!(
        xml.with_sequence(sequence(), |_| panic!(
            "scalar projection discarded history"
        ))
        .is_err()
    );
    assert!(
        xml.with_sequence_extensions(sequence(), &[], |_, _| panic!(
            "legacy profile discarded root history"
        ))
        .is_err()
    );
    let mut edited = xml;
    edited.solver.as_mut().unwrap().unimproved = Default::default();
    assert!(
        edited
            .with_sequence_controls(sequence(), &[], None, |_, _, _| panic!(
                "invalid edit reached execution"
            ))
            .is_err()
    );
    let construction = plan(&format!(
        "<constructionHeuristic><termination>{}</termination></constructionHeuristic>",
        threshold("1hard/0soft")
    ));
    assert!(
        construction
            .with_sequence_controls(
                SolverSequence::<HardSoftScore> {
                    phases: vec![SolverPhase::Construction(Default::default())],
                    ..Default::default()
                },
                &[],
                None,
                |_, _, _| panic!("construction armed search history")
            )
            .is_err()
    );
    let zero = plan(
        "<termination><unimprovedMillisecondsSpentLimit>0</unimprovedMillisecondsSpentLimit><unimprovedScoreDifferenceThreshold>1hard/0soft</unimprovedScoreDifferenceThreshold></termination>",
    );
    zero.with_sequence_controls(sequence(), &[], None, |_, _, history| {
        assert!(history.is_some())
    })
    .unwrap();
}
fn objective(value: i32) -> HardSoftScore {
    HardSoftScore::new(i64::from(value.min(2) - 2), i64::from(value))
}
fn local(max_steps: u64) -> LocalSearchConfig<'static, HardSoftScore> {
    LocalSearchConfig {
        max_steps,
        acceptance: Acceptance::HillClimbing,
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
    reason: SolverTermination,
    phase_steps: Vec<u64>,
    phase_reasons: Vec<PhaseTermination>,
}
macro_rules! extensions {
    ($provider:expr, $second:expr, $bindings:expr, $solver:expr, $clock:expr, $lifecycle:expr, $two:expr) => {{
        let mut ext = SolverExtensions {
            phase_terminations: $bindings,
            stateful_termination: $solver,
            clock: Some($clock),
            lifecycle: Some($lifecycle),
            ..Default::default()
        };
        ext.move_providers.bind(0, $provider);
        if $two {
            ext.move_providers.bind(1, $second);
        }
        ext
    }};
}
fn run(
    model: usize,
    stream: bool,
    xml: &XmlTerminationPlan,
    two: bool,
    limits: SolverLimits<'_, HardSoftScore>,
    existing: Option<&StatefulSearchTermination<'_, HardSoftScore>>,
) -> ResultRow {
    let tick = Arc::new(AtomicU64::new(0));
    let read = tick.clone();
    let clock = SolverClock::new(move || Duration::from_secs(read.load(Ordering::SeqCst)));
    let mut lifecycle = |event| {
        if matches!(event, SolverLifecycleEvent::StepEnded { .. }) {
            tick.fetch_add(1, Ordering::SeqCst);
        }
    };
    let basic = FiniteProblem::new(vec![Variable::new((0..16).collect()).with_initial(0)]).unwrap();
    let lists =
        ListProblem::new((0..16).collect(), vec![ListEntity::new((0..16).collect())]).unwrap();
    let seq = SolverSequence {
        limits,
        phases: if two {
            vec![
                SolverPhase::LocalSearch(local(2)),
                SolverPhase::LocalSearch(local(10)),
            ]
        } else {
            vec![SolverPhase::LocalSearch(local(10))]
        },
    };
    match model {
        0 => {
            let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(a.values()[0].unwrap() + 1),
                }]
            };
            let mut second = provider;
            let r = xml
                .with_sequence_controls(seq, &[], existing, |seq, bindings, solver| {
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
                            extensions!(
                                &mut provider,
                                &mut second,
                                bindings,
                                solver,
                                &clock,
                                &mut lifecycle,
                                two
                            ),
                        )
                    } else {
                        let mut n = ConstraintNetwork::new(1);
                        n.add_constraint("objective", vec![0], |v| objective(*v[0].unwrap()))
                            .unwrap();
                        solve_with_extensions(
                            &basic,
                            &n,
                            seq,
                            extensions!(
                                &mut provider,
                                &mut second,
                                bindings,
                                solver,
                                &clock,
                                &mut lifecycle,
                                two
                            ),
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
                reason: r.termination,
                phase_steps: r.phases.iter().map(|p| p.work.completed_steps).collect(),
                phase_reasons: r.phases.iter().map(|p| p.termination).collect(),
            }
        }
        1 => {
            let mut provider = |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
                vec![ListMove::Swap {
                    left: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                    right: ListPosition {
                        entity: 0,
                        index: a.lists()[0]
                            .iter()
                            .position(|v| *v == a.lists()[0][0] + 1)
                            .unwrap(),
                    },
                }]
            };
            let mut second = provider;
            let r = xml
                .with_sequence_controls(seq, &[], existing, |seq, bindings, solver| {
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
                            extensions!(
                                &mut provider,
                                &mut second,
                                bindings,
                                solver,
                                &clock,
                                &mut lifecycle,
                                two
                            ),
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
                            extensions!(
                                &mut provider,
                                &mut second,
                                bindings,
                                solver,
                                &clock,
                                &mut lifecycle,
                                two
                            ),
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
                reason: r.termination,
                phase_steps: r.phases.iter().map(|p| p.work.completed_steps).collect(),
                phase_reasons: r.phases.iter().map(|p| p.termination).collect(),
            }
        }
        _ => {
            let p = MixedProblem::new(basic, lists);
            let seq = MixedSolverSequence {
                limits,
                phases: if two {
                    vec![
                        MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                            search: local(2),
                            ..Default::default()
                        }),
                        MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                            search: local(10),
                            ..Default::default()
                        }),
                    ]
                } else {
                    vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search: local(10),
                        ..Default::default()
                    })]
                },
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
            let mut second = provider;
            let r = xml
                .with_mixed_sequence_controls(seq, &[], existing, |seq, bindings, solver| {
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
                            extensions!(
                                &mut provider,
                                &mut second,
                                bindings,
                                solver,
                                &clock,
                                &mut lifecycle,
                                two
                            ),
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
                            extensions!(
                                &mut provider,
                                &mut second,
                                bindings,
                                solver,
                                &clock,
                                &mut lifecycle,
                                two
                            ),
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
                reason: r.termination,
                phase_steps: r.phases.iter().map(|p| p.work.completed_steps).collect(),
                phase_reasons: r.phases.iter().map(|p| p.termination).collect(),
            }
        }
    }
}
