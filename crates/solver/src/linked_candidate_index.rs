//! Optional sequential candidate leaves integrated with dense borrowed leaves.
use crate::{
    CandidateBucket, CandidateIndexError, IndexedCandidate, ReadOnlyRandomCursor,
    RepeatingRandomCursor, ScalingMap, UniqueRandomCursor,
};
use std::{
    collections::{BTreeMap, LinkedList},
    fmt,
    iter::{Fuse, FusedIterator},
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeafRandomAccessUnsupported;
impl fmt::Display for LeafRandomAccessUnsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("sequential linked candidate leaves do not support random access")
    }
}
impl std::error::Error for LeafRandomAccessUnsupported {}
pub struct LeafRepeatingCursor<'a, I, T> {
    inner: Fuse<Box<dyn Iterator<Item = &'a IndexedCandidate<I, T>> + 'a>>,
}
pub struct LeafUniqueCursor<'a, I, T> {
    inner: Fuse<Box<dyn Iterator<Item = &'a IndexedCandidate<I, T>> + 'a>>,
}
macro_rules! cursor {
    ($name:ident) => {
        impl<'a, I, T> Iterator for $name<'a, I, T> {
            type Item = &'a IndexedCandidate<I, T>;
            fn next(&mut self) -> Option<Self::Item> {
                self.inner.next()
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.inner.size_hint()
            }
        }
        impl<I, T> FusedIterator for $name<'_, I, T> {}
        impl<I, T> ReadOnlyRandomCursor for $name<'_, I, T> {}
    };
}
cursor!(LeafRepeatingCursor);
cursor!(LeafUniqueCursor);
impl<I, T> RepeatingRandomCursor for LeafRepeatingCursor<'_, I, T> {}
impl<I, T> UniqueRandomCursor for LeafUniqueCursor<'_, I, T> {}
/// One borrowed leaf interface. Dense leaves delegate their existing real
/// samplers; linked leaves offer sequential traversal without materialization.
/// Optional boxed cursors are confined to this interface; existing dense APIs
/// remain unchanged. Unsupported access does not iterate or clone payloads.
pub trait CandidateLeafView<'a, I: 'a, T: 'a> {
    type Iter: Iterator<Item = &'a IndexedCandidate<I, T>>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn iter(&self) -> Self::Iter;
    fn repeating(
        &self,
        seed: u64,
    ) -> Result<LeafRepeatingCursor<'a, I, T>, LeafRandomAccessUnsupported>;
    fn unique(&self, seed: u64) -> Result<LeafUniqueCursor<'a, I, T>, LeafRandomAccessUnsupported>;
}
impl<'a, I: 'a, T: 'a> CandidateLeafView<'a, I, T> for CandidateBucket<'a, I, T> {
    type Iter = std::slice::Iter<'a, IndexedCandidate<I, T>>;
    fn len(&self) -> usize {
        CandidateBucket::len(self)
    }
    fn iter(&self) -> Self::Iter {
        CandidateBucket::iter(self)
    }
    fn repeating(
        &self,
        seed: u64,
    ) -> Result<LeafRepeatingCursor<'a, I, T>, LeafRandomAccessUnsupported> {
        let inner: Box<dyn Iterator<Item = &'a IndexedCandidate<I, T>> + 'a> =
            Box::new(CandidateBucket::repeating(self, seed));
        Ok(LeafRepeatingCursor {
            inner: inner.fuse(),
        })
    }
    fn unique(&self, seed: u64) -> Result<LeafUniqueCursor<'a, I, T>, LeafRandomAccessUnsupported> {
        let inner: Box<dyn Iterator<Item = &'a IndexedCandidate<I, T>> + 'a> =
            Box::new(CandidateBucket::unique(self, seed));
        Ok(LeafUniqueCursor {
            inner: inner.fuse(),
        })
    }
}
pub struct LinkedCandidateBucket<'a, I, T> {
    entries: Option<&'a LinkedList<IndexedCandidate<I, T>>>,
}
impl<'a, I: 'a, T: 'a> CandidateLeafView<'a, I, T> for LinkedCandidateBucket<'a, I, T> {
    type Iter = std::iter::Flatten<std::option::IntoIter<&'a LinkedList<IndexedCandidate<I, T>>>>;
    fn len(&self) -> usize {
        self.entries.map_or(0, LinkedList::len)
    }
    fn iter(&self) -> Self::Iter {
        self.entries.into_iter().flatten()
    }
    fn repeating(
        &self,
        _seed: u64,
    ) -> Result<LeafRepeatingCursor<'a, I, T>, LeafRandomAccessUnsupported> {
        Err(LeafRandomAccessUnsupported)
    }
    fn unique(
        &self,
        _seed: u64,
    ) -> Result<LeafUniqueCursor<'a, I, T>, LeafRandomAccessUnsupported> {
        Err(LeafRandomAccessUnsupported)
    }
}
/// True LinkedList-backed candidate ownership. IDs distinguish equal payloads.
/// Insertion appends; removal preserves other node order; same-key replacement
/// writes the actual node and rekeying appends to the destination bucket.
/// Borrowing follows the same mutation exclusion as CandidateIndex. Payloads
/// need no Clone/Eq; keys/IDs require stable Ord/Clone behavior. Node location
/// uses linear traversal, unlike the unchanged dense indexed backend.
pub struct LinkedCandidateIndex<K, I, T> {
    buckets: ScalingMap<K, LinkedList<IndexedCandidate<I, T>>>,
    keys: BTreeMap<I, K>,
}
impl<K, I, T> Default for LinkedCandidateIndex<K, I, T> {
    fn default() -> Self {
        Self {
            buckets: ScalingMap::new(),
            keys: BTreeMap::new(),
        }
    }
}
impl<K: Ord + Clone, I: Ord + Clone, T> LinkedCandidateIndex<K, I, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.keys.len()
    }
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
    pub fn key_count(&self) -> usize {
        self.buckets.len()
    }
    pub fn insert(&mut self, id: I, key: K, value: T) -> Result<(), CandidateIndexError<I>> {
        if self.keys.contains_key(&id) {
            return Err(CandidateIndexError::DuplicateId(id));
        }
        let entries = self
            .buckets
            .get_or_insert_with(key.clone(), LinkedList::new);
        self.keys.insert(id.clone(), key);
        entries.push_back(IndexedCandidate { id, value });
        Ok(())
    }
    pub fn get(&self, id: &I) -> Option<&IndexedCandidate<I, T>> {
        let key = self.keys.get(id)?;
        self.buckets.get(key)?.iter().find(|entry| &entry.id == id)
    }
    pub fn bucket(&self, key: &K) -> LinkedCandidateBucket<'_, I, T> {
        LinkedCandidateBucket {
            entries: self.buckets.get(key),
        }
    }
    pub fn remove(&mut self, id: &I) -> Result<IndexedCandidate<I, T>, CandidateIndexError<I>> {
        let key = self
            .keys
            .get(id)
            .ok_or_else(|| CandidateIndexError::UnknownId(id.clone()))?
            .clone();
        let entries = self.buckets.get_mut(&key).expect("captured linked bucket");
        let position = entries
            .iter()
            .position(|entry| &entry.id == id)
            .expect("captured linked occurrence");
        let mut tail = entries.split_off(position);
        let removed = tail.pop_front().expect("located linked node");
        entries.append(&mut tail);
        self.keys.remove(id);
        if entries.is_empty() {
            self.buckets.remove(&key);
        }
        Ok(removed)
    }
    pub fn replace(&mut self, id: &I, key: K, value: T) -> Result<T, CandidateIndexError<I>> {
        let old = self
            .keys
            .get(id)
            .ok_or_else(|| CandidateIndexError::UnknownId(id.clone()))?;
        if old == &key {
            let entry = self
                .buckets
                .get_mut(old)
                .expect("captured linked bucket")
                .iter_mut()
                .find(|entry| &entry.id == id)
                .expect("captured linked occurrence");
            return Ok(std::mem::replace(&mut entry.value, value));
        }
        let removed = self.remove(id)?;
        self.insert(removed.id, key, value)
            .unwrap_or_else(|_| unreachable!("removed identity free"));
        Ok(removed.value)
    }
}
