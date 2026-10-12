//! Coalesced best snapshots with explicit problem-change acknowledgement.
use crate::{BufferedSolverUpdate, Score, SolverEventProducer, SolverUpdate};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BestDeliveryStatus {
    Pending = 0,
    Delivered = 1,
    Cancelled = 2,
}
struct BestDeliveryCompletion {
    state: AtomicU8,
    lock: Mutex<()>,
    ready: Condvar,
}
impl BestDeliveryCompletion {
    fn finish_delivery_receipt(&self, status: BestDeliveryStatus) {
        let _guard = self.lock.lock().unwrap();
        if self
            .state
            .compare_exchange(0, status as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.ready.notify_all();
        }
    }
    fn delivery_receipt_status(&self) -> BestDeliveryStatus {
        match self.state.load(Ordering::Acquire) {
            0 => BestDeliveryStatus::Pending,
            1 => BestDeliveryStatus::Delivered,
            _ => BestDeliveryStatus::Cancelled,
        }
    }
}
/// Clonable one-shot acknowledgement. Submission and taking a snapshot do not
/// complete it. Completion/cancellation latch; later calls cannot revive it.
#[derive(Clone)]
pub struct BestDeliveryReceipt(Arc<BestDeliveryCompletion>);
impl BestDeliveryReceipt {
    pub fn delivery_status(&self) -> BestDeliveryStatus {
        self.0.delivery_receipt_status()
    }
    /// A timeout leaves the receipt pending; it does not cancel the edit.
    pub fn wait_for_delivery(&self, timeout: Duration) -> BestDeliveryStatus {
        let guard = self.0.lock.lock().unwrap();
        let _guard = self
            .0
            .ready
            .wait_timeout_while(guard, timeout, |_| {
                self.delivery_status() == BestDeliveryStatus::Pending
            })
            .unwrap();
        self.delivery_status()
    }
}

/// Owned snapshot and all submitted batches included when it was published.
/// Complete only after successful consumption. Dropping a taken snapshot leaves
/// its receipts pending until the slot cancels them or is dropped.
pub struct BestDelivery<T> {
    pub solution: T,
    pub producer: SolverEventProducer,
    receipts: Vec<BestDeliveryReceipt>,
}
impl<T> BestDelivery<T> {
    pub fn complete_problem_changes(&self) {
        for receipt in &self.receipts {
            receipt
                .0
                .finish_delivery_receipt(BestDeliveryStatus::Delivered);
        }
    }
    pub fn included_change_batches(&self) -> usize {
        self.receipts.len()
    }
}

/// One latest eligible snapshot, with producer and change batches coalesced
/// together. The caller serializes edits/publications through this owned slot
/// (for example with a Mutex); no thread or executor is created here. Receipts
/// can be awaited from a different thread. Callbacks run synchronously while the
/// caller exclusively borrows the slot and must not reacquire its external lock.
///
/// Register edits through the actual solver queue. Publication eligibility must
/// reflect whether that queue's preceding changes have been applied; this slot
/// does not apply edits or compare scores. Rejected publications leave the old
/// snapshot and batches intact. Replacements inherit its unconsumed batches.
/// Taking consumes only the snapshot's batches; later registrations remain for
/// another publication. Cancellation leaves the snapshot available but cancels
/// all outstanding receipts, including previously taken snapshots. Drop cancels
/// remaining receipts. Completed receipts are pruned from the front on registration.
///
/// Prefer the integrated LiveSolverManager for ordinary asynchronous jobs. This
/// component supports custom consumer scheduling and synchronous solver observers;
/// it does not change existing live-manager delivery or Java executor semantics.
pub struct BestDeliverySlot<T> {
    latest: Option<BestDelivery<T>>,
    awaiting_snapshot: Vec<BestDeliveryReceipt>,
    outstanding: VecDeque<BestDeliveryReceipt>,
}
impl<T> Default for BestDeliverySlot<T> {
    fn default() -> Self {
        Self {
            latest: None,
            awaiting_snapshot: Vec::new(),
            outstanding: VecDeque::new(),
        }
    }
}
impl<T> BestDeliverySlot<T> {
    /// Submit one owned edit batch exactly once. A failed/panicking submission
    /// registers no acknowledgement; previously retained state is untouched.
    pub fn submit_changes<C, E>(
        &mut self,
        changes: Vec<C>,
        submit: impl FnOnce(Vec<C>) -> Result<(), E>,
    ) -> Result<BestDeliveryReceipt, E> {
        submit(changes)?;
        while self
            .outstanding
            .front()
            .is_some_and(|receipt| receipt.delivery_status() != BestDeliveryStatus::Pending)
        {
            self.outstanding.pop_front();
        }
        let receipt = BestDeliveryReceipt(Arc::new(BestDeliveryCompletion {
            state: AtomicU8::new(0),
            lock: Mutex::new(()),
            ready: Condvar::new(),
        }));
        self.awaiting_snapshot.push(receipt.clone());
        self.outstanding.push_back(receipt.clone());
        Ok(receipt)
    }
    pub fn set_if(
        &mut self,
        solution: T,
        producer: SolverEventProducer,
        eligible: impl FnOnce() -> bool,
    ) -> bool {
        self.set_with(producer, eligible, || solution)
    }
    /// Construct a snapshot only after eligibility succeeds. Predicate/factory
    /// panics leave the previous snapshot and captured batches untouched.
    pub fn set_with(
        &mut self,
        producer: SolverEventProducer,
        eligible: impl FnOnce() -> bool,
        snapshot: impl FnOnce() -> T,
    ) -> bool {
        if !eligible() {
            return false;
        }
        let solution = snapshot();
        let mut receipts = self
            .latest
            .as_mut()
            .map(|old| std::mem::take(&mut old.receipts))
            .unwrap_or_default();
        receipts.append(&mut self.awaiting_snapshot);
        let replaced = self.latest.replace(BestDelivery {
            solution,
            producer,
            receipts,
        });
        drop(replaced);
        true
    }
    pub fn take_best(&mut self) -> Option<BestDelivery<T>> {
        self.latest.take()
    }
    pub fn cancel_pending_changes(&mut self) {
        for receipt in self.outstanding.drain(..) {
            receipt
                .0
                .finish_delivery_receipt(BestDeliveryStatus::Cancelled);
        }
        self.awaiting_snapshot.clear();
    }
}
impl<A: Clone, S: Score> BestDeliverySlot<BufferedSolverUpdate<A, S>> {
    /// Capture an actual retained-best observer update, cloning only when eligible.
    pub fn capture_best_update(
        &mut self,
        update: SolverUpdate<'_, A, S>,
        eligible: impl FnOnce() -> bool,
    ) -> bool {
        self.set_with(SolverEventProducer::from(&update), eligible, || {
            BufferedSolverUpdate {
                phase: update.phase,
                kind: update.kind,
                assignment: update.assignment.clone(),
                score: update.score.clone(),
                complete: update.complete,
            }
        })
    }
}
impl<T> Drop for BestDeliverySlot<T> {
    fn drop(&mut self) {
        self.cancel_pending_changes();
    }
}
