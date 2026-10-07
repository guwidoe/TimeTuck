//! Stable-key ordered distinct collection for incremental scoring.
use crate::{ReversibleCollector, ScalingSequencedSet, SnapshotMap};
use std::{hash::Hash, sync::Arc};

/// Collect `(occurrence_id, value)` pairs, retaining each distinct value once in
/// ascending order of its lowest active occurrence ID. That occurrence supplies
/// the representative payload. IDs must be unique within a collector; removal
/// uses the captured ID. Duplicate insertions/replacement IDs panic before mutation.
/// Order depends on current inputs, not event arrival, so replay yields the same
/// score. Hash/Eq must be stable, infallible and free of reentrant mutation.
///
/// Clones share a persistent occurrence index. Finish builds a compact sequenced
/// set while traversing that index and returns detached Arc handles, without
/// cloning payloads. Removing the earliest occurrence may change a representative
/// and its position. Output construction visits every active occurrence.
///
/// ```
/// use timetuck::{OrderedSetCollector, ReversibleCollector};
/// let mut c = OrderedSetCollector::<usize, i32>::default();
/// c.insert(&(20, 2)); c.insert(&(10, 1)); c.insert(&(5, 2));
/// assert_eq!(c.finish().iter().map(|v| **v).collect::<Vec<_>>(), [2, 1]);
/// c.remove(&(5, 2));
/// assert_eq!(c.finish().iter().map(|v| **v).collect::<Vec<_>>(), [1, 2]);
/// ```
pub struct OrderedSetCollector<I, V> {
    values: SnapshotMap<I, Arc<V>>,
}
impl<I, V> Default for OrderedSetCollector<I, V> {
    fn default() -> Self {
        Self {
            values: SnapshotMap::new(),
        }
    }
}
impl<I, V> Clone for OrderedSetCollector<I, V> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
        }
    }
}
impl<I: Ord + Clone, V: Eq + Hash + Clone> ReversibleCollector<(I, V)>
    for OrderedSetCollector<I, V>
{
    type Output = Vec<Arc<V>>;
    fn insert(&mut self, (id, value): &(I, V)) {
        assert!(
            !self.values.contains_key(id),
            "duplicate ordered-set occurrence ID"
        );
        self.values.insert(id.clone(), Arc::new(value.clone()));
    }
    fn remove(&mut self, (id, _): &(I, V)) -> bool {
        self.values.remove(id)
    }
    fn replace(&mut self, (old_id, _): &(I, V), (new_id, value): &(I, V)) -> bool {
        if !self.values.contains_key(old_id) {
            return false;
        }
        assert!(
            old_id == new_id || !self.values.contains_key(new_id),
            "duplicate ordered-set occurrence ID"
        );
        let mut next = self.values.clone();
        next.remove(old_id);
        next.insert(new_id.clone(), Arc::new(value.clone()));
        self.values = next;
        true
    }
    fn finish(&self) -> Self::Output {
        let ordered = ScalingSequencedSet::new();
        for value in self.values.values() {
            ordered
                .insert_shared(value.clone())
                .expect("private set has no iterator lease");
        }
        ordered.snapshot()
    }
}
