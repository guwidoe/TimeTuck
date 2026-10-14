//! Original XML budget/feasibility integration controls; no source-method credit.
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
fn local() -> LocalSearchConfig<'static, HardSoftScore> {
    LocalSearchConfig {
        max_steps: 8,
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
}
macro_rules! extensions {
    ($provider:expr, $bindings:expr) => {{
        let mut ext = SolverExtensions {
            phase_terminations: $bindings,
            ..Default::default()
        };
        ext.move_providers.bind(0, $provider);
        ext
    }};
}
fn run(model: usize, stream: bool, xml: &XmlTerminationPlan, cap: Option<u64>) -> ResultRow {
    let basic = FiniteProblem::new(vec![Variable::new((0..16).collect()).with_initial(0)]).unwrap();
    let lists =
        ListProblem::new((0..16).collect(), vec![ListEntity::new((0..16).collect())]).unwrap();
    let limits = SolverLimits {
        max_move_evaluations: cap,
        ..Default::default()
    };
    let seq = SolverSequence {
        limits,
        phases: vec![SolverPhase::LocalSearch(local())],
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
                .with_sequence_extensions(seq, &[], |seq, bindings| {
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
                            extensions!(&mut provider, bindings),
                        )
                    } else {
                        let mut n = ConstraintNetwork::new(1);
                        n.add_constraint("objective", vec![0], |v| objective(*v[0].unwrap()))
                            .unwrap();
                        solve_with_extensions(&basic, &n, seq, extensions!(&mut provider, bindings))
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
                .with_sequence_extensions(seq, &[], |seq, bindings| {
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
                            extensions!(&mut provider, bindings),
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
                            extensions!(&mut provider, bindings),
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
                    search: local(),
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
                .with_mixed_sequence_extensions(seq, &[], |seq, bindings| {
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
                            extensions!(&mut provider, bindings),
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
                            extensions!(&mut provider, bindings),
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
fn xml_counts_and_feasibility_stop_actual_basic_list_mixed_dense_stream_work() {
    for (condition, expected) in [
        ("<moveCountLimit>3</moveCountLimit>", 3),
        (
            "<scoreCalculationCountLimit>4</scoreCalculationCountLimit>",
            3,
        ),
        ("<bestScoreFeasible>true</bestScoreFeasible>", 3),
        ("<bestScoreFeasible>1</bestScoreFeasible>", 3),
        ("<bestScoreFeasible>false</bestScoreFeasible>", 8),
        ("<bestScoreFeasible>0</bestScoreFeasible>", 8),
        (
            "<terminationCompositionStyle>AND</terminationCompositionStyle><moveCountLimit>3</moveCountLimit><scoreCalculationCountLimit>6</scoreCalculationCountLimit>",
            5,
        ),
        (
            "<terminationCompositionStyle>OR</terminationCompositionStyle><moveCountLimit>3</moveCountLimit><scoreCalculationCountLimit>6</scoreCalculationCountLimit>",
            3,
        ),
        (
            "<terminationCompositionStyle>AND</terminationCompositionStyle><bestScoreFeasible>false</bestScoreFeasible><moveCountLimit>3</moveCountLimit>",
            3,
        ),
        (
            "<termination><bestScoreFeasible>true</bestScoreFeasible></termination><termination><moveCountLimit>2</moveCountLimit></termination>",
            2,
        ),
    ] {
        for phase in [false, true] {
            let body = format!("<termination>{condition}</termination>");
            let xml = plan(&if phase {
                format!("<localSearch>{body}</localSearch>")
            } else {
                body
            });
            for model in 0..3 {
                for stream in [false, true] {
                    let r = run(model, stream, &xml, None);
                    assert_eq!(
                        r.value, expected,
                        "condition={condition}, phase={phase}, model={model}, stream={stream}: {r:?}"
                    );
                    assert!(r.complete);
                    assert_eq!(r.score, Some(objective(r.value)));
                    assert_eq!(r.work.move_evaluations, expected as u64);
                    assert_eq!(r.work.evaluations, expected as u64 + 1);
                    let capped = run(model, stream, &xml, Some(2));
                    assert_eq!(capped.value, 2);
                    assert_eq!(capped.work.move_evaluations, 2);
                    assert_eq!(capped.score, Some(objective(2)));
                }
            }
        }
    }
}

#[test]
fn invalid_xml_budget_values_reject_before_execution() {
    for raw in [
        "<bestScoreFeasible>yes</bestScoreFeasible>",
        "<bestScoreFeasible>TRUE</bestScoreFeasible>",
        "<bestScoreFeasible/>",
        "<bestScoreFeasible>true</bestScoreFeasible><bestScoreFeasible>false</bestScoreFeasible>",
        "<moveCountLimit>-1</moveCountLimit>",
        "<scoreCalculationCountLimit>1.5</scoreCalculationCountLimit>",
        "<moveCountLimit>18446744073709551616</moveCountLimit>",
        "<scoreCalculationCountLimit attribute='bad'>2</scoreCalculationCountLimit>",
        "<bestScoreFeasible><child/></bestScoreFeasible>",
    ] {
        for wrapper in ["termination", "localSearch"] {
            let body = if wrapper == "termination" {
                format!("<termination>{raw}</termination>")
            } else {
                format!("<localSearch><termination>{raw}</termination></localSearch>")
            };
            if let Ok(config) = SolverXmlConfiguration::parse(&format!("<solver>{body}</solver>")) {
                assert!(config.native_termination_plan().is_err(), "{raw}");
            }
        }
    }
    let p = plan(
        "<termination><moveCountLimit>0</moveCountLimit><scoreCalculationCountLimit>0</scoreCalculationCountLimit><bestScoreFeasible> false </bestScoreFeasible></termination>",
    );
    assert_eq!(p.solver.as_ref().unwrap().move_count, Some(0));
    assert_eq!(p.solver.as_ref().unwrap().score_calculation_count, Some(0));
    assert_eq!(p.solver.as_ref().unwrap().best_score_feasible, Some(false));
}

#[test]
fn scalar_projection_binds_count_budgets_for_every_native_phase() {
    let kinds = [
        "constructionHeuristic",
        "localSearch",
        "ruinRecreate",
        "exhaustiveSearch",
        "customPhase",
    ];
    let controls = "<termination><moveCountLimit>7</moveCountLimit><scoreCalculationCountLimit>11</scoreCalculationCountLimit><bestScoreFeasible>false</bestScoreFeasible></termination>";
    for (i, kind) in kinds.into_iter().enumerate() {
        let xml = plan(&format!("{controls}<{kind}>{controls}</{kind}>"));
        let phases: Vec<SolverPhase<'_, HardSoftScore>> = vec![match i {
            0 => SolverPhase::Construction(Default::default()),
            1 => SolverPhase::LocalSearch(Default::default()),
            2 => SolverPhase::RuinRecreate(Default::default()),
            3 => SolverPhase::Exact(Default::default()),
            _ => SolverPhase::Custom(Default::default()),
        }];
        xml.with_sequence(
            SolverSequence {
                phases,
                ..Default::default()
            },
            |s| {
                assert_eq!(s.limits.max_evaluations, 11);
                assert_eq!(s.limits.max_move_evaluations, Some(7));
                assert!(!s.limits.stop_when_feasible);
                let (score, moves) = match &s.phases[0] {
                    SolverPhase::Construction(c) => (c.max_evaluations, c.max_move_evaluations),
                    SolverPhase::LocalSearch(c) => {
                        assert!(!c.stop_when_feasible);
                        (c.max_evaluations, c.max_move_evaluations)
                    }
                    SolverPhase::RuinRecreate(c) => {
                        assert!(!c.stop_when_feasible);
                        (c.max_evaluations, c.max_move_evaluations)
                    }
                    SolverPhase::Exact(c) => {
                        assert!(!c.stop_when_feasible);
                        (c.max_evaluations, c.max_move_evaluations)
                    }
                    SolverPhase::Custom(c) => (c.max_evaluations, c.max_move_evaluations),
                };
                assert_eq!((score, moves), (11, Some(7)));
            },
        )
        .unwrap();
        let phases: Vec<MixedSolverPhase<'_, HardSoftScore>> = vec![match i {
            0 => MixedSolverPhase::Construction(Default::default()),
            1 => MixedSolverPhase::LocalSearch(Default::default()),
            2 => MixedSolverPhase::RuinRecreate(Default::default()),
            3 => MixedSolverPhase::Exact(Default::default()),
            _ => MixedSolverPhase::Custom(Default::default()),
        }];
        xml.with_mixed_sequence(
            MixedSolverSequence {
                phases,
                ..Default::default()
            },
            |s| {
                assert_eq!(s.limits.max_evaluations, 11);
                assert_eq!(s.limits.max_move_evaluations, Some(7));
                assert!(!s.limits.stop_when_feasible);
                let (score, moves) = match &s.phases[0] {
                    MixedSolverPhase::Construction(c) => (
                        c.construction.max_evaluations,
                        c.construction.max_move_evaluations,
                    ),
                    MixedSolverPhase::LocalSearch(c) => {
                        assert!(!c.search.stop_when_feasible);
                        (c.search.max_evaluations, c.search.max_move_evaluations)
                    }
                    MixedSolverPhase::RuinRecreate(c) => {
                        assert!(!c.search.stop_when_feasible);
                        (c.search.max_evaluations, c.search.max_move_evaluations)
                    }
                    MixedSolverPhase::Exact(c) => {
                        assert!(!c.stop_when_feasible);
                        (c.max_evaluations, c.max_move_evaluations)
                    }
                    MixedSolverPhase::Custom(c) => (c.max_evaluations, c.max_move_evaluations),
                };
                assert_eq!((score, moves), (11, Some(7)));
            },
        )
        .unwrap();
    }
}
