//! Compact insertion-ordered membership with live borrowed reverse views.
use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, hash_map::DefaultHasher},
    fmt,
    hash::{Hash, Hasher},
    iter::FusedIterator,
    sync::Arc,
};

pub const SEQUENCED_SET_ARRAY_LIMIT: usize = 16;
const END: usize = usize::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidSequencedSetSize(pub i64);
impl fmt::Display for InvalidSequencedSetSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The expectedSize ({}) must be positive and fit usize",
            self.0
        )
    }
}
impl std::error::Error for InvalidSequencedSetSize {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequencedSetStorage {
    Empty,
    Single,
    Array,
    Indexed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequencedSetBorrowed;
impl fmt::Display for SequencedSetBorrowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("sequenced set has an active iterator")
    }
}
impl std::error::Error for SequencedSetBorrowed {}

struct Link<T> {
    value: Option<Arc<T>>,
    previous: usize,
    next: usize,
}
impl<T> Clone for Link<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            previous: self.previous,
            next: self.next,
        }
    }
}
struct Indexed<T> {
    members: HashMap<Arc<T>, usize>,
    links: Vec<Link<T>>,
    free: Vec<usize>,
    first: usize,
    last: usize,
}
impl<T: Eq + Hash> Indexed<T> {
    fn new(capacity: usize) -> Self {
        Self {
            members: HashMap::with_capacity(capacity),
            links: Vec::with_capacity(capacity),
            free: vec![],
            first: END,
            last: END,
        }
    }
    fn from_values(values: impl IntoIterator<Item = Arc<T>>, capacity: usize) -> Self {
        let mut result = Self::new(capacity);
        for value in values {
            result.insert(value);
        }
        result
    }
    fn insert(&mut self, value: Arc<T>) -> bool {
        if self.members.contains_key(value.as_ref()) {
            return false;
        }
        let index = self.free.last().copied().unwrap_or(self.links.len());
        self.links.reserve(usize::from(index == self.links.len()));
        self.members.insert(value.clone(), index);
        let link = Link {
            value: Some(value),
            previous: self.last,
            next: END,
        };
        if self.free.pop().is_some() {
            self.links[index] = link;
        } else {
            self.links.push(link);
        }
        if self.last == END {
            self.first = index;
        } else {
            self.links[self.last].next = index;
        }
        self.last = index;
        true
    }
    fn remove(&mut self, value: &T) -> bool {
        let Some(index) = self.members.remove(value) else {
            return false;
        };
        let link = &self.links[index];
        let (previous, next) = (link.previous, link.next);
        if previous == END {
            self.first = next;
        } else {
            self.links[previous].next = next;
        }
        if next == END {
            self.last = previous;
        } else {
            self.links[next].previous = previous;
        }
        self.links[index].value = None;
        self.free.push(index);
        true
    }
    fn values(&self) -> Vec<Arc<T>> {
        let mut values = Vec::with_capacity(self.members.len());
        let mut index = self.first;
        while index != END {
            let link = &self.links[index];
            values.push(link.value.as_ref().unwrap().clone());
            index = link.next;
        }
        values
    }
}
impl<T: Eq + Hash> Clone for Indexed<T> {
    fn clone(&self) -> Self {
        Self {
            members: self.members.clone(),
            links: self.links.clone(),
            free: self.free.clone(),
            first: self.first,
            last: self.last,
        }
    }
}
enum Storage<T> {
    Empty,
    Single(Arc<T>),
    Array(Vec<Arc<T>>),
    Indexed(Indexed<T>),
}
impl<T> Storage<T> {
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Single(_) => 1,
            Self::Array(a) => a.len(),
            Self::Indexed(i) => i.members.len(),
        }
    }
    fn compact(mut values: Vec<Arc<T>>) -> Self {
        match values.len() {
            0 => Self::Empty,
            1 => Self::Single(values.pop().unwrap()),
            _ => Self::Array(values),
        }
    }
}
impl<T: Eq + Hash> Storage<T> {
    fn contains(&self, value: &T) -> bool {
        match self {
            Self::Empty => false,
            Self::Single(v) => v.as_ref() == value,
            Self::Array(a) => a.iter().any(|v| v.as_ref() == value),
            Self::Indexed(i) => i.members.contains_key(value),
        }
    }
    fn insert(&mut self, value: Arc<T>) -> bool {
        match self {
            Self::Empty => *self = Self::Single(value),
            Self::Single(first) => {
                if first.as_ref() == value.as_ref() {
                    return false;
                }
                *self = Self::Array(vec![first.clone(), value]);
            }
            Self::Array(a) => {
                if a.contains(&value) {
                    return false;
                }
                if a.len() == SEQUENCED_SET_ARRAY_LIMIT {
                    // Build before publishing: hashing existing members may panic.
                    let mut next = Indexed::from_values(a.iter().cloned(), a.len() + 1);
                    next.insert(value);
                    *self = Self::Indexed(next);
                } else {
                    a.push(value);
                }
            }
            Self::Indexed(i) => return i.insert(value),
        }
        true
    }
    fn remove(&mut self, value: &T) -> bool {
        match self {
            Self::Empty => return false,
            Self::Single(v) => {
                if v.as_ref() != value {
                    return false;
                }
                *self = Self::Empty;
            }
            Self::Array(a) => {
                let Some(index) = a.iter().position(|v| v.as_ref() == value) else {
                    return false;
                };
                a.remove(index);
                if a.len() <= 1 {
                    *self = Self::compact(std::mem::take(a));
                }
            }
            Self::Indexed(i) => {
                if !i.remove(value) {
                    return false;
                }
                if i.members.len() < SEQUENCED_SET_ARRAY_LIMIT {
                    *self = Self::compact(i.values());
                }
            }
        }
        true
    }
}
impl<T: Eq + Hash> Clone for Storage<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::Single(v) => Self::Single(v.clone()),
            Self::Array(a) => Self::Array(a.clone()),
            Self::Indexed(i) => Self::Indexed(i.clone()),
        }
    }
}

