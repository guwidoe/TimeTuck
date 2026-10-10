//! Original live publication and receipt boundaries; no upstream method credit.
use std::{sync::mpsc, time::Duration};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);

#[test]
fn consumed_intermediate_acknowledges_only_its_revision_before_solve_returns() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (release_initial, initial_released) = mpsc::channel();
    let (consuming, consumption) = mpsc::channel();
    let (release_consumer, consumer_released) = mpsc::channel();
    let (release_solve, solve_released) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            0,
            Default::default(),
            move |p, old, token, progress| {
                assert_eq!(progress.revision(), *p as u64);
                match *p {
                    0 => {
                        assert!(old.is_none());
                        started.send(()).unwrap();
                        initial_released.recv_timeout(WAIT).unwrap();
                        assert!(token.is_cancelled());
                        assert!(!progress.publish(999));
                        Ok(0)
                    }
                    1 => {
                        assert_eq!(old, Some(&0));
                        assert!(progress.publish(10));
                        solve_released.recv_timeout(WAIT).unwrap();
                        assert!(token.is_cancelled());
                        assert!(!progress.publish(999));
                        Ok(11)
                    }
                    2 => {
                        // Intermediate publication does not replace the returned warm start.
                        assert_eq!(old, Some(&11));
                        assert!(progress.publish(20));
                        Ok(21)
                    }
                    _ => unreachable!(),
                }
            },
            move |_, value| {
                assert_ne!(*value, 999);
                if *value == 10 {
                    consuming.send(()).unwrap();
                    consumer_released.recv_timeout(WAIT).unwrap();
                }
            },
        )
        .unwrap();
    starting.recv_timeout(WAIT).unwrap();
    let first = job
        .add_problem_change(|p| {
            *p = 1;
            Ok(())
        })
        .unwrap();
    release_initial.send(()).unwrap();
    consumption.recv_timeout(WAIT).unwrap();
    assert!(first.try_result().is_none());
    let second = job
        .add_problem_change(|p| {
            *p = 2;
            Ok(())
        })
        .unwrap();
    release_consumer.send(()).unwrap();
    assert_eq!(first.wait_timeout(WAIT), Some(Ok(())));
    assert!(second.try_result().is_none());
    assert!(job.try_result().is_none());
    release_solve.send(()).unwrap();
    let outcome = job.wait_timeout(WAIT).unwrap();
    let result = outcome.as_ref().as_ref().unwrap();
    assert_eq!(result.revision, 2);
    assert_eq!(*result.solution, 21);
    assert_eq!(second.wait_timeout(WAIT), Some(Ok(())));
    manager.shutdown().unwrap();
}

#[test]
fn slow_consumer_coalesces_owned_snapshots_without_blocking_solver() {
    #[derive(Debug)]
    struct Owned(usize); // deliberately not Clone
    let mut manager = LiveSolverManager::<(), Owned, String>::new(1).unwrap();
    let (entered, enter) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (published, publications) = mpsc::channel();
    let (finish, finished) = mpsc::channel();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            2,
            (),
            Default::default(),
            move |_, _, _, progress| {
                assert!(progress.publish(Owned(0)));
                enter.recv_timeout(WAIT).unwrap();
                for i in 1..=3 {
                    assert!(progress.publish(Owned(i)));
                }
                published.send(()).unwrap();
                finished.recv_timeout(WAIT).unwrap();
                Ok(Owned(99))
            },
            move |_, value| {
                if value.0 == 0 {
                    entered.send(()).unwrap();
                    released.recv_timeout(WAIT).unwrap();
                }
                sent.send(value.0).unwrap();
            },
        )
        .unwrap();
    publications.recv_timeout(WAIT).unwrap();
    release.send(()).unwrap();
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 0);
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 3);
    finish.send(()).unwrap();
    assert_eq!(
        job.wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .solution
            .0,
        99
    );
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 99);
    assert!(seen.try_recv().is_err());
    manager.shutdown().unwrap();
}

#[test]
fn final_delivery_disables_intermediates_and_returns_final_result() {
    let mut manager = LiveSolverManager::<(), usize, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            3,
            (),
            LiveSolverConfig {
                delivery: LiveDelivery::Final,
                ..Default::default()
            },
            |_, _, _, progress| {
                assert!(!progress.publish(1));
                assert!(!progress.publish(2));
                Ok(3)
            },
            move |_, value| sent.send(*value).unwrap(),
        )
        .unwrap();
    assert_eq!(
        *job.wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .solution,
        3
    );
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 3);
    assert!(seen.try_recv().is_err());
    manager.shutdown().unwrap();
}

