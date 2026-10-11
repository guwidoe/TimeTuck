//! Snapshot datasets and repeatable, lazy neighborhood sampling.
use crate::{
    KeyedSampler, NeighborhoodSample, SampleError, SampleRandom, Sampler, SnapshotMap, TupleChange,
    assemble_keyed_sample, assemble_sample,
};
use std::{collections::BTreeMap, iter::FusedIterator, num::NonZeroUsize, sync::Arc};

/// Reopen the source for each sample. Construction does no source work. A missing
/// sample or an error permanently ends this iterator (an error is yielded once).
/// The caller supplies source ordering, e.g. a dataset's shuffled exhaustive order.
/// Productive sources can yield indefinitely: use a solver budget or `take`.
///
/// ```
/// use timetuck::{repeated_samples, SizeSampler};
/// let mut samples = repeated_samples(|_| 0..4, SizeSampler::exactly(2)?, 9);
/// assert_eq!(samples.next().unwrap()?.members(), &[0, 1]);
/// assert_eq!(samples.next().unwrap()?.members(), &[0, 1]);
/// # Ok::<(), timetuck::SampleError>(())
/// ```
pub fn repeated_samples<
    T: Eq,
    I: IntoIterator<Item = T>,
    F: FnMut(&mut SampleRandom) -> I,
    S: Sampler<T>,
>(
    mut source: F,
    mut rule: S,
    seed: u64,
) -> impl FusedIterator<Item = Result<NeighborhoodSample<T>, SampleError>> {
    let mut random = SampleRandom::new(seed);
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        match assemble_sample(source(&mut random), &mut rule, &mut random) {
            Ok(Some(sample)) => Some(Ok(sample)),
            Ok(None) => {
                done = true;
                None
            }
            Err(error) => {
                done = true;
                Some(Err(error))
            }
        }
    })
    .fuse()
}
/// Keyed counterpart of repeated_samples. The key is passed to the source factory
/// and to the rule's keyed reset on every attempt. The source selects the group.
pub fn repeated_keyed_samples<
    K,
    T: Eq,
    I: IntoIterator<Item = T>,
    F: FnMut(&K, &mut SampleRandom) -> I,
    S: KeyedSampler<K, T>,
