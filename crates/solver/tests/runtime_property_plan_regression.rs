// Original runtime controls reuse pinned Community recorder TEST input literals (30-minute window, 123.0 ratio); Apache-2.0 fixture-only attribution in compat/provenance.json.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn properties(raw: RuntimeDiminishedReturns) -> RuntimePropertyPlan {
    let mut pairs = Vec::new();
    if let Some(v) = raw.enabled {
        pairs.push((
            "timefold.model.termination.diminished-returns.enabled".into(),
            v.to_string(),
        ));
    }
    if let Some(v) = raw.sliding_window_duration {
        pairs.push((
            "timefold.model.termination.diminished-returns.sliding-window-duration".into(),
            format!("PT{}S", v.as_secs()),
        ));
    }
    if let Some(v) = raw.minimum_improvement_ratio {
        pairs.push((
            "timefold.model.termination.diminished-returns.minimum-improvement-ratio".into(),
            v.to_string(),
        ));
    }
    RuntimePropertyPlan::resolve(
        &OwnedPropertySource::new(pairs),
        &PropertyFallbackRules::community_test_profile(),
        "timefold.model.termination",
        &mut |_| {},
    )
    .unwrap()
}
fn local() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 40,
        max_evaluations: 80,
        move_order: MoveOrder::ShuffledVariables { seed: 37 },
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::FirstAccepted,
        ..Default::default()
    }
}
fn objective(v: i32) -> SimpleScore {
    SimpleScore::new(if v == 0 {
        0
    } else {
        1000 + i64::from(v.min(4))
    })
}
#[test]
fn parsed_properties_drive_dense_and_stream_local_phases() {
    // The same 11-second callbacks separate the 30-second default from the
    // 30-minute window. A second long-window run actually reaches its expiry.
    for (raw, quantum, expires) in [
        (
            RuntimeDiminishedReturns {
                enabled: Some(true),
                ..Default::default()
            },
            11,
            true,
        ),
        (
            RuntimeDiminishedReturns {
                sliding_window_duration: Some(Duration::from_secs(30 * 60)),
                ..Default::default()
            },
            11,
            false,
        ),
        (
            RuntimeDiminishedReturns {
                sliding_window_duration: Some(Duration::from_secs(30 * 60)),
                ..Default::default()
            },
            601,
            true,
        ),
        (
            RuntimeDiminishedReturns {
                minimum_improvement_ratio: Some(123.0),
                ..Default::default()
            },
            11,
            true,
        ),
        (
            RuntimeDiminishedReturns {
                minimum_improvement_ratio: Some(0.0001),
                ..Default::default()
            },
            11,
            true,
        ),
        (
            RuntimeDiminishedReturns {
                minimum_improvement_ratio: Some(f64::from_bits(1)),
                ..Default::default()
            },
            11,
            true,
        ),
        (
            RuntimeDiminishedReturns {
                enabled: Some(false),
                ..Default::default()
            },
            11,
            false,
        ),
    ] {
        let p =
            FiniteProblem::new(vec![Variable::new((0..100).collect()).with_initial(0)]).unwrap();
        let ticks = Arc::new(AtomicU64::new(0));
        let source = ticks.clone();
        let clock = SolverClock::new(move || Duration::from_secs(source.load(Ordering::SeqCst)));
        let mut plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::LocalSearch(local()),
            ],
            ..Default::default()
        };
        let receipt = properties(raw).bind(&mut plan).unwrap();
        if let Some(window) = raw.sliding_window_duration {
            assert_eq!(receipt.resolved.unwrap().window(), window);
        }
        if let Some(ratio) = raw.minimum_improvement_ratio {
            assert_eq!(
                receipt.resolved.unwrap(),
                DiminishedReturnsConfig::from_f64(Duration::from_secs(30), ratio).unwrap()
            );
        }
        let mut score = ConstraintNetwork::new(1);
        let t = ticks.clone();
        score
            .add_constraint("objective", vec![0], move |v| {
                t.fetch_add(quantum, Ordering::SeqCst);
                objective(*v[0].unwrap())
            })
            .unwrap();
        let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(a.values()[0].unwrap() + 1),
            }]
        };
        let mut extensions = SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let dense = solve_with_extensions(&p, &score, plan.clone(), extensions).unwrap();
        ticks.store(0, Ordering::SeqCst);
        let graph = || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            let t = ticks.clone();
            b.score("objective", source.port(), move |(_, v)| {
                t.fetch_add(quantum, Ordering::SeqCst);
                objective(v.unwrap())
            })?;
            Ok((b.build(), source))
        };
        let mut extensions = SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let stream = solve_stream_with_extensions(&p, graph, plan, extensions).unwrap();
        assert_eq!(dense.assignment, stream.assignment);
        assert_eq!(dense.score, stream.score);
        for r in [dense, stream] {
            assert!(r.work.evaluations > 0 && r.work.evaluations <= 80);
            let expected = if expires {
                LocalSearchTermination::DiminishedReturns
            } else {
                LocalSearchTermination::StepLimit
            };
            assert_eq!(
                r.phases[1].termination,
                PhaseTermination::LocalSearch(expected)
            );
            let a = r.assignment;
            assert_eq!(r.score, Some(objective(a.values()[0].unwrap())));
            assert!(r.score.unwrap() > SimpleScore::ZERO);
        }
    }
}
#[test]
fn parsed_mixed_properties_drive_both_backends_without_touching_lists() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..100).collect()).with_initial(0)]).unwrap(),
        ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![8, 7])]).unwrap(),
    );
    for (enabled, ratio) in [
        (true, None),
        (false, None),
        (true, Some(0.0001)),
        (true, Some(f64::from_bits(1))),
    ] {
        let ticks = Arc::new(AtomicU64::new(0));
        let source = ticks.clone();
        let clock = SolverClock::new(move || Duration::from_secs(source.load(Ordering::SeqCst)));
        let mut plan = MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(Default::default()),
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search: local(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let binding = properties(RuntimeDiminishedReturns {
            enabled: Some(enabled),
            minimum_improvement_ratio: ratio,
            ..Default::default()
        })
        .bind_mixed(&mut plan)
        .unwrap();
        assert_eq!(binding.applied_local_phases, usize::from(enabled));
        let mut network = MixedConstraintNetwork::new(1, 1, 2);
        let t = ticks.clone();
        network
            .add_constraint("objective", vec![MixedScope::Basic(0)], move |facts| {
                t.fetch_add(11, Ordering::SeqCst);
                let MixedFact::Basic(v) = facts[0] else {
                    unreachable!()
                };
                objective(*v.unwrap())
            })
            .unwrap();
        let mut provider = |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| {
            vec![MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(a.basic().values()[0].unwrap() + 1),
                },
                lists: ListMove::Composite(vec![]),
            }]
        };
        let mut extensions = MixedSolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let dense = solve_mixed_with_extensions(&p, &network, plan.clone(), extensions).unwrap();
        ticks.store(0, Ordering::SeqCst);
        let graph = || {
            let mut b = StreamGraphBuilder::new();
            let sources = MixedStreamSources::<i32, i32>::new(&mut b);
            let t = ticks.clone();
            b.score("objective", sources.basic.port(), move |(_, v)| {
                t.fetch_add(11, Ordering::SeqCst);
                objective(v.unwrap())
            })?;
            Ok((b.build(), sources))
        };
        let mut extensions = MixedSolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let stream = solve_mixed_stream_with_extensions(&p, graph, plan, extensions).unwrap();
        assert_eq!(dense.assignment, stream.assignment);
        assert_eq!(dense.score, stream.score);
        for r in [dense, stream] {
            assert_eq!(
                r.phases[1].termination,
                PhaseTermination::LocalSearch(if enabled {
                    LocalSearchTermination::DiminishedReturns
                } else {
                    LocalSearchTermination::StepLimit
                })
            );
            assert_eq!(r.assignment.lists().lists(), &[vec![8, 7]]);
            assert_eq!(
                r.score,
                Some(objective(r.assignment.basic().values()[0].unwrap()))
            );
            assert!(r.work.evaluations > 0 && r.work.evaluations <= 80);
        }
    }
}

