//! Original lifecycle regressions; no upstream method credit.
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);

// Deliberate allocation identity distinguishes admitted snapshots, even when
// their payload strings would otherwise share a static address.
#[allow(clippy::redundant_allocation)]
#[derive(Debug)]
struct Snapshot(Arc<&'static str>); // Non-Clone result; handoff must not clone it.
fn snapshot(text: &'static str) -> Snapshot {
    Snapshot(Arc::new(text))
}
fn origin(phase: usize) -> SolverEventProducer {
    SolverEventProducer {
        phase,
        kind: SolverPhaseKind::Custom,
    }
}

#[test]
fn interrupted_return_cannot_overwrite_admitted_snapshot_or_its_producer() {
    let mut manager =
        LiveSolverManager::<usize, Snapshot, String>::with_consumer_count(2, 1).unwrap();
    // Occupy the sole consumer so the tested job has a pending, not in-flight,
    // best event. Solver threads remain free to return and restart.
    let (occupied, occupying) = mpsc::channel();
    let (unblock, blocked) = mpsc::channel();
    let blocker = manager
        .submit(
            100,
            0,
            Default::default(),
            |_, _, _| Ok(snapshot("blocker")),
            move |_, _| {
                occupied.send(()).unwrap();
                blocked.recv_timeout(WAIT).unwrap();
            },
        )
        .unwrap();
    occupying.recv_timeout(WAIT).unwrap();

    let (ready, readiness) = mpsc::channel();
    let (continue_zero, zero_continued) = mpsc::channel();
    let (continue_one, one_continued) = mpsc::channel();
    let (finish_one, one_finished) = mpsc::channel();
    let (delivered, deliveries) = mpsc::channel();
    let first = Arc::new("admitted zero");
    let first_in_run = first.clone();
    let job = manager
        .submit_with_events(
            101,
            0,
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            move |p, old, token, progress| {
                if *p == 0 {
                    assert!(old.is_none());
                    assert!(progress.publish_from(Snapshot(first_in_run.clone()), origin(3)));
                    ready.send(0).unwrap();
                    zero_continued.recv_timeout(WAIT).unwrap();
                    assert!(token.is_cancelled());
                    assert!(!progress.publish_from(snapshot("rejected zero"), origin(4)));
                    Ok(snapshot("returned zero"))
                } else {
                    assert_eq!(*p, 1);
                    assert_eq!(old.map(|s| *s.0), Some("returned zero"));
                    ready.send(1).unwrap();
                    one_continued.recv_timeout(WAIT).unwrap();
                    assert!(progress.publish_from(snapshot("admitted one"), origin(5)));
                    one_finished.recv_timeout(WAIT).unwrap();
                    assert!(token.is_cancelled());
                    Ok(snapshot("returned one"))
                }
            },
            LiveSolverCallbacks::<Snapshot>::new(move |_, event| {
                delivered
                    .send((
                        event.solution.revision,
                        event.solution.solution.0.clone(),
                        event.producer,
                    ))
                    .unwrap();
            }),
        )
        .unwrap();
    assert_eq!(readiness.recv_timeout(WAIT).unwrap(), 0);
    let receipt = job
        .add_problem_change(|p| {
            *p = 1;
            Ok(())
        })
        .unwrap();
    continue_zero.send(()).unwrap();
    assert_eq!(readiness.recv_timeout(WAIT).unwrap(), 1);
    assert!(receipt.try_result().is_none());
    unblock.send(()).unwrap();
    let event = deliveries.recv_timeout(WAIT).unwrap();
    assert_eq!(event.0, 0);
    assert!(Arc::ptr_eq(&event.1, &first));
    assert_eq!(event.2, Some(origin(3)));
    assert!(receipt.try_result().is_none());
    assert!(deliveries.try_recv().is_err());
    continue_one.send(()).unwrap();
    let event = deliveries.recv_timeout(WAIT).unwrap();
    assert_eq!(
        (event.0, *event.1, event.2),
        (1, "admitted one", Some(origin(5)))
    );
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    assert!(job.cancel());
    finish_one.send(()).unwrap();
    let result = job.wait_timeout(WAIT).unwrap();
    assert_eq!(
        *result.as_ref().as_ref().unwrap().solution.0,
        "returned one"
    );
    let event = deliveries.recv_timeout(WAIT).unwrap();
    assert_eq!((event.0, *event.1, event.2), (1, "returned one", None));
    assert!(blocker.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}

#[test]
fn repeated_restart_returns_remain_handoffs_without_unrequested_deliveries() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let (ready, readiness) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let (delivered, deliveries) = mpsc::channel();
    let job = manager
        .submit(
            102,
            0,
            Default::default(),
            move |p, old, token| {
                assert_eq!(old.copied(), p.checked_sub(1).map(|i| i * 10));
                ready.send(*p).unwrap();
                resumed.recv_timeout(WAIT).unwrap();
                assert_eq!(token.is_cancelled(), *p < 3);
                Ok(*p * 10)
            },
            move |_, result| delivered.send(*result).unwrap(),
        )
        .unwrap();
    let mut receipts = Vec::new();
    for revision in 0..3 {
        assert_eq!(readiness.recv_timeout(WAIT).unwrap(), revision);
        assert!(deliveries.try_recv().is_err());
        receipts.push(
            job.add_problem_change(|p| {
                *p += 1;
                Ok(())
            })
            .unwrap(),
        );
        resume.send(()).unwrap();
    }
    assert_eq!(readiness.recv_timeout(WAIT).unwrap(), 3);
    assert!(receipts.iter().all(|r| r.try_result().is_none()));
    assert!(deliveries.try_recv().is_err());
    resume.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(deliveries.recv_timeout(WAIT).unwrap(), 30);
    assert!(deliveries.try_recv().is_err());
    for receipt in receipts {
        assert_eq!(receipt.try_result(), Some(Ok(())));
    }
    manager.shutdown().unwrap();
}

#[test]
fn explicit_stopping_keeps_return_delivery_with_unapplied_changes() {
    for finish in [false, true] {
        for delivery in [LiveDelivery::Best, LiveDelivery::Final] {
            let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
            let (ready, readiness) = mpsc::channel();
            let (resume, resumed) = mpsc::channel();
            let (delivered, deliveries) = mpsc::channel();
            let job = manager
                .submit_with_progress(
                    103,
                    0,
                    LiveSolverConfig {
                        daemon: true,
                        delivery,
                    },
                    move |_, _, _, progress| {
                        ready.send(()).unwrap();
                        resumed.recv_timeout(WAIT).unwrap();
                        if finish {
                            assert!(progress.finish_after_run());
                        }
                        Ok(42)
                    },
                    move |_, result| delivered.send(*result).unwrap(),
                )
                .unwrap();
            readiness.recv_timeout(WAIT).unwrap();
            let receipt = job
                .add_problem_change(|_| panic!("unapplied change ran"))
                .unwrap();
            if !finish {
                assert!(job.cancel());
            }
            resume.send(()).unwrap();
            let result = job.wait_timeout(WAIT).unwrap();
            assert_eq!(*result.as_ref().as_ref().unwrap().solution, 42);
            assert_eq!(deliveries.recv_timeout(WAIT).unwrap(), 42);
            assert!(deliveries.try_recv().is_err());
            assert_eq!(
                receipt.try_result(),
                Some(Err(ProblemChangeFailure::Cancelled))
            );
            manager.shutdown().unwrap();
        }
    }
}

#[test]
fn cancellation_after_suppression_still_delivers_last_applied_revision() {
    struct Dropping {
        value: usize,
        barrier: Option<(mpsc::Sender<()>, std::sync::Mutex<mpsc::Receiver<()>>)>,
    }
    impl Drop for Dropping {
        fn drop(&mut self) {
            if let Some((entered, release)) = self.barrier.take() {
                entered.send(()).unwrap();
                release.into_inner().unwrap().recv_timeout(WAIT).unwrap();
            }
        }
    }
    let mut manager = LiveSolverManager::<usize, Dropping, String>::new(1).unwrap();
    let (ready, readiness) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let (dropping, drop_entered) = mpsc::channel();
    let (release_drop, drop_released) = mpsc::channel();
    let mut barrier = Some((dropping, std::sync::Mutex::new(drop_released)));
    let (delivered, deliveries) = mpsc::channel();
    let job = manager
        .submit(
            104,
            0,
            Default::default(),
            move |p, old, _| {
                assert!(*p <= 1, "cancelled next revision ran");
                assert_eq!(old.map(|r| r.value), p.checked_sub(1));
                ready.send(*p).unwrap();
                resumed.recv_timeout(WAIT).unwrap();
                Ok(Dropping {
                    value: *p,
                    barrier: if *p == 0 { barrier.take() } else { None },
                })
            },
            move |_, value| delivered.send(value.value).unwrap(),
        )
        .unwrap();
    assert_eq!(readiness.recv_timeout(WAIT).unwrap(), 0);
    let first = job
        .add_problem_change(|p| {
            *p = 1;
            Ok(())
        })
        .unwrap();
    resume.send(()).unwrap();
    assert_eq!(readiness.recv_timeout(WAIT).unwrap(), 1);
    let second = job
        .add_problem_change(|_| panic!("cancelled edit ran"))
        .unwrap();
    resume.send(()).unwrap();
    // Revision1 has sampled its queued successor and suppressed publication.
    // Replacing the revision0 handoff now executes its destructor outside locks.
    drop_entered.recv_timeout(WAIT).unwrap();
    assert!(deliveries.try_recv().is_err());
    assert!(job.cancel());
    release_drop.send(()).unwrap();
    let result = job.wait_timeout(WAIT).unwrap();
    assert_eq!(result.as_ref().as_ref().unwrap().revision, 1);
    assert_eq!(deliveries.recv_timeout(WAIT).unwrap(), 1);
    assert!(deliveries.try_recv().is_err());
    assert_eq!(first.try_result(), Some(Ok(())));
    assert_eq!(
        second.try_result(),
        Some(Err(ProblemChangeFailure::Cancelled))
    );
    manager.shutdown().unwrap();
}
