//! Sparse fixed pair identities: snapshot pages share captured payload owners.
use super::*;
use crate::SnapshotMap;
const PAGE_SIZE: usize = 64;
const MAX_SLOTS: usize = 1_000_000;
type Page<V> = [Option<Arc<V>>; PAGE_SIZE];

pub(in crate::incremental_stream) struct Capture<V> {
    left: usize,
    right: usize,
    rows: Undo<V>,
}
pub(in crate::incremental_stream) struct Paged<I, V> {
    pages: Vec<Option<Arc<Page<V>>>>,
    left: usize,
    right: usize,
    index: fn(&I, usize, usize) -> Option<usize>,
    frames: Vec<Undo<V>>,
    spare: Vec<Undo<V>>,
    pool: Pool<V>,
}
impl<I, V> Clone for Paged<I, V> {
    fn clone(&self) -> Self {
        Self {
            pages: self.pages.clone(),
            left: self.left,
            right: self.right,
            index: self.index,
            frames: Vec::new(),
            spare: Vec::new(),
            pool: self.pool.empty_clone(),
        }
    }
}
impl<I, V> Paged<I, V> {
    pub(super) fn index(&self, id: &I) -> Option<usize> {
        (self.index)(id, self.left, self.right)
    }
    pub(super) fn get(&self, index: usize) -> Option<&V> {
        self.pages
            .get(index / PAGE_SIZE)?
            .as_ref()?
            .get(index % PAGE_SIZE)?
            .as_deref()
    }
    fn set(&mut self, index: usize, value: Option<Arc<V>>) -> Option<Arc<V>> {
        let page = &mut self.pages[index / PAGE_SIZE];
        if page.is_none() && value.is_none() {
            return None;
        }
        let page = page.get_or_insert_with(|| Arc::new(std::array::from_fn(|_| None)));
        std::mem::replace(&mut Arc::make_mut(page)[index % PAGE_SIZE], value)
    }
    pub(super) fn replace(&mut self, index: usize, value: Option<Arc<V>>) -> bool {
        let old = self.set(index, value);
        let present = old.is_some();
        if let Some(frame) = self.frames.last_mut() {
            frame.push((index, old));
        } else if !std::mem::needs_drop::<V>()
            && let Some(old) = old
        {
            self.pool.retire(old);
        }
        present
    }
    pub(super) fn insert(&mut self, index: usize, value: V) {
        let owner = self.pool.take(value);
        self.replace(index, Some(owner));
    }
    pub(super) fn clear(&mut self) {
        for page in 0..self.pages.len() {
            if self.pages[page].is_some() {
                for slot in 0..PAGE_SIZE {
                    if self.pages[page].as_ref().unwrap()[slot].is_some() {
                        self.replace(page * PAGE_SIZE + slot, None);
                    }
                }
            }
        }
    }
    pub(super) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(super) fn commit(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "group pair journal order");
        let mut undo = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut undo);
            } else {
                parent.append(&mut undo);
            }
        }
        if std::mem::needs_drop::<V>() {
            // Preserve the original vector destruction path for user Drop.
            undo.clear();
        } else {
            for (_, value) in undo.drain(..) {
                if let Some(value) = value {
                    self.pool.retire(value);
                }
            }
        }
        self.spare.push(undo);
    }
    pub(super) fn rollback(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "group pair journal order");
        let mut undo = self.frames.pop().unwrap();
        for (index, value) in undo.drain(..).rev() {
            if std::mem::needs_drop::<V>() {
                self.set(index, value);
            } else if let Some(retired) = self.set(index, value) {
                self.pool.retire(retired);
            }
        }
        self.spare.push(undo);
    }
    pub(super) fn capture_current(&self, token: usize) -> Capture<V> {
        assert!(token < self.frames.len(), "active group pair journal token");
        let count = self.left * self.right;
        let mut indices: Vec<_> = self.frames[token..]
            .iter()
            .flat_map(|frame| frame.iter().map(|(index, _)| *index))
            .filter(|index| *index < count)
            .collect();
        indices.sort_unstable();
        indices.dedup();
        Capture {
            left: self.left,
            right: self.right,
            rows: indices
                .into_iter()
                .map(|index| {
                    let value = self.pages[index / PAGE_SIZE]
                        .as_ref()
                        .and_then(|page| page[index % PAGE_SIZE].clone());
                    (index, value)
                })
                .collect(),
        }
    }
    pub(super) fn install_capture(&mut self, capture: Capture<V>) {
        assert_eq!(
            (self.left, self.right),
            (capture.left, capture.right),
            "group pair capture universe"
        );
        assert!(
            capture
                .rows
                .iter()
                .all(|(index, _)| *index < self.left * self.right),
            "group pair capture index"
        );
        for (index, value) in capture.rows {
            self.replace(index, value);
        }
    }
    pub(super) fn install(&mut self, saved: Self) {
        assert_eq!(
            (self.left, self.right),
            (saved.left, saved.right),
            "group pair universe"
        );
        for (page_index, old) in saved.pages.into_iter().enumerate() {
            let same = match (&self.pages[page_index], &old) {
                (None, None) => true,
                (Some(current), Some(old)) => Arc::ptr_eq(current, old),
                _ => false,
            };
            if same {
                continue;
            }
            for slot in 0..PAGE_SIZE {
                let value = old.as_ref().and_then(|page| page[slot].clone());
                self.replace(page_index * PAGE_SIZE + slot, value);
            }
        }
    }
}
impl<I: 'static, V> Paged<I, V> {
    pub(super) fn from_tree(rows: &SnapshotMap<I, V>, left: usize, right: usize) -> Option<Self> {
        if TypeId::of::<I>() != TypeId::of::<(usize, usize)>() {
            return None;
        }
        let count = left.checked_mul(right)?;
        if count > MAX_SLOTS {
            return None;
        }
        fn index<I: 'static>(id: &I, left: usize, right: usize) -> Option<usize> {
            let &(a, b) = (id as &dyn Any)
                .downcast_ref::<(usize, usize)>()
                .expect("bound pair ID");
            (a < left && b < right).then(|| a * right + b)
        }
        let entries = rows.shared_entries();
        if entries
            .iter()
            .any(|(id, _)| index::<I>(id.as_ref(), left, right).is_none())
        {
            return None;
        }
        let mut paged = Self {
            pages: (0..count.div_ceil(PAGE_SIZE)).map(|_| None).collect(),
            left,
            right,
            index: index::<I>,
            frames: Vec::new(),
            spare: Vec::new(),
            pool: Pool::new(),
        };
        for (id, value) in entries {
            paged.set(index::<I>(id.as_ref(), left, right).unwrap(), Some(value));
        }
        Some(paged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Value(usize);
    #[test]
    fn binding_checks_type_product_rows_and_keeps_sparse_payload_owners() {
        let mut rows = GroupInputs::new();
        rows.insert((0usize, 0usize), Value(1));
        rows.insert((572, 572), Value(2));
        let before = rows.clone();
        assert!(!rows.bind_pair(572, 573));
        assert!(!rows.bind_pair(usize::MAX, 2));
        assert!(!rows.bind_pair(1001, 1000));
        assert!(rows.bind_pair(573, 573));
        assert!(std::ptr::eq(
            rows.get(&(572, 572)).unwrap(),
            before.get(&(572, 572)).unwrap()
        ));
        let GroupInputs::Paged(paged) = &rows else {
            unreachable!()
        };
        assert_eq!(paged.pages.iter().filter(|p| p.is_some()).count(), 2);
        assert!(rows.get(&(573, 0)).is_none());
        assert!(rows.get(&(0, 573)).is_none());
        assert!(catch_unwind(AssertUnwindSafe(|| rows.insert((0, 573), Value(3)))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| rows.remove(&(573, 0)))).is_err());
        assert_eq!(rows.get(&(0, 0)).unwrap().0, 1);
        let mut wrong = GroupInputs::<usize, Value>::new();
        assert!(!wrong.bind_pair(1, 1));
        let mut empty = GroupInputs::<(usize, usize), Value>::new();
        assert!(empty.bind_pair(0, 0));
        assert!(empty.get(&(0, 0)).is_none());
    }
    #[test]
    fn pages_detach_from_full_snapshots_and_each_nested_undo_reuses_captured_owner() {
        let mut rows = GroupInputs::new();
        rows.insert((0usize, 0usize), Value(1));
        rows.insert((320, 572), Value(2));
        assert!(rows.bind_pair(573, 573));
        let original = rows.clone();
        let outer = rows.begin_journal();
        rows.remove(&(0, 0));
        rows.insert((1, 572), Value(3));
        let middle = rows.clone();
        let inner = rows.begin_journal();
        rows.insert((320, 572), Value(4));
        rows.insert((572, 572), Value(5));
        rows.commit_journal(inner);
        rows.install_snapshot(middle.clone());
        assert_eq!(rows.get(&(320, 572)).unwrap().0, 2);
        assert!(rows.get(&(572, 572)).is_none());
        rows.clear();
        rows.rollback_journal(outer);
        assert!(std::ptr::eq(
            rows.get(&(0, 0)).unwrap(),
            original.get(&(0, 0)).unwrap()
        ));
        assert!(std::ptr::eq(
            rows.get(&(320, 572)).unwrap(),
            original.get(&(320, 572)).unwrap()
        ));
        assert!(rows.get(&(1, 572)).is_none());
        assert!(middle.get(&(0, 0)).is_none());
        assert_eq!(middle.get(&(1, 572)).unwrap().0, 3);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut transaction = rows.journal_transaction();
                transaction.remove(&(320, 572));
                transaction.insert((572, 0), Value(9));
                panic!("injected failure");
            }))
            .is_err()
        );
        assert!(std::ptr::eq(
            rows.get(&(320, 572)).unwrap(),
            original.get(&(320, 572)).unwrap()
        ));
        assert!(rows.get(&(572, 0)).is_none());
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    struct NoClone(usize);
    #[test]
    fn sparse_pair_capture_owns_only_touched_cells_and_restores_last_partial_page() {
        let mut rows = GroupInputs::new();
        rows.insert((0usize, 0usize), NoClone(1));
        rows.insert((572, 572), NoClone(2));
        assert!(rows.bind_pair(573, 573));
        let outer = rows.begin_journal();
        rows.remove(&(0, 0));
        rows.insert((1, 571), NoClone(3));
        let saved = rows.clone();
        rows.insert((572, 572), NoClone(4));
        // Full installation logs padding of the final page; delta capture must
        // retain only valid Cartesian cells from that otherwise harmless undo.
        rows.install_snapshot(saved);
        let capture = rows.capture_current(outer);
        let super::super::Capture::Paged(capture) = capture else {
            unreachable!()
        };
        assert!(capture.rows.len() < PAGE_SIZE);
        assert!(capture.rows.iter().any(|(index, _)| *index == 0));
        assert!(
            capture
                .rows
                .iter()
                .any(|(index, _)| *index == 573 * 573 - 1)
        );
        assert!(capture.rows.iter().all(|(index, _)| *index < 573 * 573));
        rows.rollback_journal(outer);
        let install = rows.begin_journal();
        rows.install_capture(super::super::Capture::Paged(capture));
        assert!(rows.get(&(0, 0)).is_none());
        assert_eq!(rows.get(&(1, 571)).unwrap().0, 3);
        assert_eq!(rows.get(&(572, 572)).unwrap().0, 2);
        rows.rollback_journal(install);
        assert_eq!(rows.get(&(0, 0)).unwrap().0, 1);
        assert!(rows.get(&(1, 571)).is_none());
        assert_eq!(rows.get(&(572, 572)).unwrap().0, 2);
        let bad = Capture {
            left: 572,
            right: 573,
            rows: vec![(0, Some(Arc::new(NoClone(9))))],
        };
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || rows.install_capture(super::super::Capture::Paged(bad))
            ))
            .is_err()
        );
        assert_eq!(rows.get(&(0, 0)).unwrap().0, 1);
    }
}

#[cfg(test)]
mod pool_tests;
