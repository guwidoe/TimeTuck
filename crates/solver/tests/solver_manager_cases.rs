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
