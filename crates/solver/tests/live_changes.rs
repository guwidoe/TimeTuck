//! Original revision, delivery and warm-restart checks; no upstream method credit.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
#[derive(Clone, Debug)]
struct Solution {
    facts: usize,
    basic: Vec<Option<usize>>,
    lists: Vec<Vec<usize>>,
    score: Option<SimpleScore>,
    complete: bool,
}
fn solve_model(
    mode: usize,
    n: usize,
    old: Option<&Solution>,
    token: &CancellationToken,
) -> Result<Solution, String> {
    let basic = || {
        FiniteProblem::new(
            (0..2)
                .map(|i| {
                    let v = Variable::new((0..n).collect());
                    old.and_then(|s| s.basic.get(i).copied().flatten())
                        .filter(|v| *v < n)
                        .map_or(v.clone(), |x| v.with_initial(x))
                })
                .collect(),
        )
        .unwrap()
    };
    let lists = || {
        ListProblem::new(
            (0..n).collect(),
            (0..2)
                .map(|i| {
                    ListEntity::new(
                        old.and_then(|s| s.lists.get(i))
                            .map_or(vec![], |v| v.iter().copied().filter(|v| *v < n).collect()),
                    )
                })
                .collect(),
        )
        .unwrap()
    };
    let c = SolverConfig {
        limits: SolverLimits {
            cancellation: Some(token),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut output = Solution {
        facts: n,
        basic: vec![],
        lists: vec![],
        score: None,
        complete: false,
    };
    if mode < 2 {
        let p = basic();
        let r = if mode == 0 {
            let mut network = ConstraintNetwork::new(2);
            network
                .add_constraint("values", vec![0, 1], |v| {
                    SimpleScore::new(v.iter().filter_map(|x| x.copied()).sum::<usize>() as i64)
                })
                .unwrap();
            solve(&p, &network, c)
        } else {
            solve_stream(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = b.source::<usize, (usize, Option<usize>)>();
                    b.score("values", source.port(), |(_, v)| {
                        SimpleScore::new(v.unwrap_or(0) as i64)
                    })?;
                    Ok((b.build(), source))
                },
                c,
            )
        }
        .map_err(|e| e.to_string())?;
        output.basic = r.assignment.values().to_vec();
        output.score = r.score;
        output.complete = r.complete;
    } else if mode < 4 {
        let p = lists();
        let r = if mode == 2 {
            let mut network = ListConstraintNetwork::new(2, n);
            network
                .add_constraint("owners", vec![ListScope::Entity(1)], |v| {
                    let ListFact::Entity(values) = &v[0] else {
                        panic!()
                    };
                    SimpleScore::new(values.len() as i64)
                })
                .unwrap();
            solve_list(&p, &network, c)
        } else {
            solve_list_stream(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = ListStreamSources::new(&mut b);
                    b.score("owners", source.values.port(), |v| {
                        SimpleScore::new(v.position.map_or(0, |p| p.entity as i64))
                    })?;
                    Ok((b.build(), source))
                },
                c,
            )
        }
        .map_err(|e| e.to_string())?;
        output.lists = r.assignment.lists().to_vec();
        output.score = r.score;
        output.complete = r.complete;
    } else {
        let p = MixedProblem::new(basic(), lists());
        let c = MixedSolverConfig {
            limits: c.limits,
            ..Default::default()
        };
        let r = if mode == 4 {
            let mut network = MixedConstraintNetwork::new(2, 2, n);
            network
                .add_constraint(
                    "values",
                    vec![
                        MixedScope::Basic(0),
                        MixedScope::Basic(1),
                        MixedScope::ListEntity(1),
                    ],
                    |v| {
                        let (
                            MixedFact::Basic(a),
                            MixedFact::Basic(b),
                            MixedFact::ListEntity(values),
                        ) = (&v[0], &v[1], &v[2])
                        else {
                            panic!()
                        };
                        SimpleScore::new(
                            (a.copied().unwrap_or(0) + b.copied().unwrap_or(0) + values.len())
                                as i64,
                        )
                    },
                )
                .unwrap();
            solve_mixed(&p, &network, c)
        } else {
            solve_mixed_stream(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = MixedStreamSources::new(&mut b);
                    b.score("values", source.basic.port(), |(_, v)| {
                        SimpleScore::new(v.unwrap_or(0) as i64)
                    })?;
                    b.score("owners", source.lists.values.port(), |v| {
                        SimpleScore::new(v.position.map_or(0, |p| p.entity as i64))
                    })?;
                    Ok((b.build(), source))
                },
                c,
            )
        }
        .map_err(|e| e.to_string())?;
        output.basic = r.assignment.basic().values().to_vec();
        output.lists = r.assignment.lists().lists().to_vec();
        output.score = r.score;
        output.complete = r.complete;
    }
    Ok(output)
}
fn daemon() -> LiveSolverConfig {
    LiveSolverConfig {
        daemon: true,
        ..Default::default()
    }
}
#[test]
fn live_changes_rebuild_and_warm_start_all_six_solver_paths_including_lower_scores() {
    for mode in 0..6 {
        let mut manager = LiveSolverManager::<usize, Solution, String>::new(1).unwrap();
        let (sent, seen) = mpsc::channel();
        let warm = Arc::new(Mutex::new(vec![]));
        let record = warm.clone();
        let job = manager
            .submit(
                1,
                2,
                daemon(),
                move |n, old, token| {
                    record.lock().unwrap().push((*n, old.map(|s| s.facts)));
                    solve_model(mode, *n, old, token)
                },
                move |_, s| sent.send(s.clone()).unwrap(),
            )
            .unwrap();
        assert_eq!(seen.recv_timeout(WAIT).unwrap().facts, 2);
        for n in [3, 1, 4] {
            let receipt = manager
                .add_problem_change(1, move |p| {
                    *p = n;
                    Ok(())
                })
                .unwrap();
            assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
            let solution = seen.recv_timeout(WAIT).unwrap();
            assert_eq!(solution.facts, n);
            assert!(solution.complete);
            let expected = if mode < 2 {
                2 * (n - 1)
            } else if mode < 4 {
                n
            } else {
                2 * (n - 1) + n
            };
            assert_eq!(solution.score, Some(SimpleScore::new(expected as i64)));
        }
        assert_eq!(
            *warm.lock().unwrap(),
            vec![(2, None), (3, Some(2)), (1, Some(3)), (4, Some(1))]
        );
        assert!(job.cancel());
        let r = job.wait_timeout(WAIT).unwrap();
        let r = r.as_ref().as_ref().unwrap();
        assert_eq!(r.revision, 3);
        assert_eq!(r.solution.facts, 4);
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}
#[test]
fn receipt_waits_for_successful_consumer_return_not_application_or_publication() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let job = manager
        .submit(
            1,
            1,
            daemon(),
            |p, _, _| Ok(*p),
            move |_, n| {
                sent.send(*n).unwrap();
                if *n == 2 {
                    resume.recv_timeout(WAIT).unwrap();
                }
            },
        )
        .unwrap();
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 1);
    let receipt = job
        .add_problem_change(|p| {
            *p = 2;
            Ok(())
        })
        .unwrap();
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 2);
    assert_eq!(receipt.try_result(), None);
    assert_eq!(receipt.wait_timeout(Duration::ZERO), None);
    release.send(()).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    job.cancel();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}
