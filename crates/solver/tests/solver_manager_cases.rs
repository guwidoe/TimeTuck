// Adapted from pinned Timefold Community SolverManagerTest and test fixtures.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use std::{
    sync::{Arc, Barrier, mpsc},
    time::Duration,
};
use timetuck::*;
const LIMIT: Duration = Duration::from_secs(10);
fn initialized() -> Result<Vec<Option<i64>>, String> {
    let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("dummy", vec![0, 1], |_| SimpleScore::ZERO)
        .unwrap();
    let result = greedy_construction(&problem, &network, ConstructionConfig::default()).unwrap();
    Ok(result.assignment.values().to_vec())
}
fn check_initialized(job: &SolverJob<Vec<Option<i64>>, String>) {
    let result = job.wait_timeout(LIMIT).expect("job must finish");
    let values = result.as_ref().as_ref().unwrap();
    assert!(!values.is_empty());
    assert!(values.iter().all(Option::is_some));
}
#[test]
fn solve_batch_two_in_parallel() {
    let mut manager = SolverManager::new(2).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let a = barrier.clone();
    let job1 = manager
        .submit(1, move |_| {
            a.wait();
            initialized()
        })
        .unwrap();
    let job2 = manager
        .submit(2, move |_| {
            barrier.wait();
            initialized()
        })
        .unwrap();
    check_initialized(&job1);
    check_initialized(&job2);
    manager.shutdown().unwrap();
}
#[test]
fn get_solver_status() {
    let mut manager = SolverManager::new(1).unwrap();
    let (started, ready) = mpsc::channel();
    let (release1, hold1) = mpsc::channel();
    let (release2, hold2) = mpsc::channel();
    let tx = started.clone();
    let job1 = manager
        .submit(1, move |_| {
            tx.send(1).unwrap();
            hold1.recv_timeout(LIMIT).unwrap();
            initialized()
        })
        .unwrap();
    assert_eq!(ready.recv_timeout(LIMIT).unwrap(), 1);
    let job2 = manager
        .submit(2, move |_| {
            started.send(2).unwrap();
            hold2.recv_timeout(LIMIT).unwrap();
            initialized()
        })
        .unwrap();
    assert_eq!(manager.status(1), SolverStatus::Active);
    assert_eq!(job1.status(), SolverStatus::Active);
    assert_eq!(manager.status(2), SolverStatus::Scheduled);
    assert_eq!(job2.status(), SolverStatus::Scheduled);
    release1.send(()).unwrap();
    assert_eq!(ready.recv_timeout(LIMIT).unwrap(), 2);
    assert_eq!(manager.status(1), SolverStatus::NotSolving);
    assert_eq!(job1.status(), SolverStatus::NotSolving);
    assert_eq!(manager.status(2), SolverStatus::Active);
    assert_eq!(job2.status(), SolverStatus::Active);
    release2.send(()).unwrap();
    check_initialized(&job1);
    check_initialized(&job2);
    for job in [&job1, &job2] {
        assert_eq!(manager.status(job.problem_id()), SolverStatus::NotSolving);
        assert_eq!(job.status(), SolverStatus::NotSolving);
    }
    manager.shutdown().unwrap();
}
#[test]
fn run_same_id_processes_throws_illegal_state_exception() {
    let mut manager = SolverManager::new(1).unwrap();
    let (started, ready) = mpsc::channel();
    let (release, hold) = mpsc::channel();
    let job = manager
        .submit(1, move |_| {
            started.send(()).unwrap();
            hold.recv_timeout(LIMIT).unwrap();
            initialized()
        })
        .unwrap();
    ready.recv_timeout(LIMIT).unwrap();
    assert!(matches!(
        manager.submit(1, |_| initialized()),
        Err(SubmitError::AlreadySolving)
    ));
    assert!(
        SubmitError::AlreadySolving
            .to_string()
            .contains("already solving")
    );
    release.send(()).unwrap();
    check_initialized(&job);
    manager.shutdown().unwrap();
}

