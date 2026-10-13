//! Explicit scheduling and incumbent pruning for application-owned exhaustive branches.
use crate::{NodeExploration, Score, SearchNodeKey};
use std::{cmp::Ordering, collections::BinaryHeap, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UninitializedPessimisticBound;
impl fmt::Display for UninitializedPessimisticBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("pessimistic bound must describe a fully initialized feasible result")
    }
}
impl Error for UninitializedPessimisticBound {}
struct Entry<S, T> {
    key: SearchNodeKey<S>,
    payload: T,
    serial: u128,
    order: NodeExploration,
}
impl<S: Score, T> PartialEq for Entry<S, T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl<S: Score, T> Eq for Entry<S, T> {}
impl<S: Score, T> PartialOrd for Entry<S, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<S: Score, T> Ord for Entry<S, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order
            .compare(&self.key, &other.key)
            .then_with(|| other.serial.cmp(&self.serial))
    }
}
/// An explicit frontier holding real branch payloads (for example recorded moves
/// or assignment handles). Scheduling uses native NodeExploration; exact priority
/// ties retain insertion order. Unknown optimistic bounds always survive pruning.
/// A strictly improved attainable incumbent eagerly drops finite bounds no better
/// than it, using full score ordering including structural levels. The caller must
/// supply admissible optimistic bounds and an actually attainable incumbent.
///
/// Push/pop take O(log n); improving the incumbent takes O(n). No payload Clone
/// bound or scoring callback is required. Existing solver enumeration is unchanged;
/// callers can combine this frontier with BranchRestorer for external scheduling.
pub struct PruningFrontier<S: Score, T> {
    order: NodeExploration,
    heap: BinaryHeap<Entry<S, T>>,
    incumbent: Option<S>,
    serial: u128,
}
impl<S: Score, T> PruningFrontier<S, T> {
    pub fn new(order: NodeExploration) -> Self {
        Self {
            order,
            heap: BinaryHeap::new(),
            incumbent: None,
            serial: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.heap.len()
    }
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
    pub fn incumbent(&self) -> Option<&S> {
        self.incumbent.as_ref()
    }
    /// Returns false for an already dominated branch. Its payload is dropped.
    pub fn push(&mut self, key: SearchNodeKey<S>, payload: T) -> bool {
        if dominated(&key, self.incumbent.as_ref()) {
            return false;
        }
        let serial = self.serial;
        self.serial = serial.checked_add(1).expect("frontier serial overflow");
        self.heap.push(Entry {
            key,
            payload,
            serial,
            order: self.order,
        });
        true
    }
    pub fn pop(&mut self) -> Option<(SearchNodeKey<S>, T)> {
        self.heap.pop().map(|entry| (entry.key, entry.payload))
    }
    /// Register an attainable initialized lower bound, returning the number of
    /// removed branches. Ties/worse bounds do nothing; invalid bounds reject before
    /// publication. Scores must obey the native structural-first Score contract.
    pub fn register_bound(&mut self, bound: S) -> Result<usize, UninitializedPessimisticBound> {
        if bound.structural() < 0 {
            return Err(UninitializedPessimisticBound);
        }
        if self.incumbent.as_ref().is_some_and(|old| old >= &bound) {
            return Ok(0);
        }
        let before = self.heap.len();
        self.heap
            .retain(|entry| !dominated(&entry.key, Some(&bound)));
        self.incumbent = Some(bound);
        Ok(before - self.heap.len())
    }
}
fn dominated<S: Score>(key: &SearchNodeKey<S>, incumbent: Option<&S>) -> bool {
    key.optimistic_bound
        .as_ref()
        .zip(incumbent)
        .is_some_and(|(upper, lower)| upper <= lower)
}