/// Distinct values in insertion order. Reinsertion of an existing value retains
/// its first representative and position; removal followed by insertion appends.
/// Empty/single/array tiers promote above 16 entries. Indexed storage uses a hash
/// index and linked slots with free-slot reuse; it demotes below 16 after removal.
/// Membership is linear in small tiers and expected O(1) in indexed storage.
///
/// Values use Arc handles and need not implement Clone. Cloning a set copies its
/// membership, sharing immutable payloads. Equality and hashing ignore order.
/// Hash/Eq must be stable, infallible and free of reentrant mutation. This is a
/// single-threaded interior-mutable collection; reverse views observe later edits.
/// An iterator holds a read lease: edits return SequencedSetBorrowed until it is
/// dropped. snapshot() instead returns detached handles. Bulk extension commits
/// a successful prefix if the input iterator panics or a later edit is rejected.
///
/// ```
/// use timetuck::ScalingSequencedSet;
/// let set = ScalingSequencedSet::new();
/// set.extend([3, 1, 3])?;
/// let reverse = set.reversed();
/// set.insert(2)?;
/// assert_eq!(reverse.iter().map(|v| *v).collect::<Vec<_>>(), [2, 1, 3]);
/// # Ok::<(), timetuck::SequencedSetBorrowed>(())
/// ```
pub struct ScalingSequencedSet<T> {
    data: RefCell<Storage<T>>,
}
impl<T> Default for ScalingSequencedSet<T> {
    fn default() -> Self {
        Self {
            data: RefCell::new(Storage::Empty),
        }
    }
}
impl<T> ScalingSequencedSet<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.data.borrow().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn storage(&self) -> SequencedSetStorage {
        match &*self.data.borrow() {
            Storage::Empty => SequencedSetStorage::Empty,
            Storage::Single(_) => SequencedSetStorage::Single,
            Storage::Array(_) => SequencedSetStorage::Array,
            Storage::Indexed(_) => SequencedSetStorage::Indexed,
        }
    }
    pub fn clear(&self) -> Result<(), SequencedSetBorrowed> {
        *self
            .data
            .try_borrow_mut()
            .map_err(|_| SequencedSetBorrowed)? = Storage::Empty;
        Ok(())
    }
    pub fn iter(&self) -> SequencedSetIter<'_, T> {
        let data = self.data.borrow();
        let remaining = data.len();
        let (front, back) = match &*data {
            Storage::Indexed(i) => (i.first, i.last),
            _ => (0, remaining.saturating_sub(1)),
        };
        SequencedSetIter {
            data,
            remaining,
            front,
            back,
        }
    }
    pub fn snapshot(&self) -> Vec<Arc<T>> {
        self.iter().collect()
    }
    pub fn first(&self) -> Option<Arc<T>> {
        self.iter().next()
    }
    pub fn last(&self) -> Option<Arc<T>> {
        self.iter().next_back()
    }
    pub fn reversed(&self) -> ReversedSequencedSet<'_, T> {
        ReversedSequencedSet { original: self }
    }
}
impl<T: Eq + Hash> ScalingSequencedSet<T> {
    /// Validate signed external size hints. The native with_capacity(0) remains
    /// valid; this boundary requires a strictly positive expected member count.
    pub fn try_with_expected_size(expected_size: i64) -> Result<Self, InvalidSequencedSetSize> {
        let size = usize::try_from(expected_size)
            .ok()
            .filter(|n| *n > 0)
            .ok_or(InvalidSequencedSetSize(expected_size))?;
        Ok(Self::with_capacity(size))
    }
    /// A zero hint is valid. Large hints start in indexed storage; clear releases it.
    pub fn with_capacity(expected_size: usize) -> Self {
        if expected_size > SEQUENCED_SET_ARRAY_LIMIT {
            Self {
                data: RefCell::new(Storage::Indexed(Indexed::new(expected_size))),
            }
        } else {
            Self::new()
        }
    }
    pub fn contains(&self, value: &T) -> bool {
        self.data.borrow().contains(value)
    }
    pub fn contains_all<'a>(&self, values: impl IntoIterator<Item = &'a T>) -> bool
    where
        T: 'a,
    {
        values.into_iter().all(|v| self.contains(v))
    }
    pub fn insert(&self, value: T) -> Result<bool, SequencedSetBorrowed> {
        self.insert_shared(Arc::new(value))
    }
    pub fn insert_shared(&self, value: Arc<T>) -> Result<bool, SequencedSetBorrowed> {
        Ok(self
            .data
            .try_borrow_mut()
            .map_err(|_| SequencedSetBorrowed)?
            .insert(value))
    }
    pub fn extend(
        &self,
        values: impl IntoIterator<Item = T>,
    ) -> Result<bool, SequencedSetBorrowed> {
        let mut changed = false;
        for value in values {
            changed |= self.insert(value)?;
        }
        Ok(changed)
    }
    pub fn remove(&self, value: &T) -> Result<bool, SequencedSetBorrowed> {
        Ok(self
            .data
            .try_borrow_mut()
            .map_err(|_| SequencedSetBorrowed)?
            .remove(value))
    }
}
impl<T: Eq + Hash> Clone for ScalingSequencedSet<T> {
    fn clone(&self) -> Self {
        Self {
            data: RefCell::new(self.data.borrow().clone()),
        }
    }
}
impl<T: Eq + Hash> PartialEq for ScalingSequencedSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|v| other.contains(&v))
    }
}
impl<T: Eq + Hash> Eq for ScalingSequencedSet<T> {}
impl<T: Hash> Hash for ScalingSequencedSet<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut total = 0u64;
        for value in self.iter() {
            let mut h = DefaultHasher::new();
            value.hash(&mut h);
            total = total.wrapping_add(h.finish());
        }
        self.len().hash(state);
        total.hash(state);
    }
}
impl<T: fmt::Debug> fmt::Debug for ScalingSequencedSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

