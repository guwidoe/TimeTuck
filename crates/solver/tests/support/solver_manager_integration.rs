//! Original managed execution of all six actual solver entry points.
use super::*;
use std::{rc::Rc, sync::mpsc, time::Duration};
const LIMIT: Duration = Duration::from_secs(10);
#[test]
fn manager_runs_all_models_and_backends_with_worker_local_non_send_scoring() {
    let mut manager = SolverManager::<Outcome, Failure>::new(2).unwrap();
    let mut jobs = Vec::new();
    for mode in 0..6 {
        jobs.push(
            manager
                .submit(mode as u64, move |token| {
                    let calls = Rc::new(Cell::new(0));
                    let config = SolverPlan {
                        limits: SolverLimits {
                            cancellation: Some(&token),
                            ..Default::default()
                        },
                        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                            max_steps: 4,
                            assert_scores: true,
                            ..Default::default()
                        })],
                        ..Default::default()
                    };
                    let result = run_scored(
                        (mode, false, MixedConstructionOrder::BasicThenLists),
                        config,
                        &Cell::new(0),
                        None,
                        || calls.set(calls.get() + 1),
                        SimpleScore::new,
                    );
                    assert!(calls.get() > 0);
                    result
                })
                .unwrap(),
        );
    }
    for pair in jobs.chunks_exact(2) {
        let a = pair[0].wait_timeout(LIMIT).unwrap();
        let b = pair[1].wait_timeout(LIMIT).unwrap();
        let a = a.as_ref().as_ref().unwrap();
        let b = b.as_ref().as_ref().unwrap();
        assert_eq!(a.values, b.values);
        assert_eq!(a.score, b.score);
        assert_eq!(a.complete, b.complete);
        assert_eq!(a.work.evaluations, b.work.evaluations);
        assert_eq!(a.work.move_evaluations, b.work.move_evaluations);
        assert_eq!(a.work.completed_steps, b.work.completed_steps);
        assert_eq!(a.work.accepted_moves, b.work.accepted_moves);
        // The scoring backends have different constraint callback counts.
        assert_eq!(a.phases.len(), b.phases.len());
        for (x, y) in a.phases.iter().zip(&b.phases) {
            assert_eq!(x.score, y.score);
            assert_eq!(x.termination, y.termination);
        }
        assert_eq!(a.reason, b.reason);
        assert!(a.complete);
        assert_eq!(a.reason, SolverTermination::PlanCompleted);
    }
    manager.shutdown().unwrap();
}
#[test]
fn manager_cancellation_reaches_all_six_solver_configs_and_retains_valid_state() {
    let mut manager = SolverManager::<Outcome, Failure>::new(2).unwrap();
    for mode in 0..6 {
        let (started, ready) = mpsc::channel();
        let (release, hold) = mpsc::channel();
        let job = manager
            .submit(mode as u64, move |token| {
                let first = Cell::new(true);
                let config = SolverPlan {
                    limits: SolverLimits {
                        cancellation: Some(&token),
                        ..Default::default()
                    },
                    phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 20,
                        ..Default::default()
                    })],
                    ..Default::default()
                };
                run_scored(
                    (mode, true, MixedConstructionOrder::BasicThenLists),
                    config,
                    &Cell::new(0),
                    None,
                    || {
                        if first.replace(false) {
                            started.send(()).unwrap();
                            hold.recv_timeout(LIMIT).unwrap();
                        }
                    },
                    SimpleScore::new,
                )
            })
            .unwrap();
        ready.recv_timeout(LIMIT).unwrap();
        assert!(job.cancel());
        release.send(()).unwrap();
        let r = job.wait_timeout(LIMIT).unwrap();
        let r = r.as_ref().as_ref().unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert!(r.complete);
        assert_eq!(r.work.completed_steps, 0);
    }
    manager.shutdown().unwrap();
}
