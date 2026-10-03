//! Original concurrency, cancellation and ownership checks; no source-method credit.
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
fn blocked(
    manager: &SolverManager<u64, String>,
    id: u64,
) -> (SolverJob<u64, String>, mpsc::Sender<()>) {
    let (start, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit(id, move |_| {
            start.send(()).unwrap();
            hold.recv_timeout(LIMIT).unwrap();
            Ok(id)
        })
        .unwrap();
    ready.recv_timeout(LIMIT).unwrap();
    (job, release)
}
#[test]
fn queued_cancellation_releases_id_without_running_and_old_handles_are_safe() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let (active, release) = blocked(&manager, 1);
    let old = manager
        .submit(2, |_| panic!("cancelled queued closure must never run"))
        .unwrap();
    assert!(old.try_result().is_none());
    assert!(old.wait_timeout(Duration::ZERO).is_none());
    assert!(old.cancel());
    assert!(!old.cancel());
    assert!(matches!(
        result(&old).as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    let replacement = manager.submit(2, |_| Ok(22)).unwrap();
    assert!(!old.cancel());
    assert_eq!(replacement.status(), SolverStatus::Scheduled);
    assert!(matches!(
        manager.submit(2, |_| Ok(99)),
        Err(SubmitError::AlreadySolving)
    ));
    release.send(()).unwrap();
    assert_eq!(result(&active).as_ref().as_ref().unwrap(), &1);
    assert_eq!(result(&replacement).as_ref().as_ref().unwrap(), &22);
    assert!(!manager.terminate_early(2));
    assert_eq!(manager.status(999), SolverStatus::NotSolving);
    manager.shutdown().unwrap();
}
#[test]
fn active_cancellation_keeps_real_exact_search_best_and_is_not_preemptive() {
    let mut manager = SolverManager::<SearchResult<i64, SimpleScore>, String>::new(1).unwrap();
    let (start, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit(5, move |token| {
            let problem = FiniteProblem::new(vec![Variable::new(vec![7, 8, 9])]).unwrap();
            Ok(exhaustive_search_with_config(
                &problem,
                |a| {
                    start.send(()).unwrap();
                    hold.recv_timeout(LIMIT).unwrap();
                    SimpleScore::new(a.values()[0].unwrap())
                },
                ExactSearchConfig {
                    cancellation: Some(&token),
                    ..Default::default()
                },
            ))
        })
        .unwrap();
    ready.recv_timeout(LIMIT).unwrap();
    assert!(manager.terminate_early(5));
    assert!(!job.cancel());
    assert_eq!(job.status(), SolverStatus::Active);
    assert!(job.try_result().is_none());
    release.send(()).unwrap();
    let r = result(&job);
    let r = r.as_ref().as_ref().unwrap();
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.best.as_ref().unwrap().score, SimpleScore::new(7));
    manager.shutdown().unwrap();
}
#[test]
fn solve_errors_and_unwinding_panics_release_workers_and_ids() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let error = manager.submit(1, |_| Err("bad problem".into())).unwrap();
    assert!(matches!(result(&error).as_ref(),Err(SolverJobError::Solve(e)) if e=="bad problem"));
    let panic = manager.submit(1, |_| panic!("score panic")).unwrap();
    assert!(matches!(result(&panic).as_ref(),Err(SolverJobError::Panicked(e)) if e=="score panic"));
    let opaque = manager
        .submit(1, |_| std::panic::panic_any(17_u64))
        .unwrap();
    assert!(
        matches!(result(&opaque).as_ref(),Err(SolverJobError::Panicked(e)) if e=="non-string panic payload")
    );
    let good = manager.submit(1, |_| Ok(42)).unwrap();
    assert_eq!(result(&good).as_ref().as_ref().unwrap(), &42);
    for job in [error, panic, opaque, good] {
        assert_eq!(job.status(), SolverStatus::NotSolving);
    }
    manager.shutdown().unwrap();
}
#[test]
fn multiple_waiters_share_owned_results_without_clone_and_timeout_does_not_cancel() {
    struct Owned(u64);
    let mut manager = SolverManager::<Owned, String>::new(1).unwrap();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit(1, move |_| {
            hold.recv_timeout(LIMIT).unwrap();
            Ok(Owned(9))
        })
        .unwrap();
    assert!(job.wait_timeout(Duration::ZERO).is_none());
    let a = job.clone();
    let b = job.clone();
    let t1 = std::thread::spawn(move || result(&a));
    let t2 = std::thread::spawn(move || result(&b));
    release.send(()).unwrap();
    let a = t1.join().unwrap();
    let b = t2.join().unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert!(Arc::ptr_eq(&a, &job.wait()));
    assert_eq!(a.as_ref().as_ref().ok().unwrap().0, 9);
    manager.shutdown().unwrap();
}
#[test]
fn shutdown_cancels_queue_requests_active_cancellation_and_rejects_submissions() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let (start, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let active = manager
        .submit(1, move |token| {
            start.send(()).unwrap();
            hold.recv_timeout(LIMIT).unwrap();
            assert!(token.is_cancelled());
            Ok(1)
        })
        .unwrap();
    ready.recv_timeout(LIMIT).unwrap();
    let queued = manager.submit(2, |_| panic!("closed queue ran")).unwrap();
    manager.request_shutdown();
    manager.request_shutdown();
    assert!(matches!(
        manager.submit(3, |_| Ok(3)),
        Err(SubmitError::Closed)
    ));
    assert!(matches!(
        result(&queued).as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    assert_eq!(active.status(), SolverStatus::Active);
    release.send(()).unwrap();
    manager.shutdown().unwrap();
    manager.shutdown().unwrap();
    assert_eq!(result(&active).as_ref().as_ref().unwrap(), &1);
}
#[test]
fn dropping_manager_is_nonblocking_and_handles_outlive_it() {
    let manager = SolverManager::<u64, String>::new(1).unwrap();
    let (active, release) = blocked(&manager, 1);
    let queued = manager
        .submit(2, |_| panic!("dropped manager ran queue"))
        .unwrap();
    drop(manager);
    assert!(matches!(
        result(&queued).as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    assert!(active.try_result().is_none());
    release.send(()).unwrap();
    assert_eq!(result(&active).as_ref().as_ref().unwrap(), &1);
}
#[test]
fn fifo_order_and_concurrency_cap_hold_under_concurrent_submission() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let (first, release) = blocked(&manager, 0);
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut jobs = Vec::new();
    for id in 1..25 {
        let order = order.clone();
        jobs.push(
            manager
                .submit(id, move |_| {
                    order.lock().unwrap().push(id);
                    Ok(id)
                })
                .unwrap(),
        );
    }
    release.send(()).unwrap();
    result(&first);
    for job in jobs {
        result(&job);
    }
    assert_eq!(*order.lock().unwrap(), (1..25).collect::<Vec<_>>());
    manager.shutdown().unwrap();
    let manager = Arc::new(SolverManager::<u64, String>::new(2).unwrap());
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..4)
        .map(|producer| {
            let manager = manager.clone();
            let active = active.clone();
            let maximum = maximum.clone();
            std::thread::spawn(move || {
                (0..25)
                    .map(|i| {
                        let active = active.clone();
                        let maximum = maximum.clone();
                        let id = producer * 25 + i;
                        manager
                            .submit(id, move |_| {
                                let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                                maximum.fetch_max(n, Ordering::SeqCst);
                                std::thread::yield_now();
                                active.fetch_sub(1, Ordering::SeqCst);
                                Ok(id)
                            })
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let jobs: Vec<_> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    for job in jobs {
        assert_eq!(*result(&job).as_ref().as_ref().unwrap(), job.problem_id());
    }
    assert!(maximum.load(Ordering::SeqCst) <= 2);
    assert_eq!(active.load(Ordering::SeqCst), 0);
    let mut manager = Arc::try_unwrap(manager).ok().unwrap();
    manager.shutdown().unwrap();
}
#[test]
fn shutdown_from_own_worker_returns_error_and_drop_cannot_self_join() {
    let owner = Arc::new(Mutex::new(
        None::<SolverManager<std::io::ErrorKind, String>>,
    ));
    let manager = SolverManager::new(1).unwrap();
    let inside = owner.clone();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit(1, move |_| {
            hold.recv_timeout(LIMIT).unwrap();
            let mut manager = inside.lock().unwrap().take().unwrap();
            Ok(manager.shutdown().unwrap_err().kind())
        })
        .unwrap();
    *owner.lock().unwrap() = Some(manager);
    release.send(()).unwrap();
    assert_eq!(
        *result(&job).as_ref().as_ref().unwrap(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn zero_workers_rejected_and_dropping_job_handle_does_not_cancel() {
    assert_eq!(
        SolverManager::<u64, String>::new(0).err().unwrap().kind(),
        std::io::ErrorKind::InvalidInput
    );
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let (tx, rx) = mpsc::channel();
    drop(
        manager
            .submit(1, move |token| {
                assert!(!token.is_cancelled());
                tx.send(()).unwrap();
                Ok(1)
            })
            .unwrap(),
    );
    rx.recv_timeout(LIMIT).unwrap();
    manager.shutdown().unwrap();
}

#[test]
fn panic_payload_destructor_cannot_kill_worker_or_strand_next_job() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("payload destructor")
        }
    }
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let bad = manager
        .submit(1, |_| std::panic::panic_any(PanicOnDrop))
        .unwrap();
    let next = manager.submit(2, |_| Ok(2)).unwrap();
    assert!(
        matches!(result(&bad).as_ref(),Err(SolverJobError::Panicked(s)) if s=="non-string panic payload")
    );
    assert_eq!(*result(&next).as_ref().as_ref().unwrap(), 2);
    manager.shutdown().unwrap();
}
#[test]
fn queued_input_destructors_may_reenter_manager_and_finished_results_are_released() {
    struct Reenter {
        manager: Arc<SolverManager<u64, String>>,
        dropped: mpsc::Sender<()>,
    }
    impl Drop for Reenter {
        fn drop(&mut self) {
            assert_eq!(self.manager.status(2), SolverStatus::NotSolving);
            self.dropped.send(()).unwrap();
        }
    }
    let manager = Arc::new(SolverManager::<u64, String>::new(1).unwrap());
    let (first, release) = blocked(&manager, 1);
    let (tx, rx) = mpsc::channel();
    let input = Reenter {
        manager: manager.clone(),
        dropped: tx,
    };
    let queued = manager
        .submit(2, move |_| {
            drop(input);
            Ok(2)
        })
        .unwrap();
    let cancel = queued.clone();
    let controller = std::thread::spawn(move || assert!(cancel.cancel()));
    rx.recv_timeout(LIMIT).unwrap();
    controller.join().unwrap();
    release.send(()).unwrap();
    result(&first);
    let mut manager = Arc::try_unwrap(manager).ok().unwrap();
    manager.shutdown().unwrap();
    let mut manager = SolverManager::<Arc<u64>, String>::new(1).unwrap();
    let value = Arc::new(42);
    let weak = Arc::downgrade(&value);
    let job = manager.submit(1, move |_| Ok(value)).unwrap();
    let r = result(&job);
    manager.shutdown().unwrap();
    assert!(weak.upgrade().is_some());
    drop(job);
    drop(r);
    assert!(weak.upgrade().is_none());
}

#[test]
fn abandoned_result_destructor_cannot_strand_later_jobs() {
    struct ResultDrop(bool);
    impl Drop for ResultDrop {
        fn drop(&mut self) {
            assert!(!self.0, "result destructor");
        }
    }
    let mut manager = SolverManager::<ResultDrop, String>::new(1).unwrap();
    let (release, hold) = mpsc::channel();
    let abandoned = manager
        .submit(1, move |_| {
            hold.recv_timeout(LIMIT).unwrap();
            Ok(ResultDrop(true))
        })
        .unwrap();
    drop(abandoned);
    let next = manager.submit(2, |_| Ok(ResultDrop(false))).unwrap();
    release.send(()).unwrap();
    assert!(!result(&next).as_ref().as_ref().ok().unwrap().0);
    manager.shutdown().unwrap();
}