#[test]
fn exception_in_solver() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut manager = SolverManager::<Vec<Option<i64>>, String>::new(1).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let job = manager
        .submit_with_callbacks(
            1,
            |_| Err("exceptionInSolver".into()),
            SolverJobCallbacks::default().on_error(move |_, _| {
                seen.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .unwrap();
    let result = job.wait_timeout(LIMIT).unwrap();
    assert!(
        matches!(result.as_ref(),Err(SolverJobError::Solve(message)) if message=="exceptionInSolver")
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(manager.status(1), SolverStatus::NotSolving);
    assert_eq!(job.status(), SolverStatus::NotSolving);
    manager.shutdown().unwrap();
}
#[test]
fn exception_in_consumer() {
    use std::sync::Mutex;
    let mut manager = SolverManager::<Vec<Option<i64>>, String>::new(1).unwrap();
    let error = Arc::new(Mutex::new(None));
    let seen = error.clone();
    let (tx, rx) = mpsc::channel();
    let job = manager
        .submit_with_callbacks(
            1,
            |_| initialized(),
            SolverJobCallbacks::default()
                .on_final(|_, _| panic!("exceptionInConsumer"))
                .on_error(move |_, failure| {
                    if let SolverJobFailure::Callback(f) = failure {
                        *seen.lock().unwrap() = Some(f.clone());
                    }
                    tx.send(()).unwrap();
                }),
        )
        .unwrap();
    rx.recv_timeout(LIMIT).unwrap();
    assert_eq!(
        error.lock().unwrap().as_ref().unwrap(),
        &SolverCallbackFailure {
            stage: SolverCallbackStage::Final,
            message: "exceptionInConsumer".into()
        }
    );
    check_initialized(&job);
    assert_eq!(manager.status(1), SolverStatus::NotSolving);
    assert_eq!(job.status(), SolverStatus::NotSolving);
    manager.shutdown().unwrap();
}
#[test]
fn test_start_job_consumer() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut manager = SolverManager::<Vec<Option<i64>>, String>::new(1).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let job = manager
        .submit_with_callbacks(
            1,
            |_| initialized(),
            SolverJobCallbacks::default().on_started(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .unwrap();
    check_initialized(&job);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    manager.shutdown().unwrap();
}

#[test]
fn skip_ahead() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut manager = SolverManager::<Vec<Option<i64>>, String>::new(1).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let best_count = count.clone();
    let finals = Arc::new(AtomicUsize::new(0));
    let final_count = finals.clone();
    let (first, first_received) = mpsc::channel();
    let (third, third_published) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            move |_, p| {
                let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2, 3, 4]); 4]).unwrap();
                let mut working = problem.initial_assignment();
                for variable in 0..4 {
                    problem
                        .apply(
                            &mut working,
                            &Move::Change {
                                variable,
                                value: Some(variable as i64 + 1),
                            },
                        )
                        .unwrap()
                        .commit();
                    assert!(p.publish(working.values().to_vec()));
                    // Force the source test's slow first consumer and skipped second update.
                    if variable == 0 {
                        first_received.recv_timeout(LIMIT).unwrap();
                    }
                    if variable == 2 {
                        third.send(()).unwrap();
                    }
                }
                Ok(working.values().to_vec())
            },
            SolverJobCallbacks::default()
                .on_best(move |_, v: &Vec<Option<i64>>| {
                    let first_event = best_count.fetch_add(1, Ordering::SeqCst) == 0;
                    if v[1].is_none() {
                        first.send(()).unwrap();
                        third_published.recv_timeout(LIMIT).unwrap();
                    } else if !first_event {
                        assert!(v[2].is_some(), "no skip ahead occurred");
                    }
                })
                .on_final(move |_, _| {
                    final_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
        .unwrap();
    check_initialized(&job);
    assert!(count.load(Ordering::SeqCst) < 4);
    assert_eq!(finals.load(Ordering::SeqCst), 1);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[derive(Default)]
struct InitialSolutionEvent {
    is_initialized: bool,
    producer_id: Option<(SolverPhaseKind, usize)>,
}
impl InitialSolutionEvent {
    fn read(&mut self, info: SolverInitialization) {
        self.is_initialized = !info.terminated_early;
        self.producer_id = Some((info.kind, info.phase));
    }
}
fn initialized_event_job(
    manager: &SolverManager<Vec<Option<i64>>, String>,
    plan: SolverPlan<'static>,
    event: Arc<std::sync::Mutex<InitialSolutionEvent>>,
) -> SolverJob<Vec<Option<i64>>, String> {
    manager
        .submit_with_progress(
            1,
            move |_, progress| {
                let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
                let mut network = ConstraintNetwork::new(2);
                network
                    .add_constraint("dummy", vec![0, 1], |_| SimpleScore::ZERO)
                    .unwrap();
                let result = solve_with_events(&problem, &network, plan, |event| {
                    if let SolverEvent::FirstInitialized {
                        initialization,
                        assignment,
                        ..
                    } = event
                    {
                        assert!(progress.publish_first_initialized(
                            assignment.values().to_vec(),
                            initialization
                        ));
                    }
                })
                .map_err(|e| e.to_string())?;
                Ok(result.assignment.values().to_vec())
            },
            SolverJobCallbacks::default()
                .on_first_initialized(move |_, _, info| event.lock().unwrap().read(info)),
        )
        .unwrap()
}
#[test]
fn first_initialized_solution_consumer_with_default_phases() {
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let mut manager = SolverManager::new(1).unwrap();
    let mut plan = SolverPlan::default();
    plan.limits.max_unimproved_time = Some(Duration::from_millis(1));
    plan.phases = vec![SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 10,
        ..Default::default()
    })];
    let job = initialized_event_job(&manager, plan, event.clone());
    check_initialized(&job);
    assert!(event.lock().unwrap().is_initialized);
    assert_eq!(
        event.lock().unwrap().producer_id,
        Some((SolverPhaseKind::Construction, 0))
    );
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}
#[test]
fn first_initialized_solution_consumer_with_single_ch_phase() {
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let mut manager = SolverManager::new(1).unwrap();
    let job = initialized_event_job(
        &manager,
        SolverPlan {
            phases: vec![],
            ..Default::default()
        },
        event.clone(),
    );
    check_initialized(&job);
    assert!(!event.lock().unwrap().is_initialized);
    event.lock().unwrap().is_initialized = false;
    assert_eq!(event.lock().unwrap().producer_id, None);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}
