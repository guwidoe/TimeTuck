//! Original ownership, cancellation, panic and actual solver-observer checks.
use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
fn origin(phase: usize) -> SolverEventProducer {
    SolverEventProducer {
        phase,
        kind: SolverPhaseKind::Exact,
    }
}
fn receipt<T>(slot: &mut BestDeliverySlot<T>) -> BestDeliveryReceipt {
    slot.submit_changes(vec![()], |_| Ok::<_, ()>(())).unwrap()
}
#[test]
fn replacing_snapshots_inherits_only_unconsumed_batches_and_moves_nonclone_payloads() {
    struct Owned(usize);
    let mut slot = BestDeliverySlot::default();
    let first = receipt(&mut slot);
    slot.set_if(Owned(0), origin(0), || true);
    let taken = slot.take_best().unwrap();
    let mut registered = vec![];
    for i in 1..=2000 {
        registered.push(receipt(&mut slot));
        slot.set_if(Owned(i), origin(i), || true);
    }
    let last = slot.take_best().unwrap();
    assert_eq!(last.solution.0, 2000);
    assert_eq!(last.producer, origin(2000));
    assert_eq!(last.included_change_batches(), 2000);
    assert!(
        registered
            .iter()
            .all(|r| r.delivery_status() == BestDeliveryStatus::Pending)
    );
    last.complete_problem_changes();
    last.complete_problem_changes();
    assert!(
        registered
            .iter()
            .all(|r| r.delivery_status() == BestDeliveryStatus::Delivered)
    );
    assert_eq!(first.delivery_status(), BestDeliveryStatus::Pending);
    taken.complete_problem_changes();
    assert_eq!(first.delivery_status(), BestDeliveryStatus::Delivered);
    assert!(slot.take_best().is_none());
}
#[test]
fn receipt_waiters_wake_on_consumption_and_slot_drop_without_lost_notifications() {
    for consumed in [false, true] {
        let mut slot = BestDeliverySlot::default();
        let r = receipt(&mut slot);
        assert_eq!(
            r.wait_for_delivery(Duration::ZERO),
            BestDeliveryStatus::Pending
        );
        slot.set_if(0, origin(0), || true);
        let delivery = slot.take_best().unwrap();
        let (started, start) = mpsc::channel();
        let (sent, seen) = mpsc::channel();
        let copy = r.clone();
        let waiter = thread::spawn(move || {
            started.send(()).unwrap();
            sent.send(copy.wait_for_delivery(WAIT)).unwrap();
        });
        start.recv_timeout(WAIT).unwrap();
        if consumed {
            delivery.complete_problem_changes();
        }
        drop(slot);
        let expected = if consumed {
            BestDeliveryStatus::Delivered
        } else {
            BestDeliveryStatus::Cancelled
        };
        assert_eq!(seen.recv_timeout(WAIT).unwrap(), expected);
        waiter.join().unwrap();
        delivery.complete_problem_changes();
        assert_eq!(r.delivery_status(), expected);
        assert_eq!(r.wait_for_delivery(Duration::ZERO), expected);
    }
}
#[test]
fn cancellation_and_completion_races_have_one_terminal_outcome() {
    for _ in 0..32 {
        let mut slot = BestDeliverySlot::default();
        let r = receipt(&mut slot);
        slot.set_if(0, origin(0), || true);
        let delivery = slot.take_best().unwrap();
        thread::scope(|scope| {
            scope.spawn(|| delivery.complete_problem_changes());
            slot.cancel_pending_changes();
        });
        let outcome = r.delivery_status();
        assert_ne!(outcome, BestDeliveryStatus::Pending);
        delivery.complete_problem_changes();
        slot.cancel_pending_changes();
        assert_eq!(r.delivery_status(), outcome);
    }
}
#[test]
fn rejected_failed_and_panicking_callbacks_preserve_pending_delivery_and_batch_cutoff() {
    let mut slot = BestDeliverySlot::default();
    let first = receipt(&mut slot);
    slot.set_if(1, origin(0), || true);
    assert!(!slot.set_with(
        origin(1),
        || false,
        || panic!("rejected snapshot constructed")
    ));
    assert!(
        catch_unwind(AssertUnwindSafe(|| slot.set_with(
            origin(2),
            || panic!("predicate"),
            || 2
        )))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| slot.set_with(
            origin(3),
            || true,
            || panic!("factory")
        )))
        .is_err()
    );
    assert!(
        slot.submit_changes(vec![()], |_| Err::<(), _>("submission failed"))
            .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| slot
            .submit_changes(vec![()], |_| -> Result<(), ()> {
                panic!("submit")
            })))
        .is_err()
    );
    let later = receipt(&mut slot);
    let pending = slot.take_best().unwrap();
    assert_eq!((pending.solution, pending.producer), (1, origin(0)));
    assert_eq!(pending.included_change_batches(), 1);
    pending.complete_problem_changes();
    assert_eq!(first.delivery_status(), BestDeliveryStatus::Delivered);
    assert_eq!(later.delivery_status(), BestDeliveryStatus::Pending);
    slot.set_if(2, origin(4), || true);
    let next = slot.take_best().unwrap();
    assert_eq!(next.included_change_batches(), 1);
    next.complete_problem_changes();
    assert_eq!(later.delivery_status(), BestDeliveryStatus::Delivered);
}
#[test]
fn rejected_observer_updates_never_clone_assignments() {
    struct Counted(Arc<AtomicUsize>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::Relaxed);
            Self(self.0.clone())
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let a = Counted(calls.clone());
    let mut slot = BestDeliverySlot::default();
    for accepted in [false, false, true] {
        slot.capture_best_update(
            SolverUpdate {
                phase: 0,
                kind: SolverPhaseKind::Construction,
                assignment: &a,
                score: SimpleScore::ZERO,
                complete: true,
            },
            || accepted,
        );
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(slot.take_best().unwrap().solution.complete);
}
fn delivery_model(maximum: i32) -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, maximum]); 2]).unwrap()
}
#[test]
fn real_queued_edits_and_both_solver_backends_acknowledge_only_consumed_revisions() {
    for stream in [false, true] {
        let mut p = delivery_model(1);
        let mut queue = VecDeque::new();
        let mut slot = BestDeliverySlot::default();
        let mut edit = ProblemChangeBatch::new();
        edit.push(|p| {
            *p = delivery_model(2);
            Ok(())
        });
        let changed = slot
            .submit_changes(vec![edit], |edits| {
                queue.extend(edits);
                Ok::<_, ()>(())
            })
            .unwrap();
        for revision in [0, 1] {
            if revision == 1 {
                while let Some(batch) = queue.pop_front() {
                    batch.apply(&mut p).unwrap();
                }
            }
            let config = SolverSequence {
                phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
                ..Default::default()
            };
            let mut observer = |u: SolverUpdate<'_, Assignment<i32>, SimpleScore>| {
                slot.capture_best_update(u, || queue.is_empty());
            };
            let result = if stream {
                solve_stream_with_observer(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<i32>)>();
                        b.score("values", source.port(), |(_, v)| {
                            SimpleScore::new(i64::from(v.unwrap_or(0)))
                        })?;
                        Ok((b.build(), source))
                    },
                    config,
                    &mut observer,
                )
                .unwrap()
            } else {
                let mut n = ConstraintNetwork::new(2);
                for i in 0..2 {
                    n.add_constraint(format!("v{i}"), vec![i], |v| {
                        SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
                    })
                    .unwrap();
                }
                solve_with_observer(&p, &n, config, &mut observer).unwrap()
            };
            assert_eq!(
                result.score,
                Some(SimpleScore::new(if revision == 0 { 2 } else { 4 }))
            );
            assert_eq!(changed.delivery_status(), BestDeliveryStatus::Pending);
            if revision == 0 {
                assert!(slot.take_best().is_none());
            } else {
                let delivered = slot.take_best().unwrap();
                assert_eq!(delivered.solution.assignment, result.assignment);
                assert_eq!(delivered.solution.score, SimpleScore::new(4));
                assert_eq!(
                    delivered.producer,
                    SolverEventProducer {
                        phase: 0,
                        kind: SolverPhaseKind::Construction
                    }
                );
                assert!(delivered.solution.complete);
                assert_eq!(delivered.included_change_batches(), 1);
                let unconsumed = receipt(&mut slot);
                delivered.complete_problem_changes();
                assert_eq!(changed.delivery_status(), BestDeliveryStatus::Delivered);
                assert_eq!(unconsumed.delivery_status(), BestDeliveryStatus::Pending);
            }
        }
    }
}
