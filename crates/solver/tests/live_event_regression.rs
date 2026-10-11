//! Original live-job event, restart and receipt boundaries; no upstream credit.
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
const INIT: SolverInitialization = SolverInitialization {
    phase: 0,
    kind: SolverPhaseKind::Construction,
    terminated_early: true,
};
fn producer(phase: usize) -> SolverEventProducer {
    SolverEventProducer {
        phase,
        kind: SolverPhaseKind::LocalSearch,
    }
}

#[test]
fn coalescing_preserves_revision_origin_and_separate_initialization_across_restarts() {
    struct Owned(usize); // No Clone requirement or payload clones.
    let mut manager = LiveSolverManager::<usize, Owned, String>::with_consumer_count(1, 3).unwrap();
    let (entered, entering) = mpsc::channel();
    let (release_consumer, consumer_released) = mpsc::channel();
    let (published, publications) = mpsc::channel();
    let (restart, restarting) = mpsc::channel();
    let (finish, finishing) = mpsc::channel();
    let (sent, seen) = mpsc::channel();
    let initialized = sent.clone();
    let job = manager
        .submit_with_events(
            1,
            0,
            Default::default(),
            move |p, old, token, progress| {
                if *p == 0 {
                    assert!(old.is_none());
                    assert!(progress.publish_from(Owned(0), producer(0)));
                    entering.recv_timeout(WAIT).unwrap();
                    assert!(progress.publish_from(Owned(1), producer(1)));
                    assert!(progress.publish_first_initialized(Owned(2), INIT));
                    published.send(0).unwrap();
                    restarting.recv_timeout(WAIT).unwrap();
                    assert!(token.is_cancelled());
                    assert!(!progress.publish_from(Owned(999), producer(999)));
                    assert!(!progress.publish_first_initialized(Owned(999), INIT));
                    Ok(Owned(3))
                } else {
                    assert_eq!(*p, 1);
                    assert_eq!(old.unwrap().0, 3);
                    assert_eq!(progress.revision(), 1);
                    assert!(!progress.publish_first_initialized(Owned(999), INIT));
                    assert!(progress.publish_from(Owned(11), producer(11)));
                    assert!(progress.publish_from(Owned(12), producer(12)));
                    published.send(1).unwrap();
                    finishing.recv_timeout(WAIT).unwrap();
                    Ok(Owned(13))
                }
            },
            LiveSolverCallbacks::new(move |id, event: SolverBestEvent<'_, LiveSolution<Owned>>| {
                assert_eq!(id, 1);
                if event.solution.solution.0 == 0 {
                    entered.send(()).unwrap();
                    consumer_released.recv_timeout(WAIT).unwrap();
                }
                sent.send((
                    "best",
                    event.solution.revision,
                    event.solution.solution.0,
                    event.producer,
                ))
                .unwrap();
            })
            .on_first_initialized(move |id, snapshot, metadata| {
                assert_eq!(id, 1);
                assert_eq!(metadata, INIT);
                initialized
                    .send(("init", snapshot.revision, snapshot.solution.0, None))
                    .unwrap();
            }),
        )
        .unwrap();
    assert_eq!(publications.recv_timeout(WAIT).unwrap(), 0);
    let receipt = job
        .add_problem_change(|p| {
            *p = 1;
            Ok(())
        })
        .unwrap();
    restart.send(()).unwrap();
    assert_eq!(publications.recv_timeout(WAIT).unwrap(), 1);
    assert!(receipt.try_result().is_none());
    release_consumer.send(()).unwrap();
    assert_eq!(
        seen.recv_timeout(WAIT).unwrap(),
        ("best", 0, 0, Some(producer(0)))
    );
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), ("init", 0, 2, None));
    assert_eq!(
        seen.recv_timeout(WAIT).unwrap(),
        ("best", 1, 12, Some(producer(12)))
    );
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    assert!(job.try_result().is_none());
    finish.send(()).unwrap();
    let outcome = job.wait_timeout(WAIT).unwrap();
    assert_eq!(outcome.as_ref().as_ref().unwrap().solution.0, 13);
    // A returned result does not inherit the last intermediate producer.
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), ("best", 1, 13, None));
    assert!(seen.try_recv().is_err());
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn final_only_initialization_does_not_acknowledge_changes_or_wait_for_final_result() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (restart, restarting) = mpsc::channel();
    let (finish, finishing) = mpsc::channel();
    let (initialized, initialization) = mpsc::channel();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit_with_events(
            2,
            0,
            LiveSolverConfig {
                delivery: LiveDelivery::Final,
                ..Default::default()
            },
            move |p, _, _, progress| {
                assert!(!progress.publish_from(*p, producer(0)));
                if *p == 0 {
                    started.send(()).unwrap();
                    restarting.recv_timeout(WAIT).unwrap();
                } else {
                    assert!(progress.publish_first_initialized(10, INIT));
                    finishing.recv_timeout(WAIT).unwrap();
                }
                Ok(*p)
            },
            LiveSolverCallbacks::new(move |_, event: SolverBestEvent<'_, LiveSolution<usize>>| {
                assert!(event.producer.is_none());
                sent.send((event.solution.revision, *event.solution.solution))
                    .unwrap();
            })
            .on_first_initialized(move |_, snapshot, metadata| {
                assert_eq!(metadata, INIT);
                initialized
                    .send((snapshot.revision, *snapshot.solution))
                    .unwrap();
            }),
        )
        .unwrap();
    starting.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(|p| {
            *p = 1;
            Ok(())
        })
        .unwrap();
    restart.send(()).unwrap();
    assert_eq!(initialization.recv_timeout(WAIT).unwrap(), (1, 10));
    assert!(receipt.try_result().is_none());
    assert!(seen.try_recv().is_err());
    assert!(job.try_result().is_none());
    finish.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), (1, 1));
    assert!(seen.try_recv().is_err());
    manager.shutdown().unwrap();
}

