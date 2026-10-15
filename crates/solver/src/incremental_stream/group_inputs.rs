//! Private bounded storage for identity-preserving planning inputs. Generic
//! sources retain the original map. Bound universes never change, may be sparse,
//! and contain at most256 usize IDs; only the binding boundary needs 'static.
//! Snapshots share payload owners without user Clone/Ord. Dense journals move
//! every replaced Arc into an ordered undo log, matching map/source journals.
//! Sparse inputs need no manufactured ID keys: this private map-like boundary
//! exposes values only. Group K representatives remain in their shared payloads
//! and in the unchanged groups map; the complete-key join helper stays separate.
use crate::SnapshotMap;
use std::{
    any::{Any, TypeId},
    sync::Arc,
};

type Undo<V> = Vec<(usize, Option<Arc<V>>)>;
pub(super) enum GroupInputs<I, V> {
    Tree(SnapshotMap<I, V>),
    Dense(Box<Dense<I, V>>),
}
pub(super) struct Dense<I, V> {
    values: Vec<Option<Arc<V>>>,
    index: fn(&I) -> usize,
    frames: Vec<Undo<V>>,
    spare: Vec<Undo<V>>,
}
impl<I, V> Clone for Dense<I, V> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            index: self.index,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
}
impl<I, V> Clone for GroupInputs<I, V> {
    fn clone(&self) -> Self {
        match self {
            Self::Tree(rows) => Self::Tree(rows.clone()),
            Self::Dense(rows) => Self::Dense(rows.clone()),
        }
    }
}
impl<I, V> Dense<I, V> {
    fn replace(&mut self, index: usize, value: Option<Arc<V>>) -> bool {
        let old = std::mem::replace(&mut self.values[index], value);
        let present = old.is_some();
        if let Some(frame) = self.frames.last_mut() {
            frame.push((index, old));
        }
        present
    }
    fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    fn commit(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "group input journal order");
        let mut undo = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut undo);
            } else {
                parent.append(&mut undo);
            }
        }
        undo.clear();
        self.spare.push(undo);
    }
    fn rollback(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "group input journal order");
        let mut undo = self.frames.pop().unwrap();
        for (index, value) in undo.drain(..).rev() {
            self.values[index] = value;
        }
        self.spare.push(undo);
    }
}
impl<I, V> GroupInputs<I, V> {
    pub(super) fn new() -> Self {
        Self::Tree(SnapshotMap::new())
    }
    pub(super) fn clear(&mut self) {
        match self {
            Self::Tree(rows) => rows.clear(),
            Self::Dense(rows) => {
                for index in 0..rows.values.len() {
                    rows.replace(index, None);
                }
            }
        }
    }
    pub(super) fn begin_journal(&mut self) -> usize {
        match self {
            Self::Tree(rows) => rows.begin_journal(),
            Self::Dense(rows) => rows.begin(),
        }
    }
    pub(super) fn commit_journal(&mut self, token: usize) {
        match self {
            Self::Tree(rows) => rows.commit_journal(token),
            Self::Dense(rows) => rows.commit(token),
        }
    }
    pub(super) fn rollback_journal(&mut self, token: usize) {
        match self {
            Self::Tree(rows) => rows.rollback_journal(token),
            Self::Dense(rows) => rows.rollback(token),
        }
    }
    pub(super) fn install_snapshot(&mut self, snapshot: Self) {
        match (self, snapshot) {
            (Self::Tree(rows), Self::Tree(old)) => rows.install_snapshot(old),
            (Self::Dense(rows), Self::Dense(old)) => {
                assert_eq!(rows.values.len(), old.values.len(), "group input universe");
                for (index, value) in old.values.into_iter().enumerate() {
                    rows.replace(index, value);
                }
            }
            _ => unreachable!("group input binding is immutable after initialization"),
        }
    }
    pub(super) fn journal_transaction(&mut self) -> Transaction<'_, I, V> {
        let token = self.begin_journal();
        Transaction {
            rows: self,
            token: Some(token),
        }
    }
}
impl<I: Ord, V> GroupInputs<I, V> {
    pub(super) fn get(&self, id: &I) -> Option<&V> {
        match self {
            Self::Tree(rows) => rows.get(id),
            Self::Dense(rows) => rows.values.get((rows.index)(id)).and_then(Option::as_deref),
        }
    }
    pub(super) fn insert(&mut self, id: I, value: V) {
        match self {
            Self::Tree(rows) => rows.insert(id, value),
            Self::Dense(rows) => {
                let index = (rows.index)(&id);
                assert!(
                    index < rows.values.len(),
                    "bound group input outside universe"
                );
                drop(id);
                rows.replace(index, Some(Arc::new(value)));
            }
        }
    }
    pub(super) fn remove(&mut self, id: &I) -> bool {
        match self {
            Self::Tree(rows) => rows.remove(id),
            Self::Dense(rows) => {
                let index = (rows.index)(id);
                assert!(
                    index < rows.values.len(),
                    "bound group input outside universe"
                );
                rows.replace(index, None)
            }
        }
    }
}
impl<I: 'static, V> GroupInputs<I, V> {
    pub(super) fn bind_fixed(&mut self, count: usize) -> bool {
        if TypeId::of::<I>() != TypeId::of::<usize>() || count > 256 {
            return false;
        }
        let Self::Tree(rows) = self else {
            return false;
        };
        fn index<I: 'static>(id: &I) -> usize {
            *(id as &dyn Any)
                .downcast_ref::<usize>()
                .expect("bound usize ID")
        }
        let entries = rows.shared_entries();
        if entries.iter().any(|(id, _)| index(id.as_ref()) >= count) {
            return false;
        }
        let mut values: Vec<_> = (0..count).map(|_| None).collect();
        for (id, value) in entries {
            values[index(id.as_ref())] = Some(value);
        }
        *self = Self::Dense(Box::new(Dense {
            values,
            index: index::<I>,
            frames: Vec::new(),
            spare: Vec::new(),
        }));
        true
    }
}
pub(super) struct Transaction<'a, I, V> {
    rows: &'a mut GroupInputs<I, V>,
    token: Option<usize>,
}
impl<I: Ord, V> Transaction<'_, I, V> {
    pub(super) fn insert(&mut self, id: I, value: V) {
        self.rows.insert(id, value);
    }
    pub(super) fn remove(&mut self, id: &I) -> bool {
        self.rows.remove(id)
    }
}
impl<I, V> Transaction<'_, I, V> {
    pub(super) fn commit(mut self) {
        self.rows.commit_journal(self.token.take().unwrap());
    }
}
impl<I, V> Drop for Transaction<'_, I, V> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.rows.rollback_journal(token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct NoClone(usize);
    #[test]
    fn sparse_upgrade_retains_owners_and_checks_zero_and_universe_bounds() {
        let mut rows = GroupInputs::new();
        rows.insert(0usize, NoClone(7));
        rows.insert(255, NoClone(9));
        let owners = match &rows {
            GroupInputs::Tree(map) => map.shared_entries(),
            _ => unreachable!(),
        };
        assert!(rows.bind_fixed(256));
        assert!(rows.get(&1).is_none());
        assert!(std::ptr::eq(rows.get(&0).unwrap(), owners[0].1.as_ref()));
        let held = rows.clone();
        rows.insert(0, NoClone(11));
        assert_eq!(held.get(&0).unwrap().0, 7);
        assert_eq!(rows.get(&0).unwrap().0, 11);
        assert!(catch_unwind(AssertUnwindSafe(|| rows.insert(256, NoClone(13)))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| rows.remove(&256))).is_err());
        assert_eq!(rows.get(&0).unwrap().0, 11);
        assert_eq!(rows.get(&255).unwrap().0, 9);
        rows.install_snapshot(held);
        assert!(std::ptr::eq(rows.get(&0).unwrap(), owners[0].1.as_ref()));
        let mut zero = GroupInputs::<usize, NoClone>::new();
        assert!(zero.bind_fixed(0));
        assert!(zero.get(&0).is_none());
        let mut large = GroupInputs::new();
        large.insert(256usize, NoClone(1));
        assert!(!large.bind_fixed(256));
        assert!(!large.bind_fixed(257));
        assert_eq!(large.get(&256).unwrap().0, 1);
        let mut custom = GroupInputs::new();
        custom.insert("x", NoClone(1));
        assert!(!custom.bind_fixed(1));
    }
    #[test]
    fn dense_nested_commits_full_installs_and_transaction_unwind_restore_each_entry() {
        let mut rows = GroupInputs::new();
        rows.insert(0usize, NoClone(1));
        rows.insert(255, NoClone(2));
        assert!(rows.bind_fixed(256));
        let original = rows.clone();
        let outer = rows.begin_journal();
        rows.insert(0, NoClone(10));
        let saved = rows.clone();
        let inner = rows.begin_journal();
        rows.insert(0, NoClone(20));
        rows.insert(255, NoClone(30));
        rows.commit_journal(inner);
        rows.install_snapshot(saved.clone());
        assert_eq!(rows.get(&0).unwrap().0, 10);
        assert_eq!(rows.get(&255).unwrap().0, 2);
        rows.clear();
        rows.insert(1, NoClone(40));
        rows.rollback_journal(outer);
        assert_eq!(rows.get(&0).unwrap().0, 1);
        assert_eq!(rows.get(&255).unwrap().0, 2);
        assert!(rows.get(&1).is_none());
        assert!(std::ptr::eq(
            rows.get(&0).unwrap(),
            original.get(&0).unwrap()
        ));
        assert_eq!(saved.get(&0).unwrap().0, 10);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tx = rows.journal_transaction();
                tx.insert(0, NoClone(90));
                tx.remove(&255);
                panic!("injected edit");
            }))
            .is_err()
        );
        assert!(std::ptr::eq(
            rows.get(&0).unwrap(),
            original.get(&0).unwrap()
        ));
        assert_eq!(rows.get(&255).unwrap().0, 2);
        let mut tx = rows.journal_transaction();
        tx.insert(1, NoClone(3));
        tx.commit();
        assert_eq!(rows.get(&1).unwrap().0, 3);
    }
}
