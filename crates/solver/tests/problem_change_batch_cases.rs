// Adapted from pinned Community BasicPlumbingTerminationTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
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

fn deferred_changes(increments: &[usize], expected: usize) {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let (started, starting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let runs = calls.clone();
    let job = manager
        .submit(
            1,
            0,
            LiveSolverConfig::default(),
            move |p, _, token| {
                if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                    started.send(token.clone()).unwrap();
                    released.recv_timeout(WAIT).unwrap();
                } else {
                    assert!(!token.is_cancelled());
                }
                Ok(*p)
            },
            |_, _| {},
        )
        .unwrap();
    let token = starting.recv_timeout(WAIT).unwrap();
    assert!(!token.is_cancelled());
    let mut batch = ProblemChangeBatch::new();
    for &increment in increments {
        let count = count.clone();
        batch.push(move |p| {
            count.fetch_add(increment, Ordering::SeqCst);
            *p += increment;
            Ok(())
        });
    }
    let receipt = batch.submit(&job).unwrap();
    assert!(token.is_cancelled());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(receipt.try_result(), None);
    release.send(()).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    let result = job.wait_timeout(WAIT).unwrap();
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!(result.revision, 1);
    assert_eq!(*result.solution, expected);
    assert_eq!(count.load(Ordering::SeqCst), expected);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(job.status(), SolverStatus::NotSolving);
    manager.shutdown().unwrap();
}
#[test]
fn add_problem_change_without_daemon() {
    deferred_changes(&[1], 1);
}
#[test]
fn add_problem_changes_without_daemon() {
    deferred_changes(&[1, 20], 21);
}