#[test]
fn intermediate_consumer_panic_fails_receipt_and_cancels_running_revision() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (release_initial, initial_released) = mpsc::channel();
    let (release_solve, solve_released) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            4,
            0,
            Default::default(),
            move |p, _, token, progress| {
                if *p == 0 {
                    started.send(()).unwrap();
                    initial_released.recv_timeout(WAIT).unwrap();
                } else {
                    assert!(progress.publish(1));
                    solve_released.recv_timeout(WAIT).unwrap();
                    assert!(token.is_cancelled());
                    assert!(!progress.publish(2));
                }
                Ok(*p)
            },
            |_, value| {
                assert_ne!(*value, 1, "consumer failed");
            },
        )
        .unwrap();
    starting.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(|p| {
            *p = 1;
            Ok(())
        })
        .unwrap();
    release_initial.send(()).unwrap();
    assert!(matches!(
        receipt.wait_timeout(WAIT),
        Some(Err(ProblemChangeFailure::ConsumerPanicked(_)))
    ));
    release_solve.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(job.callback_failures().len(), 1);
    assert_eq!(job.callback_failures()[0].stage, SolverCallbackStage::Best);
    manager.shutdown().unwrap();
}

#[test]
fn actual_solver_observer_delivers_progress_and_counts_work_once() {
    let mut manager =
        LiveSolverManager::<FiniteProblem<i32>, (Assignment<i32>, SimpleScore), String>::new(1)
            .unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap();
    let (sent, seen) = mpsc::channel();
    let (acknowledge, acknowledged) = mpsc::channel();
    let (work_sent, work_seen) = mpsc::channel();
    let job = manager
        .submit_with_statistics_and_progress(
            5,
            p,
            Default::default(),
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
                let config = SolverConfig {
                    limits: SolverLimits {
                        cancellation: Some(token),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut first = true;
                let result = solve_with_observer(
                    p,
                    &network,
                    config,
                    |u: SolverUpdate<'_, Assignment<i32>, SimpleScore>| {
                        assert!(progress.publish((u.assignment.clone(), u.score)));
                        if first {
                            first = false;
                            // A real solver observer can deliver before solving returns.
                            acknowledged.recv_timeout(WAIT).unwrap();
                        }
                    },
                )
                .map_err(|e| e.to_string())?;
                assert!(!first);
                work_sent.send(result.work).unwrap();
                Ok(LiveRun {
                    solution: (result.assignment, result.score.unwrap()),
                    work: result.work,
                })
            },
            move |_, value| {
                sent.send(value.clone()).unwrap();
            },
        )
        .unwrap();
    let (assignment, score) = seen.recv_timeout(WAIT).unwrap();
    assert_eq!(
        score,
        SimpleScore::new(
            assignment
                .values()
                .iter()
                .flatten()
                .copied()
                .map(i64::from)
                .sum()
        )
        .with_structural(-(assignment.values().iter().filter(|v| v.is_none()).count() as i64))
    );
    assert!(job.try_result().is_none());
    acknowledge.send(()).unwrap();
    let outcome = job.wait_timeout(WAIT).unwrap();
    let result = outcome.as_ref().as_ref().unwrap();
    assert_eq!(result.solution.1, SimpleScore::new(6));
    assert_eq!(
        job.statistics().work,
        Some(work_seen.recv_timeout(WAIT).unwrap())
    );
    assert_eq!(job.statistics().problem_size.unwrap().revision, 0);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn delivered_receipt_stays_successful_when_the_run_later_fails() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (release_initial, initial_released) = mpsc::channel();
    let (release_solve, solve_released) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            6,
            0,
            Default::default(),
            move |p, _, _, progress| {
                if *p == 0 {
                    started.send(()).unwrap();
                    initial_released.recv_timeout(WAIT).unwrap();
                    Ok(0)
                } else {
                    assert!(progress.publish(1));
                    solve_released.recv_timeout(WAIT).unwrap();
                    Err("later failure".into())
                }
            },
            |_, _| {},
        )
        .unwrap();
    starting.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(|p| {
            *p = 1;
            Ok(())
        })
        .unwrap();
    release_initial.send(()).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    assert!(job.try_result().is_none());
    release_solve.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_err());
    assert_eq!(receipt.try_result(), Some(Ok(())));
    manager.shutdown().unwrap();
}
