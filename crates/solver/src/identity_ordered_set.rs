//! Insertion-ordered membership for shared domain objects, independent of payload equality.
use std::{collections::HashMap, fmt, iter::FusedIterator, sync::Arc};

struct Entry<T> {
    object: Arc<T>,
    previous: Option<usize>,
    next: Option<usize>,
}

/// A set of shared objects compared by allocation identity.
///
/// Equal-valued objects remain distinct; changing an object's interior state does
/// not change membership. The set owns each member's `Arc`, keeping addresses
/// alive until removal. Reinserted objects join the end of the order.
///
/// Membership, insertion and removal take expected constant time. Iteration is
/// borrowed and performs no allocations or reference-count updates. Storage is
/// bounded by the peak simultaneous membership; removed slots are reused.
/// No `Clone`, `Eq` or `Hash` bound is imposed on the payload.
///
/// ```
/// use std::sync::{Arc, atomic::{AtomicI32, Ordering}};
/// use timetuck::IdentityOrderedSet;
/// let first = Arc::new(AtomicI32::new(7));
/// let second = Arc::new(AtomicI32::new(7));
/// let mut objects = IdentityOrderedSet::new();
/// assert!(objects.insert(first.clone()));
/// assert!(objects.insert(second.clone()));
/// assert!(!objects.insert(first.clone()));
/// first.store(9, Ordering::Relaxed);
/// assert!(objects.contains(&first));
/// assert_eq!(objects.iter().map(|v| v.load(Ordering::Relaxed)).collect::<Vec<_>>(), [9, 7]);
/// ```
pub struct IdentityOrderedSet<T> {
    members: HashMap<usize, usize>,
    entries: Vec<Option<Entry<T>>>,
    free: Vec<usize>,
    first: Option<usize>,
    last: Option<usize>,
}

/// Runtime rejection of bulk retention on an identity-ordered collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedIdentityRetention;
impl fmt::Display for UnsupportedIdentityRetention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("retain_all is unsupported for IdentityOrderedSet")
    }
}
impl std::error::Error for UnsupportedIdentityRetention {}

/// A snapshot placed in the caller's reusable buffer, or allocated when too small.
///
/// A borrowed result contains the entire supplied buffer. Its first slot after
/// the members is cleared as an end marker; any later slots remain untouched.
pub enum IdentitySnapshot<'a, T> {
    Borrowed(&'a mut [Option<Arc<T>>]),
    Owned(Vec<Option<Arc<T>>>),
}
impl<T> IdentitySnapshot<'_, T> {
    pub fn as_slice(&self) -> &[Option<Arc<T>>] {
        match self {
            Self::Borrowed(buffer) => buffer,
            Self::Owned(buffer) => buffer,
        }
    }
}

