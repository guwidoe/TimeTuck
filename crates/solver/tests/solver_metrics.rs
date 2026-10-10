//! Original live-monitor, concurrent use, backend equivalence and failure checks.
mod metrics_support;
use metrics_support::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn config() -> SolverConfig<'static> {
    SolverConfig {
        local_search: vec![LocalSearchConfig {
            max_steps: 3,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn final_matches<A>(metrics: &SolverMetrics<SimpleScore>, r: &SolverResult<A, SimpleScore>) {
    let s = metrics.snapshot();
    assert!(s.active.is_empty());
    let last = s.latest.unwrap();
    assert_eq!(last.work, r.work);
    assert_eq!(last.move_evaluations.total(), r.work.move_evaluations);
    assert_eq!(last.best_score, r.score);
    assert_eq!(last.complete, r.complete);
    assert_eq!(
        last.outcome,
        Some(SolveMetricOutcome::Completed(r.termination))
    );
    assert_eq!(last.last_phase, r.phases.last().map(|p| p.index));
}
#[test]
fn metrics_preserve_assignments_scores_and_work_across_all_twelve_paths() {
    let bp = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    let lp = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2]).unwrap();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    let metrics = SolverMetrics::default();
    for backend in 0..4 {
        macro_rules! compare {
            ($observed:expr,$plain:expr) => {{
                let r = $observed;
                let plain = $plain;
                assert_eq!(r, plain);
                final_matches(&metrics, &r);
            }};
        }
        compare!(
            basic_run(&bp, backend, config(), None, Some(&metrics)),
            basic_run(&bp, backend, config(), None, None)
        );
        compare!(
            list_run(&lp, backend, None, Some(&metrics)),
            list_run(&lp, backend, None, None)
        );
        compare!(
            mixed_run(&mp, backend, None, Some(&metrics)),
            mixed_run(&mp, backend, None, None)
        );
    }
    assert_eq!(metrics.snapshot().completed_runs, 12);
    assert_eq!(metrics.snapshot().failed_runs, 0);
}
#[test]
fn live_best_phase_work_clock_regression_and_callback_time_are_visible() {
    let now = Arc::new(AtomicU64::new(100));
    let clock = now.clone();
    let metrics = SolverMetrics::with_clock(None, move || {
        Duration::from_millis(clock.load(Ordering::SeqCst))
    });
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2, 3]); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(v.iter().filter_map(|v| *v).map(|v| i64::from(*v)).sum())
    })
    .unwrap();
    let mut best_count = 0;
    let mut phases = vec![];
    let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
        if let SolverEvent::Best(u) = event {
            now.store(2100, Ordering::SeqCst);
            let s = metrics.snapshot();
            assert_eq!(s.active.len(), 1);
            assert_eq!(s.active[0].best_score, Some(u.score));
            assert_eq!(s.active[0].complete, u.complete);
            assert_eq!(s.active_duration(), Duration::from_secs(2));
            now.store(500, Ordering::SeqCst);
            assert_eq!(metrics.snapshot().active_duration(), Duration::from_secs(2));
            best_count += 1;
        }
    };
    let mut lifecycle = |e| {
        if let SolverLifecycleEvent::PhaseEnded { phase, .. } = e {
            let s = metrics.snapshot();
            assert_eq!(s.active[0].last_phase, Some(phase));
            phases.push(s.active[0].work);
        }
        if e == SolverLifecycleEvent::SolveEnded {
            now.store(3100, Ordering::SeqCst);
        }
    };
    let r = solve_with_extensions(
        &p,
        &n,
        config(),
        SolverExtensions {
            metrics: Some(&metrics),
            events: Some(&mut events),
            lifecycle: Some(&mut lifecycle),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(best_count > 0);
    assert_eq!(phases.len(), r.phases.len());
    assert_eq!(phases.last(), Some(&r.work));
    final_matches(&metrics, &r);
    assert_eq!(
        metrics.snapshot().latest.unwrap().duration,
        Duration::from_secs(3)
    );
}
#[test]
fn concurrent_runs_have_independent_identity_work_and_bounded_finished_history() {
    let metrics = SolverMetrics::<SimpleScore>::default();
    std::thread::scope(|scope| {
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let mut releases = vec![];
        let mut handles = vec![];
        for count in [2, 4] {
            let (tx, rx) = std::sync::mpsc::channel();
            releases.push(tx);
            let metrics = metrics.clone();
            let started = started_tx.clone();
            handles.push(scope.spawn(move || {
                let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); count]).unwrap();
                let n = ConstraintNetwork::<_, SimpleScore>::new(count);
                let mut lifecycle = |e| {
                    if e == SolverLifecycleEvent::SolveStarted {
                        started.send(()).unwrap();
                        rx.recv_timeout(Duration::from_secs(10)).unwrap();
                    }
                };
                solve_with_extensions(
                    &p,
                    &n,
                    config(),
                    SolverExtensions {
                        metrics: Some(&metrics),
                        lifecycle: Some(&mut lifecycle),
                        ..Default::default()
                    },
                )
                .unwrap()
            }));
        }
        for _ in 0..2 {
            started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        let s = metrics.snapshot();
        assert_eq!(s.active.len(), 2);
        assert_ne!(s.active[0].run, s.active[1].run);
        assert!(s.latest.is_none());
        releases[0].send(()).unwrap();
        let first = handles.remove(0).join().unwrap();
        let s = metrics.snapshot();
        assert_eq!(s.active.len(), 1);
        let first_snapshot = s.latest.unwrap();
        assert_eq!(first_snapshot.work, first.work);
        assert_eq!(
            first_snapshot.move_evaluations.total(),
            first.work.move_evaluations
        );
        assert_eq!(s.active[0].move_evaluations.total(), 0);
        releases[1].send(()).unwrap();
        let second = handles.remove(0).join().unwrap();
        final_matches(&metrics, &second);
        assert_eq!(metrics.snapshot().completed_runs, 2);
    });
}
#[test]
fn failed_providers_callbacks_and_end_callbacks_close_runs_without_hiding_panics() {
    for mode in 0..4 {
        let metrics = SolverMetrics::<SimpleScore>::default();
        let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap();
        let n = ConstraintNetwork::<_, SimpleScore>::new(1);
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            assert_ne!(mode, 1, "provider panic");
            vec![Move::Change {
                variable: 99,
                value: Some(2),
            }]
        };
        let mut lifecycle = |e| {
            assert!(
                !(mode == 2 && matches!(e, SolverLifecycleEvent::StepStarted { .. })),
                "step callback panic"
            );
            assert!(
                !(mode == 3 && e == SolverLifecycleEvent::SolveEnded),
                "end callback panic"
            );
        };
        let mut ext = SolverExtensions {
            metrics: Some(&metrics),
            lifecycle: Some(&mut lifecycle),
            ..Default::default()
        };
        ext.move_providers.bind(0, &mut provider);
        let c = SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig::default())],
            ..Default::default()
        };
        let result = catch_unwind(AssertUnwindSafe(|| solve_with_extensions(&p, &n, c, ext)));
        if mode == 0 {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        let s = metrics.snapshot();
        assert!(s.active.is_empty());
        assert_eq!(s.failed_runs, 1);
        assert_eq!(
            s.latest.unwrap().outcome,
            Some(if mode == 0 {
                SolveMetricOutcome::Failed
            } else {
                SolveMetricOutcome::Panicked
            })
        );
        // Reuse after either an error or panic, including a callback unwind.
        let r = solve_with_extensions(
            &p,
            &n,
            config(),
            SolverExtensions {
                metrics: Some(&metrics),
                ..Default::default()
            },
        )
        .unwrap();
        final_matches(&metrics, &r);
        assert_eq!(metrics.snapshot().completed_runs, 2);
    }
}
#[test]
fn invalid_plans_do_not_start_runs_and_frozen_cancelled_and_exact_runs_finish() {
    let metrics = SolverMetrics::<SimpleScore>::default();
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    assert!(
        solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![],
                ..Default::default()
            },
            SolverExtensions {
                metrics: Some(&metrics),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(metrics.snapshot().completed_runs, 0);
    assert!(metrics.snapshot().active.is_empty());
    let r = solve_with_extensions(
        &p,
        &n,
        config(),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    final_matches(&metrics, &r);
    assert!(r.phases.is_empty());
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let c = SolverConfig {
        limits: SolverLimits {
            cancellation: Some(&cancel),
            ..Default::default()
        },
        ..config()
    };
    let r = solve_with_extensions(
        &p,
        &n,
        c,
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    final_matches(&metrics, &r);
    assert_eq!(r.termination, SolverTermination::Cancelled);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    final_matches(&metrics, &r);
    assert!(r.is_optimal());
    assert_eq!(metrics.snapshot().failed_runs, 0);
}