#[test]
fn first_initialized_solution_consumer_with_two_ch_and_ls() {
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let mut manager = SolverManager::new(1).unwrap();
    let mut plan = SolverPlan::default();
    plan.limits.max_unimproved_time = Some(Duration::from_millis(1));
    plan.phases
        .insert(0, SolverPhase::Construction(ConstructionConfig::default()));
    event.lock().unwrap().is_initialized = false;
    let job = initialized_event_job(&manager, plan, event.clone());
    check_initialized(&job);
    assert!(event.lock().unwrap().is_initialized);
    assert_eq!(
        event.lock().unwrap().producer_id,
        Some((SolverPhaseKind::Construction, 1))
    );
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn first_initialized_solution_consumer_with_single_ls_phase() {
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let seen = event.clone();
    let mut manager = SolverManager::<Vec<Option<i64>>, String>::new(1).unwrap();
    let job = manager
        .submit_with_progress(
            1,
            |_, progress| {
                let p =
                    FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1); 2]).unwrap();
                let mut n = ConstraintNetwork::new(2);
                n.add_constraint("dummy", vec![0, 1], |_| SimpleScore::ZERO)
                    .unwrap();
                let c = SolverSequence {
                    limits: SolverLimits {
                        best_score_limit: Some(SimpleScore::ZERO),
                        ..Default::default()
                    },
                    phases: vec![SolverPhase::LocalSearch(LocalSearchConfig::default())],
                };
                let r = solve_with_events(&p, &n, c, |event| {
                    if let SolverEvent::FirstInitialized {
                        initialization,
                        assignment,
                        ..
                    } = event
                    {
                        assert!(progress.publish_first_initialized(
                            assignment.values().to_vec(),
                            initialization
                        ));
                    }
                })
                .map_err(|e| e.to_string())?;
                assert_eq!(r.phases.len(), 1);
                assert_eq!(r.phases[0].index, 0);
                assert_eq!(r.termination, SolverTermination::BestScoreLimit);
                Ok(r.assignment.values().to_vec())
            },
            SolverJobCallbacks::default()
                .on_first_initialized(move |_, _, info| seen.lock().unwrap().read(info))
                .on_final(|_, _| {}),
        )
        .unwrap();
    check_initialized(&job);
    assert!(!event.lock().unwrap().is_initialized);
    assert_eq!(event.lock().unwrap().producer_id, None);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn first_initialized_solution_consumer_early_terminated_ch_list_var() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let called = Arc::new(AtomicBool::new(false));
    let seen_called = called.clone();
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let seen = event.clone();
    let mut manager = SolverManager::<ListAssignment<i64>, String>::new(1).unwrap();
    let job = manager
        .submit_with_progress(
            1,
            |_, progress| {
                let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![]); 2])
                    .unwrap()
                    .allowing_unassigned();
                let mut n = ListConstraintNetwork::new(2, 2);
                for index in 0..2 {
                    n.add_constraint(
                        format!("entity {index} size"),
                        vec![ListScope::Entity(index)],
                        |v| {
                            let ListFact::Entity(list) = &v[0] else {
                                panic!()
                            };
                            SimpleScore::new(-(list.len() as i64))
                        },
                    )
                    .unwrap();
                    n.add_constraint(
                        format!("value {index} unassigned"),
                        vec![ListScope::Value(index)],
                        |v| {
                            let ListFact::Value { element, .. } = &v[0] else {
                                panic!()
                            };
                            SimpleScore::new(-i64::from(element.is_none()))
                        },
                    )
                    .unwrap();
                }
                let c = SolverConfig {
                    construction: ConstructionConfig {
                        max_steps: Some(1),
                        optional: OptionalConstruction::NonWorsening,
                        ..Default::default()
                    },
                    local_search: vec![LocalSearchConfig {
                        max_steps: 0,
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                let r = solve_list_with_events(&p, &n, c, |event| {
                    if let SolverEvent::FirstInitialized {
                        initialization,
                        assignment,
                        ..
                    } = event
                    {
                        assert!(
                            progress.publish_first_initialized(assignment.clone(), initialization)
                        );
                    }
                })
                .map_err(|e| e.to_string())?;
                assert_eq!(r.phases.len(), 2);
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Construction(ConstructionTermination::StepLimit)
                );
                assert_eq!(r.phases[1].work.completed_steps, 0);
                assert_eq!(r.score, Some(SimpleScore::new(-2)));
                assert_eq!(r.assignment.lists().iter().map(Vec::len).sum::<usize>(), 1);
                Ok(r.assignment)
            },
            SolverJobCallbacks::default().on_first_initialized(move |_, _, info| {
                seen_called.store(true, Ordering::SeqCst);
                seen.lock().unwrap().read(info);
            }),
        )
        .unwrap();
    assert!(job.wait_timeout(LIMIT).unwrap().is_ok());
    assert!(called.load(Ordering::SeqCst));
    assert!(!event.lock().unwrap().is_initialized);
    assert_eq!(
        event.lock().unwrap().producer_id,
        Some((SolverPhaseKind::Construction, 0))
    );
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

fn custom_initialized_event_job(
    manager: &SolverManager<Vec<Option<i64>>, String>,
    phases: Vec<SolverPhase<'static>>,
    event: Arc<std::sync::Mutex<InitialSolutionEvent>>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
) -> SolverJob<Vec<Option<i64>>, String> {
    let command_event = event.clone();
    manager
        .submit_with_progress(
            1,
            move |_, progress| {
                let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
                let mut network = ConstraintNetwork::new(2);
                network
                    .add_constraint("dummy", vec![0, 1], |_| SimpleScore::ZERO)
                    .unwrap();
                // Deterministic replacement for the source's 1 ms stagnation timeout.
                let plan = SolverSequence {
                    phases,
                    limits: SolverLimits {
                        max_unimproved_steps: Some(1),
                        ..Default::default()
                    },
                };
                let result = solve_with_commands_and_events(
                    &problem,
                    &network,
                    plan,
                    move |_, _| {
                        assert!(!command_event.lock().unwrap().is_initialized);
                        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    },
                    |event| {
                        if let SolverEvent::FirstInitialized {
                            initialization,
                            assignment,
                            ..
                        } = event
                        {
                            assert!(progress.publish_first_initialized(
                                assignment.values().to_vec(),
                                initialization
                            ));
                        }
                    },
                )
                .map_err(|e| e.to_string())?;
                Ok(result.assignment.values().to_vec())
            },
            SolverJobCallbacks::default()
                .on_first_initialized(move |_, _, info| event.lock().unwrap().read(info)),
        )
        .unwrap()
}

