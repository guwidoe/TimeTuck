//! Ordered distinct planning domains with indexed membership.
use crate::ValueRange;
use std::{cmp::Ordering, collections::HashMap, hash::Hash, sync::Arc};

#[derive(Debug)]
struct SetRangeStorage<V> {
    ordered: Vec<Arc<V>>,
    ranks: HashMap<Arc<V>, usize>,
}

/// An immutable set in first-encounter order. Duplicate inputs retain the first
/// value, and random sampling weights each distinct value once. Hash iteration
/// order never affects traversal, sorting or seeded sampling.
///
/// Values are shared individually: construction, cloning, reordering and set
/// operations do not clone payloads. Owned `ValueRange` reads clone one value.
/// Equality and hashing must remain stable while any snapshot exists.
/// Rank access is constant time; membership and reverse lookup are expected O(1).
///
/// ```
/// use timetuck::*;
/// let offered = SetValueRange::new([3, 1, 3, 2]);
/// let allowed = SetValueRange::new([2, 3]);
/// let domain = offered.intersection(&allowed);
/// assert_eq!(domain.materialize(2)?, [3, 2]);
/// let variable = variable_from_range(&domain, 2)?;
/// assert_eq!(variable.domain(), &[3, 2]);
/// # Ok::<(), ValueRangeError>(())
/// ```
#[derive(Debug)]
pub struct SetValueRange<V> {
    storage: Arc<SetRangeStorage<V>>,
}
impl<V> Clone for SetValueRange<V> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
        }
    }
}
impl<V: Eq + Hash> SetValueRange<V> {
    pub fn new(values: impl IntoIterator<Item = V>) -> Self {
        Self::from_shared(values.into_iter().map(Arc::new))
    }
    fn from_shared(values: impl IntoIterator<Item = Arc<V>>) -> Self {
        let mut ordered = Vec::new();
        let mut ranks = HashMap::new();
        for value in values {
            if let std::collections::hash_map::Entry::Vacant(entry) = ranks.entry(value.clone()) {
                entry.insert(ordered.len());
                ordered.push(value);
            }
        }
        Self {
            storage: Arc::new(SetRangeStorage { ordered, ranks }),
        }
    }
    /// Borrow distinct values in their selection order without cloning payloads.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.storage.ordered.iter().map(Arc::as_ref)
    }
    pub fn get_ref(&self, index: u128) -> Option<&V> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.storage.ordered.get(i))
            .map(Arc::as_ref)
    }
    pub fn index_of(&self, value: &V) -> Option<u128> {
        self.storage.ranks.get(value).map(|&i| i as u128)
    }
    /// Stable sorted snapshot retaining the set index and shared payloads.
    /// The original remains unchanged even if the comparator panics.
    /// Unlike bounded `ValueRange::sorted_by`, this operates on already stored
    /// distinct members and does not materialize or clone their payloads.
    pub fn reordered_by(&self, mut compare: impl FnMut(&V, &V) -> Ordering) -> Self {
        let mut ordered = self.storage.ordered.clone();
        ordered.sort_by(|a, b| compare(a, b));
        Self::from_shared(ordered)
    }
    /// All left members followed by previously unseen right members.
    pub fn union(&self, other: &Self) -> Self {
        Self::from_shared(
            self.storage
                .ordered
                .iter()
                .chain(&other.storage.ordered)
                .cloned(),
        )
    }
    /// Members present in both ranges, preserving left order and representatives.
    pub fn intersection(&self, other: &Self) -> Self {
        Self::from_shared(
            self.storage
                .ordered
                .iter()
                .filter(|v| other.storage.ranks.contains_key(v.as_ref()))
                .cloned(),
        )
    }
    /// Left members absent from the right range, preserving left order.
    pub fn difference(&self, other: &Self) -> Self {
        Self::from_shared(
            self.storage
                .ordered
                .iter()
                .filter(|v| !other.storage.ranks.contains_key(v.as_ref()))
                .cloned(),
        )
    }
}
impl<V: Clone + Eq + Hash> ValueRange for SetValueRange<V> {
    type Value = V;
    fn len(&self) -> u128 {
        self.storage.ordered.len() as u128
    }
    fn get(&self, index: u128) -> Option<V> {
        self.get_ref(index).cloned()
    }
    fn contains(&self, value: &V) -> bool {
        self.storage.ranks.contains_key(value)
    }
}