>(
    key: K,
    mut source: F,
    mut rule: S,
    seed: u64,
) -> impl FusedIterator<Item = Result<NeighborhoodSample<T>, SampleError>> {
    let mut random = SampleRandom::new(seed);
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        match assemble_keyed_sample(source(&key, &mut random), &key, &mut rule, &mut random) {
            Ok(Some(sample)) => Some(Ok(sample)),
            Ok(None) => {
                done = true;
                None
            }
            Err(error) => {
                done = true;
                Some(Err(error))
            }
        }
    })
    .fuse()
}
/// Immutable rows. Equal rows can occur at separate source identities; exhaustive
/// iteration preserves those occurrences, whereas samples deduplicate by T::eq.
/// Use immutable identity-bearing values when entity identity matters.
#[derive(Debug)]
pub struct NeighborhoodDataset<T> {
    rows: Arc<[T]>,
}
impl<T> Clone for NeighborhoodDataset<T> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
        }
    }
}
impl<T> Default for NeighborhoodDataset<T> {
    fn default() -> Self {
        Self::new(std::iter::empty())
    }
}
impl<T> NeighborhoodDataset<T> {
    pub fn new(rows: impl IntoIterator<Item = T>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
        }
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.rows.iter()
    }
    /// Whether two handles share the exact same snapshot, regardless of contents.
    pub fn same_snapshot(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.rows, &other.rows)
    }
    pub fn join<U, F: Fn(&T, &U) -> bool>(
        &self,
        right: &NeighborhoodDataset<U>,
        predicate: F,
    ) -> NeighborhoodJoin<T, U, F> {
        NeighborhoodJoin {
            left: self.clone(),
            right: right.clone(),
            predicate,
        }
    }
}
impl<T: Clone> NeighborhoodDataset<T> {
    /// Every row occurrence exactly once, in a seeded Fisher-Yates permutation.
    /// Only row indices are shuffled; values are cloned as they are consumed.
    pub fn exhaustive_iter(&self, random: &mut SampleRandom) -> DatasetPermutation<T> {
        let mut order: Vec<_> = (0..self.len()).collect();
        for i in (1..order.len()).rev() {
            let j = random.index(NonZeroUsize::new(i + 1).unwrap());
            order.swap(i, j);
        }
        DatasetPermutation {
            rows: self.rows.clone(),
            order: order.into_iter(),
        }
    }
    /// Lazy uniform permutation for prefix sampling. Opening takes one seed
    /// from the caller and clones only the snapshot handle. Each consumed row
    /// uses adaptive sparse/dense rank bookkeeping; payloads clone on demand.
    /// The independent random stream means partial consumption does not change
    /// the caller's later draws. Order differs from exhaustive_iter's eager
    /// Fisher-Yates permutation; both preserve every source occurrence once.
    pub fn lazy_exhaustive_iter(&self, random: &mut SampleRandom) -> LazyDatasetPermutation<T> {
        let seed = random.index(NonZeroUsize::new(usize::MAX).unwrap()) as u64;
        LazyDatasetPermutation {
            rows: self.rows.clone(),
            indices: crate::RetiringIndices::new(self.len() as u128, seed),
        }
    }
    fn sample_order(&self, random: &mut SampleRandom, eager: bool) -> SampleOrder<T> {
        if eager {
            SampleOrder::Eager(self.exhaustive_iter(random))
        } else {
            SampleOrder::Lazy(self.lazy_exhaustive_iter(random))
        }
    }
    /// Sampling with replacement. Nonempty snapshots never run dry. Standard Rust
    /// consumers such as collect/for_each therefore require an explicit bound.
    pub fn random_iter<'a>(&self, random: &'a mut SampleRandom) -> DatasetRandom<'a, T> {
        DatasetRandom {
            rows: self.rows.clone(),
            random,
        }
    }
}
impl<T: Eq + Clone> NeighborhoodDataset<T> {
    pub fn samples<S: Sampler<T>>(
        &self,
        rule: S,
        seed: u64,
    ) -> impl FusedIterator<Item = Result<NeighborhoodSample<T>, SampleError>> + use<T, S> {
        let snapshot = self.clone();
        let eager = rule.exhausts_source() || rule.minimum_size() >= self.len().div_ceil(32);
        repeated_samples(move |r| snapshot.sample_order(r, eager), rule, seed)
    }
}
/// Snapshot-owning lazy permutation. Equal payloads at different source rows
/// remain distinct occurrences. Bookkeeping starts sparse, then switches to
/// dense indices after sufficient consumption. No Clone work is
/// performed until next() yields a member. Native random ordering is reproducible
/// but not compatible with the eager permutation or Java PRNG traces.
pub struct LazyDatasetPermutation<T> {
    rows: Arc<[T]>,
    indices: crate::RetiringIndices,
}
impl<T> LazyDatasetPermutation<T> {
    pub fn stored_indices(&self) -> usize {
        self.indices.stored_indices()
    }
}
impl<T: Clone> Iterator for LazyDatasetPermutation<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let index = self.indices.draw()?;
        self.indices.retire().expect("rank was drawn");
        Some(self.rows[index as usize].clone())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.indices.remaining() as usize;
        (n, Some(n))
    }
}
impl<T: Clone> ExactSizeIterator for LazyDatasetPermutation<T> {}
impl<T: Clone> FusedIterator for LazyDatasetPermutation<T> {}

enum SampleOrder<T> {
    Eager(DatasetPermutation<T>),
    Lazy(LazyDatasetPermutation<T>),
}
impl<T: Clone> Iterator for SampleOrder<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self {
            Self::Eager(i) => i.next(),
            Self::Lazy(i) => i.next(),
        }
    }
}

