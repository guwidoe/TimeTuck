//! Original live-delivery concurrency and exact-search integration regressions.
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const LIMIT: Duration = Duration::from_secs(10);
fn result<R, E>(job: &SolverJob<R, E>) -> SolverJobResult<R, E> {
    job.wait_timeout(LIMIT).expect("job timed out")
}
#[test]
fn slow_consumer_coalesces_live_updates_serially_and_shutdown_drains_latest() {
    let mut manager = SolverManager::<u64, String>::with_consumer_count(1, 3).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let best = events.clone();
    let final_seen = events.clone();
    let (entered, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let (published, all_published) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            move |_, progress| {
                assert!(progress.publish(1));
                ready.recv_timeout(LIMIT).unwrap();
                for value in 2..=100 {
                    assert!(progress.publish(value));
                }
                published.send(()).unwrap();
                Ok(100)
            },
            SolverJobCallbacks::default()
                .on_best(move |id, value| {
                    assert_eq!(id, 1);
                    assert!(
                        std::thread::current()
                            .name()
                            .unwrap()
                            .starts_with("timetuck-consumer-")
                    );
                    best.lock().unwrap().push((*value, false));
                    if *value == 1 {
                        entered.send(()).unwrap();
                        hold.recv_timeout(LIMIT).unwrap();
                    }
                })
                .on_final(move |_, value| final_seen.lock().unwrap().push((*value, true))),
        )
        .unwrap();
    all_published.recv_timeout(LIMIT).unwrap();
    // Completion of an independent job proves solving is free despite delivery.
    assert!(result(&manager.submit(2, |_| Ok(2)).unwrap()).is_ok());
    assert!(job.try_result().is_none());
    assert_eq!(job.status(), SolverStatus::Active);
    manager.request_shutdown();
    release.send(()).unwrap();
    manager.shutdown().unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 100);
    assert!(job.callback_failures().is_empty());
    assert_eq!(
        *events.lock().unwrap(),
        vec![(1, false), (100, false), (100, true)]
    );
}
#[test]
fn publishers_have_no_clone_bound_and_cannot_touch_reused_ids() {
    struct Value(u64);
    let mut manager = SolverManager::<Value, ()>::new(1).unwrap();
    let (tx, rx) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            4,
            move |_, p| {
                tx.send(p.clone()).unwrap();
                assert!(!p.publish(Value(1))); // No listener.
                Ok(Value(2))
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    assert_eq!(result(&job).as_ref().as_ref().unwrap().0, 2);
    let old = rx.recv_timeout(LIMIT).unwrap();
    let (tx, rx) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job2 = manager
        .submit_with_progress(
            4,
            move |_, p| {
                tx.send(()).unwrap();
                hold.recv_timeout(LIMIT).unwrap();
                assert!(p.publish(Value(3)));
                Ok(Value(4))
            },
            SolverJobCallbacks::default().on_best(|_, v: &Value| assert_eq!(v.0, 3)),
        )
        .unwrap();
    rx.recv_timeout(LIMIT).unwrap();
    assert!(!old.publish(Value(999)));
    release.send(()).unwrap();
    assert_eq!(result(&job2).as_ref().as_ref().unwrap().0, 4);
    manager.shutdown().unwrap();
    assert!(!old.publish(Value(999)));
}
#[test]
fn best_failure_disables_listener_and_reports_each_cause_once_before_final() {
    let mut manager = SolverManager::<u64, String>::new(1).unwrap();
    let stages = Arc::new(Mutex::new(Vec::new()));
    let seen = stages.clone();
    let (tx, rx) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            move |_, p| {
                assert!(p.publish(1));
                rx.recv_timeout(LIMIT).unwrap();
                // A racing publish can still be accepted before disablement is published.
                p.publish(2);
                Ok(3)
            },
            SolverJobCallbacks::default()
                .on_started(|_| panic!("start failed"))
                .on_best(|_, _| panic!("best failed"))
                .on_error(move |_, failure| {
                    if let SolverJobFailure::Callback(f) = failure {
                        seen.lock().unwrap().push(f.stage);
                        if f.stage == SolverCallbackStage::Best {
                            tx.send(()).unwrap();
                        }
                    }
                })
                .on_final(|_, v| assert_eq!(*v, 3)),
        )
        .unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 3);
    assert_eq!(
        *stages.lock().unwrap(),
        vec![SolverCallbackStage::Started, SolverCallbackStage::Best]
    );
    assert_eq!(
        job.callback_failures()
            .iter()
            .map(|f| f.stage)
            .collect::<Vec<_>>(),
        *stages.lock().unwrap()
    );
    manager.shutdown().unwrap();
}
#[test]
fn solve_error_follows_last_update_and_error_handler_panic_does_not_strand_job() {
    let mut manager = SolverManager::<u64, &'static str>::new(1).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let b = seen.clone();
    let e = seen.clone();
    let job = manager
        .submit_with_progress(
            1,
            |_, p| {
                p.publish(2);
                Err("solver")
            },
            SolverJobCallbacks::default()
                .on_best(move |_, v| b.lock().unwrap().push(*v))
                .on_final(|_, _| panic!("must not run"))
                .on_error(move |_, f| {
                    assert!(matches!(
                        f,
                        SolverJobFailure::Solving(SolverJobError::Solve("solver"))
                    ));
                    e.lock().unwrap().push(9);
                    panic!("handler");
                }),
        )
        .unwrap();
    assert!(matches!(
        result(&job).as_ref(),
        Err(SolverJobError::Solve("solver"))
    ));
    assert_eq!(*seen.lock().unwrap(), vec![2, 9]);
    assert_eq!(
        job.callback_failures()[0].stage,
        SolverCallbackStage::ErrorHandler
    );
    assert!(result(&manager.submit(2, |_| Ok(0)).unwrap()).is_ok());
    manager.shutdown().unwrap();
}
#[test]
fn overwritten_snapshot_destructor_can_reenter_manager_and_panic() {
    struct Value {
        n: u64,
        cleanup: Option<Box<dyn Fn() + Send + Sync>>,
    }
    impl Drop for Value {
        fn drop(&mut self) {
            if let Some(f) = &self.cleanup {
                f();
            }
        }
    }
    let manager = Arc::new(SolverManager::<Value, ()>::new(1).unwrap());
    let weak = Arc::downgrade(&manager);
    let (entered, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            move |_, p| {
                p.publish(Value {
                    n: 1,
                    cleanup: None,
                });
                ready.recv_timeout(LIMIT).unwrap();
                p.publish(Value {
                    n: 2,
                    cleanup: Some(Box::new(move || {
                        assert_eq!(weak.upgrade().unwrap().status(1), SolverStatus::Active);
                        panic!("replaced snapshot");
                    })),
                });
                assert!(p.publish(Value {
                    n: 3,
                    cleanup: None
                }));
                release.send(()).unwrap();
                Ok(Value {
                    n: 4,
                    cleanup: None,
                })
            },
            SolverJobCallbacks::default().on_best(move |_, v: &Value| {
                if v.n == 1 {
                    entered.send(()).unwrap();
                    hold.recv_timeout(LIMIT).unwrap();
                }
            }),
        )
        .unwrap();
    assert_eq!(result(&job).as_ref().as_ref().unwrap().n, 4);
    assert!(job.callback_failures().is_empty());
    Arc::try_unwrap(manager).ok().unwrap().shutdown().unwrap();
}
#[test]
fn real_exact_searches_publish_only_improvements_for_all_three_models() {
    let mut manager = SolverManager::<SimpleScore, String>::new(2).unwrap();
    for mode in 0..3 {
        let scores = Arc::new(Mutex::new(Vec::new()));
        let seen = scores.clone();
        let job = manager
            .submit_with_progress(
                mode,
                move |token, p| {
                    let basic = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2])]).unwrap();
                    let lists = ListProblem::new(
                        vec![0],
                        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
                    )
                    .unwrap();
                    let config = ExactSearchConfig {
                        cancellation: Some(&token),
                        ..Default::default()
                    };
                    let score = match mode {
                        0 => {
                            exhaustive_search_with_observer(
                                &basic,
                                |a| SimpleScore::new(a.values()[0].unwrap()),
                                config,
                                |b| {
                                    assert!(p.publish(b.score));
                                },
                            )
                            .best
                            .unwrap()
                            .score
                        }
                        1 => {
                            exhaustive_list_search_with_observer(
                                &lists,
                                |a| SimpleScore::new(a.location(&0).unwrap().entity as i64),
                                config,
                                |b| {
                                    assert!(p.publish(b.score));
                                },
                            )
                            .best
                            .unwrap()
                            .score
                        }
                        _ => {
                            exhaustive_mixed_search_with_observer(
                                &MixedProblem::new(basic, lists),
                                |a| {
                                    SimpleScore::new(
                                        a.basic().values()[0].unwrap()
                                            + 3 * a.lists().location(&0).unwrap().entity as i64,
                                    )
                                },
                                config,
                                |b| {
                                    assert!(p.publish(b.score));
                                },
                            )
                            .best
                            .unwrap()
                            .score
                        }
                    };
                    Ok(score)
                },
                SolverJobCallbacks::default().on_best(move |_, s| seen.lock().unwrap().push(*s)),
            )
            .unwrap();
        let r = result(&job);
        let scores = scores.lock().unwrap();
        assert!(!scores.is_empty());
        assert!(scores.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(scores.last().unwrap(), r.as_ref().as_ref().unwrap());
        assert!(job.callback_failures().is_empty());
    }
    manager.shutdown().unwrap();
}

