use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn properties(raw: RuntimeDiminishedReturns) -> RuntimeSolverProperties {
    RuntimeSolverProperties {
        termination: Some(RuntimeTerminationProperties {
            diminished_returns: Some(raw),
        }),
    }
}
#[test]
fn exact_positive_binary_ratios_and_invalid_inputs_are_staged_before_mutation() {
    for (raw, expected) in [
        (123.0, (123, 1)),
        (0.125, (1, 8)),
        (0.1, (3602879701896397, 36028797018963968)),
        (2f64.powi(63), (1u64 << 63, 1)),
    ] {
        let mut p = SolverSequence::<SimpleScore>::default();
        let b = properties(RuntimeDiminishedReturns {
            minimum_improvement_ratio: Some(raw),
            ..Default::default()
        })
        .bind(&mut p)
        .unwrap();
        assert_eq!(b.resolved.unwrap().ratio(), expected);
        assert_eq!(b.applied_local_phases, 1);
    }
    for raw in [
        0.0,
        -0.0,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        2f64.powi(64),
        f64::from_bits(1),
        1e300,
    ] {
        let mut p = SolverSequence::<SimpleScore>::default();
        let before = p.clone();
        assert!(
            properties(RuntimeDiminishedReturns {
                minimum_improvement_ratio: Some(raw),
                ..Default::default()
            })
            .bind(&mut p)
            .is_err()
        );
        assert_eq!(p, before);
    }
    let mut p = SolverSequence::<SimpleScore>::default();
    let before = p.clone();
    assert_eq!(
        properties(RuntimeDiminishedReturns {
            sliding_window_duration: Some(Duration::ZERO),
            ..Default::default()
        })
        .bind(&mut p),
        Err(DiminishedRuntimeError::Policy(
            DiminishedReturnsError::ZeroWindow
        ))
    );
    assert_eq!(p, before);
}
#[test]
fn missing_disabled_and_reapplication_preserve_all_explicit_controls_in_both_sequences() {
    let explicit = DiminishedReturnsConfig::new(Duration::from_secs(7), 3, 4).unwrap();
    let config = LocalSearchConfig {
        max_steps: 13,
        max_time: Some(Duration::from_secs(91)),
        diminished_returns: Some(explicit),
        move_order: MoveOrder::ShuffledVariables { seed: 37 },
        ..Default::default()
    };
    let mut p = SolverSequence::<SimpleScore> {
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(config.clone()),
            SolverPhase::LocalSearch(Default::default()),
        ],
        ..Default::default()
    };
    let mut m = MixedSolverSequence::<SimpleScore> {
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: config,
                ..Default::default()
            }),
            MixedSolverPhase::LocalSearch(Default::default()),
        ],
        ..Default::default()
    };
    for raw in [
        RuntimeSolverProperties::default(),
        RuntimeSolverProperties {
            termination: Some(Default::default()),
        },
        properties(Default::default()),
        properties(RuntimeDiminishedReturns {
            enabled: Some(false),
            sliding_window_duration: Some(Duration::ZERO),
            minimum_improvement_ratio: Some(f64::NAN),
        }),
    ] {
        let bp = p.clone();
        let bm = m.clone();
        assert_eq!(raw.bind(&mut p).unwrap().applied_local_phases, 0);
        assert_eq!(raw.bind_mixed(&mut m).unwrap().applied_local_phases, 0);
        assert_eq!(p, bp);
        assert_eq!(m, bm);
    }
    let before = p.clone();
    let mixed_before = m.clone();
    let raw = properties(RuntimeDiminishedReturns {
        enabled: Some(true),
        ..Default::default()
    });
    assert_eq!(raw.bind(&mut p).unwrap().applied_local_phases, 1);
    assert_eq!(raw.bind_mixed(&mut m).unwrap().applied_local_phases, 1);
    assert_eq!(p.phases[0], before.phases[0]);
    assert_eq!(p.phases[1], before.phases[1]);
    assert_eq!(p.limits, before.limits);
    assert_eq!(m.phases[0], mixed_before.phases[0]);
    assert_eq!(m.phases[1], mixed_before.phases[1]);
    assert_eq!(m.limits, mixed_before.limits);
    assert_eq!(raw.bind(&mut p).unwrap().applied_local_phases, 0);
    assert_eq!(raw.bind_mixed(&mut m).unwrap().applied_local_phases, 0);
    let bp = p.clone();
    let bm = m.clone();
    let off = properties(RuntimeDiminishedReturns {
        enabled: Some(false),
        ..Default::default()
    });
    off.bind(&mut p).unwrap();
    off.bind_mixed(&mut m).unwrap();
    assert_eq!(p, bp);
    assert_eq!(m, bm);
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
fn runtime_defaults_windows_and_ratios_stop_actual_dense_and_stream_local_phases() {
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
            assert_eq!(ratio, 123.0);
            assert_eq!(receipt.resolved.unwrap().ratio(), (123, 1));
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
fn mixed_runtime_binding_drives_both_real_backends_without_touching_list_state() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..100).collect()).with_initial(0)]).unwrap(),
        ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![8, 7])]).unwrap(),
    );
    for enabled in [true, false] {
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
