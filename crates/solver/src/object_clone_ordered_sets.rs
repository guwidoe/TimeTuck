//! Native comparator-ordered and insertion-ordered object sets.
use crate::*;
use std::{
    any::Any,
    cmp::Ordering,
    collections::{BTreeSet, HashSet},
    sync::{Arc, Mutex},
};
pub type ObjectComparator<T> = Arc<dyn Fn(&T, &T) -> Ordering + Send + Sync>;
struct SortedEntry<T> {
    object: Arc<T>,
    comparator: ObjectComparator<T>,
}
impl<T> PartialEq for SortedEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl<T> Eq for SortedEntry<T> {}
impl<T> PartialOrd for SortedEntry<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for SortedEntry<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.comparator)(&self.object, &other.object)
    }
}
/// Tree-backed set with a retained comparator allocation. Comparator equality
/// determines uniqueness. Ordering must remain stable while objects are stored,
/// as for ordinary Rust tree keys; mutating unrelated payload fields is allowed.
pub struct ObjectSortedSet<T> {
    entries: BTreeSet<SortedEntry<T>>,
    comparator: ObjectComparator<T>,
}
impl<T> ObjectSortedSet<T> {
    pub fn new(comparator: ObjectComparator<T>) -> Self {
        Self {
            entries: BTreeSet::new(),
            comparator,
        }
    }
    pub fn comparator(&self) -> &ObjectComparator<T> {
        &self.comparator
    }
    pub fn insert(&mut self, object: Arc<T>) -> bool {
        self.entries.insert(SortedEntry {
            object,
            comparator: self.comparator.clone(),
        })
    }
    pub fn contains(&self, object: &Arc<T>) -> bool {
        self.entries.contains(&SortedEntry {
            object: object.clone(),
            comparator: self.comparator.clone(),
        })
    }
    pub fn remove(&mut self, object: &Arc<T>) -> bool {
        self.entries.remove(&SortedEntry {
            object: object.clone(),
            comparator: self.comparator.clone(),
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Arc<T>> + DoubleEndedIterator {
        self.entries.iter().map(|e| &e.object)
    }
}
/// Allocation-identity membership with first-insertion iteration order. Inserts
/// and membership use a hash index; removal preserves order and is linear.
pub struct ObjectLinkedIdentitySet<T> {
    entries: Vec<Arc<T>>,
    members: HashSet<ObjectIdentity<T>>,
}
impl<T> Default for ObjectLinkedIdentitySet<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> ObjectLinkedIdentitySet<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            members: HashSet::new(),
        }
    }
    pub fn insert(&mut self, object: Arc<T>) -> bool {
        if self.members.insert(ObjectIdentity::new(object.clone())) {
            self.entries.push(object);
            true
        } else {
            false
        }
    }
    pub fn contains(&self, object: &Arc<T>) -> bool {
        self.members.contains(&ObjectIdentity::new(object.clone()))
    }
    pub fn remove(&mut self, object: &Arc<T>) -> bool {
        if self.members.remove(&ObjectIdentity::new(object.clone())) {
            self.entries.retain(|e| !Arc::ptr_eq(e, object));
            true
        } else {
            false
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Arc<T>> + DoubleEndedIterator {
        self.entries.iter()
    }
}
impl<'a> ObjectGraphCloneRegistry<'a> {
    pub fn register_sorted_set<T: Any + Send + Sync>(
        &mut self,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<ObjectSortedSet<T>>>(
            |s| {
                Ok(Mutex::new(ObjectSortedSet::new(
                    s.lock().unwrap().comparator().clone(),
                )))
            },
            move |s, c| {
                for item in s.lock().unwrap().iter() {
                    c.field(item, policy)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let source = s.lock().unwrap();
                let mut out = ObjectSortedSet::new(source.comparator().clone());
                for item in source.iter() {
                    if !out.insert(c.field(item, policy)?) {
                        return Err(ObjectGraphCloneError::Callback(
                            "clone sorted set members collided".into(),
                        ));
                    }
                }
                *t.lock().unwrap() = out;
                Ok(())
            },
        )
    }
    pub fn register_linked_identity_set<T: Any + Send + Sync>(
        &mut self,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<ObjectLinkedIdentitySet<T>>>(
            |_| Ok(Mutex::new(ObjectLinkedIdentitySet::new())),
            move |s, c| {
                for item in s.lock().unwrap().iter() {
                    c.field(item, policy)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let mut out = ObjectLinkedIdentitySet::new();
                for item in s.lock().unwrap().iter() {
                    if !out.insert(c.field(item, policy)?) {
                        return Err(ObjectGraphCloneError::Callback(
                            "clone linked set members collided".into(),
                        ));
                    }
                }
                *t.lock().unwrap() = out;
                Ok(())
            },
        )
    }
    /// Register ordinary Rust Clone for a leaf. Inner references keep the type's
    /// Clone semantics; deep edges require a separately described graph factory.
    pub fn register_cloned_leaf<T: Any + Send + Sync + Clone>(
        &mut self,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<T>(|s| Ok(s.clone()), |_, _| Ok(()), |_, _, _| Ok(()))
    }
}