pub struct DatasetPermutation<T> {
    rows: Arc<[T]>,
    order: std::vec::IntoIter<usize>,
}
impl<T: Clone> Iterator for DatasetPermutation<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.order.next().map(|i| self.rows[i].clone())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.order.size_hint()
    }
}
impl<T: Clone> ExactSizeIterator for DatasetPermutation<T> {}
impl<T: Clone> FusedIterator for DatasetPermutation<T> {}
/// Bulk consumption of a nonempty random dataset would never terminate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnboundedDatasetIteration;
impl std::fmt::Display for UnboundedDatasetIteration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("random dataset iteration requires an explicit draw limit")
    }
}
impl std::error::Error for UnboundedDatasetIteration {}
/// Sampling with replacement. Use a bounded adapter such as take(limit) for
/// bulk consumption. Direct for_each rejects a nonempty dataset with a typed
/// panic; try_for_each_remaining returns the same error without panicking.
pub struct DatasetRandom<'a, T> {
    rows: Arc<[T]>,
    random: &'a mut SampleRandom,
}
impl<T> DatasetRandom<'_, T> {
    /// Inspect availability without consuming a random draw.
    pub fn has_next(&self) -> bool {
        !self.rows.is_empty()
    }
    /// Empty datasets complete; unbounded datasets reject before invoking the
    /// callback or consuming randomness. Bound the iterator to process draws.
    pub fn try_for_each_remaining(
        &mut self,
        _: impl FnMut(T),
    ) -> Result<(), UnboundedDatasetIteration> {
        if self.has_next() {
            Err(UnboundedDatasetIteration)
        } else {
            Ok(())
        }
    }
}
impl<T: Clone> Iterator for DatasetRandom<'_, T> {
    type Item = T;
    fn for_each<F: FnMut(T)>(self, _: F) {
        if self.has_next() {
            std::panic::panic_any(UnboundedDatasetIteration);
        }
    }
    fn next(&mut self) -> Option<T> {
        let bound = NonZeroUsize::new(self.rows.len())?;
        Some(self.rows[self.random.index(bound)].clone())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.rows.is_empty() {
            (0, Some(0))
        } else {
            (usize::MAX, None)
        }
    }
}
impl<T: Clone> FusedIterator for DatasetRandom<'_, T> {}

