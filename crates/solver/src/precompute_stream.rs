//! Per-occurrence memoization with explicit fact invalidation keys.
use crate::{SnapshotMap, TupleChange};
use std::any::Any;

/// Cache a fact-derived output per input ID. The key selector runs on each put;
/// computation and downstream updates occur only for new IDs or changed keys.
/// Retraction removes the cached row without invoking either callback. Reinserted
/// IDs compute again. Different IDs never share a cache entry merely by key value.
///
/// Keys must describe every fact the computation depends on; computation must not
/// depend on planning variables omitted from the key. Keys and cached outputs must
/// remain immutable. Join stable IDs from this output with a live stream when a
/// downstream constraint needs current planning values. This deliberately freezes
/// the output for an unchanged key, including newly allocated output objects.
/// Ordinary callback/clone/equality panics leave this node unchanged.
pub struct PrecomputeNode<'a, I, T, K, U> {
    rows: SnapshotMap<I, (K, U)>,
    key: Box<dyn Fn(&T) -> K + 'a>,
    compute: Box<dyn Fn(&T) -> U + 'a>,
}
impl<'a, I: Ord + Clone, T, K: PartialEq + Clone, U: Clone> PrecomputeNode<'a, I, T, K, U> {
    pub fn new(key: impl Fn(&T) -> K + 'a, compute: impl Fn(&T) -> U + 'a) -> Self {
        Self {
            rows: SnapshotMap::new(),
            key: Box::new(key),
            compute: Box::new(compute),
        }
    }
    pub fn rows(&self) -> impl Iterator<Item = (&I, &U)> {
        self.rows.iter().map(|(id, (_, output))| (id, output))
    }
    pub fn apply(&mut self, change: TupleChange<I, T>) -> Vec<TupleChange<I, U>> {
        let mut next = self.rows.clone();
        let output = if let Some(input) = change.value {
            let key = (self.key)(&input);
            if self
                .rows
                .get(&change.id)
                .is_some_and(|(previous, _)| *previous == key)
            {
                return Vec::new();
            }
            let value = (self.compute)(&input);
            next.insert(change.id.clone(), (key, value.clone()));
            vec![TupleChange::put(change.id, value)]
        } else if next.remove(&change.id) {
            vec![TupleChange::retract(change.id)]
        } else {
            Vec::new()
        };
        self.rows = next;
        output
    }
    pub(crate) fn clear(&mut self) {
        self.rows.clear();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        K: 'static,
        U: 'static,
    {
        Box::new(self.rows.clone())
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        K: 'static,
        U: 'static,
    {
        self.rows = *checkpoint.downcast().expect("precompute checkpoint type");
    }
}
