//! Original previous-run change contexts; no upstream method credit.
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);

#[test]
fn interrupted_run_result_is_borrowed_despite_lagging_consumer_and_interim_progress() {
    struct Owned(usize); // Deliberately not Clone.
    let mut manager = LiveSolverManager::<usize, Owned, String>::new(1).unwrap();
    let (consumer_started, consumer_start) = mpsc::channel();
    let (release_consumer, consumer_release) = mpsc::channel();
    let (return_run, run_return) = mpsc::channel();
    let (restart_started, restart_start) = mpsc::channel();
    let (return_restart, restart_return) = mpsc::channel();
    let (observed, observations) = mpsc::channel();
    let job = manager
        .submit_with_events(
            1,
            0,
            Default::default(),
            move |p, old, token, progress| {
                if *p == 0 {
                    assert!(old.is_none());
                    assert!(progress.publish(Owned(100))); // Not the returned result.
                    run_return.recv_timeout(WAIT).unwrap();
                    assert!(token.is_cancelled());
                    Ok(Owned(20))
                } else {
                    assert_eq!(*p, 3);
                    assert_eq!(old.unwrap().0, 20);
                    assert_eq!(progress.revision(), 3);
                    restart_started.send(()).unwrap();
                    restart_return.recv_timeout(WAIT).unwrap();
                    Ok(Owned(30))
                }
            },
            LiveSolverCallbacks::new(move |_, event: SolverBestEvent<'_, LiveSolution<Owned>>| {
                if event.solution.solution.0 == 100 {
                    consumer_started.send(()).unwrap();
                    consumer_release.recv_timeout(WAIT).unwrap();
                }
            }),
        )
        .unwrap();
    consumer_start.recv_timeout(WAIT).unwrap();
    let first = job
        .add_problem_change_with_previous(move |p, previous| {
            let previous = previous.unwrap();
            assert_eq!((*p, previous.revision, previous.solution.0), (0, 0, 20));
            observed
                .send(Arc::as_ptr(&previous.solution) as usize)
                .unwrap();
            *p = 1;
            Ok(())
        })
        .unwrap();
    let second = job
        .add_problem_change(|p| {
            assert_eq!(*p, 1);
            *p = 2;
            Ok(())
        })
        .unwrap();
    let (observed, last_observation) = mpsc::channel();
    let third = manager
        .add_problem_change_with_previous(1, move |p, previous| {
            let previous = previous.unwrap();
            assert_eq!((*p, previous.revision, previous.solution.0), (2, 0, 20));
            observed
                .send(Arc::as_ptr(&previous.solution) as usize)
                .unwrap();
            *p = 3;
            Ok(())
        })
        .unwrap();
    return_run.send(()).unwrap();
    restart_start.recv_timeout(WAIT).unwrap();
    assert_eq!(
        observations.recv_timeout(WAIT).unwrap(),
        last_observation.recv_timeout(WAIT).unwrap()
    );
    for receipt in [&first, &second, &third] {
        assert!(receipt.try_result().is_none());
    }
    release_consumer.send(()).unwrap();
    return_restart.send(()).unwrap();
    let result = job.wait_timeout(WAIT).unwrap();
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!((result.revision, result.solution.0), (3, 30));
    for receipt in [&first, &second, &third] {
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    }
    manager.shutdown().unwrap();
}

