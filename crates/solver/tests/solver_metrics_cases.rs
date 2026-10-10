// Adapted from pinned Community SolverMetricsIT.solveMetrics and solveMetricsError.
// Apache-2.0; see compat/provenance.json. Native snapshots replace Micrometer publication.
#[path = "lifecycle_support/mod.rs"]
mod lifecycle;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
#[test]
fn solve_metrics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let now = Arc::new(AtomicU64::new(0));
    let clock = now.clone();
    let metrics =
        SolverMetrics::with_clock(Some((&p.problem_size_statistics()).into()), move || {
            Duration::from_secs(clock.load(Ordering::SeqCst))
        });
    let mut observed = false;
    let mut callback = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
        if matches!(event, SolverEvent::Best(_)) && !observed {
            now.store(2, Ordering::SeqCst);
            let s = metrics.snapshot();
            assert_eq!(s.active.len(), 1);
            assert_eq!(s.active_duration(), Duration::from_secs(2));
            let d = s.active[0].dimensions.unwrap();
            assert_eq!(d.entities, 2);
            assert_eq!(d.variables, 2);
            assert_eq!(d.values, 2);
            observed = true;
        }
    };
    let solution = solve_with_extensions(
        &p,
        &n,
        lifecycle::config(),
        SolverExtensions {
            metrics: Some(&metrics),
            events: Some(&mut callback),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(observed);
    assert!(solution.assignment.values().iter().all(Option::is_some));
    let s = metrics.snapshot();
    assert_eq!(s.active_duration(), Duration::ZERO);
    assert!(s.active.is_empty());
    assert_eq!(s.failed_runs, 0);
    let last = s.latest.unwrap();
    assert!(last.work.evaluations > 0);
    assert!(last.work.move_evaluations > 0);
}
#[test]
fn solve_metrics_error() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let mut n = ConstraintNetwork::<_, SimpleScore>::new(2);
    n.add_constraint("failure", vec![0, 1], |_| {
        panic!("Thrown exception in constraint provider")
    })
    .unwrap();
    let now = Arc::new(AtomicU64::new(0));
    let clock = now.clone();
    let metrics = SolverMetrics::with_clock(None, move || {
        Duration::from_secs(clock.load(Ordering::SeqCst))
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        solve_with_extensions(
            &p,
            &n,
            lifecycle::config(),
            SolverExtensions {
                metrics: Some(&metrics),
                ..Default::default()
            },
        )
    }));
    let error = result.unwrap_err();
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.contains("Thrown exception in constraint provider"));
    now.store(1, Ordering::SeqCst);
    let s = metrics.snapshot();
    assert!(s.active.is_empty());
    assert_eq!(s.active_duration(), Duration::ZERO);
    assert_eq!(s.failed_runs, 1);
}
