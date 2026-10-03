//! Original callback ordering, isolation, cancellation and shutdown regressions.
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use timetuck::*;
const LIMIT: Duration = Duration::from_secs(10);
fn result<R, E>(job: &SolverJob<R, E>) -> SolverJobResult<R, E> {
    job.wait_timeout(LIMIT).expect("job timed out")
}
#[test]
fn ordered_callbacks_use_separate_threads_and_wait_includes_delivery() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let start = events.clone();
    let solve = events.clone();
    let done = events.clone();
    let (tx, rx) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit_with_callbacks(
            9,
            move |_| {
                solve
                    .lock()
                    .unwrap()
                    .push(("solve", std::thread::current().name().unwrap().to_owned()));
                Ok(42)
            },
            SolverJobCallbacks::default()
                .on_started(move |id| {
                    assert_eq!(id, 9);
                    start
                        .lock()
                        .unwrap()
                        .push(("start", std::thread::current().name().unwrap().to_owned()));
                })
                .on_final(move |id, value| {
                    assert_eq!((id, *value), (9, 42));
                    done.lock()
                        .unwrap()
                        .push(("final", std::thread::current().name().unwrap().to_owned()));
                    tx.send(()).unwrap();
                    hold.recv_timeout(LIMIT).unwrap();
                }),
        )
        .unwrap();
    rx.recv_timeout(LIMIT).unwrap();
    assert_eq!(job.status(), SolverStatus::Active);
    assert!(job.try_result().is_none());
    assert!(job.wait_timeout(Duration::ZERO).is_none());
    assert!(matches!(
        manager.submit(9, |_| Ok(0)),
        Err(SubmitError::AlreadySolving)
    ));
    // A blocked final callback does not occupy the sole solver thread.
    let other = manager.submit(10, |_| Ok(10)).unwrap();
    assert_eq!(*result(&other).as_ref().as_ref().unwrap(), 10);
    release.send(()).unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 42);
    let events = events.lock().unwrap();
    assert_eq!(
        events.iter().map(|x| x.0).collect::<Vec<_>>(),
        vec!["start", "solve", "final"]
    );
    assert!(events[0].1.starts_with("timetuck-solver-"));
    assert_eq!(events[0].1, events[1].1);
    assert!(events[2].1.starts_with("timetuck-consumer-"));
    manager.shutdown().unwrap();
}
#[test]
fn started_and_final_panics_report_each_cause_without_changing_success() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let errors = seen.clone();
    let job = manager
        .submit_with_callbacks(
            1,
            |_| Ok(7),
            SolverJobCallbacks::default()
                .on_started(|_| panic!("start failure"))
                .on_final(|_, _| panic!("final failure"))
                .on_error(move |id, cause| {
                    assert_eq!(id, 1);
                    assert!(
                        std::thread::current()
                            .name()
                            .unwrap()
                            .starts_with("timetuck-consumer-")
                    );
                    if let SolverJobFailure::Callback(f) = cause {
                        errors.lock().unwrap().push(f.clone());
                    } else {
                        panic!("unexpected solve error")
                    }
                }),
        )
        .unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 7);
    assert_eq!(job.callback_failures(), *seen.lock().unwrap());
    assert_eq!(
        job.callback_failures()
            .iter()
            .map(|x| x.stage)
            .collect::<Vec<_>>(),
        vec![SolverCallbackStage::Started, SolverCallbackStage::Final]
    );
    manager.shutdown().unwrap();
}
#[test]
fn failing_error_handler_is_recorded_and_disabled_without_recursive_notification() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let job = manager
        .submit_with_callbacks(
            1,
            |_| Err("solve error".into()),
            SolverJobCallbacks::default()
                .on_started(|_| panic!("start error"))
                .on_final(|_, _| panic!("failed solve has no final callback"))
                .on_error(move |_, _| {
                    seen.fetch_add(1, Ordering::SeqCst);
                    panic!("handler error")
                }),
        )
        .unwrap();
    assert!(matches!(result(&job).as_ref(),Err(SolverJobError::Solve(e)) if e=="solve error"));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        job.callback_failures()
            .iter()
            .map(|f| f.stage)
            .collect::<Vec<_>>(),
        vec![
            SolverCallbackStage::Started,
            SolverCallbackStage::ErrorHandler
        ]
    );
    let next = manager
        .submit_with_callbacks(
            2,
            |_| Ok(8),
            SolverJobCallbacks::default().on_final(|_, v| assert_eq!(*v, 8)),
        )
        .unwrap();
    assert_eq!(*result(&next).as_ref().as_ref().unwrap(), 8);
    manager.shutdown().unwrap();
}
#[test]
fn solve_panics_are_delivered_once_and_success_callback_is_skipped() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let job=manager.submit_with_callbacks(4, |_|panic!("objective"),SolverJobCallbacks::default()
        .on_final(|_,_|panic!("unexpected final"))
        .on_error(move |id,cause| {assert_eq!(id,4);assert!(matches!(cause,SolverJobFailure::Solving(SolverJobError::Panicked(s)) if s=="objective"));seen.fetch_add(1,Ordering::SeqCst);})).unwrap();
    assert!(matches!(result(&job).as_ref(),Err(SolverJobError::Panicked(s)) if s=="objective"));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}