/// Identity-indexed input store accepting ordinary stream changes. A batch is
/// installed atomically. Repeated snapshot() calls without updates share storage;
/// held snapshots/samples remain isolated from later insertions and retractions.
/// T::clone must preserve this isolation (do not put mutable shared facts in T).
///
/// ```
/// use timetuck::{NeighborhoodDatasetStore, TupleChange};
/// let mut store = NeighborhoodDatasetStore::new();
/// store.apply(TupleChange::put(0, "before"));
/// let held = store.snapshot();
/// store.apply(TupleChange::put(0, "after"));
/// assert_eq!(held.iter().copied().collect::<Vec<_>>(), vec!["before"]);
/// assert_eq!(store.snapshot().iter().copied().collect::<Vec<_>>(), vec!["after"]);
/// ```
pub struct NeighborhoodDatasetStore<I, T> {
    rows: SnapshotMap<I, T>,
    cached: Option<NeighborhoodDataset<T>>,
}
impl<I, T> Default for NeighborhoodDatasetStore<I, T> {
    fn default() -> Self {
        Self {
            rows: SnapshotMap::new(),
            cached: None,
        }
    }
}
impl<I: Ord, T> NeighborhoodDatasetStore<I, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn rows(&self) -> &SnapshotMap<I, T> {
        &self.rows
    }
    pub fn apply(&mut self, change: TupleChange<I, T>) {
        self.apply_batch([change]);
    }
    pub fn apply_batch(&mut self, changes: impl IntoIterator<Item = TupleChange<I, T>>) {
        let mut next = self.rows.clone();
        let mut changed = false;
        for change in changes {
            match change.value {
                Some(value) => {
                    next.insert(change.id, value);
                    changed = true;
                }
                None => {
                    changed |= next.remove(&change.id);
                }
            }
        }
        if changed {
            self.rows = next;
            self.cached = None;
        }
    }
    pub fn clear(&mut self) {
        if !self.is_empty() {
            self.rows = SnapshotMap::new();
            self.cached = None;
        }
    }
}
impl<I: Ord, T: Clone> NeighborhoodDatasetStore<I, T> {
    pub fn snapshot(&mut self) -> NeighborhoodDataset<T> {
        self.cached
            .get_or_insert_with(|| NeighborhoodDataset::new(self.rows.values().cloned()))
            .clone()
    }
}
/// A lazy Cartesian join filtered by a user predicate. The two input snapshots
/// are held independently of their stores; create a new join after store updates.
pub struct NeighborhoodJoin<A, B, F> {
    left: NeighborhoodDataset<A>,
    right: NeighborhoodDataset<B>,
    predicate: F,
}
impl<A, B, F: Fn(&A, &B) -> bool> NeighborhoodJoin<A, B, F> {
    pub fn iter(&self) -> impl Iterator<Item = (&A, &B)> {
        self.left.iter().flat_map(move |a| {
            self.right
                .iter()
                .filter(move |b| (self.predicate)(a, b))
                .map(move |b| (a, b))
        })
    }
    pub fn len(&self) -> usize {
        self.iter().count()
    }
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}
impl<A: Eq + Clone, B: Eq + Clone, F: Fn(&A, &B) -> bool> NeighborhoodJoin<A, B, F> {
    /// Exhaustively traverse the right snapshot for each attempt, filtering the
    /// entire source as needed. Rare matches cannot be lost through retry limits.
    pub fn samples_for<S: KeyedSampler<A, B>>(
        &self,
        key: A,
        rule: S,
        seed: u64,
    ) -> impl FusedIterator<Item = Result<NeighborhoodSample<B>, SampleError>> {
        let present = self.left.iter().any(|a| a == &key);
        let eager = rule.exhausts_source() || rule.minimum_size() >= self.right.len().div_ceil(32);
        repeated_keyed_samples(
            key,
            move |key, r| {
                let key = key.clone();
                self.right
                    .sample_order(r, eager)
                    .filter(move |b| present && (self.predicate)(&key, b))
            },
            rule,
            seed,
        )
    }
}
impl<A: Ord + Clone, B: Clone, F: Fn(&A, &B) -> bool> NeighborhoodJoin<A, B, F> {
    pub fn cache(&self) -> CachedNeighborhoodJoin<A, B> {
        let mut buckets: BTreeMap<A, Vec<B>> = BTreeMap::new();
        let mut pairs = Vec::new();
        for (a, b) in self.iter() {
            buckets.entry(a.clone()).or_default().push(b.clone());
            pairs.push((a.clone(), b.clone()));
        }
        CachedNeighborhoodJoin {
            pairs: NeighborhoodDataset::new(pairs),
            buckets: buckets
                .into_iter()
                .map(|(k, v)| (k, NeighborhoodDataset::new(v)))
                .collect(),
        }
    }
}
/// Materialized join rows and per-left buckets. Refresh explicitly from a new
/// join after source changes. Refresh installs the entire new cache at once;
/// a predicate/clone panic leaves the old cache and all held snapshots unchanged.
pub struct CachedNeighborhoodJoin<A, B> {
    pairs: NeighborhoodDataset<(A, B)>,
    buckets: BTreeMap<A, NeighborhoodDataset<B>>,
}
impl<A: Ord, B> CachedNeighborhoodJoin<A, B> {
    pub fn len(&self) -> usize {
        self.pairs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
    pub fn rows(&self) -> NeighborhoodDataset<(A, B)> {
        self.pairs.clone()
    }
    pub fn slice(&self, key: &A) -> NeighborhoodDataset<B> {
        self.buckets.get(key).cloned().unwrap_or_default()
    }
}
impl<A: Ord + Clone, B: Clone> CachedNeighborhoodJoin<A, B> {
    pub fn refresh<F: Fn(&A, &B) -> bool>(&mut self, join: &NeighborhoodJoin<A, B, F>) {
        *self = join.cache();
    }
}
impl<A: Ord + Clone, B: Eq + Clone> CachedNeighborhoodJoin<A, B> {
    pub fn samples_for<S: KeyedSampler<A, B>>(
        &self,
        key: A,
        rule: S,
        seed: u64,
    ) -> impl FusedIterator<Item = Result<NeighborhoodSample<B>, SampleError>> + use<A, B, S> {
        let slice = self.slice(&key);
        let eager = rule.exhausts_source() || rule.minimum_size() >= slice.len().div_ceil(32);
        repeated_keyed_samples(key, move |_, r| slice.sample_order(r, eager), rule, seed)
    }
}
