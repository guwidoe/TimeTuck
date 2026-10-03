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
