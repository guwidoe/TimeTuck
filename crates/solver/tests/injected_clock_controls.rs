//! Original clock-control integration cases; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn source() -> (Arc<AtomicU64>, SolverClock) {
    let ticks = Arc::new(AtomicU64::new(100));
    let a = ticks.clone();
    (
        ticks,
        SolverClock::new(move || ms(a.load(Ordering::SeqCst))),
    )
}
fn exact(strategy: ExactSearchStrategy) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy,
            max_time: Some(ms(5)),
            ..Default::default()
        })],
        ..Default::default()
    }
}
#[test]
fn phase_only_exact_clocks_charge_callbacks_and_rebase_between_solves() {
    let (ticks, clock) = source();
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("time", vec![0, 1, 2], |_| {
        ticks.fetch_add(7, Ordering::SeqCst);
        SimpleScore::ZERO
    })
    .unwrap();
    for epoch in [1000, 10, 500] {
        for strategy in [
            ExactSearchStrategy::Enumerate,
            ExactSearchStrategy::BranchAndBound(Default::default()),
            ExactSearchStrategy::Complete,
            ExactSearchStrategy::CompleteBranchAndBound(Default::default()),
        ] {
            ticks.store(epoch, Ordering::SeqCst);
            let r = solve_with_extensions(
                &p,
                &n,
                exact(strategy),
                SolverExtensions {
                    clock: Some(&clock),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(r.work.evaluations, 1);
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::Exact(ExactPhaseTermination::TimeLimit)
            );
            assert!(!r.is_optimal());
        }
    }
}
#[test]
fn construction_and_provider_initialization_share_clock_without_other_monitors() {
    let (ticks, clock) = source();
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("time", vec![0, 1], |_| {
        ticks.fetch_add(7, Ordering::SeqCst);
        SimpleScore::ZERO
    })
    .unwrap();
    for provider in [false, true] {
        let plan = SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig {
                max_time: Some(ms(5)),
                ..Default::default()
            })],
            ..Default::default()
        };
        let mut proposals = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(1),
            }]
        };
        let mut extensions = SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        };
        if provider {
            extensions.move_providers.bind(0, &mut proposals);
        }
        let r = solve_with_extensions(&p, &n, plan, extensions).unwrap();
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Construction(ConstructionTermination::TimeLimit)
        );
        assert!(!r.complete);
    }
}
#[test]
fn custom_phase_reuses_monotonic_high_water_and_stops_without_applying_move() {
    let (ticks, clock) = source();
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let mut command = |_: usize,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i32>,
        Move<i32>,
        SimpleScore,
        ScoringError,
    >| {
        ticks.store(105, Ordering::SeqCst);
        assert!(!c.is_terminated());
        ticks.store(90, Ordering::SeqCst);
        assert!(!c.is_terminated());
        ticks.store(109, Ordering::SeqCst);
        assert!(!c.is_terminated());
        ticks.store(110, Ordering::SeqCst);
        assert!(c.is_terminated());
        assert_eq!(
            c.execute(&Move::Change {
                variable: 0,
                value: Some(1)
            })?,
            None
        );
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig {
                max_time: Some(ms(10)),
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            clock: Some(&clock),
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.assignment, p.initial_assignment());
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Custom(CustomPhaseTermination::TimeLimit)
    );
}
#[test]
fn diagnostic_work_consumes_shared_time_without_search_evaluations() {
    let (ticks, clock) = source();
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let mut check = |_, _, _: &Assignment<i32>| {
        ticks.fetch_add(11, Ordering::SeqCst);
        Ok(Some(SimpleScore::ZERO))
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![
                SolverPhase::Custom(Default::default()),
                SolverPhase::Exact(Default::default()),
            ],
            limits: SolverLimits {
                max_time: Some(ms(10)),
                ..Default::default()
            },
        },
        SolverExtensions {
            clock: Some(&clock),
            command: Some(&mut |_, _| Ok(())),
            score_check: Some(&mut check),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.phases.len(), 1);
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(r.termination, SolverTermination::TimeLimit);
}
#[test]
fn clock_panics_propagate_and_a_following_solve_has_no_stale_session() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<SolverClock>();
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let clock = SolverClock::new(|| panic!("clock source failed"));
    assert!(
        catch_unwind(AssertUnwindSafe(|| solve_with_extensions(
            &p,
            &n,
            exact(ExactSearchStrategy::Enumerate),
            SolverExtensions {
                clock: Some(&clock),
                ..Default::default()
            }
        )))
        .is_err()
    );
    let calls = Cell::new(0);
    let mut good = ConstraintNetwork::new(1);
    good.add_constraint("count", vec![0], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let r = solve(&p, &good, exact(ExactSearchStrategy::Enumerate)).unwrap();
    assert!(r.is_optimal());
    assert_eq!(calls.get(), 2);
}
#[test]
fn phase_stagnation_and_rate_windows_use_injected_time() {
    let (ticks, clock) = source();
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("clock", vec![0], |_| {
        ticks.fetch_add(1, Ordering::SeqCst);
        SimpleScore::ZERO
    })
    .unwrap();
    let significant = SignificantImprovementConfig::new(ms(5), SimpleScore::new(1)).unwrap();
    for mode in 0..3 {
        for with_provider in [false, true] {
            let mut c = LocalSearchConfig {
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            };
            let expected = match mode {
                0 => {
                    c.max_unimproved_time = Some(ms(5));
                    LocalSearchTermination::UnimprovedTimeLimit
                }
                1 => {
                    c.significant_improvement = Some(&significant);
                    LocalSearchTermination::SignificantImprovementTimeLimit
                }
                _ => {
                    c.diminished_returns = Some(DiminishedReturnsConfig::new(ms(5), 1, 2).unwrap());
                    LocalSearchTermination::DiminishedReturns
                }
            };
            let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
                vec![Move::Change {
                    variable: 0,
                    value: Some(1 - a.values()[0].unwrap()),
                }]
            };
            let mut extensions = SolverExtensions {
                clock: Some(&clock),
                ..Default::default()
            };
            if with_provider {
                extensions.move_providers.bind(0, &mut provider);
            }
            let r = solve_with_extensions(
                &p,
                &n,
                SolverSequence {
                    phases: vec![SolverPhase::LocalSearch(c)],
                    ..Default::default()
                },
                extensions,
            )
            .unwrap();
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::LocalSearch(expected)
            );
            assert!(r.work.evaluations < 20);
            assert_eq!(r.assignment, p.initial_assignment());
        }
    }
}
#[test]
fn repair_stops_inside_reconstruction_without_score_count_monitor() {
    let (ticks, clock) = source();
    let calls = Cell::new(0);
    let p =
        FiniteProblem::new(vec![Variable::new(vec![0_i32, 1, 2, 3]).with_initial(0); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("rebuild clock", vec![0, 1, 2], |_| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            ticks.fetch_add(7, Ordering::SeqCst);
        }
        SimpleScore::ZERO
    })
    .unwrap();
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                max_time: Some(ms(5)),
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.move_evaluations, 1);
    assert_eq!(r.assignment, p.initial_assignment());
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::RuinRecreate(RuinRecreateTermination::TimeLimit)
    );
}
#[test]
fn stream_repair_factory_time_is_charged_before_reconstruction() {
    let (ticks, clock) = source();
    let calls = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]).with_initial(0); 2]).unwrap();
    let graph = || {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            ticks.fetch_add(7, Ordering::SeqCst);
        }
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = g.source::<usize, (usize, Option<i32>)>();
        g.score("zero", s.port(), |_| SimpleScore::ZERO)?;
        Ok((g.build(), s))
    };
    let r = solve_stream_with_extensions(
        &p,
        graph,
        SolverSequence {
            phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                max_time: Some(ms(5)),
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(r.work.move_evaluations, 0);
    assert_eq!(r.assignment, p.initial_assignment());
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::RuinRecreate(RuinRecreateTermination::TimeLimit)
    );
}
