//! Original reliable initialization delivery and failure/concurrency regressions.
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const LIMIT: Duration = Duration::from_secs(10);
const INFO: SolverInitialization = SolverInitialization {
    phase: 1,
    kind: SolverPhaseKind::Construction,
    terminated_early: false,
};
fn wait<R, E>(job: &SolverJob<R, E>) -> SolverJobResult<R, E> {
    job.wait_timeout(LIMIT).expect("job timed out")
}
#[test]
fn initialization_is_not_coalesced_and_retained_publication_order_is_serial() {
    for best_after in [false, true] {
        let mut manager = SolverManager::<u64, String>::with_consumer_count(1, 3).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let best = events.clone();
        let init = events.clone();
        let done = events.clone();
        let (entered, ready) = mpsc::channel();
        let (release, hold) = mpsc::channel();
        let (sent, published) = mpsc::channel();
        let job = manager
            .submit_with_progress(
                1,
                move |_, p| {
                    assert!(p.publish(0));
                    ready.recv_timeout(LIMIT).unwrap();
                    assert!(p.publish(1));
                    assert!(p.publish_first_initialized(2, INFO));
                    assert!(!p.clone().publish_first_initialized(999, INFO));
                    if best_after {
                        for n in 3..100 {
                            assert!(p.publish(n));
                        }
                    }
                    sent.send(()).unwrap();
                    Ok(100)
                },
                SolverJobCallbacks::default()
                    .on_best(move |_, v| {
                        best.lock().unwrap().push(("best", *v));
                        if *v == 0 {
                            entered.send(()).unwrap();
                            hold.recv_timeout(LIMIT).unwrap();
                        }
                    })
                    .on_first_initialized(move |_, v, info| {
                        assert_eq!(info, INFO);
                        init.lock().unwrap().push(("initialized", *v));
                    })
                    .on_final(move |_, v| done.lock().unwrap().push(("final", *v))),
            )
            .unwrap();
        published.recv_timeout(LIMIT).unwrap();
        assert!(wait(&manager.submit(2, |_| Ok(2)).unwrap()).is_ok());
        assert!(job.try_result().is_none());
        assert_eq!(job.status(), SolverStatus::Active);
        manager.request_shutdown();
        release.send(()).unwrap();
        manager.shutdown().unwrap();
        assert_eq!(*wait(&job).as_ref().as_ref().unwrap(), 100);
        assert!(job.callback_failures().is_empty());
        assert_eq!(
            *events.lock().unwrap(),
            if best_after {
                vec![
                    ("best", 0),
                    ("initialized", 2),
                    ("best", 99),
                    ("final", 100),
                ]
            } else {
                vec![("best", 0), ("best", 1), ("initialized", 2), ("final", 100)]
            }
        );
    }
}
#[test]
fn initialization_waits_include_callback_and_late_publishers_cannot_touch_reused_ids() {
    struct Value(u64);
    let mut manager = SolverManager::<Value, ()>::with_consumer_count(1, 2).unwrap();
    let (sender, receiver) = mpsc::channel();
    let (entered, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            4,
            move |_, p| {
                sender.send(p.clone()).unwrap();
                assert!(p.publish_first_initialized(Value(1), INFO));
                Ok(Value(2))
            },
            SolverJobCallbacks::default().on_first_initialized(move |id, v: &Value, info| {
                assert_eq!(id, 4);
                assert_eq!(v.0, 1);
                assert_eq!(info, INFO);
                entered.send(()).unwrap();
                hold.recv_timeout(LIMIT).unwrap();
            }),
        )
        .unwrap();
    let old = receiver.recv_timeout(LIMIT).unwrap();
    ready.recv_timeout(LIMIT).unwrap();
    assert!(job.try_result().is_none());
    assert!(matches!(
        manager.submit(4, |_| Ok(Value(3))),
        Err(SubmitError::AlreadySolving)
    ));
    release.send(()).unwrap();
    assert_eq!(wait(&job).as_ref().as_ref().unwrap().0, 2);
    let (tx, rx) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            4,
            move |_, p| {
                tx.send(()).unwrap();
                hold.recv_timeout(LIMIT).unwrap();
                assert!(p.publish_first_initialized(Value(3), INFO));
                Ok(Value(4))
            },
            SolverJobCallbacks::default()
                .on_first_initialized(|_, v: &Value, _| assert_eq!(v.0, 3)),
        )
        .unwrap();
    rx.recv_timeout(LIMIT).unwrap();
    assert!(!old.publish_first_initialized(Value(99), INFO));
    release.send(()).unwrap();
    assert_eq!(wait(&job).as_ref().as_ref().unwrap().0, 4);
    let job = manager
        .submit_with_progress(
            5,
            |_, p| {
                assert!(!p.publish_first_initialized(Value(5), INFO));
                Ok(Value(6))
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    assert!(wait(&job).is_ok());
    manager.shutdown().unwrap();
    assert!(!old.publish_first_initialized(Value(99), INFO));
}
#[test]
fn initialization_panic_is_reported_once_and_best_final_delivery_continue() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let seen = errors.clone();
    let (init_tx, init_rx) = mpsc::channel();
    let (best_tx, best_rx) = mpsc::channel();
    let (final_tx, final_rx) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            move |_, p| {
                assert!(p.publish_first_initialized(1, INFO));
                init_rx.recv_timeout(LIMIT).unwrap();
                assert!(!p.publish_first_initialized(2, INFO));
                assert!(p.publish(3));
                Ok(4)
            },
            SolverJobCallbacks::default()
                .on_first_initialized(|_, _, _| panic!("initialization consumer"))
                .on_error(move |_, e| {
                    if let SolverJobFailure::Callback(f) = e {
                        seen.lock().unwrap().push(f.clone());
                        init_tx.send(()).unwrap();
                    }
                })
                .on_best(move |_, v| best_tx.send(*v).unwrap())
                .on_final(move |_, v| final_tx.send(*v).unwrap()),
        )
        .unwrap();
    assert_eq!(*wait(&job).as_ref().as_ref().unwrap(), 4);
    assert_eq!(best_rx.recv_timeout(LIMIT).unwrap(), 3);
    assert_eq!(final_rx.recv_timeout(LIMIT).unwrap(), 4);
    assert_eq!(
        job.callback_failures(),
        vec![SolverCallbackFailure {
            stage: SolverCallbackStage::FirstInitialized,
            message: "initialization consumer".into()
        }]
    );
    assert_eq!(*errors.lock().unwrap(), job.callback_failures());
    manager.shutdown().unwrap();
}
#[test]
fn published_initialization_survives_solve_error_and_handler_failure() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    for panics in [false, true] {
        let (tx, rx) = mpsc::channel();
        let job = manager
            .submit_with_progress(
                1,
                move |_, p| {
                    assert!(p.publish_first_initialized(1, INFO));
                    if panics {
                        panic!("solve failure")
                    }
                    Err("solve error".into())
                },
                SolverJobCallbacks::default()
                    .on_first_initialized(move |_, v, _| {
                        tx.send(*v).unwrap();
                        panic!("initialization failure")
                    })
                    .on_final(|_, _| panic!("must not deliver failed solve as final"))
                    .on_error(|_, _| panic!("handler failure")),
            )
            .unwrap();
        let result = wait(&job);
        assert!(if panics {
            matches!(result.as_ref(), Err(SolverJobError::Panicked(_)))
        } else {
            matches!(result.as_ref(), Err(SolverJobError::Solve(_)))
        });
        assert_eq!(rx.recv_timeout(LIMIT).unwrap(), 1);
        let failures = job.callback_failures();
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].stage, SolverCallbackStage::FirstInitialized);
        assert_eq!(failures[1].stage, SolverCallbackStage::ErrorHandler);
    }
    manager.shutdown().unwrap();
}
#[test]
fn concurrent_initialization_publishers_accept_exactly_one() {
    let mut manager = SolverManager::<usize, ()>::with_consumer_count(1, 3).unwrap();
    let (tx, rx) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            move |token, p| {
                let accepted = std::thread::scope(|scope| {
                    let handles: Vec<_> = (0..8)
                        .map(|n| {
                            let p = p.clone();
                            scope.spawn(move || p.publish_first_initialized(n, INFO))
                        })
                        .collect();
                    handles
                        .into_iter()
                        .map(|h| usize::from(h.join().unwrap()))
                        .sum::<usize>()
                });
                assert_eq!(accepted, 1);
                let winner = rx.recv_timeout(LIMIT).unwrap();
                token.cancel();
                assert!(p.publish(winner));
                Ok(winner)
            },
            SolverJobCallbacks::default()
                .on_first_initialized(move |_, v, _| tx.send(*v).unwrap())
                .on_best(|_, _| {}),
        )
        .unwrap();
    assert!(*wait(&job).as_ref().as_ref().unwrap() < 8);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}
