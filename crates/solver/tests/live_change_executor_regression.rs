//! Original real queue/executor/revision tests; no source-method credit.
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
#[test]
fn executor_cannot_acknowledge_skipped_or_failed_edits() {
    for skip in [true, false] {
        let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
        let (ended, first_ended) = mpsc::channel();
        let job = manager
            .submit_with_change_executor(
                1,
                4,
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                move |p, _, _, _| {
                    ended.send(()).unwrap();
                    Ok(*p)
                },
                move |op| {
                    assert_eq!(*op.problem(), 4);
                    assert_eq!(*op.previous().unwrap().solution, 4);
                    if !skip {
                        let _ = op.apply();
                    } // Ignoring the edit's failure must not acknowledge it.
                    Ok(())
                },
                LiveSolverCallbacks::new(|_, _| {}),
            )
            .unwrap();
        first_ended.recv_timeout(WAIT).unwrap();
        let receipt = job
            .add_problem_change(|_| Err("queued failure".into()))
            .unwrap();
        let error = receipt.wait_timeout(WAIT).unwrap().unwrap_err();
        assert!(
            matches!(error, ProblemChangeFailure::ChangeFailed(ref s) if s == if skip { "problem change executor did not apply the edit" } else { "queued failure" })
        );
        assert!(job.wait_timeout(WAIT).unwrap().is_err());
        manager.shutdown().unwrap();
    }
}
#[test]
fn ordered_edits_share_previous_result_but_observe_current_problem_and_apply_once() {
    let mut manager = LiveSolverManager::<Vec<usize>, Vec<usize>, String>::new(1).unwrap();
    let (ended, first_ended) = mpsc::channel();
    let (continue_run, released) = mpsc::channel();
    let seen = Arc::new(Mutex::new(vec![]));
    let log = seen.clone();
    let job = manager
        .submit_with_change_executor(
            1,
            vec![1],
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            move |p, old, _, _| {
                if old.is_none() {
                    ended.send(()).unwrap();
                    released.recv_timeout(WAIT).unwrap();
                }
                Ok(p.clone())
            },
            move |op| {
                log.lock().unwrap().push((
                    op.problem().clone(),
                    op.previous().unwrap().solution.as_ref().clone(),
                ));
                op.apply()?;
                assert_eq!(
                    op.apply().unwrap_err(),
                    "problem change was already applied"
                );
                Ok(())
            },
            LiveSolverCallbacks::new(|_, _| {}),
        )
        .unwrap();
    first_ended.recv_timeout(WAIT).unwrap();
    let a = job
        .add_problem_change(|p| {
            p.push(2);
            Ok(())
        })
        .unwrap();
    let b = job
        .add_problem_change(|p| {
            p.push(3);
            Ok(())
        })
        .unwrap();
    continue_run.send(()).unwrap();
    assert_eq!(a.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(b.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(
        *seen.lock().unwrap(),
        vec![(vec![1], vec![1]), (vec![1, 2], vec![1])]
    );
    job.cancel();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}
#[test]
fn executor_failure_prevents_restarted_publication_and_fails_later_receipts() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (ended, first_ended) = mpsc::channel();
    let (continue_run, released) = mpsc::channel();
    let events = Arc::new(Mutex::new(vec![]));
    let delivered = events.clone();
    let job = manager
        .submit_with_change_executor(
            1,
            1,
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            move |p, _, _, _| {
                ended.send(()).unwrap();
                released.recv_timeout(WAIT).unwrap();
                Ok(*p)
            },
            |op| {
                op.apply()?;
                Err("context release failed".into())
            },
            LiveSolverCallbacks::new(move |_, e| {
                delivered.lock().unwrap().push(e.solution.revision);
            }),
        )
        .unwrap();
    first_ended.recv_timeout(WAIT).unwrap();
    let a = job
        .add_problem_change(|p| {
            *p = 2;
            Ok(())
        })
        .unwrap();
    let b = job
        .add_problem_change(|_| panic!("later edit must not run"))
        .unwrap();
    continue_run.send(()).unwrap();
    assert!(
        matches!(a.wait_timeout(WAIT), Some(Err(ProblemChangeFailure::ChangeFailed(s))) if s == "context release failed")
    );
    assert!(b.wait_timeout(WAIT).unwrap().is_err());
    assert!(job.wait_timeout(WAIT).unwrap().is_err());
    manager.shutdown().unwrap();
    assert!(!events.lock().unwrap().contains(&1));
}
