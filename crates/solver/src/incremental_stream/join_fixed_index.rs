//! Complete, bounded usize input universes use compact membership buckets.
//! Keys retain their original Arc representatives; bit order is input ID order.
//! Only changed buckets are copied, and ordinary key-map journals undo updates.
use super::{JoinComparison, SnapshotMap};
use std::{ops::Bound, sync::Arc};
type SharedBuckets<I, K> = Vec<(Arc<K>, Arc<SnapshotMap<I, ()>>)>;

#[derive(Clone)]
struct Members {
    words: Vec<u64>,
    len: usize,
}
impl Members {
    fn new(count: usize) -> Self {
        Self {
            words: vec![0; count.div_ceil(64)],
            len: 0,
        }
    }
    fn insert(&mut self, id: usize) {
        let bit = 1u64 << (id % 64);
        let word = &mut self.words[id / 64];
        if *word & bit == 0 {
            *word |= bit;
            self.len += 1;
        }
    }
    fn remove(&mut self, id: usize) {
        let bit = 1u64 << (id % 64);
        let word = &mut self.words[id / 64];
        if *word & bit != 0 {
            *word &= !bit;
            self.len -= 1;
        }
    }
    fn ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(word_index, word)| {
                let mut bits = *word;
                std::iter::from_fn(move || {
                    if bits == 0 {
                        None
                    } else {
                        let bit = bits.trailing_zeros() as usize;
                        bits &= bits - 1;
                        Some(word_index * 64 + bit)
                    }
                })
            })
    }
}

pub(super) struct FixedIndex<I, K> {
    keys: Vec<Arc<I>>,
    buckets: SnapshotMap<K, Members>,
    index: fn(&I) -> usize,
}
impl<I, K> Clone for FixedIndex<I, K> {
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            buckets: self.buckets.clone(),
            index: self.index,
        }
    }
}
impl<I, K> FixedIndex<I, K> {
    pub(super) fn new(
        keys: Vec<Arc<I>>,
        buckets: SharedBuckets<I, K>,
        index: fn(&I) -> usize,
    ) -> Self {
        let buckets = buckets
            .into_iter()
            .map(|(key, ids)| {
                let mut members = Members::new(keys.len());
                for id in ids.keys() {
                    members.insert(index(id));
                }
                (key, Arc::new(members))
            })
            .collect();
        Self {
            keys,
            buckets: SnapshotMap::from_shared_sorted(buckets),
            index,
        }
    }
    pub(super) fn clear(&mut self) {
        self.buckets.clear();
    }
    pub(super) fn begin(&mut self) -> usize {
        self.buckets.begin_journal()
    }
    pub(super) fn commit(&mut self, token: usize) {
        self.buckets.commit_journal(token);
    }
    pub(super) fn rollback(&mut self, token: usize) {
        self.buckets.rollback_journal(token);
    }
    pub(super) fn install(&mut self, saved: Self) {
        self.buckets.install_snapshot(saved.buckets);
    }
}
impl<I, K: Ord + Clone> FixedIndex<I, K> {
    pub(super) fn remove(&mut self, key: &K, id: &I) {
        let mut members = self.buckets.get(key).expect("indexed key").clone();
        members.remove((self.index)(id));
        if members.len == 0 {
            self.buckets.remove(key);
        } else {
            self.buckets.insert(key.clone(), members);
        }
    }
    pub(super) fn insert(&mut self, key: &K, id: &I) {
        let mut members = self
            .buckets
            .get(key)
            .cloned()
            .unwrap_or_else(|| Members::new(self.keys.len()));
        members.insert((self.index)(id));
        self.buckets.insert(key.clone(), members);
    }
    pub(super) fn matching<'a>(
        &'a self,
        key: &'a K,
        comparison: JoinComparison,
    ) -> (usize, impl Iterator<Item = &'a I>) {
        use Bound::{Excluded, Included, Unbounded};
        let (exact, range) = match comparison {
            JoinComparison::Equal => (self.buckets.get(key), None),
            JoinComparison::LessThan => (None, Some(self.buckets.range(Excluded(key), Unbounded))),
            JoinComparison::LessThanOrEqual => {
                (None, Some(self.buckets.range(Included(key), Unbounded)))
            }
            JoinComparison::GreaterThan => {
                (None, Some(self.buckets.range(Unbounded, Excluded(key))))
            }
            JoinComparison::GreaterThanOrEqual => {
                (None, Some(self.buckets.range(Unbounded, Included(key))))
            }
        };
        (
            exact.map_or(0, |members| members.len),
            exact
                .into_iter()
                .chain(range.into_iter().flatten().map(|(_, members)| members))
                .flat_map(|members| members.ids())
                .map(|id| self.keys[id].as_ref()),
        )
    }
}
