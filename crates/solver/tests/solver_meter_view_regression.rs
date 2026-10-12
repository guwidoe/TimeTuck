//! Original exporter values, failure history, validation and concurrency checks.
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};
use timetuck::*;
fn meter_model() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap()
}
fn meter_monitor(p: &FiniteProblem<i32>) -> SolverMetrics<SimpleScore> {
    SolverMetrics::new(Some((&p.problem_size_statistics()).into()))
}
fn meter_config() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
        ..Default::default()
    }
}
#[test]
fn actual_values_use_monitor_counts_scale_and_monotonic_duration_without_rescoring() {
    let p = meter_model();
    let now = Arc::new(AtomicU64::new(0));
    let clock = now.clone();
    let metrics =
        SolverMetrics::with_clock(Some((&p.problem_size_statistics()).into()), move || {
            Duration::from_millis(clock.load(Ordering::SeqCst))
        });
    let mut view = SolverMeterView::for_problem(metrics.clone(), "test", &p);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("ones", vec![0, 1], |v| {
        SimpleScore::new(v.iter().filter_map(|v| *v).map(|v| i64::from(*v)).sum())
    })
    .unwrap();
    let mut observations = 0;
    let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
        if let SolverEvent::Best(_) = event {
            now.store(2500, Ordering::SeqCst);
            let readings = view.publish_solver_meters().unwrap();
            let source = metrics.snapshot();
            assert_eq!(readings.len(), 8);
            let value = |name| readings.iter().find(|r| r.id.name == name).unwrap().value;
            assert_eq!(value("timefold.solver.solve.duration"), 2.5);
            assert_eq!(value("timefold.solver.errors"), 0.0);
            assert_eq!(
                value("timefold.solver.score.calculation.count"),
                source.active[0].work.evaluations as f64
            );
            assert_eq!(
                value("timefold.solver.move.evaluation.count"),
                source.active[0].move_evaluations.total() as f64
            );
            assert_eq!(value("timefold.solver.problem.entities"), 2.0);
            assert_eq!(value("timefold.solver.problem.variables"), 2.0);
            assert_eq!(value("timefold.solver.problem.values"), 2.0);
            assert!((value("timefold.solver.problem.size.log") - 4_f64.log10()).abs() < 1e-15);
            now.store(1000, Ordering::SeqCst);
            assert_eq!(view.current_solver_meters().unwrap()[0].value, 2.5);
            observations += 1;
        }
    };
    let observed = solve_with_extensions(
        &p,
        &n,
        meter_config(),
        SolverExtensions {
            metrics: Some(&metrics),
            events: Some(&mut events),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(observations > 0);
    let plain = solve(&p, &n, meter_config()).unwrap();
    assert_eq!(observed.assignment, plain.assignment);
    assert_eq!(observed.score, plain.score);
    assert_eq!(observed.work, plain.work);
    assert_eq!(view.current_solver_meters().unwrap().len(), 2);
    assert_eq!(view.current_solver_meters().unwrap()[0].value, 0.0);
}
#[test]
fn failed_run_counter_survives_fresh_monitor_rebinding_and_cannot_be_double_counted() {
    let p = meter_model();
    let metrics = meter_monitor(&p);
    let mut view = SolverMeterView::for_problem(metrics.clone(), "test", &p);
    let mut bad = ConstraintNetwork::<i32, SimpleScore>::new(2);
    bad.add_constraint("fail", vec![0], |_| panic!("scoring failed"))
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| solve_with_extensions(
            &p,
            &bad,
            meter_config(),
            SolverExtensions {
                metrics: Some(&metrics),
                ..Default::default()
            }
        )))
        .is_err()
    );
    let readings = view.publish_solver_meters().unwrap();
    assert_eq!(readings.len(), 2);
    assert_eq!(readings[1].value, 1.0);
    assert_eq!(
        view.rebind_problem_monitor(metrics.clone(), &p),
        Err(SolverMeterViewError::UsedRebind)
    );
    let next = meter_monitor(&p);
    view.rebind_problem_monitor(next.clone(), &p).unwrap();
    solve_with_extensions(
        &p,
        &ConstraintNetwork::<_, SimpleScore>::new(2),
        meter_config(),
        SolverExtensions {
            metrics: Some(&next),
            ..Default::default()
        },
    )
    .unwrap();
    let current = view.current_solver_meters().unwrap();
    assert_eq!(current.len(), 2);
    assert_eq!(current[1].value, 1.0);
}
#[test]
fn wrong_dimensions_and_active_rebinding_are_rejected_without_publishing_samples() {
    let p = meter_model();
    let wrong = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let metrics = meter_monitor(&p);
    let mut view = SolverMeterView::for_problem(metrics.clone(), "test", &wrong);
    let fresh = meter_monitor(&p);
    let mut tested = false;
    let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
        if matches!(event, SolverEvent::Best(_)) {
            assert_eq!(
                view.publish_solver_meters(),
                Err(SolverMeterViewError::DimensionMismatch)
            );
            assert_eq!(
                view.rebind_problem_monitor(fresh.clone(), &p),
                Err(SolverMeterViewError::ActiveRebind)
            );
            tested = true;
        }
    };
    solve_with_extensions(
        &p,
        &ConstraintNetwork::<_, SimpleScore>::new(2),
        meter_config(),
        SolverExtensions {
            metrics: Some(&metrics),
            events: Some(&mut events),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(tested);
    assert_eq!(view.current_solver_meters().unwrap().len(), 2);
}
#[test]
fn concurrent_runs_reject_ambiguous_gauges_instead_of_averaging_them() {
    let p = meter_model();
    let metrics = meter_monitor(&p);
    let view = SolverMeterView::for_problem(metrics.clone(), "same-id", &p);
    let barrier = Arc::new(Barrier::new(3));
    thread::scope(|scope| {
        for _ in 0..2 {
            let barrier = barrier.clone();
            let metrics = metrics.clone();
            let p = p.clone();
            scope.spawn(move || {
                let mut waiting = false;
                let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
                    if matches!(event, SolverEvent::Best(_)) && !waiting {
                        waiting = true;
                        barrier.wait();
                        barrier.wait();
                    }
                };
                solve_with_extensions(
                    &p,
                    &ConstraintNetwork::<_, SimpleScore>::new(2),
                    meter_config(),
                    SolverExtensions {
                        metrics: Some(&metrics),
                        events: Some(&mut events),
                        ..Default::default()
                    },
                )
                .unwrap();
            });
        }
        barrier.wait();
        assert_eq!(
            view.current_solver_meters(),
            Err(SolverMeterViewError::ConcurrentProblemRuns)
        );
        barrier.wait();
    });
    assert_eq!(metrics.snapshot().completed_runs, 2);
    assert_eq!(view.current_solver_meters().unwrap().len(), 2);
}