pub struct SequencedSetIter<'a, T> {
    data: Ref<'a, Storage<T>>,
    front: usize,
    back: usize,
    remaining: usize,
}
impl<T> Iterator for SequencedSetIter<'_, T> {
    type Item = Arc<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let value = match &*self.data {
            Storage::Empty => unreachable!(),
            Storage::Single(v) => v.clone(),
            Storage::Array(a) => {
                let v = a[self.front].clone();
                self.front += 1;
                v
            }
            Storage::Indexed(i) => {
                let link = &i.links[self.front];
                self.front = link.next;
                link.value.as_ref().unwrap().clone()
            }
        };
        self.remaining -= 1;
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<T> DoubleEndedIterator for SequencedSetIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let value = match &*self.data {
            Storage::Empty => unreachable!(),
            Storage::Single(v) => v.clone(),
            Storage::Array(a) => {
                let v = a[self.back].clone();
                self.back = self.back.saturating_sub(1);
                v
            }
            Storage::Indexed(i) => {
                let link = &i.links[self.back];
                self.back = link.previous;
                link.value.as_ref().unwrap().clone()
            }
        };
        self.remaining -= 1;
        Some(value)
    }
}
impl<T> ExactSizeIterator for SequencedSetIter<'_, T> {}
impl<T> FusedIterator for SequencedSetIter<'_, T> {}

/// A live read-only orientation of the same set. Reversing it returns the original.
pub struct ReversedSequencedSet<'a, T> {
    original: &'a ScalingSequencedSet<T>,
}
impl<'a, T> ReversedSequencedSet<'a, T> {
    pub fn len(&self) -> usize {
        self.original.len()
    }
    pub fn is_empty(&self) -> bool {
        self.original.is_empty()
    }
    pub fn first(&self) -> Option<Arc<T>> {
        self.original.last()
    }
    pub fn last(&self) -> Option<Arc<T>> {
        self.original.first()
    }
    pub fn iter(&self) -> std::iter::Rev<SequencedSetIter<'a, T>> {
        self.original.iter().rev()
    }
    pub fn reversed(&self) -> &'a ScalingSequencedSet<T> {
        self.original
    }
}
impl<T: Eq + Hash> ReversedSequencedSet<'_, T> {
    pub fn contains(&self, value: &T) -> bool {
        self.original.contains(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_churn_reuses_slots_and_clear_releases_storage() {
        let set = ScalingSequencedSet::new();
        set.extend(0..40).unwrap();
        for i in 0..2000 {
            set.remove(&i).unwrap();
            set.insert(i + 40).unwrap();
        }
        let data = set.data.borrow();
        let Storage::Indexed(indexed) = &*data else {
            panic!("indexed")
        };
        assert_eq!(indexed.links.len(), 40);
        assert!(indexed.free.is_empty());
        drop(data);
        set.clear().unwrap();
        assert_eq!(set.storage(), SequencedSetStorage::Empty);
    }
}