impl<T> IdentityOrderedSet<T> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            members: HashMap::with_capacity(capacity),
            entries: Vec::with_capacity(capacity),
            free: Vec::new(),
            first: None,
            last: None,
        }
    }
    fn key(object: &Arc<T>) -> usize {
        Arc::as_ptr(object) as usize
    }
    pub fn len(&self) -> usize {
        self.members.len()
    }
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
    pub fn contains(&self, object: &Arc<T>) -> bool {
        self.members.contains_key(&Self::key(object))
    }
    pub fn contains_all<'a>(&self, objects: impl IntoIterator<Item = &'a Arc<T>>) -> bool
    where
        T: 'a,
    {
        objects.into_iter().all(|object| self.contains(object))
    }
    /// Inserts an existing shared allocation without cloning its payload.
    pub fn insert(&mut self, object: Arc<T>) -> bool {
        let key = Self::key(&object);
        if self.members.contains_key(&key) {
            return false;
        }
        let index = self.free.pop().unwrap_or(self.entries.len());
        let entry = Some(Entry {
            object,
            previous: self.last,
            next: None,
        });
        if index == self.entries.len() {
            self.entries.push(entry);
        } else {
            self.entries[index] = entry;
        }
        if let Some(last) = self.last {
            self.entries[last].as_mut().unwrap().next = Some(index);
        } else {
            self.first = Some(index);
        }
        self.last = Some(index);
        self.members.insert(key, index);
        true
    }
    /// Returns whether at least one new identity was inserted.
    pub fn extend(&mut self, objects: impl IntoIterator<Item = Arc<T>>) -> bool {
        let mut changed = false;
        for object in objects {
            changed |= self.insert(object);
        }
        changed
    }
    pub fn remove(&mut self, object: &Arc<T>) -> bool {
        let Some(index) = self.members.remove(&Self::key(object)) else {
            return false;
        };
        let entry = self.entries[index].take().unwrap();
        if let Some(previous) = entry.previous {
            self.entries[previous].as_mut().unwrap().next = entry.next;
        } else {
            self.first = entry.next;
        }
        if let Some(next) = entry.next {
            self.entries[next].as_mut().unwrap().previous = entry.previous;
        } else {
            self.last = entry.previous;
        }
        self.free.push(index);
        true
    }
    pub fn remove_all<'a>(&mut self, objects: impl IntoIterator<Item = &'a Arc<T>>) -> bool
    where
        T: 'a,
    {
        let mut changed = false;
        for object in objects {
            changed |= self.remove(object);
        }
        changed
    }
    /// Rejects bulk retention without traversing the supplied iterator or
    /// changing members. Individual removals and `remove_all` remain available.
    pub fn retain_all<'a>(
        &mut self,
        _objects: impl IntoIterator<Item = &'a Arc<T>>,
    ) -> Result<bool, UnsupportedIdentityRetention>
    where
        T: 'a,
    {
        Err(UnsupportedIdentityRetention)
    }
    /// Releases members while keeping the allocated capacity for reuse.
    pub fn clear(&mut self) {
        self.members.clear();
        self.free.clear();
        self.first = None;
        self.last = None;
        self.entries.clear();
    }
    pub fn first(&self) -> Option<&Arc<T>> {
        self.first
            .map(|i| &self.entries[i].as_ref().unwrap().object)
    }
    pub fn last(&self) -> Option<&Arc<T>> {
        self.last.map(|i| &self.entries[i].as_ref().unwrap().object)
    }
    pub fn iter(&self) -> IdentityOrderedIter<'_, T> {
        IdentityOrderedIter {
            entries: &self.entries,
            front: self.first,
            back: self.last,
            remaining: self.len(),
        }
    }
    pub fn snapshot(&self) -> Vec<Arc<T>> {
        self.iter().cloned().collect()
    }
    /// Writes members in order without allocating when the buffer fits.
    /// A short buffer is left untouched and an owned snapshot is returned.
    pub fn snapshot_into<'a>(&self, buffer: &'a mut [Option<Arc<T>>]) -> IdentitySnapshot<'a, T> {
        if buffer.len() < self.len() {
            return IdentitySnapshot::Owned(self.iter().map(|v| Some(v.clone())).collect());
        }
        for (slot, object) in buffer.iter_mut().zip(self.iter()) {
            *slot = Some(object.clone());
        }
        if let Some(terminator) = buffer.get_mut(self.len()) {
            *terminator = None;
        }
        IdentitySnapshot::Borrowed(buffer)
    }
}
impl<T> Default for IdentityOrderedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> Clone for IdentityOrderedSet<T> {
    fn clone(&self) -> Self {
        self.iter().cloned().collect()
    }
}
impl<T> FromIterator<Arc<T>> for IdentityOrderedSet<T> {
    fn from_iter<I: IntoIterator<Item = Arc<T>>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}
impl<T: fmt::Debug> fmt::Debug for IdentityOrderedSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
impl<'a, T> IntoIterator for &'a IdentityOrderedSet<T> {
    type Item = &'a Arc<T>;
    type IntoIter = IdentityOrderedIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Borrowed insertion-order iteration, with exact size and reverse traversal.
pub struct IdentityOrderedIter<'a, T> {
    entries: &'a [Option<Entry<T>>],
    front: Option<usize>,
    back: Option<usize>,
    remaining: usize,
}
impl<'a, T> Iterator for IdentityOrderedIter<'a, T> {
    type Item = &'a Arc<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let entry = self.entries[self.front.unwrap()].as_ref().unwrap();
        self.front = entry.next;
        self.remaining -= 1;
        Some(&entry.object)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<T> DoubleEndedIterator for IdentityOrderedIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let entry = self.entries[self.back.unwrap()].as_ref().unwrap();
        self.back = entry.previous;
        self.remaining -= 1;
        Some(&entry.object)
    }
}
impl<T> ExactSizeIterator for IdentityOrderedIter<'_, T> {}
impl<T> FusedIterator for IdentityOrderedIter<'_, T> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_lived_churn_reuses_storage_at_peak_membership() {
        let mut set = IdentityOrderedSet::new();
        let mut objects: Vec<_> = (0..64).map(Arc::new).collect();
        set.extend(objects.iter().cloned());
        for n in 0..10_000 {
            let index = n % objects.len();
            assert!(set.remove(&objects[index]));
            objects[index] = Arc::new(n + 64);
            assert!(set.insert(objects[index].clone()));
            assert_eq!(set.entries.len(), 64);
            assert!(set.free.is_empty());
        }
    }
}
