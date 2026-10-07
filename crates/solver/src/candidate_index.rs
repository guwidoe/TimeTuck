//! Mutable keyed candidate storage with borrowed indexed query views.
use crate::{LazyConcat, ScalingMap, random::SplitMix64, unique_slice_samples};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    iter::FusedIterator,
    ops::RangeBounds,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateIndexError<I> {
    DuplicateId(I),
    UnknownId(I),
}
impl<I: fmt::Debug> fmt::Display for CandidateIndexError<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "candidate index: {self:?}")
    }
}
impl<I: fmt::Debug> Error for CandidateIndexError<I> {}
/// Identity distinguishes occurrences even when payloads are equal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedCandidate<I, T> {
    pub id: I,
    pub value: T,
}
/// A borrowed dense bucket. Mutating the index requires releasing this view and
/// its iterators. No payload Clone bound or copied candidate list is needed.
pub struct CandidateBucket<'a, I, T> {
    entries: &'a [IndexedCandidate<I, T>],
}
impl<'a, I, T> CandidateBucket<'a, I, T> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> std::slice::Iter<'a, IndexedCandidate<I, T>> {
        self.entries.iter()
    }
    pub fn get(&self, index: usize) -> Option<&'a IndexedCandidate<I, T>> {
        self.entries.get(index)
    }
    /// Uniform sampling without replacement. The permutation bookkeeping is
    /// sparse and grows with draws; payloads remain borrowed from the bucket.
    pub fn unique(
        &self,
        seed: u64,
    ) -> impl FusedIterator<Item = &'a IndexedCandidate<I, T>> + use<'a, I, T> {
        unique_slice_samples(self.entries, seed)
            .map(|v| v.expect("seeded draws stay within their bound"))
    }
    /// Uniform draws with replacement. Empty buckets terminate immediately;
    /// nonempty buckets never exhaust. Equal payloads remain distinct occurrences.
    /// Explicit seeds are reproducible, without Java PRNG trace compatibility.
    pub fn repeating(
        &self,
        seed: u64,
    ) -> impl FusedIterator<Item = &'a IndexedCandidate<I, T>> + use<'a, I, T> {
        let entries = self.entries;
        let mut random = SplitMix64::new(seed);
        std::iter::from_fn(move || {
            if entries.is_empty() {
                None
            } else {
                let index = if entries.len() == 1 {
                    0
                } else {
                    random.below_wide(entries.len() as u128) as usize
                };
                Some(&entries[index])
            }
        })
        .fuse()
    }
}
/// Independent candidate index for constructing native neighborhoods.
///
/// Each identity belongs to one captured key. Dense buckets support direct
/// indexed sampling; an identity-to-position map supports removal and rekeying.
/// Removal swaps in the last bucket entry, so iteration order can change. Queries
/// borrow immutable buckets, preventing concurrent mutation. Keys and IDs need
/// stable, consistent Ord/Clone behavior. Payloads need neither Clone nor Eq.
/// Duplicate insertion and unknown removal/replacement leave the index unchanged.
/// Equality-key types may be native tuples. This is not a stream-graph node and
/// callers explicitly synchronize it with their working assignment.
///
/// ```
/// use timetuck::CandidateIndex;
/// let mut index=CandidateIndex::new();
/// index.insert(0, ("team",40), "Ann")?;
/// index.insert(1, ("team",40), "Edna")?;
/// assert_eq!(index.bucket(&("team",40)).len(),2);
/// index.replace(&0, ("team",30), "Ann updated")?;
/// assert_eq!(index.bucket(&("team",40)).unique(0).next().unwrap().value,"Edna");
/// assert_eq!(index.remove(&0)?.value,"Ann updated");
/// # Ok::<(), timetuck::CandidateIndexError<usize>>(())
/// ```
pub struct CandidateIndex<K, I, T> {
    buckets: ScalingMap<K, Vec<IndexedCandidate<I, T>>>,
    positions: BTreeMap<I, (K, usize)>,
}
impl<K, I, T> Default for CandidateIndex<K, I, T> {
    fn default() -> Self {
        Self {
            buckets: ScalingMap::new(),
            positions: BTreeMap::new(),
        }
    }
}
impl<K: Ord + Clone, I: Ord + Clone, T> CandidateIndex<K, I, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.positions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
    pub fn key_count(&self) -> usize {
        self.buckets.len()
    }
    pub fn insert(&mut self, id: I, key: K, value: T) -> Result<(), CandidateIndexError<I>> {
        if self.positions.contains_key(&id) {
            return Err(CandidateIndexError::DuplicateId(id));
        }
        let bucket = self.buckets.get_or_insert_with(key.clone(), Vec::new);
        self.positions.insert(id.clone(), (key, bucket.len()));
        bucket.push(IndexedCandidate { id, value });
        Ok(())
    }
    pub fn remove(&mut self, id: &I) -> Result<IndexedCandidate<I, T>, CandidateIndexError<I>> {
        let (key, position) = self
            .positions
            .remove(id)
            .ok_or_else(|| CandidateIndexError::UnknownId(id.clone()))?;
        let bucket = self.buckets.get_mut(&key).expect("indexed bucket");
        let removed = bucket.swap_remove(position);
        if let Some(moved) = bucket.get(position) {
            self.positions
                .get_mut(&moved.id)
                .expect("indexed occurrence")
                .1 = position;
        }
        if bucket.is_empty() {
            self.buckets.remove(&key);
        }
        Ok(removed)
    }
    /// Replace the captured key and payload while keeping the identity. Returns
    /// the previous payload; equal-key replacement keeps the bucket position.
    pub fn replace(&mut self, id: &I, key: K, value: T) -> Result<T, CandidateIndexError<I>> {
        let (old_key, position) = self
            .positions
            .get(id)
            .ok_or_else(|| CandidateIndexError::UnknownId(id.clone()))?;
        if old_key == &key {
            return Ok(std::mem::replace(
                &mut self.buckets.get_mut(old_key).expect("indexed bucket")[*position].value,
                value,
            ));
        }
        let removed = self.remove(id)?;
        self.insert(removed.id, key, value)
            .unwrap_or_else(|_| unreachable!("removed identity is free"));
        Ok(removed.value)
    }
    pub fn get(&self, id: &I) -> Option<&IndexedCandidate<I, T>> {
        let (key, pos) = self.positions.get(id)?;
        self.buckets.get(key).and_then(|b| b.get(*pos))
    }
    pub fn bucket(&self, key: &K) -> CandidateBucket<'_, I, T> {
        CandidateBucket {
            entries: self.buckets.get(key).map_or(&[], Vec::as_slice),
        }
    }
    /// Matching groups in first-query-key order, ignoring repeated/missing keys.
    /// The view borrows buckets and supports global unique occurrence sampling.
    pub fn query_many(
        &self,
        keys: impl IntoIterator<Item = K>,
    ) -> LazyConcat<'_, IndexedCandidate<I, T>> {
        let mut seen = BTreeSet::new();
        let mut out = LazyConcat::new();
        for key in keys {
            let bucket = self.buckets.get(&key);
            if seen.insert(key)
                && let Some(bucket) = bucket
            {
                out.push_slice(bucket)
                    .expect("disjoint buckets fit index length");
            }
        }
        out
    }
    /// Ordered-key range query, borrowing groups without flattening candidates.
    /// Bounds follow BTreeMap::range, including its rejection of invalid ranges.
    pub fn range(&self, bounds: impl RangeBounds<K>) -> LazyConcat<'_, IndexedCandidate<I, T>> {
        let mut out = LazyConcat::new();
        for (_, bucket) in self.buckets.range(bounds) {
            out.push_slice(bucket)
                .expect("disjoint buckets fit index length");
        }
        out
    }
    /// All occurrence pairs satisfying `left_key comparison right_key`.
    /// Uses borrowed Cartesian blocks for matching key groups, never a vector
    /// of all pairs. Equal payloads with different IDs remain distinct. Equality
    /// uses one lookup per left key; ordered relations enumerate matching key
    /// groups. Metadata may reach left-key-count times right-key-count.
    /// Queries borrow both indexes; build a fresh view after mutation.
    pub fn join<'a, J: Ord + Clone, U>(
        &'a self,
        right: &'a CandidateIndex<K, J, U>,
        comparison: crate::JoinComparison,
    ) -> Result<crate::IndexedCandidatePairs<'a, I, T, J, U>, crate::PairCountOverflow> {
        use crate::JoinComparison::*;
        use std::ops::Bound::{Excluded, Included, Unbounded};
        let mut pairs = crate::CandidatePairs::new();
        for (key, left) in self.buckets.iter() {
            if comparison == Equal {
                if let Some(other) = right.buckets.get(key) {
                    pairs.push_product(left, other)?;
                }
                continue;
            }
            let bounds = match comparison {
                LessThan => (Excluded(key.clone()), Unbounded),
                LessThanOrEqual => (Included(key.clone()), Unbounded),
                GreaterThan => (Unbounded, Excluded(key.clone())),
                GreaterThanOrEqual => (Unbounded, Included(key.clone())),
                Equal => unreachable!(),
            };
            for (_, other) in right.buckets.range(bounds) {
                pairs.push_product(left, other)?;
            }
        }
        Ok(pairs)
    }
    pub fn clear(&mut self) {
        self.positions.clear();
        self.buckets.clear();
    }
}

/// Uniform repeated draws from a concatenated query view, without copying its
/// candidates. Empty views exhaust immediately. Every selected occurrence is
/// fetched through the view, retaining its cached supplier error behavior.
/// Sampling bookkeeping is constant-size; loading a deferred group materializes
/// the entire group. Nonempty views do not exhaust, including after item errors.
pub fn repeating_candidates<'s, T>(
    candidates: &'s LazyConcat<'_, T>,
    seed: u64,
) -> impl FusedIterator<Item = Result<&'s T, crate::LazyConcatError>> + 's {
    let mut random = SplitMix64::new(seed);
    std::iter::from_fn(move || {
        if candidates.is_empty() {
            None
        } else {
            let i = if candidates.len() == 1 {
                0
            } else {
                random.below_wide(candidates.len() as u128) as usize
            };
            Some(candidates.get(i))
        }
    })
    .fuse()
}
