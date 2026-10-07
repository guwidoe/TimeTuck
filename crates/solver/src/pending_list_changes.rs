//! Pending list-edit brackets, with inline storage for the common single owner.
use crate::ListRecordingError;
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingListChangeStorage {
    Empty,
    Single,
    Tree,
}
/// One open action per stable entity key. Sequential edits reuse inline storage;
/// a second simultaneously open entity activates an ordered overflow map.
/// The first slot remains inline. Once activated, the map mode lasts until clear.
/// Insertion/removal never clones keys or actions, including during promotion.
/// Duplicate inserts return AlreadyOpen without replacing the original action.
#[derive(Clone, Debug)]
pub struct PendingListChanges<E, A> {
    first: Option<(E, A)>,
    overflow: BTreeMap<E, A>,
    promoted: bool,
}
impl<E, A> Default for PendingListChanges<E, A> {
    fn default() -> Self {
        Self {
            first: None,
            overflow: BTreeMap::new(),
            promoted: false,
        }
    }
}
impl<E: Ord, A> PendingListChanges<E, A> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn storage(&self) -> PendingListChangeStorage {
        if self.promoted {
            PendingListChangeStorage::Tree
        } else if self.first.is_some() {
            PendingListChangeStorage::Single
        } else {
            PendingListChangeStorage::Empty
        }
    }
    pub fn len(&self) -> usize {
        usize::from(self.first.is_some()) + self.overflow.len()
    }
    pub fn is_empty(&self) -> bool {
        self.first.is_none() && self.overflow.is_empty()
    }
    pub fn get(&self, key: &E) -> Option<&A> {
        if let Some((k, a)) = &self.first
            && k == key
        {
            Some(a)
        } else {
            self.overflow.get(key)
        }
    }
    pub fn contains_key(&self, key: &E) -> bool {
        self.get(key).is_some()
    }
    pub fn remove(&mut self, key: &E) -> Option<A> {
        if self.first.as_ref().is_some_and(|(k, _)| k == key) {
            self.first.take().map(|(_, a)| a)
        } else {
            self.overflow.remove(key)
        }
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn insert(&mut self, key: E, action: A) -> Result<(), ListRecordingError> {
        if let Some((k, _)) = &self.first {
            if *k == key {
                return Err(ListRecordingError::AlreadyOpen);
            }
            match self.overflow.entry(key) {
                Entry::Vacant(entry) => {
                    entry.insert(action);
                    self.promoted = true;
                }
                Entry::Occupied(_) => return Err(ListRecordingError::AlreadyOpen),
            }
        } else {
            if self.overflow.contains_key(&key) {
                return Err(ListRecordingError::AlreadyOpen);
            }
            self.first = Some((key, action));
        }
        Ok(())
    }
}