#[test]
fn parsed_shared_time_limits_stop_both_mixed_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..100).collect()).with_initial(0)]).unwrap(),
        ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![8, 7])]).unwrap(),
    );
    for spent in [true, false] {
        let ticks = Arc::new(AtomicU64::new(0));
        let source = ticks.clone();
        let clock = SolverClock::new(move || Duration::from_secs(source.load(Ordering::SeqCst)));
        let mut plan = MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(Default::default()),
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search: local(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let suffix = if spent {
            "spent-limit"
        } else {
            "unimproved-spent-limit"
        };
        let raw = RuntimePropertyPlan::resolve(
            &OwnedPropertySource::new([(
                format!("timefold.model.termination.{suffix}"),
                "30s".into(),
            )]),
            &PropertyFallbackRules::community_test_profile(),
            "timefold.model.termination",
            &mut |_| {},
        )
        .unwrap();
        raw.bind_mixed(&mut plan).unwrap();
        let mut network = MixedConstraintNetwork::new(1, 1, 2);
        let t = ticks.clone();
        network
            .add_constraint("objective", vec![MixedScope::Basic(0)], move |facts| {
                t.fetch_add(11, Ordering::SeqCst);
                let MixedFact::Basic(v) = facts[0] else {
                    unreachable!()
                };
                {
                    let _ = v;
                    SimpleScore::ZERO
                }
            })
            .unwrap();
        let mut provider = |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, _: u64| {
            vec![MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(a.basic().values()[0].unwrap() + 1),
                },
                lists: ListMove::Composite(vec![]),
            }]
        };
        let mut extensions = MixedSolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let dense = solve_mixed_with_extensions(&p, &network, plan.clone(), extensions).unwrap();
        ticks.store(0, Ordering::SeqCst);
        let graph = || {
            let mut b = StreamGraphBuilder::new();
            let sources = MixedStreamSources::<i32, i32>::new(&mut b);
            let t = ticks.clone();
            b.score("objective", sources.basic.port(), move |(_, v)| {
                t.fetch_add(11, Ordering::SeqCst);
                {
                    let _ = v;
                    SimpleScore::ZERO
                }
            })?;
            Ok((b.build(), sources))
        };
        let mut extensions = MixedSolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let stream = solve_mixed_stream_with_extensions(&p, graph, plan, extensions).unwrap();
        assert_eq!(dense.assignment, stream.assignment);
        assert_eq!(dense.score, stream.score);
        for r in [dense, stream] {
            assert_eq!(
                r.termination,
                if spent {
                    SolverTermination::TimeLimit
                } else {
                    SolverTermination::UnimprovedTimeLimit
                }
            );
            assert_eq!(r.assignment.lists().lists(), &[vec![8, 7]]);
            assert_eq!(r.score, Some(SimpleScore::ZERO));
            assert!(r.work.evaluations > 0 && r.work.evaluations <= 80);
        }
    }
}

