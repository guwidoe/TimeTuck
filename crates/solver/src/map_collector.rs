//! Persistent maps of independently reversible collectors.
use crate::{ReversibleCollector, SnapshotMap};
use std::marker::PhantomData;

/// Collect mapped `(key, value)` occurrences into an ordered map of results.
///
/// Each key owns a separate `C`. A key disappears only after its last occurrence
/// is removed, even when its result is zero or empty. With `SetCollector<V>` this
/// is a map of distinct values; with `IntCollector` it is a map of integer
/// summaries. Collectors may be nested or paired. Removal requires a previously
/// inserted mapped occurrence; value validation follows the inner collector.
///
/// Finished maps and clones share persistent roots. Updating a key clones only
/// its inner collector and map paths. Inner methods must obey ReversibleCollector's
/// isolation contract. Candidate state and output are prepared before installation,
/// so ordinary clone, ordering, or collector panics preserve the previous map.
///
/// ```
/// use timetuck::{MapCollector, ReversibleCollector, SetCollector};
/// let mut by_team = MapCollector::<&str, i32, SetCollector<i32>>::default();
/// by_team.insert(&("blue", 7));
/// by_team.insert(&("blue", 7));
/// let saved = by_team.finish();
/// assert!(by_team.remove(&("blue", 7)));
/// assert_eq!(by_team.finish()[&"blue"].len(), 1);
/// assert!(by_team.remove(&("blue", 7)));
/// assert!(by_team.finish().is_empty());
/// assert!(saved[&"blue"].contains_key(&7));
/// ```
pub struct MapCollector<K, V, C: ReversibleCollector<V>> {
    groups: SnapshotMap<K, (usize, C)>,
    results: SnapshotMap<K, C::Output>,
    value_type: PhantomData<fn(&V)>,
}
impl<K, V, C: ReversibleCollector<V>> Clone for MapCollector<K, V, C> {
    fn clone(&self) -> Self {
        Self {
            groups: self.groups.clone(),
            results: self.results.clone(),
            value_type: PhantomData,
        }
    }
}
impl<K, V, C: ReversibleCollector<V>> Default for MapCollector<K, V, C> {
    fn default() -> Self {
        Self {
            groups: SnapshotMap::new(),
            results: SnapshotMap::new(),
            value_type: PhantomData,
        }
    }
}
impl<K: Ord + Clone, V, C: ReversibleCollector<V>> ReversibleCollector<(K, V)>
    for MapCollector<K, V, C>
{
    type Output = SnapshotMap<K, C::Output>;
    fn insert(&mut self, (key, value): &(K, V)) {
        let (count, mut collector) = self.groups.get(key).cloned().unwrap_or_default();
        let count = count.checked_add(1).expect("map collector count overflow");
        collector.insert(value);
        let output = collector.finish();
        let mut next = self.clone();
        next.groups.insert(key.clone(), (count, collector));
        next.results.insert(key.clone(), output);
        *self = next;
    }
    fn remove(&mut self, (key, value): &(K, V)) -> bool {
        let Some((count, mut collector)) = self.groups.get(key).cloned() else {
            return false;
        };
        if !collector.remove(value) {
            return false;
        }
        let mut next = self.clone();
        if count == 1 {
            next.groups.remove(key);
            next.results.remove(key);
        } else {
            let output = collector.finish();
            next.groups.insert(key.clone(), (count - 1, collector));
            next.results.insert(key.clone(), output);
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        self.results.clone()
    }
}
