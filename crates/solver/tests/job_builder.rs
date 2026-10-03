//! Original deferred-submission and attributed-delivery regressions.
use std::{
    cell::Cell,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
fn origin(phase: usize) -> SolverEventProducer {
    SolverEventProducer {
        phase,
        kind: SolverPhaseKind::LocalSearch,
    }
}
#[test]
fn deferred_lookup_runs_once_after_started_on_worker_with_distinct_owned_types() {
    // Cell is Send, not Sync; neither input nor output implements Clone.
    struct Input(Cell<usize>);
    struct Output(usize);
    let caller = thread::current().id();
    let calls = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(Mutex::new(vec![]));
    let start = events.clone();
    let find = events.clone();
    let solve = events.clone();
    let consumed = events.clone();
    let count = calls.clone();
    let mut manager = SolverManager::<Output, String>::new(1).unwrap();
    let size = FiniteProblem::new(vec![Variable::new(vec![1, 2])])
        .unwrap()
        .problem_size_statistics();
    let job = manager
        .job_builder(41, move |p: Input, _, progress| {
            solve.lock().unwrap().push("solve");
            progress.record_work(SolverWork {
                evaluations: 3,
                move_evaluations: 2,
                ..Default::default()
            });
            Ok(Output(p.0.get() + 1))
        })
        .with_problem_finder(move |id| {
            assert_ne!(thread::current().id(), caller);
            assert_eq!(id, 41);
            count.fetch_add(1, Ordering::SeqCst);
            find.lock().unwrap().push("find");
            Ok(Input(Cell::new(7)))
        })
        .with_problem_size(size)
        .with_callbacks(
            SolverJobCallbacks::default()
                .on_started(move |_| start.lock().unwrap().push("start"))
                .on_final(move |_, r: &Output| {
                    assert_eq!(r.0, 8);
                    consumed.lock().unwrap().push("final");
                }),
        )
        .run()
        .unwrap();
    assert_eq!(
        job.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap().0,
        8
    );
    assert!(job.callback_failures().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(*events.lock().unwrap(), ["start", "find", "solve", "final"]);
    let stats = job.statistics();
    assert_eq!(stats.work.unwrap().evaluations, 3);
    assert_eq!(
        stats.problem_size.unwrap().size.assignment_space.decimal(),
        "2"
    );
    manager.shutdown().unwrap();
}
#[test]
fn invalid_duplicate_closed_and_unused_builders_never_lookup_or_reserve_ids() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    assert!(matches!(
        manager.job_builder(1, |_: usize, _, _| Ok(1)).run(),
        Err(SolverJobBuildError::MissingProblem)
    ));
    assert_eq!(manager.status(1), SolverStatus::NotSolving);
    let (sent, seen) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let active = manager
        .submit(1, move |_| {
            sent.send(()).unwrap();
            resume.recv_timeout(WAIT).unwrap();
            Ok(1)
        })
        .unwrap();
    seen.recv_timeout(WAIT).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let result = manager
        .job_builder(1, |p, _, _| Ok(p))
        .with_problem_finder(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(2)
        })
        .run();
    assert!(matches!(
        result,
        Err(SolverJobBuildError::Submission(SubmitError::AlreadySolving))
    ));
    let count = calls.clone();
    drop(
        manager
            .job_builder(2, |p, _, _| Ok(p))
            .with_problem_finder(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(2)
            }),
    );
    assert_eq!(manager.status(2), SolverStatus::NotSolving);
    release.send(()).unwrap();
    assert!(active.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
    let count = calls.clone();
    assert!(matches!(
        manager
            .job_builder(3, |p, _, _| Ok(p))
            .with_problem_finder(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(3)
            })
            .run(),
        Err(SolverJobBuildError::Submission(SubmitError::Closed))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[test]
fn queued_cancellation_skips_lookup_and_callbacks_and_drops_replaced_owned_sources() {
    struct Input(Arc<AtomicUsize>);
    impl Drop for Input {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let active = manager
        .submit(1, move |_| {
            sent.send(()).unwrap();
            resume.recv_timeout(WAIT).unwrap();
            Ok(1)
        })
        .unwrap();
    seen.recv_timeout(WAIT).unwrap();
    let queued = manager
        .job_builder(2, |_: Input, _, _| panic!("must not solve"))
        .with_problem(Input(drops.clone()))
        .with_problem_finder(|_| panic!("must not find"))
        .with_problem(Input(drops.clone()))
        .with_callbacks(
            SolverJobCallbacks::default()
                .on_started(|_| panic!("must not start"))
                .on_error(|_, _| panic!("must not notify")),
        )
        .run()
        .unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(matches!(
        queued.cancel_and_wait().unwrap().as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    assert!(queued.callback_failures().is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    // A deferred finder is also skipped, independently of the direct source path.
    let lazy = manager
        .job_builder(3, |_: usize, _, _| panic!("must not solve"))
        .with_problem_finder(|_| panic!("must not find"))
        .run()
        .unwrap();
    assert!(matches!(
        lazy.cancel_and_wait().unwrap().as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    release.send(()).unwrap();
    assert!(active.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}
#[test]
fn finder_errors_and_panics_deliver_once_and_allow_reuse_without_running_solver() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    for panic in [false, true] {
        let errors = Arc::new(Mutex::new(vec![]));
        let seen = errors.clone();
        let job = manager
            .job_builder(1, |_: usize, _, _| panic!("solver must not run"))
            .with_problem_finder(move |_| {
                if panic {
                    panic!("lookup panic")
                } else {
                    Err("lookup error".into())
                }
            })
            .with_callbacks(SolverJobCallbacks::default().on_error(move |id, e| {
                assert_eq!(id, 1);
                let SolverJobFailure::Solving(e) = e else {
                    panic!("unexpected callback error")
                };
                seen.lock().unwrap().push(e.to_string());
            }))
            .run()
            .unwrap();
        let result = job.wait_timeout(WAIT).unwrap();
        match result.as_ref() {
            Err(SolverJobError::Solve(e)) if !panic => assert_eq!(e, "lookup error"),
            Err(SolverJobError::Panicked(e)) if panic => assert_eq!(e, "lookup panic"),
            other => panic!("unexpected result: {other:?}"),
        }
        assert_eq!(errors.lock().unwrap().len(), 1);
        assert!(job.callback_failures().is_empty());
        assert_eq!(job.status(), SolverStatus::NotSolving);
        assert_eq!(manager.status(1), SolverStatus::NotSolving);
    }
    let ok = manager
        .job_builder(1, |p, _, _| Ok(p))
        .with_problem(42)
        .run()
        .unwrap();
    assert_eq!(
        *ok.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap(),
        42
    );
    manager.shutdown().unwrap();
}
#[test]
fn coalescing_keeps_origin_and_snapshot_together_and_legacy_publication_is_unattributed() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let (entered, first) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (published, ready) = mpsc::channel();
    let (saved, retained) = mpsc::channel();
    let events = Arc::new(Mutex::new(vec![]));
    let seen = events.clone();
    let job = manager
        .job_builder(1, move |_: (), _, p| {
            saved.send(p.clone()).unwrap();
            assert!(p.publish_from(1, origin(1)));
            first.recv_timeout(WAIT).unwrap();
            assert!(p.publish_from(2, origin(2)));
            assert!(p.publish(3));
            published.send(()).unwrap();
            Ok(4)
        })
        .with_problem(())
        .with_callbacks(SolverJobCallbacks::default().on_best_event(move |_, e| {
            seen.lock().unwrap().push((*e.solution, e.producer));
            if *e.solution == 1 {
                entered.send(()).unwrap();
                resume.recv_timeout(WAIT).unwrap();
            }
        }))
        .run()
        .unwrap();
    ready.recv_timeout(WAIT).unwrap();
    release.send(()).unwrap();
    assert_eq!(
        *job.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap(),
        4
    );
    assert!(job.callback_failures().is_empty());
    assert_eq!(*events.lock().unwrap(), [(1, Some(origin(1))), (3, None)]);
    let stale = retained.recv_timeout(WAIT).unwrap();
    let next = manager.submit(1, |_| Ok(5)).unwrap();
    assert!(!stale.publish_from(9, origin(9)));
    assert!(next.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}
#[test]
fn attributed_callback_failure_disables_best_only_and_final_delivery_survives() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let (notified, failure) = mpsc::channel();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .job_builder(1, move |_: (), _, p| {
            assert!(p.publish_from(1, origin(1)));
            failure.recv_timeout(WAIT).unwrap();
            // The callback is disabled, but its consumer may not have re-locked state
            // yet. A concurrently accepted update is harmless and never delivered.
            p.publish_from(2, origin(2));
            Ok(3)
        })
        .with_problem(())
        .with_callbacks(
            SolverJobCallbacks::default()
                .on_best(|_, _| panic!("replaced callback"))
                .on_best_event(|_, e| {
                    assert_eq!(e.producer, Some(origin(1)));
                    panic!("best failed");
                })
                .on_error(move |_, e| {
                    assert!(matches!(e, SolverJobFailure::Callback(_)));
                    notified.send(()).unwrap();
                })
                .on_final(move |_, r| sent.send(*r).unwrap()),
        )
        .run()
        .unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 3);
    let failures = job.callback_failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].stage, SolverCallbackStage::Best);
    assert_eq!(failures[0].message, "best failed");
    manager.shutdown().unwrap();
}