#[test]
fn best_callback_can_cancel_and_republish_without_locks_or_concurrent_delivery() {
    let manager = Arc::new(SolverManager::<u64, ()>::with_consumer_count(1, 2).unwrap());
    let weak = Arc::downgrade(&manager);
    let (tx, rx) = mpsc::channel::<SolverProgress<u64>>();
    let (done, received) = mpsc::channel();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let events = seen.clone();
    let job = manager
        .submit_with_progress(
            1,
            move |token, p| {
                tx.send(p.clone()).unwrap();
                assert!(p.publish(1));
                received.recv_timeout(LIMIT).unwrap();
                assert!(token.is_cancelled());
                Ok(2)
            },
            SolverJobCallbacks::default().on_best(move |_, v| {
                events.lock().unwrap().push(*v);
                if *v == 1 {
                    assert!(weak.upgrade().unwrap().terminate_early(1));
                    assert!(rx.recv_timeout(LIMIT).unwrap().publish(2));
                } else {
                    assert_eq!(*v, 2);
                    done.send(()).unwrap();
                }
            }),
        )
        .unwrap();
    assert_eq!(*result(&job).as_ref().as_ref().unwrap(), 2);
    assert_eq!(*seen.lock().unwrap(), vec![1, 2]);
    assert!(job.callback_failures().is_empty());
    Arc::try_unwrap(manager).ok().unwrap().shutdown().unwrap();
}