#[test]
fn initialization_panic_fails_pending_receipts_and_stops_both_delivery_modes() {
    for delivery in [LiveDelivery::Best, LiveDelivery::Final] {
        let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
        let (started, starting) = mpsc::channel();
        let (restart, restarting) = mpsc::channel();
        let (finish, finishing) = mpsc::channel();
        let job = manager
            .submit_with_events(
                3,
                0,
                LiveSolverConfig {
                    daemon: true,
                    delivery,
                },
                move |p, _, token, progress| {
                    if *p == 0 {
                        started.send(()).unwrap();
                        restarting.recv_timeout(WAIT).unwrap();
                    } else {
                        assert!(progress.publish_first_initialized(1, INIT));
                        finishing.recv_timeout(WAIT).unwrap();
                        assert!(token.is_cancelled());
                        assert!(!progress.publish_from(999, producer(0)));
                        assert!(!progress.publish_first_initialized(999, INIT));
                    }
                    Ok(*p)
                },
                LiveSolverCallbacks::new(|_, _| {})
                    .on_first_initialized(|_, _, _| panic!("init failed")),
            )
            .unwrap();
        starting.recv_timeout(WAIT).unwrap();
        let receipt = job
            .add_problem_change(|p| {
                *p = 1;
                Ok(())
            })
            .unwrap();
        restart.send(()).unwrap();
        assert_eq!(
            receipt.wait_timeout(WAIT),
            Some(Err(ProblemChangeFailure::ConsumerPanicked(
                "init failed".into()
            )))
        );
        assert!(job.add_problem_change(|_| Ok(())).is_err());
        finish.send(()).unwrap();
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        let failures = job.callback_failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].stage, SolverCallbackStage::FirstInitialized);
        assert_eq!(failures[0].message, "init failed");
        // Later successful solution delivery cannot change failed receipt outcome.
        assert!(receipt.try_result().unwrap().is_err());
        manager.shutdown().unwrap();
    }
}

#[test]
fn initialization_survives_later_solve_failure_without_synthesizing_best_delivery() {
    for delivery in [LiveDelivery::Best, LiveDelivery::Final] {
        let mut manager = LiveSolverManager::<(), usize, String>::new(1).unwrap();
        let (sent, seen) = mpsc::channel();
        let job = manager
            .submit_with_events(
                4,
                (),
                LiveSolverConfig {
                    delivery,
                    ..Default::default()
                },
                |_, _, _, progress| {
                    assert!(progress.publish_first_initialized(7, INIT));
                    Err("solve failed".into())
                },
                LiveSolverCallbacks::new(|_, _| panic!("unexpected solution"))
                    .on_first_initialized(move |_, snapshot, metadata| {
                        sent.send((snapshot.revision, *snapshot.solution, metadata))
                            .unwrap()
                    }),
            )
            .unwrap();
        assert!(
            matches!(job.wait_timeout(WAIT).unwrap().as_ref(), Err(SolverJobError::Solve(LiveSolveError::Solve(s))) if s == "solve failed")
        );
        assert_eq!(seen.recv_timeout(WAIT).unwrap(), (0, 7, INIT));
        assert!(seen.try_recv().is_err());
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}

#[test]
fn legacy_consumer_can_receive_produced_best_without_initialization_subscription() {
    let mut manager = LiveSolverManager::<(), usize, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let (finish, finishing) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            5,
            (),
            Default::default(),
            move |_, _, _, progress| {
                assert!(!progress.publish_first_initialized(99, INIT));
                assert!(progress.publish_from(1, producer(1)));
                finishing.recv_timeout(WAIT).unwrap();
                Ok(2)
            },
            move |_, snapshot| sent.send(*snapshot).unwrap(),
        )
        .unwrap();
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 1);
    finish.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 2);
    assert!(seen.try_recv().is_err());
    manager.shutdown().unwrap();
}