#[test]
fn first_initialized_solution_consumer_with_custom_and_ch_and_ls() {
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut manager = SolverManager::new(1).unwrap();
    let job = custom_initialized_event_job(
        &manager,
        vec![
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
        ],
        event.clone(),
        calls.clone(),
    );
    check_initialized(&job);
    assert!(event.lock().unwrap().is_initialized);
    assert_eq!(
        event.lock().unwrap().producer_id,
        Some((SolverPhaseKind::Construction, 1))
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn first_initialized_solution_consumer_with_ch_and_custom_and_ls() {
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut manager = SolverManager::new(1).unwrap();
    let job = custom_initialized_event_job(
        &manager,
        vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
        ],
        event.clone(),
        calls.clone(),
    );
    check_initialized(&job);
    assert!(event.lock().unwrap().is_initialized);
    assert_eq!(
        event.lock().unwrap().producer_id,
        Some((SolverPhaseKind::Custom, 1))
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn first_initialized_solution_consumer_with_two_custom() {
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut manager = SolverManager::new(1).unwrap();
    let job = custom_initialized_event_job(
        &manager,
        vec![
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::Custom(CustomPhaseConfig::default()),
        ],
        event.clone(),
        calls.clone(),
    );
    assert!(job.wait_timeout(LIMIT).unwrap().as_ref().is_ok());
    assert!(!event.lock().unwrap().is_initialized);
    assert_eq!(event.lock().unwrap().producer_id, None);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn first_initialized_solution_consumer_early_terminated_ch() {
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let called = Arc::new(AtomicBool::new(false));
    let event = Arc::new(std::sync::Mutex::new(InitialSolutionEvent::default()));
    let finals = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut manager = SolverManager::<
        (Vec<Option<i64>>, Option<SimpleScore>),
        SolverError<LocalSearchError>,
    >::new(1)
    .unwrap();
    let seen = event.clone();
    let was_called = called.clone();
    let final_count = finals.clone();
    let job = manager
        .submit_with_progress(
            1,
            move |_, progress| {
                // Source generates two initialized required entities, then clears both.
                let problem = FiniteProblem::new(vec![Variable::new(vec![1i64, 2]); 2]).unwrap();
                let mut network = ConstraintNetwork::new(2);
                for i in 0..2 {
                    network
                        .add_constraint(format!("Always penalize {i}"), vec![i], |v| {
                            SimpleScore::new(-i64::from(v[0].is_some()))
                        })
                        .unwrap();
                }
                let c = SolverConfig {
                    limits: SolverLimits {
                        incomplete_handoff: IncompleteHandoffPolicy::Error,
                        ..Default::default()
                    },
                    construction: ConstructionConfig {
                        max_steps: Some(1),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let r = solve_with_events(&problem, &network, c, |e| {
                    if let SolverEvent::FirstInitialized {
                        initialization,
                        assignment,
                        score,
                        ..
                    } = e
                    {
                        assert!(progress.publish_first_initialized(
                            (assignment.values().to_vec(), score),
                            initialization
                        ));
                    }
                })?;
                Ok((r.assignment.values().to_vec(), r.score))
            },
            SolverJobCallbacks::default()
                .on_first_initialized(
                    move |_, snapshot: &(Vec<Option<i64>>, Option<SimpleScore>), info| {
                        was_called.store(true, Ordering::SeqCst);
                        seen.lock().unwrap().read(info);
                        assert_eq!(snapshot.0.iter().filter(|v| v.is_some()).count(), 1);
                        assert_eq!(snapshot.1, Some(SimpleScore::new(-1).with_structural(-1)));
                        entered_tx.send(()).unwrap();
                        release_rx.recv_timeout(LIMIT).unwrap();
                    },
                )
                .on_final(move |_, _| {
                    final_count.fetch_add(1, Ordering::SeqCst);
                }),
        )
        .unwrap();
    entered_rx.recv_timeout(LIMIT).unwrap();
    assert!(
        job.wait_timeout(Duration::ZERO).is_none(),
        "error result waits for initialization delivery"
    );
    release_tx.send(()).unwrap();
    let result = job.wait_timeout(LIMIT).unwrap();
    let error = result.as_ref().as_ref().unwrap_err();
    assert!(matches!(
        error,
        SolverJobError::Solve(SolverError::IncompleteLocalSearch { phase: 1 })
    ));
    let root = error.source().expect("typed solve error");
    assert!(root.is::<SolverError<LocalSearchError>>());
    assert!(root.source().is_none());
    assert!(
        root.to_string()
            .contains("needs to start from an initialized solution")
    );
    assert!(called.load(Ordering::SeqCst));
    assert!(!event.lock().unwrap().is_initialized);
    assert_eq!(
        event.lock().unwrap().producer_id,
        Some((SolverPhaseKind::Construction, 0))
    );
    assert_eq!(finals.load(Ordering::SeqCst), 0);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[derive(Clone, Debug)]
struct ChangedProblemSnapshot {
    values: Vec<i64>,
    assignment: Vec<Option<i64>>,
}
// PlannerTestUtils' four initially unassigned entities and constant-zero objective.
fn solve_changed_problem(
    values: &[i64],
    previous: Option<&ChangedProblemSnapshot>,
    token: &CancellationToken,
    pause: Option<&(std::sync::Mutex<bool>, std::sync::Condvar)>,
) -> Result<ChangedProblemSnapshot, String> {
    solve_changed_problem_measured(values, previous, token, pause).map(|run| run.solution)
}
fn solve_changed_problem_measured(
    values: &[i64],
    previous: Option<&ChangedProblemSnapshot>,
    token: &CancellationToken,
    pause: Option<&(std::sync::Mutex<bool>, std::sync::Condvar)>,
) -> Result<LiveRun<ChangedProblemSnapshot>, String> {
    let problem = FiniteProblem::new(
        (0..4)
            .map(|i| {
                let variable = Variable::new(values.to_vec());
                previous
                    .and_then(|p| p.assignment[i])
                    .filter(|v| values.contains(v))
                    .map_or(variable.clone(), |v| variable.with_initial(v))
            })
            .collect(),
    )
    .unwrap();
    let mut network = ConstraintNetwork::new(4);
    network
        .add_constraint("dummy", vec![0, 1, 2, 3], |_| SimpleScore::ZERO)
        .unwrap();
    let phases = if pause.is_some() {
        vec![
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::Construction(ConstructionConfig::default()),
        ]
    } else {
        vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 10,
                ..Default::default()
            }),
        ]
    };
    let result = solve_with_commands(
        &problem,
        &network,
        SolverSequence {
            phases,
            limits: SolverLimits {
                cancellation: Some(token),
                ..Default::default()
            },
        },
        |_, _| {
            let (lock, changed) = pause.unwrap();
            let (open, timeout) = changed
                .wait_timeout_while(lock.lock().unwrap(), LIMIT, |open| !*open)
                .unwrap();
            assert!(
                *open && !timeout.timed_out(),
                "paused custom phase must be released"
            );
            Ok(())
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(LiveRun {
        solution: ChangedProblemSnapshot {
            values: values.to_vec(),
            assignment: result.assignment.values().to_vec(),
        },
        work: result.work,
    })
}

#[test]
fn add_problem_change() {
    let mut manager =
        LiveSolverManager::<Vec<i64>, ChangedProblemSnapshot, String>::new(1).unwrap();
    let best = Arc::new(std::sync::Mutex::new(None));
    let seen = best.clone();
    let job = manager
        .submit(
            1,
            vec![1, 2, 3, 4],
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            |p, old, token| solve_changed_problem(p, old, token, None),
            move |_, s| *seen.lock().unwrap() = Some(s.clone()),
        )
        .unwrap();
    let receipt = manager
        .add_problem_change(1, |p| {
            p.push(5);
            Ok(())
        })
        .unwrap();
    assert_eq!(receipt.wait_timeout(LIMIT), Some(Ok(())));
    assert_eq!(receipt.try_result(), Some(Ok(())));
    assert_eq!(best.lock().unwrap().as_ref().unwrap().values.len(), 5);
    job.cancel();
    assert!(job.wait_timeout(LIMIT).unwrap().is_ok());
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn add_problem_change_to_non_existing_problem_fails_fast() {
    let mut manager =
        LiveSolverManager::<Vec<i64>, ChangedProblemSnapshot, String>::new(1).unwrap();
    let job = manager
        .submit(
            1,
            vec![1, 2, 3, 4],
            LiveSolverConfig::default(),
            |p, old, token| solve_changed_problem(p, old, token, None),
            |_, _| {},
        )
        .unwrap();
    let error = match manager.add_problem_change(999, |p| {
        p.push(5);
        Ok(())
    }) {
        Err(e) => e,
        Ok(_) => panic!("unknown ID accepted"),
    };
    assert_eq!(
        error,
        ProblemChangeSubmitError::Unavailable { problem_id: 999 }
    );
    assert!(error.to_string().contains("999"));
    assert!(job.wait_timeout(LIMIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}

#[test]
fn add_problem_change_to_waiting_solver() {
    let mut manager =
        LiveSolverManager::<Vec<i64>, ChangedProblemSnapshot, String>::new(1).unwrap();
    let latch = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let first_latch = latch.clone();
    let (entered, started) = mpsc::channel();
    let first = manager
        .submit(
            1,
            vec![1, 2, 3, 4],
            LiveSolverConfig::default(),
            move |p, old, token| {
                entered.send(()).unwrap();
                solve_changed_problem(p, old, token, Some(&first_latch))
            },
            |_, _| {},
        )
        .unwrap();
    started.recv_timeout(LIMIT).unwrap();
    let second_latch = latch.clone();
    let best = Arc::new(std::sync::Mutex::new(None));
    let seen = best.clone();
    let second = manager
        .submit(
            2,
            vec![1, 2, 3, 4],
            LiveSolverConfig {
                delivery: LiveDelivery::Final,
                ..Default::default()
            },
            move |p, old, token| solve_changed_problem(p, old, token, Some(&second_latch)),
            move |_, s| *seen.lock().unwrap() = Some(s.clone()),
        )
        .unwrap();
    assert_eq!(second.status(), SolverStatus::Scheduled);
    let receipt = manager
        .add_problem_change(2, |p| {
            p.push(5);
            Ok(())
        })
        .unwrap();
    assert_eq!(receipt.try_result(), None);
    *latch.0.lock().unwrap() = true;
    latch.1.notify_all();
    assert_eq!(receipt.wait_timeout(LIMIT), Some(Ok(())));
    assert_eq!(receipt.try_result(), Some(Ok(())));
    let snapshot = best.lock().unwrap().clone().unwrap();
    assert_eq!(snapshot.values.len(), 5);
    assert_eq!(snapshot.assignment.len(), 4);
    assert!(snapshot.assignment.iter().all(Option::is_some));
    assert!(first.wait_timeout(LIMIT).unwrap().is_ok());
    assert!(second.wait_timeout(LIMIT).unwrap().is_ok());
    assert!(second.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

fn owned_manager_problem(count: usize) -> Arc<ChangedProblemSnapshot> {
    Arc::new(ChangedProblemSnapshot {
        values: (1..=count as i64).collect(),
        assignment: vec![None; count],
    })
}
fn paused_owned_manager_job(
    manager: &SolverManager<Arc<ChangedProblemSnapshot>, String>,
    id: u64,
    input: Arc<ChangedProblemSnapshot>,
    started: mpsc::Sender<u64>,
    local: bool,
) -> SolverJob<Arc<ChangedProblemSnapshot>, String> {
    manager
        .submit_owned(
            id,
            input,
            move |input, token, _| {
                let count = input.assignment.len();
                let problem =
                    FiniteProblem::new(vec![Variable::new(input.values.clone()); count]).unwrap();
                let mut network = ConstraintNetwork::new(count);
                network
                    .add_constraint("dummy", (0..count).collect(), |_| SimpleScore::ZERO)
                    .unwrap();
                let mut phases = vec![
                    SolverPhase::Custom(CustomPhaseConfig {
                        max_time: Some(LIMIT),
                        ..Default::default()
                    }),
                    SolverPhase::Construction(ConstructionConfig::default()),
                ];
                if local {
                    phases.push(SolverPhase::LocalSearch(LocalSearchConfig::default()));
                }
                let result = solve_with_commands(
                    &problem,
                    &network,
                    SolverSequence {
                        phases,
                        limits: SolverLimits {
                            cancellation: Some(&token),
                            ..Default::default()
                        },
                    },
                    |_, context| {
                        started.send(id).unwrap();
                        // Deterministically keep the source's active job at its custom phase
                        // until cancellation; the timeout only bounds a failed test.
                        while !context.is_terminated() {
                            std::thread::yield_now();
                        }
                        assert!(token.is_cancelled(), "test must cancel before its timeout");
                        Ok(())
                    },
                )
                .map_err(|e| e.to_string())?;
                Ok(Arc::new(ChangedProblemSnapshot {
                    values: input.values.clone(),
                    assignment: result.assignment.values().to_vec(),
                }))
            },
            SolverJobCallbacks::default(),
        )
        .unwrap()
}
#[test]
fn terminate_early() {
    let mut manager = SolverManager::new(1).unwrap();
    let (started, ready) = mpsc::channel();
    let jobs: Vec<_> = (1..=3)
        .map(|id| {
            paused_owned_manager_job(
                &manager,
                id,
                owned_manager_problem(4),
                started.clone(),
                true,
            )
        })
        .collect();
    assert_eq!(ready.recv_timeout(LIMIT).unwrap(), 1);
    let status = |id: u64, expected| {
        assert_eq!(manager.status(id), expected);
        assert_eq!(jobs[id as usize - 1].status(), expected);
    };
    status(1, SolverStatus::Active);
    status(2, SolverStatus::Scheduled);
    status(3, SolverStatus::Scheduled);
    assert!(
        manager
            .terminate_early_and_wait(2)
            .unwrap()
            .unwrap()
            .is_ok()
    );
    status(1, SolverStatus::Active);
    status(2, SolverStatus::NotSolving);
    status(3, SolverStatus::Scheduled);
    assert!(
        manager
            .terminate_early_and_wait(1)
            .unwrap()
            .unwrap()
            .is_ok()
    );
    status(1, SolverStatus::NotSolving);
    assert_eq!(ready.recv_timeout(LIMIT).unwrap(), 3);
    status(3, SolverStatus::Active);
    assert!(
        manager
            .terminate_early_and_wait(3)
            .unwrap()
            .unwrap()
            .is_ok()
    );
    status(3, SolverStatus::NotSolving);
    assert!(jobs.iter().all(SolverJob::is_termination_requested));
    manager.shutdown().unwrap();
}
#[test]
fn terminate_solver_job_early_still_returns_best_solution() {
    let mut manager = SolverManager::new(1).unwrap();
    let (started, ready) = mpsc::channel();
    let job = paused_owned_manager_job(&manager, 1, owned_manager_problem(2), started, false);
    assert_eq!(ready.recv_timeout(LIMIT).unwrap(), 1);
    let result = job.cancel_and_wait().unwrap();
    let solution = result.as_ref().as_ref().unwrap();
    assert_eq!(solution.assignment.len(), 2);
    assert!(job.is_termination_requested());
    manager.shutdown().unwrap();
}
#[test]
fn terminate_scheduled_solver_job_early_returns_input_problem() {
    let mut manager = SolverManager::new(1).unwrap();
    let (started, ready) = mpsc::channel();
    let first = paused_owned_manager_job(
        &manager,
        1,
        owned_manager_problem(4),
        started.clone(),
        false,
    );
    assert_eq!(ready.recv_timeout(LIMIT).unwrap(), 1);
    let input = owned_manager_problem(4);
    let second = paused_owned_manager_job(&manager, 2, input.clone(), started, false);
    let result = second.cancel_and_wait().unwrap();
    assert!(Arc::ptr_eq(result.as_ref().as_ref().unwrap(), &input));
    assert!(second.is_termination_requested());
    assert_eq!(second.status(), SolverStatus::NotSolving);
    assert!(
        ready.try_recv().is_err(),
        "scheduled custom phase never started"
    );
    assert!(first.cancel_and_wait().unwrap().is_ok());
    manager.shutdown().unwrap();
}

#[test]
fn submit_more_problems_than_cpus_all_get_solved() {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let cpus = std::thread::available_parallelism().unwrap().get();
    let problem_count = cpus * 2;
    let mut manager = SolverManager::<Vec<Option<i64>>, String>::new(cpus).unwrap();
    for mode in 0..3 {
        let consumed = Arc::new(Mutex::new(vec![vec![]; problem_count]));
        let finals = Arc::new(AtomicUsize::new(0));
        let mut jobs = vec![];
        for id in 0..problem_count {
            let mut callbacks = SolverJobCallbacks::default();
            if mode == 1 {
                let seen = consumed.clone();
                callbacks = callbacks.on_best(move |id, v: &Vec<Option<i64>>| {
                    seen.lock().unwrap()[id as usize].push(v.clone())
                });
            }
            if mode != 0 {
                let seen = consumed.clone();
                let finished = finals.clone();
                callbacks = callbacks.on_final(move |id, v: &Vec<Option<i64>>| {
                    if mode == 2 {
                        seen.lock().unwrap()[id as usize].push(v.clone());
                    }
                    finished.fetch_add(1, Ordering::SeqCst);
                });
            }
            jobs.push(
                manager
                    .submit_with_progress(
                        id as u64,
                        |token, progress| {
                            let problem =
                                FiniteProblem::new(vec![Variable::new(vec![1i64, 2]); 2]).unwrap();
                            let mut score = ConstraintNetwork::new(2);
                            score
                                .add_constraint("dummy", vec![0, 1], |_| SimpleScore::ZERO)
                                .unwrap();
                            let plan = SolverSequence {
                                phases: vec![SolverPhase::Custom(CustomPhaseConfig::default()); 2],
                                limits: SolverLimits {
                                    cancellation: Some(&token),
                                    ..Default::default()
                                },
                            };
                            let r = solve_with_commands_and_events(
                                &problem,
                                &score,
                                plan,
                                |index, context| {
                                    context.execute(&Move::Change {
                                        variable: index,
                                        value: Some(index as i64 + 1),
                                    })?;
                                    Ok(())
                                },
                                |event| {
                                    if let SolverEvent::Best(update) = event
                                        && update.assignment.values().iter().any(Option::is_some)
                                    {
                                        // The source observes the two assignment improvements,
                                        // not the native initial all-unassigned score snapshot.
                                        progress.publish(update.assignment.values().to_vec());
                                    }
                                },
                            )
                            .map_err(|e| e.to_string())?;
                            Ok(r.assignment.values().to_vec())
                        },
                        callbacks,
                    )
                    .unwrap(),
            );
        }
        for job in &jobs {
            check_initialized(job);
        }
        if mode != 0 {
            assert_eq!(finals.load(Ordering::SeqCst), problem_count);
        }
        let consumed = consumed.lock().unwrap();
        for solutions in consumed.iter() {
            match mode {
                0 => assert!(solutions.is_empty()),
                1 => {
                    assert!((1..=2).contains(&solutions.len()));
                    if solutions.len() == 2 {
                        assert_eq!(solutions[0], vec![Some(1), None]);
                    }
                    assert_eq!(solutions.last().unwrap(), &vec![Some(1), Some(2)]);
                }
                2 => assert_eq!(solutions, &vec![vec![Some(1), Some(2)]]),
                _ => unreachable!(),
            }
        }
    }
    manager.shutdown().unwrap();
}

fn statistics_problem(entities: usize, values: Vec<i64>) -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(values); entities]).unwrap()
}
fn statistics_job(
    manager: &SolverManager<Vec<Option<i64>>, String>,
    max_evaluations: u64,
) -> SolverJob<Vec<Option<i64>>, String> {
    let problem = statistics_problem(2, vec![1, 2]);
    let size = problem.problem_size_statistics();
    manager
        .submit_with_statistics(
            2,
            size,
            move |token, progress| {
                let mut network = ConstraintNetwork::new(2);
                network
                    .add_constraint("dummy", vec![0, 1], |_| SimpleScore::ZERO)
                    .unwrap();
                let result = solve(
                    &problem,
                    &network,
                    SolverConfig {
                        limits: SolverLimits {
                            max_evaluations,
                            cancellation: Some(&token),
                            ..Default::default()
                        },
                        local_search: vec![LocalSearchConfig {
                            max_steps: 10,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
                assert!(progress.record_work(result.work));
                Ok(result.assignment.values().to_vec())
            },
            SolverJobCallbacks::default(),
        )
        .unwrap()
}
#[test]
fn test_score_calculation_count_for_finished_job() {
    let mut manager = SolverManager::new(1).unwrap();
    let job = statistics_job(&manager, 5);
    assert!(job.wait_timeout(LIMIT).unwrap().is_ok());
    let statistics = job.statistics();
    let work = statistics.work.unwrap();
    assert_eq!(work.evaluations, 5);
    assert_eq!(work.move_evaluations, 4);
    assert!(statistics.solving_duration >= Duration::ZERO);
    assert!(statistics.score_calculation_speed().is_some());
    assert!(statistics.move_evaluation_speed().is_some());
    // Native Duration/u64 make the source nonnegative checks structural.
    assert!(!statistics.solving);
    assert!(!statistics.work_overflowed);
    manager.shutdown().unwrap();
}
#[test]
fn test_problem_size_statistics_for_finished_job() {
    let mut manager = SolverManager::new(1).unwrap();
    let job = statistics_job(&manager, 100_000);
    check_initialized(&job);
    let statistics = job.statistics().problem_size.unwrap();
    assert_eq!(statistics.size.entity_count, 2);
    assert_eq!(statistics.size.variable_count, 2);
    assert_eq!(statistics.size.value_count, 2);
    assert_eq!(statistics.size.assignment_space.decimal(), "4");
    assert!(statistics.size.assignment_space.exact);
    manager.shutdown().unwrap();
}
#[test]
fn test_problem_size_statistics_for_waiting_job() {
    let mut manager =
        LiveSolverManager::<Vec<i64>, ChangedProblemSnapshot, String>::new(1).unwrap();
    let latch = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let first_latch = latch.clone();
    let (entered, started) = mpsc::channel();
    let first = manager
        .submit(
            1,
            vec![1, 2, 3, 4],
            LiveSolverConfig::default(),
            move |p, old, token| {
                entered.send(()).unwrap();
                solve_changed_problem(p, old, token, Some(&first_latch))
            },
            |_, _| {},
        )
        .unwrap();
    started.recv_timeout(LIMIT).unwrap();
    let second_latch = latch.clone();
    let best = Arc::new(std::sync::Mutex::new(None));
    let seen = best.clone();
    let second = manager
        .submit_with_statistics(
            2,
            vec![1, 2, 3, 4],
            LiveSolverConfig::default(),
            |p| statistics_problem(4, p.clone()).problem_size_statistics(),
            move |p, old, token| solve_changed_problem_measured(p, old, token, Some(&second_latch)),
            move |_, s| *seen.lock().unwrap() = Some(s.clone()),
        )
        .unwrap();
    assert_eq!(second.status(), SolverStatus::Scheduled);
    let before = second.statistics().problem_size.unwrap();
    assert_eq!(before.revision, 0);
    assert_eq!(before.size.entity_count, 4);
    assert_eq!(before.size.variable_count, 4);
    assert_eq!(before.size.value_count, 4);
    assert_eq!(before.size.assignment_space.decimal(), "256");
    let receipt = manager
        .add_problem_change(2, |p| {
            p.push(5);
            Ok(())
        })
        .unwrap();
    *latch.0.lock().unwrap() = true;
    latch.1.notify_all();
    assert_eq!(receipt.wait_timeout(LIMIT), Some(Ok(())));
    assert_eq!(receipt.try_result(), Some(Ok(())));
    assert_eq!(best.lock().unwrap().as_ref().unwrap().values.len(), 5);
    let after = second.statistics().problem_size.unwrap();
    assert_eq!(after.revision, 1);
    assert_eq!(after.size.entity_count, 4);
    assert_eq!(after.size.variable_count, 4);
    assert_eq!(after.size.value_count, 5);
    assert_eq!(after.size.assignment_space.decimal(), "625");
    assert_eq!(
        before.size.assignment_space.decimal(),
        "256",
        "retained metadata must be immutable"
    );
    assert!(first.wait_timeout(LIMIT).unwrap().is_ok());
    assert!(second.wait_timeout(LIMIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}

#[test]
fn thread_factory_is_used() {
    let mut manager =
        SolverManager::<Vec<Option<i64>>, String>::with_thread_factory(1, 1, |_, task| {
            std::thread::Builder::new()
                .name("CustomThread".into())
                .spawn(task)
        })
        .unwrap();
    let job = manager
        .submit(1, |token| {
            let problem = statistics_problem(4, vec![1, 2, 3, 4]);
            let mut network = ConstraintNetwork::new(4);
            network
                .add_constraint("dummy", vec![0, 1, 2, 3], |_| SimpleScore::ZERO)
                .unwrap();
            let result = solve_with_commands(
                &problem,
                &network,
                SolverSequence {
                    phases: vec![
                        SolverPhase::Custom(CustomPhaseConfig::default()),
                        SolverPhase::Construction(ConstructionConfig::default()),
                    ],
                    limits: SolverLimits {
                        cancellation: Some(&token),
                        ..Default::default()
                    },
                },
                |_, _| {
                    assert_eq!(
                        std::thread::current().name(),
                        Some("CustomThread"),
                        "Custom thread factory not used"
                    );
                    Ok(())
                },
            )
            .map_err(|e| e.to_string())?;
            Ok(result.assignment.values().to_vec())
        })
        .unwrap();
    check_initialized(&job);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

// DEFAULT_PROBLEM_FINDER and PlannerTestUtils config use two entities/values,
// a constant-zero scorer, construction and local search bounded by ten steps.
fn builder_problem_finder(id: u64) -> Result<FiniteProblem<i64>, String> {
    assert_eq!(id, 1);
    Ok(statistics_problem(2, vec![1, 2]))
}
fn builder_solve(
    problem: FiniteProblem<i64>,
    token: CancellationToken,
    progress: SolverProgress<Vec<Option<i64>>>,
) -> Result<Vec<Option<i64>>, String> {
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("dummy", vec![0, 1], |_| SimpleScore::ZERO)
        .unwrap();
    let result = solve_with_observer(
        &problem,
        &network,
        SolverConfig {
            limits: SolverLimits {
                cancellation: Some(&token),
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                max_steps: 10,
                ..Default::default()
            }],
            ..Default::default()
        },
        |update| {
            progress.publish_from(
                update.assignment.values().to_vec(),
                SolverEventProducer::from(&update),
            );
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(result.assignment.values().to_vec())
}
#[test]
fn solve_with_builder() {
    let mut manager = SolverManager::new(1).unwrap();
    let (sent, received) = mpsc::channel();
    let job = manager
        .job_builder(1, builder_solve)
        .with_problem_finder(builder_problem_finder)
        .with_callbacks(
            SolverJobCallbacks::default()
                .on_final(move |_, solution: &Vec<Option<i64>>| {
                    sent.send(solution.clone()).unwrap()
                })
                .on_error(|_, _| panic!("Solving failed.")),
        )
        .run()
        .unwrap();
    let final_best = received.recv_timeout(LIMIT).unwrap();
    assert!(!final_best.is_empty());
    check_initialized(&job);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}
#[test]
fn solve_and_listen_with_builder() {
    let mut manager = SolverManager::new(1).unwrap();
    let (sent, received) = mpsc::channel();
    let best = Arc::new(std::sync::Mutex::new(None));
    let seen = best.clone();
    let job = manager
        .job_builder(1, builder_solve)
        .with_problem_finder(builder_problem_finder)
        .with_callbacks(
            SolverJobCallbacks::default()
                .on_best_event(move |_, event: SolverBestEvent<'_, Vec<Option<i64>>>| {
                    *seen.lock().unwrap() = Some((event.solution.clone(), event.producer));
                })
                .on_final(move |_, solution: &Vec<Option<i64>>| {
                    sent.send(solution.clone()).unwrap()
                })
                .on_error(|_, _| panic!("Solving failed.")),
        )
        .run()
        .unwrap();
    let final_best = received.recv_timeout(LIMIT).unwrap();
    assert!(!final_best.is_empty());
    let best = best.lock().unwrap();
    let (solution, producer) = best.as_ref().expect("best solution must be consumed");
    assert!(!solution.is_empty());
    assert!(producer.is_some());
    assert_eq!(producer.unwrap().kind, SolverPhaseKind::Construction);
    assert_eq!(producer.unwrap().phase, 0);
    check_initialized(&job);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}