#[test]
fn pre_run_changes_receive_none_and_cancelled_queued_changes_never_execute() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let blocker = manager
        .submit(
            1,
            0,
            Default::default(),
            move |p, _, _| {
                started.send(()).unwrap();
                released.recv_timeout(WAIT).unwrap();
                Ok(*p)
            },
            |_, _| {},
        )
        .unwrap();
    starting.recv_timeout(WAIT).unwrap();
    let job = manager
        .submit(
            2,
            0,
            Default::default(),
            |p, old, _| {
                assert!(old.is_none());
                assert_eq!(*p, 2);
                Ok(*p)
            },
            |_, _| {},
        )
        .unwrap();
    let first = job
        .add_problem_change_with_previous(|p, previous| {
            assert!(previous.is_none());
            *p += 1;
            Ok(())
        })
        .unwrap();
    let second = manager
        .add_problem_change_with_previous(2, |p, previous| {
            assert!(previous.is_none());
            assert_eq!(*p, 1);
            *p += 1;
            Ok(())
        })
        .unwrap();
    let cancelled = manager
        .submit(
            3,
            0,
            Default::default(),
            |_, _, _| panic!("cancelled solve"),
            |_, _| {},
        )
        .unwrap();
    let receipt = cancelled
        .add_problem_change_with_previous(|_, _| panic!("cancelled edit"))
        .unwrap();
    assert!(cancelled.cancel());
    release.send(()).unwrap();
    assert!(blocker.wait_timeout(WAIT).unwrap().is_ok());
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(first.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(second.wait_timeout(WAIT), Some(Ok(())));
    assert!(matches!(
        cancelled.wait_timeout(WAIT).unwrap().as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    assert_eq!(
        receipt.wait_timeout(WAIT),
        Some(Err(ProblemChangeFailure::Cancelled))
    );
    assert!(matches!(
        manager.add_problem_change_with_previous(99, |_, _| Ok(())),
        Err(ProblemChangeSubmitError::Unavailable { problem_id: 99 })
    ));
    manager.shutdown().unwrap();
}

#[test]
fn failed_or_panicking_edit_keeps_snapshot_immutable_and_stops_later_edits() {
    for panics in [false, true] {
        let mut manager = LiveSolverManager::<Vec<usize>, Vec<usize>, String>::new(1).unwrap();
        let (published, publication) = mpsc::channel();
        let job = manager
            .submit_with_events(
                1,
                vec![1],
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                |p, _, _, _| Ok(p.clone()),
                LiveSolverCallbacks::new(
                    move |_, event: SolverBestEvent<'_, LiveSolution<Vec<usize>>>| {
                        published.send(event.solution.clone()).unwrap();
                    },
                ),
            )
            .unwrap();
        let snapshot = publication.recv_timeout(WAIT).unwrap();
        let (entered, entering) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let first = job
            .add_problem_change_with_previous(move |p, previous| {
                assert_eq!(previous.unwrap().solution.as_ref(), &[1]);
                p.push(2);
                entered.send(()).unwrap();
                released.recv_timeout(WAIT).unwrap();
                if panics {
                    panic!("edit panic");
                }
                Err("edit error".into())
            })
            .unwrap();
        entering.recv_timeout(WAIT).unwrap();
        let second = job
            .add_problem_change_with_previous(|_, _| panic!("must not execute"))
            .unwrap();
        release.send(()).unwrap();
        let result = job.wait_timeout(WAIT).unwrap();
        if panics {
            assert!(
                matches!(result.as_ref(), Err(SolverJobError::Panicked(message)) if message == "edit panic")
            );
        } else {
            assert!(
                matches!(result.as_ref(), Err(SolverJobError::Solve(LiveSolveError::Change { revision: 1, message })) if message == "edit error")
            );
        }
        assert_eq!(snapshot.solution.as_ref(), &[1]);
        assert!(first.wait_timeout(WAIT).unwrap().is_err());
        assert!(second.wait_timeout(WAIT).unwrap().is_err());
        assert!(job.add_problem_change_with_previous(|_, _| Ok(())).is_err());
        manager.shutdown().unwrap();
    }
}

struct BasicFacts {
    domain: Vec<i32>,
    pinned: Option<i32>,
}
struct BasicResult {
    assignments: AssignmentSnapshot<u64, i32>,
    initial: Assignment<i32>,
    score: SimpleScore,
}
#[test]
fn completed_basic_assignment_can_be_pinned_before_reconciliation_without_rescoring() {
    for streamed in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let score_calls = calls.clone();
        let mut manager = LiveSolverManager::<BasicFacts, BasicResult, String>::new(1).unwrap();
        let (published, publication) = mpsc::channel();
        let job = manager
            .submit_with_events(
                1,
                BasicFacts {
                    domain: vec![1, 2],
                    pinned: None,
                },
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                move |facts, previous, token, _| {
                    let variable = match facts.pinned {
                        Some(v) => Variable::new(facts.domain.clone())
                            .with_initial(v)
                            .with_pinned(true),
                        None => Variable::new(facts.domain.clone()),
                    };
                    let problem = FiniteProblem::new(vec![variable]).unwrap();
                    let problem =
                        reconcile_problem(&problem, &[7], previous.map(|p| &p.assignments))
                            .unwrap();
                    let initial = problem.initial_assignment();
                    let config = SolverConfig {
                        limits: SolverLimits {
                            cancellation: Some(token),
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    let result = if streamed {
                        solve_stream(
                            &problem,
                            || {
                                let mut builder = StreamGraphBuilder::new();
                                let source = builder.source::<usize, (usize, Option<i32>)>();
                                let calls = score_calls.clone();
                                builder.score("value", source.port(), move |(_, value)| {
                                    calls.fetch_add(1, Ordering::SeqCst);
                                    SimpleScore::new(i64::from(value.unwrap_or(0)))
                                })?;
                                Ok((builder.build(), source))
                            },
                            config,
                        )
                    } else {
                        let mut network = ConstraintNetwork::new(1);
                        network
                            .add_constraint("value", vec![0], |values| {
                                score_calls.fetch_add(1, Ordering::SeqCst);
                                SimpleScore::new(i64::from(values[0].copied().unwrap_or(0)))
                            })
                            .unwrap();
                        solve(&problem, &network, config)
                    }
                    .map_err(|e| e.to_string())?;
                    assert!(result.complete);
                    Ok(BasicResult {
                        assignments: AssignmentSnapshot::new(vec![7], result.assignment).unwrap(),
                        initial,
                        score: result.score.unwrap(),
                    })
                },
                LiveSolverCallbacks::new(
                    move |_, event: SolverBestEvent<'_, LiveSolution<BasicResult>>| {
                        published.send(event.solution.clone()).unwrap();
                    },
                ),
            )
            .unwrap();
        let original = publication.recv_timeout(WAIT).unwrap();
        assert_eq!(
            original.solution.assignments.assignment().values(),
            &[Some(2)]
        );
        let before = calls.load(Ordering::SeqCst);
        let during = calls.clone();
        let receipt = job
            .add_problem_change_with_previous(move |facts, previous| {
                assert_eq!(during.load(Ordering::SeqCst), before);
                let previous = previous.unwrap();
                assert_eq!(previous.revision, 0);
                facts.pinned = previous.solution.assignments.assignment().values()[0];
                facts.domain.push(9); // Without the pin the next optimum would be 9.
                assert_eq!(during.load(Ordering::SeqCst), before);
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        let next = publication.recv_timeout(WAIT).unwrap();
        assert_eq!(next.revision, 1);
        assert_eq!(next.solution.initial.values(), &[Some(2)]);
        assert_eq!(next.solution.assignments.assignment().values(), &[Some(2)]);
        assert_eq!(next.solution.score, SimpleScore::new(2));
        assert_eq!(
            original.solution.assignments.assignment().values(),
            &[Some(2)]
        );
        job.cancel();
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        manager.shutdown().unwrap();
    }
}

struct ListFacts {
    values: Vec<i32>,
    pinned: Vec<i32>,
}
struct ListResult {
    assignments: ListAssignmentSnapshot<u64, i32>,
    initial: ListAssignment<i32>,
}
#[test]
fn completed_list_prefix_can_be_pinned_while_new_value_is_replanned() {
    for streamed in [false, true] {
        let mut manager = LiveSolverManager::<ListFacts, ListResult, String>::new(1).unwrap();
        let (published, publication) = mpsc::channel();
        let job = manager
            .submit_with_events(
                1,
                ListFacts {
                    values: vec![1, 2],
                    pinned: vec![],
                },
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                move |facts, previous, token, _| {
                    let problem = ListProblem::new(
                        facts.values.clone(),
                        vec![
                            ListEntity::new(facts.pinned.clone())
                                .with_pinned_prefix(facts.pinned.len()),
                        ],
                    )
                    .unwrap();
                    let problem =
                        reconcile_list_problem(&problem, &[7], previous.map(|p| &p.assignments))
                            .unwrap();
                    let initial = problem.initial_assignment();
                    let config = SolverConfig {
                        limits: SolverLimits {
                            cancellation: Some(token),
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    let result = if streamed {
                        solve_list_stream(
                            &problem,
                            || {
                                let mut builder = StreamGraphBuilder::new();
                                let sources = ListStreamSources::<i32>::new(&mut builder);
                                builder.score("zero", sources.entities.port(), |_| {
                                    SimpleScore::ZERO
                                })?;
                                Ok((builder.build(), sources))
                            },
                            config,
                        )
                    } else {
                        let network =
                            ListConstraintNetwork::<_, SimpleScore>::new(1, facts.values.len());
                        solve_list(&problem, &network, config)
                    }
                    .map_err(|e| e.to_string())?;
                    assert!(result.complete);
                    Ok(ListResult {
                        assignments: ListAssignmentSnapshot::new(vec![7], result.assignment)
                            .unwrap(),
                        initial,
                    })
                },
                LiveSolverCallbacks::new(
                    move |_, event: SolverBestEvent<'_, LiveSolution<ListResult>>| {
                        published.send(event.solution.clone()).unwrap();
                    },
                ),
            )
            .unwrap();
        let original = publication.recv_timeout(WAIT).unwrap();
        let old_row = original.solution.assignments.assignment().lists()[0].clone();
        assert_eq!(old_row.len(), 2);
        let receipt = manager
            .add_problem_change_with_previous(1, |facts, previous| {
                let previous = previous.unwrap();
                assert_eq!(previous.revision, 0);
                facts.pinned = previous.solution.assignments.assignment().lists()[0].clone();
                facts.values.push(3);
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        let next = publication.recv_timeout(WAIT).unwrap();
        assert_eq!(next.revision, 1);
        assert_eq!(
            next.solution.initial.lists(),
            std::slice::from_ref(&old_row)
        );
        let mut expected = old_row.clone();
        expected.push(3);
        assert_eq!(next.solution.assignments.assignment().lists(), &[expected]);
        assert_eq!(
            original.solution.assignments.assignment().lists(),
            &[old_row]
        );
        job.cancel();
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        manager.shutdown().unwrap();
    }
}
