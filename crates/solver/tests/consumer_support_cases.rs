// Adapted from pinned Timefold Community ConsumerSupportTest and TestdataSolution.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);

#[derive(Debug)]
struct Solution {
    values: Vec<usize>,
    entities: Vec<usize>,
}
fn generated(values: usize, entities: usize) -> Arc<Solution> {
    Arc::new(Solution {
        values: (0..values).collect(),
        entities: (0..entities).map(|i| i % values).collect(),
    })
}
fn default_solution() -> Arc<Solution> {
    generated(5, 7)
}
// A real queued live job replaces the mocked Solver in addProblemChange. The
// occupied worker lets the receipt be registered before any solution is published.
type Manager = LiveSolverManager<(), Arc<Solution>, String>;
type Job = LiveSolverJob<(), Arc<Solution>, String>;
fn occupy_worker(manager: &Manager) -> (Job, mpsc::Sender<()>) {
    let (started, starting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let job = manager
        .submit(
            0,
            (),
            Default::default(),
            move |_, _, _| {
                started.send(()).unwrap();
                released.recv_timeout(WAIT).unwrap();
                Ok(default_solution())
            },
            |_, _| {},
        )
        .unwrap();
    starting.recv_timeout(WAIT).unwrap();
    (job, release)
}

#[test]
fn skip_ahead() {
    let mut manager = SolverManager::<Arc<Solution>, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (published, publications) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let consumed = Arc::new(Mutex::new(Vec::new()));
    let recorded = consumed.clone();
    let producer = SolverEventProducer {
        phase: 0,
        kind: SolverPhaseKind::Construction,
    };
    let mut first = true;
    let job = manager
        .submit_with_progress(
            1,
            move |_, progress| {
                assert!(progress.publish_from(generated(1, 1), producer));
                starting.recv_timeout(WAIT).unwrap();
                assert!(progress.publish_from(generated(2, 2), producer));
                let last = generated(3, 3);
                assert!(progress.publish_from(last.clone(), producer));
                published.send(()).unwrap();
                Ok(last)
            },
            SolverJobCallbacks::<Arc<Solution>, String>::default().on_best_event(
                move |id, event| {
                    assert_eq!(id, 1);
                    assert_eq!(event.producer, Some(producer));
                    if first {
                        first = false;
                        started.send(()).unwrap();
                        released.recv_timeout(WAIT).unwrap();
                    }
                    recorded.lock().unwrap().push(event.solution.clone());
                },
            ),
        )
        .unwrap();
    publications.recv_timeout(WAIT).unwrap();
    release.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    let consumed = consumed.lock().unwrap();
    assert_eq!(consumed.len(), 2);
    assert_eq!(consumed[0].entities.len(), 1);
    assert_eq!(consumed[1].entities.len(), 3);
    assert_eq!(consumed[0].values.len(), 1);
    assert_eq!(consumed[1].values.len(), 3);
    assert!(job.callback_failures().is_empty());
    drop(consumed);
    manager.shutdown().unwrap();
}

#[test]
fn problem_changes_complete_after_final_best_solution_is_consumed() {
    let mut manager = Manager::new(1).unwrap();
    let (blocker, release_worker) = occupy_worker(&manager);
    let (published, publications) = mpsc::channel();
    let (finish, finished) = mpsc::channel();
    let final_solution = default_solution();
    let returned = final_solution.clone();
    let final_reference = Arc::new(Mutex::new(None));
    let recorded = final_reference.clone();
    let job = manager
        .submit_with_progress(
            1,
            (),
            LiveSolverConfig {
                delivery: LiveDelivery::Final,
                ..Default::default()
            },
            move |_, _, _, progress| {
                assert_eq!(progress.revision(), 1);
                assert!(!progress.publish(default_solution()));
                published.send(()).unwrap();
                finished.recv_timeout(WAIT).unwrap();
                Ok(returned.clone())
            },
            move |_, solution| {
                *recorded.lock().unwrap() = Some(solution.clone());
            },
        )
        .unwrap();
    let receipt = job.add_problem_change(|_| Ok(())).unwrap();
    release_worker.send(()).unwrap();
    publications.recv_timeout(WAIT).unwrap();
    assert!(receipt.try_result().is_none());
    assert!(final_reference.lock().unwrap().is_none());
    finish.send(()).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    assert!(Arc::ptr_eq(
        final_reference.lock().unwrap().as_ref().unwrap(),
        &final_solution
    ));
    assert_eq!(receipt.try_result(), Some(Ok(())));
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert!(blocker.wait_timeout(WAIT).unwrap().is_ok());
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}

#[test]
fn problem_changes_complete_exceptionally_after_exception_in_consumer() {
    let mut manager = Manager::new(1).unwrap();
    let (blocker, release_worker) = occupy_worker(&manager);
    let (finish, finished) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            (),
            Default::default(),
            move |_, _, token, progress| {
                assert_eq!(progress.revision(), 1);
                let solution = default_solution();
                assert!(progress.publish(solution.clone()));
                finished.recv_timeout(WAIT).unwrap();
                assert!(token.is_cancelled());
                Ok(solution)
            },
            |_, _| panic!("Test exception"),
        )
        .unwrap();
    let receipt = job.add_problem_change(|_| Ok(())).unwrap();
    release_worker.send(()).unwrap();
    let expected = Err(ProblemChangeFailure::ConsumerPanicked(
        "Test exception".into(),
    ));
    assert_eq!(receipt.wait_timeout(WAIT), Some(expected.clone()));
    assert_eq!(receipt.try_result(), Some(expected));
    finish.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert!(blocker.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(
        job.callback_failures(),
        vec![SolverCallbackFailure {
            stage: SolverCallbackStage::Best,
            message: "Test exception".into(),
        }]
    );
    manager.shutdown().unwrap();
}
