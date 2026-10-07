//! Indexed collection-membership queries with identity deduplication.
use crate::{CandidateIndexError, IndexedCandidate, random::SplitMix64, unique_slice_samples};
use std::{
    collections::{BTreeMap, BTreeSet},
    iter::FusedIterator,
};
struct MembershipRow<K, I, T> {
    keys: BTreeSet<K>,
    candidate: IndexedCandidate<I, T>,
}
/// Captured membership matches in native identity order. A query materializes
/// only deduplicated references, not payload copies. All sampling is over these
/// identities, so overlap between keys never increases an occurrence's weight.
/// Holding this view prevents mutation of its source index.
pub struct MembershipMatches<'a, I, T> {
    matches: Vec<&'a IndexedCandidate<I, T>>,
}
impl<'a, I, T> MembershipMatches<'a, I, T> {
    pub fn len(&self) -> usize {
        self.matches.len()
    }
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&'a IndexedCandidate<I, T>> {
        self.matches.get(index).copied()
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a IndexedCandidate<I, T>>
    + ExactSizeIterator
    + FusedIterator
    + '_ {
        self.matches.iter().copied()
    }
    pub fn unique(&self, seed: u64) -> impl FusedIterator<Item = &'a IndexedCandidate<I, T>> + '_ {
        unique_slice_samples(&self.matches, seed)
            .map(|r| *r.expect("seeded draws stay within their bound"))
    }
    /// Repeated uniform draws with replacement; empty matches exhaust immediately.
    /// PRNG traces are native, not Java-compatible.
    pub fn repeating(
        &self,
        seed: u64,
    ) -> impl FusedIterator<Item = &'a IndexedCandidate<I, T>> + '_ {
        let mut random = SplitMix64::new(seed);
        std::iter::from_fn(move || {
            if self.matches.is_empty() {
                None
            } else {
                let index = if self.matches.len() == 1 {
                    0
                } else {
                    random.below_wide(self.matches.len() as u128) as usize
                };
                Some(self.matches[index])
            }
        })
        .fuse()
    }
}
/// Mutable inverted index: one stable candidate identity can have several keys.
///
/// put captures/deduplicates the complete key set and replaces any old row with
/// the same identity. Empty-key candidates are retained, but match no any-key
/// query. remove deletes all captured memberships and drops empty buckets.
/// Keys may be tuples to encode equality partitions or combined membership tests.
/// Explicit queries provide their key combinations; no reflective joiner inference.
///
/// Queries union postings and resolve each matching identity once, in ID order.
/// Query cost includes the visited postings and O(matches) stored references;
/// the view does not defer deduplication until the first draw. Payloads require
/// neither Clone nor Eq. Key and identity Ord/Clone must be stable and consistent.
/// Callers explicitly update this index when their working assignment changes;
/// this is not an automatically maintained stream-graph node.
///
/// ```
/// use timetuck::MembershipCandidateIndex;
/// let mut workers=MembershipCandidateIndex::new();
/// workers.put(0,["Rust","Java"],"Ann");
/// workers.put(1,["Rust"],"Beth");
/// workers.put(2,[],"No skills");
/// let matches=workers.query_any(["Rust","Java","Rust"]);
/// assert_eq!(matches.len(),2);
/// assert_eq!(matches.unique(0).count(),2);
/// assert_eq!(workers.len(),3);
/// ```
pub struct MembershipCandidateIndex<K, I, T> {
    rows: BTreeMap<I, MembershipRow<K, I, T>>,
    postings: BTreeMap<K, BTreeSet<I>>,
}
impl<K, I, T> Default for MembershipCandidateIndex<K, I, T> {
    fn default() -> Self {
        Self {
            rows: BTreeMap::new(),
            postings: BTreeMap::new(),
        }
    }
}
impl<K: Ord + Clone, I: Ord + Clone, T> MembershipCandidateIndex<K, I, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn key_count(&self) -> usize {
        self.postings.len()
    }
    pub fn get(&self, id: &I) -> Option<&IndexedCandidate<I, T>> {
        self.rows.get(id).map(|r| &r.candidate)
    }
    fn unlink(&mut self, id: &I) -> Option<MembershipRow<K, I, T>> {
        let row = self.rows.remove(id)?;
        for key in &row.keys {
            let ids = self
                .postings
                .get_mut(key)
                .expect("captured membership bucket");
            ids.remove(id);
            if ids.is_empty() {
                self.postings.remove(key);
            }
        }
        Some(row)
    }
    /// Upsert an occurrence. Returns its former payload when replacing an ID.
    /// Keys are collected before changing the index, so a panicking input iterator
    /// leaves the old row intact. Arbitrary panics in Ord/Clone are not transactional.
    pub fn put(&mut self, id: I, keys: impl IntoIterator<Item = K>, value: T) -> Option<T> {
        let keys: BTreeSet<_> = keys.into_iter().collect();
        let old = self.unlink(&id).map(|r| r.candidate.value);
        for key in &keys {
            self.postings
                .entry(key.clone())
                .or_default()
                .insert(id.clone());
        }
        self.rows.insert(
            id.clone(),
            MembershipRow {
                keys,
                candidate: IndexedCandidate { id, value },
            },
        );
        old
    }
    pub fn remove(&mut self, id: &I) -> Result<IndexedCandidate<I, T>, CandidateIndexError<I>> {
        self.unlink(id)
            .map(|r| r.candidate)
            .ok_or_else(|| CandidateIndexError::UnknownId(id.clone()))
    }
    pub fn query_any(&self, keys: impl IntoIterator<Item = K>) -> MembershipMatches<'_, I, T> {
        let keys: BTreeSet<_> = keys.into_iter().collect();
        let mut matched = BTreeSet::new();
        for key in &keys {
            if let Some(ids) = self.postings.get(key) {
                matched.extend(ids.iter());
            }
        }
        MembershipMatches {
            matches: matched
                .into_iter()
                .map(|id| &self.rows[id].candidate)
                .collect(),
        }
    }
    pub fn clear(&mut self) {
        self.postings.clear();
        self.rows.clear();
    }
}
