//! Deterministically ordered collection of distinct occurrences, including equal values.
use crate::{ReversibleCollector, SnapshotMap};
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::Index,
};

/// Immutable collected list, ordered by stable occurrence ID rather than value.
/// Equal values with different IDs remain separate elements. Cloning shares the
/// persistent root; indexing uses subtree sizes and takes logarithmic time.
/// Iteration visits all values in order. IDs and values must remain immutable.
pub struct ListSnapshot<I, V>(SnapshotMap<I, V>);
impl<I, V> Clone for ListSnapshot<I, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<I, V> Default for ListSnapshot<I, V> {
    fn default() -> Self {
        Self(SnapshotMap::new())
    }
}
impl<I, V> ListSnapshot<I, V> {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &V> {
        self.0.values()
    }
    pub fn get(&self, index: usize) -> Option<&V> {
        self.0.entry_at(index).map(|(_, value)| value)
    }
}
impl<I, V> Index<usize> for ListSnapshot<I, V> {
    type Output = V;
    fn index(&self, index: usize) -> &V {
        self.get(index).expect("collected list index out of bounds")
    }
}
impl<I, V: PartialEq> PartialEq for ListSnapshot<I, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<I, V: Eq> Eq for ListSnapshot<I, V> {}
impl<I, V: PartialOrd> PartialOrd for ListSnapshot<I, V> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}
impl<I, V: Ord> Ord for ListSnapshot<I, V> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}
impl<I, V: Hash> Hash for ListSnapshot<I, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for value in self.iter() {
            value.hash(state);
        }
    }
}
impl<I, V: fmt::Debug> fmt::Debug for ListSnapshot<I, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Collect `(occurrence_id, value)` pairs into a list in ascending ID order.
/// IDs must be unique within this collector. Duplicate IDs panic before mutation;
/// equal values at different IDs are retained. Removal uses the captured ID and
/// returns false if it is absent, without comparing the supplied value. Replace
/// an occurrence by removing its captured old pair and inserting its new pair.
///
/// This native order is explicit: it does not claim Java encounter-order parity.
/// Graph `collect_list` / `group_list` supply input tuple IDs automatically, making
/// ordering independent of update delivery and consistent with full rebuilds.
/// Updates prepare a persistent path before installation; ordinary clone/ordering
/// panics preserve the previous state. Finish and clone share the current root.
///
/// ```
/// use timetuck::{ListCollector, ReversibleCollector};
/// let mut list = ListCollector::<usize, &str>::default();
/// list.insert(&(20, "same"));
/// list.insert(&(10, "same"));
/// let saved = list.finish();
/// assert_eq!(saved.len(), 2);
/// assert!(list.remove(&(10, "same")));
/// list.insert(&(10, "changed"));
/// assert_eq!(list.finish().iter().copied().collect::<Vec<_>>(), ["changed", "same"]);
/// assert_eq!(saved[0], "same");
/// ```
pub struct ListCollector<I, V> {
    values: SnapshotMap<I, V>,
}
impl<I, V> Clone for ListCollector<I, V> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
        }
    }
}
impl<I, V> Default for ListCollector<I, V> {
    fn default() -> Self {
        Self {
            values: SnapshotMap::new(),
        }
    }
}
impl<I: Ord + Clone, V: Clone> ReversibleCollector<(I, V)> for ListCollector<I, V> {
    type Output = ListSnapshot<I, V>;
    fn insert(&mut self, (id, value): &(I, V)) {
        assert!(
            !self.values.contains_key(id),
            "duplicate list occurrence ID"
        );
        self.values.insert(id.clone(), value.clone());
    }
    fn remove(&mut self, (id, _): &(I, V)) -> bool {
        self.values.remove(id)
    }
    fn finish(&self) -> Self::Output {
        ListSnapshot(self.values.clone())
    }
}