#[test]
fn actual_solver_events_keep_origin_initialization_and_exact_work_accounting() {
    for delivery in [LiveDelivery::Best, LiveDelivery::Final] {
        type Snapshot = (Assignment<i32>, Option<SimpleScore>);
        let mut manager =
            LiveSolverManager::<FiniteProblem<i32>, Snapshot, String>::new(1).unwrap();
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap();
        let best = Arc::new(Mutex::new(Vec::new()));
        let seen = best.clone();
        let (initialized, initialization) = mpsc::channel();
        let (acknowledge, acknowledged) = mpsc::channel();
        let (work_sent, work_seen) = mpsc::channel();
        let job = manager
            .submit_with_statistics_and_events(
                6,
                p,
                LiveSolverConfig {
                    delivery,
                    ..Default::default()
                },
                ProblemStatistics::problem_size_statistics,
                move |p, _, token, progress| {
                    let mut network = ConstraintNetwork::new(3);
                    network
                        .add_constraint("sum", vec![0, 1, 2], |values| {
                            SimpleScore::new(
                                values
                                    .iter()
                                    .filter_map(|v| v.copied())
                                    .map(i64::from)
                                    .sum(),
                            )
                        })
                        .unwrap();
                    let result = solve_with_events(
                        p,
                        &network,
                        SolverConfig {
                            limits: SolverLimits {
                                cancellation: Some(token),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        |event| match event {
                            SolverEvent::Best(update) => {
                                let origin = SolverEventProducer::from(&update);
                                assert_eq!(
                                    progress.publish_from(
                                        (update.assignment.clone(), Some(update.score)),
                                        origin
                                    ),
                                    delivery == LiveDelivery::Best
                                );
                            }
                            SolverEvent::FirstInitialized {
                                assignment,
                                score,
                                initialization,
                                ..
                            } => {
                                assert!(progress.publish_first_initialized(
                                    (assignment.clone(), score),
                                    initialization
                                ));
                                acknowledged.recv_timeout(WAIT).unwrap();
                            }
                        },
                    )
                    .map_err(|e| e.to_string())?;
                    work_sent.send(result.work).unwrap();
                    Ok(LiveRun {
                        solution: (result.assignment, result.score),
                        work: result.work,
                    })
                },
                LiveSolverCallbacks::new(
                    move |_, event: SolverBestEvent<'_, LiveSolution<Snapshot>>| {
                        assert_eq!(event.solution.revision, 0);
                        seen.lock()
                            .unwrap()
                            .push((event.solution.solution.1, event.producer));
                    },
                )
                .on_first_initialized(move |_, snapshot, info| {
                    assert_eq!(snapshot.revision, 0);
                    initialized.send((snapshot.solution.1, info)).unwrap();
                }),
            )
            .unwrap();
        let (score, info) = initialization.recv_timeout(WAIT).unwrap();
        assert_eq!(score, Some(SimpleScore::new(6)));
        assert_eq!(info.phase, 0);
        assert_eq!(info.kind, SolverPhaseKind::Construction);
        assert!(!info.terminated_early);
        assert!(job.try_result().is_none());
        acknowledge.send(()).unwrap();
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        assert_eq!(
            job.statistics().work,
            Some(work_seen.recv_timeout(WAIT).unwrap())
        );
        assert_eq!(job.statistics().problem_size.unwrap().revision, 0);
        let events = best.lock().unwrap();
        assert_eq!(events.last(), Some(&(Some(SimpleScore::new(6)), None)));
        if delivery == LiveDelivery::Best {
            assert!(events.iter().any(|(_, origin)| {
                origin.is_some_and(|o| o.kind == SolverPhaseKind::Construction)
            }));
        } else {
            assert_eq!(events.len(), 1);
        }
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}
