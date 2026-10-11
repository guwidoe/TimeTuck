//! Borrowed sample membership without copying the backing collection.
use crate::{NeighborhoodSample, ScalingSequencedSet, SequencedSetIter};
use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    sync::Arc,
};

/// A member borrowed from a slice or sharing the set's original allocation.
#[derive(Debug)]
pub enum SampleMember<'a, T> {
    Borrowed(&'a T),
    Shared(Arc<T>),
}
impl<T> Deref for SampleMember<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Self::Borrowed(v) => v,
            Self::Shared(v) => v,
        }
    }
}
enum Backing<'a, T> {
    Set(&'a ScalingSequencedSet<T>),
    Slice(&'a [T]),
}
/// An order-preserving distinct view. Set edits are visible between iterations;
/// an active set iterator prevents mutation through the set's existing borrow
/// guard. Slice duplicates are skipped lazily without allocating or cloning T.
/// Equality and hashing match owned samples and ignore encounter order. A live
/// view's hash changes with its backing set: do not mutate it while used as a key.
pub struct NeighborhoodSampleView<'a, T> {
    backing: Backing<'a, T>,
}
impl<T> Clone for NeighborhoodSampleView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for NeighborhoodSampleView<'_, T> {}
impl<T> Copy for Backing<'_, T> {}
impl<T> Clone for Backing<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: Eq + Hash> NeighborhoodSampleView<'a, T> {
    pub fn from_set(set: &'a ScalingSequencedSet<T>) -> Self {
        Self {
            backing: Backing::Set(set),
        }
    }
    pub fn from_slice(slice: &'a [T]) -> Self {
        Self {
            backing: Backing::Slice(slice),
        }
    }
    pub fn contains(&self, value: &T) -> bool {
        match self.backing {
            Backing::Set(s) => s.contains(value),
            Backing::Slice(s) => s.contains(value),
        }
    }
    pub fn len(&self) -> usize {
        match self.backing {
            Backing::Set(s) => s.len(),
            Backing::Slice(_) => self.iter().count(),
        }
    }
    pub fn is_empty(&self) -> bool {
        match self.backing {
            Backing::Set(s) => s.is_empty(),
            Backing::Slice(s) => s.is_empty(),
        }
    }
    pub fn representative(&self) -> Option<SampleMember<'a, T>> {
        match self.backing {
            Backing::Set(s) => s.first().map(SampleMember::Shared),
            Backing::Slice(s) => s.first().map(SampleMember::Borrowed),
        }
    }
    pub fn iter(&self) -> SampleViewIter<'a, T> {
        SampleViewIter {
            inner: match self.backing {
                Backing::Set(s) => Members::Set(s.iter()),
                Backing::Slice(s) => Members::Slice { slice: s, index: 0 },
            },
        }
    }
}
enum Members<'a, T> {
    Set(SequencedSetIter<'a, T>),
    Slice { slice: &'a [T], index: usize },
}
pub struct SampleViewIter<'a, T> {
    inner: Members<'a, T>,
}
impl<'a, T: Eq> Iterator for SampleViewIter<'a, T> {
    type Item = SampleMember<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            Members::Set(s) => s.next().map(SampleMember::Shared),
            Members::Slice { slice, index } => {
                while *index < slice.len() {
                    let i = *index;
                    *index += 1;
                    if !slice[..i].contains(&slice[i]) {
                        return Some(SampleMember::Borrowed(&slice[i]));
                    }
                }
                None
            }
        }
    }
}
impl<T: Eq> std::iter::FusedIterator for SampleViewIter<'_, T> {}
impl<T: Eq + Hash> PartialEq for NeighborhoodSampleView<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|v| other.contains(&v))
    }
}
impl<T: Eq + Hash> Eq for NeighborhoodSampleView<'_, T> {}
impl<T: Eq + Hash> PartialEq<NeighborhoodSample<T>> for NeighborhoodSampleView<'_, T> {
    fn eq(&self, other: &NeighborhoodSample<T>) -> bool {
        self.len() == other.len() && self.iter().all(|v| other.contains(&v))
    }
}
impl<T: Eq + Hash> PartialEq<NeighborhoodSampleView<'_, T>> for NeighborhoodSample<T> {
    fn eq(&self, other: &NeighborhoodSampleView<'_, T>) -> bool {
        other == self
    }
}
impl<T: Eq + Hash> Hash for NeighborhoodSampleView<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let sum = self.iter().fold(0u64, |sum, member| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            member.deref().hash(&mut h);
            sum.wrapping_add(h.finish())
        });
        self.len().hash(state);
        sum.hash(state);
    }
}
