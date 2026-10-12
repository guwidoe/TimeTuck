//! Ordered storage that promotes from a sorted vector to a tree.
use std::{
    collections::BTreeMap,
    ops::{Bound, RangeBounds},
};

/// Native tuning parameters. These are not asserted to match upstream tuning.
/// A successful removal leaving at least `array_threshold` entries counts as churn.
/// Promotion
/// occurs on the next absent-key insertion after `churn_tolerance` such removals,
/// or when that insertion would exceed `maximum_array_size`. Missing removals and
/// existing-key writes do not count. Churn counts persist until promotion/clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalingMapConfig {
    pub array_threshold: usize,
    pub churn_tolerance: usize,
    pub maximum_array_size: usize,
}
impl Default for ScalingMapConfig {
    fn default() -> Self {
        Self {
            array_threshold: 64,
            churn_tolerance: 8,
            maximum_array_size: 1024,
        }
    }
}
#[derive(Clone, Debug)]
enum ScalingMapStorage<K, V> {
    Array(Vec<(K, V)>),
    Tree(BTreeMap<K, V>),
}
/// A sorted small-map representation with irreversible promotion to BTreeMap.
/// Keys and values need not be Clone. Lookups use binary search in array mode;
/// insertions/removals shift entries. Tree mode provides normal ordered-map costs.
/// Iteration and bounded queries always honor requested key ordering, independent
/// of representation. This intentionally avoids exposing upstream array traversal
/// quirks. Supplier callbacks run only for missing keys, before mutation; supplier
/// panics leave the map unchanged. Panicking Ord implementations are unsupported.
///
/// ```
/// use timetuck::ScalingMap;
/// let mut map = ScalingMap::new();
/// map.get_or_insert_with(4, || "four");
/// map.get_or_insert_with(2, || "two");
/// assert_eq!(map.range(2..4).map(|(k, _)| *k).collect::<Vec<_>>(), [2]);
/// assert_eq!(map.iter().rev().map(|(k, _)| *k).collect::<Vec<_>>(), [4, 2]);
/// ```
#[derive(Clone, Debug)]
pub struct ScalingMap<K, V> {
    storage: ScalingMapStorage<K, V>,
    config: ScalingMapConfig,
    churn: usize,
}
impl<K, V> Default for ScalingMap<K, V> {
    fn default() -> Self {
        Self::with_config(ScalingMapConfig::default())
    }
}
impl<K, V> ScalingMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
    /// Every configuration is valid. A zero ceiling promotes on the first insert;
    /// zero tolerance makes size at the threshold sufficient for promotion.
    pub fn with_config(config: ScalingMapConfig) -> Self {
        Self {
            storage: ScalingMapStorage::Array(Vec::new()),
            config,
            churn: 0,
        }
    }
    pub fn len(&self) -> usize {
        match &self.storage {
            ScalingMapStorage::Array(a) => a.len(),
            ScalingMapStorage::Tree(t) => t.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn is_array_based(&self) -> bool {
        matches!(self.storage, ScalingMapStorage::Array(_))
    }
    pub fn config(&self) -> ScalingMapConfig {
        self.config
    }
    /// Clearing retains tree mode once promoted; array-mode churn is reset.
    pub fn clear(&mut self) {
        match &mut self.storage {
            ScalingMapStorage::Array(a) => a.clear(),
            ScalingMapStorage::Tree(t) => t.clear(),
        }
        self.churn = 0;
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator
    where
        K: Ord,
    {
        ScalingMapIter::new(
            match &self.storage {
                ScalingMapStorage::Array(a) => ScalingMapIterInner::Array(a.iter()),
                ScalingMapStorage::Tree(t) => ScalingMapIterInner::Tree(t.range::<K, _>(..)),
            },
            self.len(),
        )
    }
}
// One iterator for borrowed arrays and tree ranges, without materializing keys.
enum ScalingMapIterInner<'a, K, V> {
    Array(std::slice::Iter<'a, (K, V)>),
    Tree(std::collections::btree_map::Range<'a, K, V>),
}
struct ScalingMapIter<'a, K, V> {
    inner: ScalingMapIterInner<'a, K, V>,
    remaining: usize,
}
impl<'a, K, V> ScalingMapIter<'a, K, V> {
    fn new(inner: ScalingMapIterInner<'a, K, V>, remaining: usize) -> Self {
        Self { inner, remaining }
    }
}
impl<'a, K, V> Iterator for ScalingMapIter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let item = match &mut self.inner {
            ScalingMapIterInner::Array(a) => a.next().map(|(k, v)| (k, v)),
            ScalingMapIterInner::Tree(t) => t.next(),
        };
        if item.is_some() {
            self.remaining -= 1;
        }
        item
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K, V> DoubleEndedIterator for ScalingMapIter<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let item = match &mut self.inner {
            ScalingMapIterInner::Array(a) => a.next_back().map(|(k, v)| (k, v)),
            ScalingMapIterInner::Tree(t) => t.next_back(),
        };
        if item.is_some() {
            self.remaining -= 1;
        }
        item
    }
}
impl<K, V> ExactSizeIterator for ScalingMapIter<'_, K, V> {}
impl<K, V> std::iter::FusedIterator for ScalingMapIter<'_, K, V> {}
impl<K: Ord, V> ScalingMap<K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        match &self.storage {
            ScalingMapStorage::Array(a) => a
                .binary_search_by(|(k, _)| k.cmp(key))
                .ok()
                .map(|i| &a[i].1),
            ScalingMapStorage::Tree(t) => t.get(key),
        }
    }
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match &mut self.storage {
            ScalingMapStorage::Array(a) => a
                .binary_search_by(|(k, _)| k.cmp(key))
                .ok()
                .map(|i| &mut a[i].1),
            ScalingMapStorage::Tree(t) => t.get_mut(key),
        }
    }
    pub fn get_or_insert_with(&mut self, key: K, supplier: impl FnOnce() -> V) -> &mut V {
        if self.get(&key).is_some() {
            return self.get_mut(&key).unwrap();
        }
        let value = supplier();
        let promote = self.is_array_based()
            && (self.len() >= self.config.maximum_array_size
                || (self.len() >= self.config.array_threshold
                    && self.churn >= self.config.churn_tolerance));
        if promote {
            let ScalingMapStorage::Array(a) =
                std::mem::replace(&mut self.storage, ScalingMapStorage::Array(Vec::new()))
            else {
                unreachable!()
            };
            self.storage = ScalingMapStorage::Tree(a.into_iter().collect());
        }
        match &mut self.storage {
            ScalingMapStorage::Array(a) => {
                let at = a.binary_search_by(|(k, _)| k.cmp(&key)).unwrap_err();
                a.insert(at, (key, value));
                &mut a[at].1
            }
            ScalingMapStorage::Tree(t) => t.entry(key).or_insert(value),
        }
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        match &mut self.storage {
            ScalingMapStorage::Array(a) => {
                let at = a.binary_search_by(|(k, _)| k.cmp(key)).ok()?;
                if a.len() > self.config.array_threshold {
                    self.churn = self.churn.saturating_add(1);
                }
                Some(a.remove(at).1)
            }
            ScalingMapStorage::Tree(t) => t.remove(key),
        }
    }
    /// Bounds follow BTreeMap: reversed bounds and equal exclusive bounds panic.
    /// Queries borrow storage; neither payloads nor a candidate catalog are copied.
    pub fn range(&self, bounds: impl RangeBounds<K>) -> impl DoubleEndedIterator<Item = (&K, &V)> {
        let start = bounds.start_bound();
        let end = bounds.end_bound();
        let low = match start {
            Bound::Included(k) | Bound::Excluded(k) => Some(k),
            Bound::Unbounded => None,
        };
        let high = match end {
            Bound::Included(k) | Bound::Excluded(k) => Some(k),
            Bound::Unbounded => None,
        };
        if let (Some(l), Some(h)) = (low, high) {
            assert!(l <= h, "range start exceeds end");
            assert!(
                l != h || !matches!((start, end), (Bound::Excluded(_), Bound::Excluded(_))),
                "equal exclusive bounds"
            );
        }
        let inner = match &self.storage {
            ScalingMapStorage::Array(a) => {
                let from = match start {
                    Bound::Unbounded => 0,
                    Bound::Included(k) => a.partition_point(|(v, _)| v < k),
                    Bound::Excluded(k) => a.partition_point(|(v, _)| v <= k),
                };
                let to = match end {
                    Bound::Unbounded => a.len(),
                    Bound::Included(k) => a.partition_point(|(v, _)| v <= k),
                    Bound::Excluded(k) => a.partition_point(|(v, _)| v < k),
                };
                ScalingMapIterInner::Array(a[from..to.max(from)].iter())
            }
            ScalingMapStorage::Tree(t) => ScalingMapIterInner::Tree(t.range(bounds)),
        };
        // A tree range does not provide an exact length. This wrapper deliberately
        // exposes only Iterator/DoubleEndedIterator for queries.
        match inner {
            ScalingMapIterInner::Array(a) => ScalingRangeIter::Array(a),
            ScalingMapIterInner::Tree(t) => ScalingRangeIter::Tree(t),
        }
    }
}
enum ScalingRangeIter<'a, K, V> {
    Array(std::slice::Iter<'a, (K, V)>),
    Tree(std::collections::btree_map::Range<'a, K, V>),
}
impl<'a, K, V> Iterator for ScalingRangeIter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Array(a) => a.next().map(|(k, v)| (k, v)),
            Self::Tree(t) => t.next(),
        }
    }
}
impl<K, V> DoubleEndedIterator for ScalingRangeIter<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Array(a) => a.next_back().map(|(k, v)| (k, v)),
            Self::Tree(t) => t.next_back(),
        }
    }
}
