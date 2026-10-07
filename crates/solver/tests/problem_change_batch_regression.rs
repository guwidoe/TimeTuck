//! Original grouped-change integration regressions; no upstream method credit.
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
struct Dropped(Arc<AtomicUsize>, bool);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
        assert!(!self.1, "capture destructor");
    }
}
#[test]
fn errors_and_panics_stop_later_edits_without_rolling_back_and_dispose_every_capture() {
    for panic in [false, true] {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut batch = ProblemChangeBatch::new();
        assert!(batch.is_empty());
        batch.push(|p| {
            *p = 7;
            Ok(())
        });
        batch.push(move |p| {
            *p += 1;
            assert!(!panic, "bad edit");
            Err("bad edit".into())
        });
        for explode in [true, true, false] {
            let owned = Dropped(drops.clone(), explode);
            batch.push(move |_| {
                drop(owned);
                panic!("must not execute")
            });
        }
        assert_eq!(batch.len(), 5);
        let mut problem = 0;
        let error = batch.apply(&mut problem).unwrap_err();
        assert_eq!(
            error,
            format!(
                "batch edit 2 {}: bad edit",
                if panic { "panicked" } else { "failed" }
            )
        );
        assert_eq!(problem, 8);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    }
}
#[test]
fn concurrently_submitted_edit_cannot_split_a_batch_and_receipts_wait_for_delivery() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (observed, observations) = mpsc::channel();
    let (delivering, delivery) = mpsc::channel();
    let (finish, finished) = mpsc::channel();
    let job = manager
        .submit(
            1,
            0,
            Default::default(),
            move |p, _, _| {
                observed.send(*p).unwrap();
                if *p == 0 {
                    started.send(()).unwrap();
                    released.recv_timeout(WAIT).unwrap();
                }
                Ok(*p)
            },
            move |_, p| {
                if *p == 123 {
                    delivering.send(()).unwrap();
                    finished.recv_timeout(WAIT).unwrap();
                }
            },
        )
        .unwrap();
    starting.recv_timeout(WAIT).unwrap();
    let (editing, edit_started) = mpsc::channel();
    let (next, next_edit) = mpsc::channel();
    let mut batch = ProblemChangeBatch::new();
    batch.push(move |p| {
        *p = 1;
        editing.send(()).unwrap();
        next_edit.recv_timeout(WAIT).unwrap();
        Ok(())
    });
    batch.push(|p| {
        *p = *p * 10 + 2;
        Ok(())
    });
    let first = batch.submit_to(&manager, 1).unwrap();
    assert_eq!(first.revision, 1);
    release.send(()).unwrap();
    edit_started.recv_timeout(WAIT).unwrap();
    let second = job
        .add_problem_change(|p| {
            *p = *p * 10 + 3;
            Ok(())
        })
        .unwrap();
    assert_eq!(second.revision, 2);
    next.send(()).unwrap();
    delivery.recv_timeout(WAIT).unwrap();
    assert_eq!(second.try_result(), None);
    assert!(job.try_result().is_none());
    finish.send(()).unwrap();
    assert_eq!(first.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(second.wait_timeout(WAIT), Some(Ok(())));
    let result = job.wait_timeout(WAIT).unwrap();
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!(result.revision, 2);
    assert_eq!(*result.solution, 123);
    assert_eq!(observations.try_iter().collect::<Vec<_>>(), [0, 12, 123]);
    manager.shutdown().unwrap();
}
#[test]
fn empty_batch_requests_a_new_revision_and_failed_batch_fails_its_receipt() {
    for panic in [false, true] {
        let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
        let (sent, received) = mpsc::channel();
        let job = manager
            .submit(
                1,
                9,
                LiveSolverConfig {
                    daemon: true,
                    ..Default::default()
                },
                |p, _, _| Ok(*p),
                move |_, p| {
                    sent.send(*p).unwrap();
                },
            )
            .unwrap();
        assert_eq!(received.recv_timeout(WAIT).unwrap(), 9);
        let receipt = ProblemChangeBatch::new().submit(&job).unwrap();
        assert_eq!(receipt.revision, 1);
        assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
        assert_eq!(received.recv_timeout(WAIT).unwrap(), 9);
        let mut batch = ProblemChangeBatch::new();
        batch.push(move |_| {
            assert!(!panic, "failure");
            Err("failure".into())
        });
        let receipt = batch.submit(&job).unwrap();
        assert_eq!(receipt.revision, 2);
        let message = format!(
            "batch edit 1 {}: failure",
            if panic { "panicked" } else { "failed" }
        );
        assert_eq!(
            receipt.wait_timeout(WAIT),
            Some(Err(ProblemChangeFailure::ChangeFailed(message)))
        );
        assert!(job.wait_timeout(WAIT).unwrap().is_err());
        assert!(received.try_recv().is_err());
        manager.shutdown().unwrap();
    }
}
#[test]
fn cancellation_and_rejection_dispose_pending_batches_without_executing_them() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (started, starting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let job = manager
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
    let drops = Arc::new(AtomicUsize::new(0));
    let make_batch = || {
        let mut batch = ProblemChangeBatch::new();
        for explode in [true, false] {
            let owned = Dropped(drops.clone(), explode);
            batch.push(move |_| {
                drop(owned);
                panic!("must not execute")
            });
        }
        batch
    };
    let receipt = make_batch().submit(&job).unwrap();
    assert!(job.cancel());
    release.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).is_some());
    assert_eq!(
        receipt.wait_timeout(WAIT),
        Some(Err(ProblemChangeFailure::Cancelled))
    );
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert!(matches!(
        make_batch().submit(&job),
        Err(ProblemChangeSubmitError::Unavailable { problem_id: 1 })
    ));
    assert!(matches!(
        make_batch().submit_to(&manager, 999),
        Err(ProblemChangeSubmitError::Unavailable { problem_id: 999 })
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 6);
    manager.shutdown().unwrap();
}