#[test]
fn queued_cancel_and_shutdown_skip_callbacks_but_active_delivery_is_drained() {
    let mut manager = SolverManager::<u64, String>::with_consumer_count(1, 3).unwrap();
    let (started, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let active = manager
        .submit_with_callbacks(
            1,
            move |_| {
                started.send(()).unwrap();
                hold.recv_timeout(LIMIT).unwrap();
                Ok(1)
            },
            SolverJobCallbacks::default().on_final(|_, v| assert_eq!(*v, 1)),
        )
        .unwrap();
    ready.recv_timeout(LIMIT).unwrap();
    let impossible = || {
        SolverJobCallbacks::default()
            .on_started(|_| panic!("queued start"))
            .on_final(|_, _| panic!("queued final"))
            .on_error(|_, _| panic!("queued error"))
    };
    let cancelled = manager
        .submit_with_callbacks(2, |_| panic!("queued solve"), impossible())
        .unwrap();
    assert!(cancelled.cancel());
    let closed = manager
        .submit_with_callbacks(3, |_| panic!("queued solve"), impossible())
        .unwrap();
    manager.request_shutdown();
    release.send(()).unwrap();
    // Joining three consumers verifies all exit after the last solver dispatch.
    manager.shutdown().unwrap();
    assert_eq!(*result(&active).as_ref().as_ref().unwrap(), 1);
    for job in [cancelled, closed] {
        assert!(matches!(
            result(&job).as_ref(),
            Err(SolverJobError::CancelledBeforeStart)
        ));
        assert!(job.callback_failures().is_empty());
    }
}
#[test]
fn consumer_parallelism_allows_two_blocked_final_callbacks() {
    let mut manager = SolverManager::<u64, String>::with_consumer_count(1, 2).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut jobs = Vec::new();
    let mut releases = Vec::new();
    for id in 0..2 {
        let tx = tx.clone();
        let (release, hold) = mpsc::channel();
        releases.push(release);
        jobs.push(
            manager
                .submit_with_callbacks(
                    id,
                    move |_| Ok(id),
                    SolverJobCallbacks::default().on_final(move |id, _| {
                        tx.send(id).unwrap();
                        hold.recv_timeout(LIMIT).unwrap();
                    }),
                )
                .unwrap(),
        );
    }
    let mut ids = vec![
        rx.recv_timeout(LIMIT).unwrap(),
        rx.recv_timeout(LIMIT).unwrap(),
    ];
    ids.sort();
    assert_eq!(ids, vec![0, 1]);
    for release in releases {
        release.send(()).unwrap();
    }
    for job in jobs {
        assert_eq!(*result(&job).as_ref().as_ref().unwrap(), job.problem_id());
    }
    manager.shutdown().unwrap();
}
#[test]
fn final_callback_can_reenter_manager_cancel_itself_and_submit_other_work() {
    let manager = Arc::new(SolverManager::<u64, String>::new(1).unwrap());
    let inside = Arc::downgrade(&manager);
    let job = manager
        .submit_with_callbacks(
            1,
            |_| Ok(1),
            SolverJobCallbacks::default().on_final(move |id, _| {
                let manager = inside.upgrade().unwrap();
                assert_eq!(manager.status(id), SolverStatus::Active);
                assert!(manager.terminate_early(id));
                let other = manager.submit(2, |_| Ok(2)).unwrap();
                assert_eq!(*result(&other).as_ref().as_ref().unwrap(), 2);
            }),
        )
        .unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 1);
    assert!(job.callback_failures().is_empty());
    let mut manager = Arc::try_unwrap(manager).ok().unwrap();
    manager.shutdown().unwrap();
}
#[test]
fn consumer_self_shutdown_avoids_self_join() {
    let owner = Arc::new(Mutex::new(None::<SolverManager<u64, String>>));
    let inside = owner.clone();
    let manager = SolverManager::new(1).unwrap();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit_with_callbacks(
            1,
            |_| Ok(1),
            SolverJobCallbacks::default().on_final(move |_, _| {
                hold.recv_timeout(LIMIT).unwrap();
                let mut manager = inside.lock().unwrap().take().unwrap();
                assert_eq!(
                    manager.shutdown().unwrap_err().kind(),
                    std::io::ErrorKind::WouldBlock
                );
            }),
        )
        .unwrap();
    *owner.lock().unwrap() = Some(manager);
    release.send(()).unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 1);
    assert!(job.callback_failures().is_empty());
}

#[test]
fn consumer_counts_validate_and_wait_releases_callback_captures() {
    assert_eq!(
        SolverManager::<u64, String>::with_consumer_count(1, 0)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        SolverManager::<u64, String>::with_consumer_count(usize::MAX, 1)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let captured = Arc::new(1);
    let weak = Arc::downgrade(&captured);
    let job = manager
        .submit_with_callbacks(
            1,
            |_| Ok(1),
            SolverJobCallbacks::default().on_error(move |_, _| {
                assert_eq!(*captured, 1);
            }),
        )
        .unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 1);
    // Even an unused error callback is destroyed before publishing completion.
    assert!(weak.upgrade().is_none());
    manager.shutdown().unwrap();
}
