//! Private storage for captured input occurrences. Generic and borrowed IDs keep
//! the original map; explicitly selected primitive IDs support unbounded numeric
//! coordinates without inferred source bounds. Fixed universes remain immutable;
//! only binding boundaries require 'static.
//! Snapshots share payload owners without user Clone/Ord. Dense journals move
//! every replaced Arc into an ordered undo log, matching map/source journals.
//! Sparse inputs need no manufactured ID keys: this private map-like boundary
//! exposes values only. Group K representatives remain in their shared payloads
//! and in the unchanged groups map; the complete-key join helper stays separate.
mod paged;
use crate::primitive_rows::{PrimitiveRows, RowsCapture};
use paged::Paged;
use std::{
    any::{Any, TypeId},
    sync::Arc,
};

type Undo<V> = Vec<(usize, Option<Arc<V>>)>;
pub(super) enum GroupInputs<I, V> {
    Tree(PrimitiveRows<I, V>),
    Dense(Box<Dense<I, V>>),
    Paged(Box<Paged<I, V>>),
}
// A trial capture is installed only over the same unchanged entry state.
// Bound captures keep final owners for touched cells, not the full universe.
pub(super) enum Capture<I, V> {
    Tree(RowsCapture<I, V>),
    Dense { len: usize, rows: Undo<V> },
    Paged(paged::Capture<V>),
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
            Self::Paged(rows) => Self::Paged(rows.clone()),
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
        Self::Tree(PrimitiveRows::new())
    }
    pub(super) fn clear(&mut self) {
        match self {
            Self::Tree(rows) => rows.clear(),
            Self::Paged(rows) => rows.clear(),
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
            Self::Paged(rows) => rows.begin(),
        }
    }
    pub(super) fn commit_journal(&mut self, token: usize) {
        match self {
            Self::Tree(rows) => rows.commit_journal(token),
            Self::Dense(rows) => rows.commit(token),
            Self::Paged(rows) => rows.commit(token),
        }
    }
    pub(super) fn rollback_journal(&mut self, token: usize) {
        match self {
            Self::Tree(rows) => rows.rollback_journal(token),
            Self::Dense(rows) => rows.rollback(token),
            Self::Paged(rows) => rows.rollback(token),
        }
    }
    pub(super) fn capture_current(&self, token: usize) -> Capture<I, V> {
        match self {
            Self::Tree(rows) => Capture::Tree(rows.capture_current(token)),
            Self::Dense(rows) => {
                assert!(
                    token < rows.frames.len(),
                    "active group input journal token"
                );
                let mut indices: Vec<_> = rows.frames[token..]
                    .iter()
                    .flat_map(|frame| frame.iter().map(|(index, _)| *index))
                    .collect();
                indices.sort_unstable();
                indices.dedup();
                Capture::Dense {
                    len: rows.values.len(),
                    rows: indices
                        .into_iter()
                        .map(|index| (index, rows.values[index].clone()))
                        .collect(),
                }
            }
            Self::Paged(rows) => Capture::Paged(rows.capture_current(token)),
        }
    }
    pub(super) fn install_capture(&mut self, capture: Capture<I, V>) {
        match capture {
            Capture::Tree(captured) => {
                let Self::Tree(rows) = self else {
                    panic!("group input capture binding");
                };
                rows.install_capture(captured);
            }
            Capture::Dense {
                len,
                rows: captured,
            } => {
                let Self::Dense(rows) = self else {
                    panic!("group input capture binding");
                };
                assert_eq!(rows.values.len(), len, "group input capture universe");
                assert!(
                    captured.iter().all(|(index, _)| *index < len),
                    "group input capture index"
                );
                for (index, value) in captured {
                    rows.replace(index, value);
                }
            }
            Capture::Paged(captured) => {
                let Self::Paged(rows) = self else {
                    panic!("group input capture binding");
                };
                rows.install_capture(captured);
            }
        }
    }
    pub(super) fn install_snapshot(&mut self, snapshot: Self) {
        match (self, snapshot) {
            (Self::Tree(rows), Self::Tree(old)) => rows.install_snapshot(old),
            (Self::Paged(rows), Self::Paged(old)) => rows.install(*old),
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
pub(super) enum PreparedChange<'a, I, V> {
    Tree(crate::primitive_rows::PreparedChange<'a, I, V>),
    Paged {
        rows: &'a mut Paged<I, V>,
        index: Option<usize>,
    },
    Dense {
        rows: &'a mut Dense<I, V>,
        index: usize,
    },
}
impl<I, V> PreparedChange<'_, I, V> {
    pub(super) fn insert(self, id: I, value: V) {
        match self {
            Self::Tree(prepared) => prepared.insert(id, value),
            Self::Paged { rows, index } => {
                let index = index.expect("bound group pair outside universe");
                drop(id);
                rows.replace(index, Some(Arc::new(value)));
            }
            Self::Dense { rows, index } => {
                assert!(
                    index < rows.values.len(),
                    "bound group input outside universe"
                );
                drop(id);
                rows.replace(index, Some(Arc::new(value)));
            }
        }
    }
    pub(super) fn remove(self) {
        match self {
            Self::Tree(prepared) => prepared.remove(),
            Self::Paged { rows, index } => {
                rows.replace(index.expect("bound group pair outside universe"), None);
            }
            Self::Dense { rows, index } => {
                assert!(
                    index < rows.values.len(),
                    "bound group input outside universe"
                );
                rows.replace(index, None);
            }
        }
    }
}
impl<I: Ord, V> GroupInputs<I, V> {
    pub(super) fn prepare_change<R>(
        &mut self,
        id: &I,
        inspect: impl FnOnce(Option<&V>) -> R,
    ) -> (PreparedChange<'_, I, V>, R) {
        match self {
            Self::Tree(rows) => {
                let (prepared, inspected) = rows.prepare_change(id, inspect);
                (PreparedChange::Tree(prepared), inspected)
            }
            Self::Paged(rows) => {
                let index = rows.index(id);
                let inspected = inspect(index.and_then(|i| rows.get(i)));
                (PreparedChange::Paged { rows, index }, inspected)
            }
            Self::Dense(rows) => {
                let index = (rows.index)(id);
                let inspected = inspect(rows.values.get(index).and_then(Option::as_deref));
                (PreparedChange::Dense { rows, index }, inspected)
            }
        }
    }
    pub(super) fn get(&self, id: &I) -> Option<&V> {
        match self {
            Self::Tree(rows) => rows.get(id),
            Self::Paged(rows) => rows.index(id).and_then(|i| rows.get(i)),
            Self::Dense(rows) => rows.values.get((rows.index)(id)).and_then(Option::as_deref),
        }
    }
    pub(super) fn insert(&mut self, id: I, value: V) {
        match self {
            Self::Tree(rows) => rows.insert(id, value),
            Self::Paged(rows) => {
                let index = rows.index(&id).expect("bound group pair outside universe");
                drop(id);
                rows.replace(index, Some(Arc::new(value)));
            }
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
            Self::Paged(rows) => rows.replace(
                rows.index(id).expect("bound group pair outside universe"),
                None,
            ),
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
    pub(super) fn bind_primitive(&mut self) -> bool {
        match self {
            Self::Tree(rows) => rows.bind_primitive(),
            _ => false,
        }
    }
    pub(super) fn bind_pair(&mut self, left: usize, right: usize) -> bool {
        let Self::Tree(rows) = self else {
            return false;
        };
        let Some(paged) = Paged::from_tree(rows.materialized(), left, right) else {
            return false;
        };
        *self = Self::Paged(Box::new(paged));
        true
    }
    pub(super) fn bind_fixed(&mut self, count: usize) -> bool {
        self.bind_fixed_with_limit(count, 256)
    }
    pub(super) fn bind_fixed_extended(&mut self, count: usize) -> bool {
        self.bind_fixed_with_limit(count, 4096)
    }
    fn bind_fixed_with_limit(&mut self, count: usize, limit: usize) -> bool {
        if TypeId::of::<I>() != TypeId::of::<usize>() || count > limit {
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
        let entries = rows.materialized().shared_entries();
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
            GroupInputs::Tree(map) => map.materialized().shared_entries(),
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

#[cfg(test)]
mod capture_tests {
    use super::*;
    struct NoClone(usize);
    #[test]
    fn dense_capture_deduplicates_final_owners_and_validates_universe_before_install() {
        let mut rows = GroupInputs::new();
        rows.insert(0usize, NoClone(1));
        rows.insert(1023, NoClone(2));
        assert!(rows.bind_fixed_extended(1024));
        let outer = rows.begin_journal();
        rows.insert(0, NoClone(3));
        let inner = rows.begin_journal();
        rows.insert(0, NoClone(4));
        rows.remove(&1023);
        rows.insert(800, NoClone(5));
        let capture = rows.capture_current(outer);
        let Capture::Dense { len, rows: changed } = &capture else {
            unreachable!()
        };
        assert_eq!(*len, 1024);
        assert_eq!(changed.len(), 3);
        assert_eq!(
            changed
                .iter()
                .map(|(i, v)| (*i, v.as_deref().map(|v| v.0)))
                .collect::<Vec<_>>(),
            [(0, Some(4)), (800, Some(5)), (1023, None)]
        );
        rows.rollback_journal(inner);
        rows.rollback_journal(outer);
        let install = rows.begin_journal();
        rows.install_capture(capture);
        assert_eq!(rows.get(&0).unwrap().0, 4);
        assert_eq!(rows.get(&800).unwrap().0, 5);
        assert!(rows.get(&1023).is_none());
        rows.rollback_journal(install);
        assert_eq!(rows.get(&0).unwrap().0, 1);
        assert_eq!(rows.get(&1023).unwrap().0, 2);
        assert!(rows.get(&800).is_none());
        let malformed = Capture::Dense {
            len: 1023,
            rows: vec![(0, Some(Arc::new(NoClone(9))))],
        };
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || rows.install_capture(malformed)
            ))
            .is_err()
        );
        assert_eq!(rows.get(&0).unwrap().0, 1);
    }
}

#[cfg(test)]
mod unbounded_input_tests {
    use super::*;
    struct NoClone(usize);

    #[test]
    fn primitive_input_upgrade_to_bound_storage_keeps_every_original_owner() {
        let mut dense = GroupInputs::new();
        dense.insert(0usize, NoClone(7));
        dense.insert(255, NoClone(9));
        assert!(dense.bind_primitive());
        let held = dense.clone();
        assert!(dense.bind_fixed(256));
        assert!(std::ptr::eq(dense.get(&0).unwrap(), held.get(&0).unwrap()));
        assert!(std::ptr::eq(
            dense.get(&255).unwrap(),
            held.get(&255).unwrap()
        ));
        let mut paired = GroupInputs::new();
        paired.insert((0usize, 0usize), NoClone(1));
        paired.insert((572, 572), NoClone(2));
        paired.insert((usize::MAX, usize::MAX), NoClone(3));
        assert!(paired.bind_primitive());
        assert!(!paired.bind_pair(573, 573));
        assert_eq!(paired.get(&(usize::MAX, usize::MAX)).unwrap().0, 3);
        paired.remove(&(usize::MAX, usize::MAX));
        let held = paired.clone();
        assert!(paired.bind_pair(573, 573));
        assert!(std::ptr::eq(
            paired.get(&(0, 0)).unwrap(),
            held.get(&(0, 0)).unwrap()
        ));
        assert!(std::ptr::eq(
            paired.get(&(572, 572)).unwrap(),
            held.get(&(572, 572)).unwrap()
        ));
    }

    #[test]
    fn unbounded_input_capture_tracks_flat_map_positions_without_coordinate_limits() {
        let mut rows = GroupInputs::new();
        rows.insert((0usize, usize::MAX), NoClone(1));
        rows.insert((usize::MAX, 0), NoClone(2));
        assert!(rows.bind_primitive());
        let original = rows.clone();
        let outer = rows.begin_journal();
        rows.remove(&(0, usize::MAX));
        rows.insert((usize::MAX, 0), NoClone(3));
        rows.insert((usize::MAX, usize::MAX), NoClone(4));
        let capture = rows.capture_current(outer);
        assert!(matches!(&capture, Capture::Tree(RowsCapture::Delta(_))));
        rows.rollback_journal(outer);
        let other = rows.begin_journal();
        rows.insert((0, 16), NoClone(99));
        rows.rollback_journal(other);
        let install = rows.begin_journal();
        rows.install_capture(capture);
        assert!(rows.get(&(0, usize::MAX)).is_none());
        assert_eq!(rows.get(&(usize::MAX, 0)).unwrap().0, 3);
        assert_eq!(rows.get(&(usize::MAX, usize::MAX)).unwrap().0, 4);
        rows.rollback_journal(install);
        assert!(std::ptr::eq(
            rows.get(&(0, usize::MAX)).unwrap(),
            original.get(&(0, usize::MAX)).unwrap()
        ));
        assert!(std::ptr::eq(
            rows.get(&(usize::MAX, 0)).unwrap(),
            original.get(&(usize::MAX, 0)).unwrap()
        ));
        assert!(rows.get(&(usize::MAX, usize::MAX)).is_none());
    }
}