#[test]
fn coalesced_later_revisions_acknowledge_all_included_changes() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (entered, started) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (solved, runs) = mpsc::channel();
    let seen = Arc::new(Mutex::new(vec![]));
    let record = seen.clone();
    let job = manager
        .submit(
            1,
            1,
            daemon(),
            move |p, _, _| {
                solved.send(*p).unwrap();
                Ok(*p)
            },
            move |_, n| {
                if *n == 1 {
                    entered.send(()).unwrap();
                    resume.recv_timeout(WAIT).unwrap();
                }
                record.lock().unwrap().push(*n);
            },
        )
        .unwrap();
    started.recv_timeout(WAIT).unwrap();
    assert_eq!(runs.recv_timeout(WAIT).unwrap(), 1);
    let mut receipts = vec![];
    for n in 2..=4 {
        receipts.push(
            job.add_problem_change(move |p| {
                *p = n;
                Ok(())
            })
            .unwrap(),
        );
        assert_eq!(runs.recv_timeout(WAIT).unwrap(), n);
    }
    assert!(receipts.iter().all(|r| r.try_result().is_none()));
    release.send(()).unwrap();
    for receipt in receipts {
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    }
    job.cancel();
    job.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0], 1);
    assert_eq!(seen.last(), Some(&4));
    assert!(seen.len() < 4);
    manager.shutdown().unwrap();
}
#[test]
fn queued_changes_run_before_first_solve_and_final_only_receipts_wait_for_final_consumer() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (entered, started) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let first = manager
        .submit(
            1,
            0,
            LiveSolverConfig::default(),
            move |p, _, _| {
                entered.send(()).unwrap();
                resume.recv_timeout(WAIT).unwrap();
                Ok(*p)
            },
            |_, _| {},
        )
        .unwrap();
    started.recv_timeout(WAIT).unwrap();
    let delivered = Arc::new(AtomicUsize::new(0));
    let seen = delivered.clone();
    let second = manager
        .submit(
            2,
            4,
            LiveSolverConfig {
                delivery: LiveDelivery::Final,
                ..Default::default()
            },
            |p, old, _| {
                assert!(old.is_none());
                Ok(*p)
            },
            move |_, n| {
                seen.store(*n, Ordering::SeqCst);
            },
        )
        .unwrap();
    assert_eq!(second.status(), SolverStatus::Scheduled);
    let receipt = manager
        .add_problem_change(2, |p| {
            *p += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(receipt.try_result(), None);
    release.send(()).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(delivered.load(Ordering::SeqCst), 5);
    assert!(first.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(
        *second
            .wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .solution,
        5
    );
    manager.shutdown().unwrap();
}
#[test]
fn queued_cancellation_fails_receipts_and_stale_handles_cannot_change_reused_ids() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (entered, started) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let first = manager
        .submit(
            1,
            0,
            LiveSolverConfig::default(),
            move |p, _, _| {
                entered.send(()).unwrap();
                resume.recv_timeout(WAIT).unwrap();
                Ok(*p)
            },
            |_, _| {},
        )
        .unwrap();
    started.recv_timeout(WAIT).unwrap();
    let old = manager
        .submit(
            2,
            1,
            daemon(),
            |_, _, _| panic!("cancelled job ran"),
            |_, _| panic!("cancelled callback"),
        )
        .unwrap();
    let receipt = old
        .add_problem_change(|_| panic!("cancelled change ran"))
        .unwrap();
    assert!(old.cancel());
    assert_eq!(
        receipt.wait_timeout(WAIT),
        Some(Err(ProblemChangeFailure::Cancelled))
    );
    assert!(matches!(
        old.wait_timeout(WAIT).unwrap().as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    let new = manager
        .submit(
            2,
            9,
            LiveSolverConfig::default(),
            |p, _, _| Ok(*p),
            |_, _| {},
        )
        .unwrap();
    assert!(matches!(
        old.add_problem_change(|p| {
            *p = 77;
            Ok(())
        }),
        Err(ProblemChangeSubmitError::Unavailable { problem_id: 2 })
    ));
    assert!(!old.cancel());
    release.send(()).unwrap();
    first.wait_timeout(WAIT).unwrap();
    assert_eq!(
        *new.wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .solution,
        9
    );
    assert!(matches!(
        manager.add_problem_change(999, |_| Ok(())),
        Err(ProblemChangeSubmitError::Unavailable { problem_id: 999 })
    ));
    manager.shutdown().unwrap();
}
#[test]
fn failures_settle_change_receipts_and_do_not_corrupt_delivered_snapshots() {
    for mode in 0..4 {
        let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
        let (sent, seen) = mpsc::channel();
        let job = manager
            .submit(
                1,
                1,
                daemon(),
                move |p, _, _| {
                    if *p == 2 {
                        if mode == 1 {
                            return Err("solve failed".into());
                        }
                        if mode == 2 {
                            panic!("solve panic");
                        }
                    }
                    Ok(*p)
                },
                move |_, n| {
                    if mode == 3 && *n == 2 {
                        panic!("consumer panic");
                    }
                    sent.send(*n).unwrap();
                },
            )
            .unwrap();
        assert_eq!(seen.recv_timeout(WAIT).unwrap(), 1);
        let receipt = job
            .add_problem_change(move |p| {
                *p = 2;
                if mode == 0 {
                    Err("change failed".into())
                } else {
                    Ok(())
                }
            })
            .unwrap();
        let r = receipt.wait_timeout(WAIT).unwrap();
        assert!(matches!(
            (mode, r),
            (0, Err(ProblemChangeFailure::ChangeFailed(_)))
                | (1 | 2, Err(ProblemChangeFailure::SolveFailed(_)))
                | (3, Err(ProblemChangeFailure::ConsumerPanicked(_)))
        ));
        let r = job.wait_timeout(WAIT).unwrap();
        if mode == 3 {
            assert!(r.is_ok());
            assert_eq!(job.callback_failures()[0].stage, SolverCallbackStage::Best);
        } else {
            assert!(r.is_err());
        }
        assert!(seen.try_recv().is_err());
        manager.shutdown().unwrap();
    }
}
#[test]
fn daemon_cancellation_and_manager_drop_wake_idle_jobs_without_an_extra_solve() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let (sent, seen) = mpsc::channel();
    let manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let job = manager
        .submit(
            1,
            1,
            daemon(),
            move |p, _, _| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(*p)
            },
            move |_, _| sent.send(()).unwrap(),
        )
        .unwrap();
    seen.recv_timeout(WAIT).unwrap();
    drop(manager);
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn a_change_interrupts_native_scoring_and_restarts_with_fresh_limits() {
    let mut manager = LiveSolverManager::<usize, (bool, bool, u64), String>::new(1).unwrap();
    let (entered, scoring) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit(
            1,
            1,
            daemon(),
            move |p, old, token| {
                let problem = FiniteProblem::new(vec![Variable::new(vec![1usize, 2])]).unwrap();
                let first = std::cell::Cell::new(true);
                let mut network = ConstraintNetwork::new(1);
                network
                    .add_constraint("score", vec![0], |v| {
                        if *p == 1 && first.replace(false) {
                            entered.send(()).unwrap();
                            resume.recv_timeout(WAIT).unwrap();
                        }
                        SimpleScore::new(v[0].copied().unwrap_or(0) as i64)
                    })
                    .unwrap();
                if *p == 2 {
                    assert!(old.is_some());
                    assert!(!token.is_cancelled());
                }
                let r = solve(
                    &problem,
                    &network,
                    SolverConfig {
                        limits: SolverLimits {
                            cancellation: Some(token),
                            max_evaluations: 3,
                            ..Default::default()
                        },
                        local_search: vec![],
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
                Ok((token.is_cancelled(), r.complete, r.work.evaluations))
            },
            move |_, r| sent.send(*r).unwrap(),
        )
        .unwrap();
    scoring.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(|p| {
            *p = 2;
            Ok(())
        })
        .unwrap();
    release.send(()).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    job.cancel();
    let result = job.wait_timeout(WAIT).unwrap();
    assert_eq!(
        *result.as_ref().as_ref().unwrap().solution,
        (false, true, 3)
    );
    let delivered: Vec<_> = seen.try_iter().collect();
    assert_eq!(delivered.last(), Some(&(false, true, 3)));
    manager.shutdown().unwrap();
}

#[test]
fn cancellation_during_change_delivers_applied_revision_and_rejects_later_work() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit(
            1,
            1,
            daemon(),
            |p, _, _| Ok(*p),
            move |_, r| sent.send(*r).unwrap(),
        )
        .unwrap();
    seen.recv_timeout(WAIT).unwrap();
    let (entered, applying) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let applied = job
        .add_problem_change(move |p| {
            entered.send(()).unwrap();
            resume.recv_timeout(WAIT).unwrap();
            *p = 2;
            Ok(())
        })
        .unwrap();
    applying.recv_timeout(WAIT).unwrap();
    let skipped = job
        .add_problem_change(|_| panic!("cancelled change applied"))
        .unwrap();
    assert!(manager.terminate_early(1));
    release.send(()).unwrap();
    assert_eq!(applied.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(
        skipped.wait_timeout(WAIT),
        Some(Err(ProblemChangeFailure::Cancelled))
    );
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), 2);
    assert_eq!(
        job.wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .revision,
        1
    );
    manager.shutdown().unwrap();
}

#[test]
fn final_delivery_in_daemon_mode_waits_for_explicit_termination() {
    for panic_consumer in [false, true] {
        let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
        let (sent, seen) = mpsc::channel();
        let job = manager
            .submit(
                1,
                1,
                LiveSolverConfig {
                    daemon: true,
                    delivery: LiveDelivery::Final,
                },
                move |p, _, _| {
                    sent.send(*p).unwrap();
                    Ok(*p)
                },
                move |_, r| {
                    assert_eq!(*r, 2);
                    assert!(!panic_consumer, "final panic");
                },
            )
            .unwrap();
        assert_eq!(seen.recv_timeout(WAIT).unwrap(), 1);
        let receipt = job
            .add_problem_change(|p| {
                *p = 2;
                Ok(())
            })
            .unwrap();
        assert_eq!(seen.recv_timeout(WAIT).unwrap(), 2);
        assert_eq!(receipt.try_result(), None);
        manager.request_shutdown();
        let r = receipt.wait_timeout(WAIT).unwrap();
        if panic_consumer {
            assert!(matches!(r, Err(ProblemChangeFailure::ConsumerPanicked(_))));
        } else {
            assert_eq!(r, Ok(()));
        }
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        assert_eq!(job.callback_failures().len(), usize::from(panic_consumer));
        manager.shutdown().unwrap();
    }
}

#[test]
fn concurrent_shutdown_cannot_leave_a_new_daemon_waiting() {
    for _ in 0..40 {
        let manager = Arc::new(LiveSolverManager::<usize, usize, String>::new(1).unwrap());
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let producer = manager.clone();
        let start = barrier.clone();
        let thread = std::thread::spawn(move || {
            start.wait();
            producer.submit(1, 1, daemon(), |p, _, _| Ok(*p), |_, _| {})
        });
        barrier.wait();
        manager.request_shutdown();
        match thread.join().unwrap() {
            Ok(job) => {
                assert!(
                    job.wait_timeout(WAIT).is_some(),
                    "shutdown must wake the daemon"
                );
            }
            Err(e) => assert_eq!(e, SubmitError::Closed),
        }
        let mut manager = Arc::try_unwrap(manager).ok().unwrap();
        manager.shutdown().unwrap();
    }
}