#[test]
fn parsed_shared_time_limits_stop_both_standard_backends() {
    let p = FiniteProblem::new(vec![Variable::new((0..100).collect()).with_initial(0)]).unwrap();
    for spent in [true, false] {
        let ticks = Arc::new(AtomicU64::new(0));
        let source = ticks.clone();
        let clock = SolverClock::new(move || Duration::from_secs(source.load(Ordering::SeqCst)));
        let mut plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::LocalSearch(local()),
            ],
            ..Default::default()
        };
        let suffix = if spent {
            "spent-limit"
        } else {
            "unimproved-spent-limit"
        };
        let raw = RuntimePropertyPlan::resolve(
            &OwnedPropertySource::new([(
                format!("timefold.model.termination.{suffix}"),
                "30s".into(),
            )]),
            &PropertyFallbackRules::community_test_profile(),
            "timefold.model.termination",
            &mut |_| {},
        )
        .unwrap();
        raw.bind(&mut plan).unwrap();
        let mut network = ConstraintNetwork::new(1);
        let t = ticks.clone();
        network
            .add_constraint("flat", vec![0], move |_| {
                t.fetch_add(11, Ordering::SeqCst);
                SimpleScore::ZERO
            })
            .unwrap();
        let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(a.values()[0].unwrap() + 1),
            }]
        };
        let mut extensions = SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let dense = solve_with_extensions(&p, &network, plan.clone(), extensions).unwrap();
        ticks.store(0, Ordering::SeqCst);
        let graph = || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            let t = ticks.clone();
            b.score("flat", source.port(), move |_| {
                t.fetch_add(11, Ordering::SeqCst);
                SimpleScore::ZERO
            })?;
            Ok((b.build(), source))
        };
        let mut extensions = SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        extensions.move_providers.bind(1, &mut provider);
        let stream = solve_stream_with_extensions(&p, graph, plan, extensions).unwrap();
        assert_eq!(dense.assignment, stream.assignment);
        assert_eq!(dense.score, stream.score);
        for r in [dense, stream] {
            assert_eq!(
                r.termination,
                if spent {
                    SolverTermination::TimeLimit
                } else {
                    SolverTermination::UnimprovedTimeLimit
                }
            );

            assert_eq!(r.score, Some(SimpleScore::ZERO));
            assert!(r.work.evaluations > 0 && r.work.evaluations <= 80);
        }
    }
}
