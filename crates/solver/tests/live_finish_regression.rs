//! Original graceful live completion regressions; no upstream method credit.
use std::{sync::mpsc, time::Duration};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);

#[test]
fn finish_cancels_unapplied_changes_and_delivers_success_in_all_modes() {
    for daemon in [false, true] {
        for delivery in [LiveDelivery::Best, LiveDelivery::Final] {
            let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
            let (requested, requests) = mpsc::channel();
            let (release, released) = mpsc::channel();
            let (consuming, consumption) = mpsc::channel();
            let (consume, consumed) = mpsc::channel();
            let work = SolverWork {
                evaluations: 7,
                completed_steps: 3,
                ..Default::default()
            };
            let job = manager
                .submit_with_statistics_and_progress(
                    1,
                    42,
                    LiveSolverConfig { daemon, delivery },
                    |_| ProblemSizeStatistics {
                        entity_count: 1,
                        variable_count: 1,
                        value_count: 1,
                        assignment_space: AssignmentSpace {
                            count: 1u8.into(),
                            exact: true,
                        },
                    },
                    move |p, previous, token, progress| {
                        assert_eq!(*p, 42);
                        assert!(previous.is_none());
                        assert_eq!(progress.revision(), 0);
                        // A scoped thread can request finishing without cancelling the solve.
                        std::thread::scope(|scope| {
                            scope
                                .spawn(|| assert!(progress.finish_after_run()))
                                .join()
                                .unwrap();
                        });
                        assert!(!progress.finish_after_run());
                        assert!(!token.is_cancelled());
                        requested.send(()).unwrap();
                        released.recv_timeout(WAIT).unwrap();
                        assert!(token.is_cancelled()); // The queued change, not finishing.
                        Ok(LiveRun { solution: *p, work })
                    },
                    move |_, result| {
                        assert_eq!(*result, 42);
                        consuming.send(()).unwrap();
                        consumed.recv_timeout(WAIT).unwrap();
                    },
                )
                .unwrap();
            requests.recv_timeout(WAIT).unwrap();
            let receipt = job
                .add_problem_change(|_| panic!("unapplied change executed"))
                .unwrap();
            assert!(receipt.try_result().is_none());
            release.send(()).unwrap();
            consumption.recv_timeout(WAIT).unwrap();
            assert_eq!(
                receipt.wait_timeout(WAIT),
                Some(Err(ProblemChangeFailure::Cancelled))
            );
            assert!(matches!(
                job.add_problem_change(|_| Ok(())),
                Err(ProblemChangeSubmitError::Unavailable { problem_id: 1 })
            ));
            assert!(job.try_result().is_none()); // Wait includes the consumer.
            consume.send(()).unwrap();
            let outcome = job.wait_timeout(WAIT).unwrap();
            let result = outcome.as_ref().as_ref().unwrap();
            assert_eq!(result.revision, 0);
            assert_eq!(*result.solution, 42);
            assert_eq!(job.statistics().work, Some(work));
            assert_eq!(job.statistics().problem_size.unwrap().revision, 0);
            assert!(job.callback_failures().is_empty());
            manager.shutdown().unwrap();
        }
    }
}

#[test]
fn finish_exits_idle_daemon_without_cancelling_its_token() {
    for delivery in [LiveDelivery::Best, LiveDelivery::Final] {
        let mut manager = LiveSolverManager::<(), usize, String>::new(1).unwrap();
        let job = manager
            .submit_with_progress(
                2,
                (),
                LiveSolverConfig {
                    daemon: true,
                    delivery,
                },
                |_, _, token, progress| {
                    assert!(progress.finish_after_run());
                    assert!(!token.is_cancelled());
                    Ok(9)
                },
                |_, result| assert_eq!(*result, 9),
            )
            .unwrap();
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}

#[test]
fn finish_does_not_mask_solve_or_consumer_failure_for_applied_receipts() {
    for solve_error in [false, true] {
        let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
        let (started, starting) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let job = manager
            .submit_with_progress(
                3,
                0,
                LiveSolverConfig {
                    daemon: true,
                    delivery: LiveDelivery::Final,
                },
                move |p, _, _, progress| {
                    if *p == 0 {
                        started.send(()).unwrap();
                        released.recv_timeout(WAIT).unwrap();
                        return Ok(0);
                    }
                    assert_eq!(progress.revision(), 1);
                    assert!(progress.finish_after_run());
                    if solve_error {
                        Err("solve failed".into())
                    } else {
                        Ok(*p)
                    }
                },
                |_, _| panic!("consumer failed"),
            )
            .unwrap();
        starting.recv_timeout(WAIT).unwrap();
        let receipt = job
            .add_problem_change(|p| {
                *p = 1;
                Ok(())
            })
            .unwrap();
        release.send(()).unwrap();
        let expected = if solve_error {
            ProblemChangeFailure::SolveFailed("solver failed: solve failed".into())
        } else {
            ProblemChangeFailure::ConsumerPanicked("consumer failed".into())
        };
        assert_eq!(receipt.wait_timeout(WAIT), Some(Err(expected)));
        assert_eq!(job.wait_timeout(WAIT).unwrap().is_err(), solve_error);
        assert_eq!(job.callback_failures().len(), usize::from(!solve_error));
        manager.shutdown().unwrap();
    }
}
