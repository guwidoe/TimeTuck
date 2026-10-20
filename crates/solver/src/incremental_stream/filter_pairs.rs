//! Private paged storage for a validated Cartesian universe of tuple IDs.
//! Per-slot journals retain replaced payloads and removed key representatives
//! for rollback. Full saved states share page owners; ordinary trial updates
//! copy no page table. No user key/value Clone is needed for snapshots/restores.
use crate::SnapshotMap;
use std::{
    any::{Any, TypeId},
    sync::Arc,
};

const PAGE: usize = 64;
const MAX_SLOTS: usize = 1_048_576;
struct Entry<I, V> {
    key: Arc<I>,
    value: Arc<V>,
}
impl<I, V> Clone for Entry<I, V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}
struct Page<I, V>([Option<Entry<I, V>>; PAGE]);
impl<I, V> Clone for Page<I, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
type Root<I, V> = Arc<Vec<Option<Arc<Page<I, V>>>>>;
enum Change<I, V> {
    Value(usize, Arc<V>),
    Entry(usize, Option<Entry<I, V>>),
}
type Undo<I, V> = Vec<Change<I, V>>;
// A current candidate relative to one still-active journal's entry state.
// No page/table owner is retained, so subsequent candidates keep exclusive pages.
pub(super) struct Capture<I, V> {
    left: usize,
    right: usize,
    entries: Vec<(usize, Option<Entry<I, V>>)>,
}
pub(super) struct PairRows<I, V> {
    root: Root<I, V>,
    left: usize,
    right: usize,
    index: fn(&I) -> (usize, usize),
    frames: Vec<Undo<I, V>>,
    spare: Vec<Undo<I, V>>,
}
impl<I, V> Clone for PairRows<I, V> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            left: self.left,
            right: self.right,
            index: self.index,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
}
fn entry<I, V>(root: &Root<I, V>, slot: usize) -> Option<&Entry<I, V>> {
    root.get(slot / PAGE)?.as_ref()?.0[slot % PAGE].as_ref()
}
fn page_mut<I, V>(root: &mut Root<I, V>, slot: usize) -> &mut Page<I, V> {
    let page = Arc::make_mut(root)[slot / PAGE]
        .get_or_insert_with(|| Arc::new(Page(std::array::from_fn(|_| None))));
    Arc::make_mut(page)
}
fn replace<I, V>(root: &mut Root<I, V>, slot: usize, value: Arc<V>) {
    page_mut(root, slot).0[slot % PAGE]
        .as_mut()
        .expect("existing pair row")
        .value = value;
}
impl<I: 'static, V> PairRows<I, V> {
    pub(super) fn bind(rows: &SnapshotMap<I, V>, left: usize, right: usize) -> Option<Self> {
        if TypeId::of::<I>() != TypeId::of::<(usize, usize)>() {
            return None;
        }
        let slots = left.checked_mul(right)?;
        if slots > MAX_SLOTS {
            return None;
        }
        fn index<I: 'static>(id: &I) -> (usize, usize) {
            *(id as &dyn Any)
                .downcast_ref::<(usize, usize)>()
                .expect("bound pair ID")
        }
        // Validate the complete sparse state before acquiring any row owners.
        if rows.iter().any(|(id, _)| {
            let (a, b) = index(id);
            a >= left || b >= right
        }) {
            return None;
        }
        let mut bound = Self {
            root: Arc::new(vec![None; slots.div_ceil(PAGE)]),
            left,
            right,
            index: index::<I>,
            frames: Vec::new(),
            spare: Vec::new(),
        };
        for (key, value) in rows.shared_entries() {
            let slot = bound.position(&key).expect("validated pair ID");
            page_mut(&mut bound.root, slot).0[slot % PAGE] = Some(Entry { key, value });
        }
        Some(bound)
    }
}
impl<I, V> PairRows<I, V> {
    fn position(&self, id: &I) -> Option<usize> {
        let (a, b) = (self.index)(id);
        (a < self.left && b < self.right).then(|| a * self.right + b)
    }
    pub(super) fn get(&self, id: &I) -> Option<&V> {
        entry(&self.root, self.position(id)?).map(|row| row.value.as_ref())
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
        self.root
            .iter()
            .filter_map(Option::as_deref)
            .flat_map(|page| page.0.iter())
            .filter_map(Option::as_ref)
            .map(|row| (row.key.as_ref(), row.value.as_ref()))
    }
    pub(super) fn insert(&mut self, id: I, value: V) {
        let slot = self.position(&id).expect("validated bound pair row ID");
        if entry(&self.root, slot).is_some() {
            // Equal incoming representatives drop before replacing the payload.
            drop(id);
            let value = Arc::new(value);
            let old = std::mem::replace(
                &mut page_mut(&mut self.root, slot).0[slot % PAGE]
                    .as_mut()
                    .unwrap()
                    .value,
                value,
            );
            if let Some(undo) = self.frames.last_mut() {
                undo.push(Change::Value(slot, old));
            }
        } else {
            page_mut(&mut self.root, slot).0[slot % PAGE] = Some(Entry {
                key: Arc::new(id),
                value: Arc::new(value),
            });
            if let Some(undo) = self.frames.last_mut() {
                undo.push(Change::Entry(slot, None));
            }
        }
    }
    pub(super) fn remove(&mut self, id: &I) {
        let Some(slot) = self.position(id) else {
            return;
        };
        if entry(&self.root, slot).is_some() {
            let old = page_mut(&mut self.root, slot).0[slot % PAGE].take();
            if let Some(undo) = self.frames.last_mut() {
                undo.push(Change::Entry(slot, old));
            }
        }
    }
    pub(super) fn clear(&mut self) {
        if self.frames.is_empty() {
            self.root = Arc::new(vec![None; self.root.len()]);
        } else {
            for slot in 0..self.left * self.right {
                if entry(&self.root, slot).is_some() {
                    let old = page_mut(&mut self.root, slot).0[slot % PAGE].take();
                    self.frames
                        .last_mut()
                        .unwrap()
                        .push(Change::Entry(slot, old));
                }
            }
        }
    }
    pub(super) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(super) fn capture_current(&self, token: usize) -> Capture<I, V> {
        assert!(token < self.frames.len(), "active pair row capture token");
        let mut slots: Vec<_> = self.frames[token..]
            .iter()
            .flatten()
            .map(|change| match change {
                Change::Value(slot, _) | Change::Entry(slot, _) => *slot,
            })
            .collect();
        slots.sort_unstable();
        slots.dedup();
        Capture {
            left: self.left,
            right: self.right,
            entries: slots
                .into_iter()
                .map(|slot| (slot, entry(&self.root, slot).cloned()))
                .collect(),
        }
    }
    // The graph restores the capture's journal-entry base before installing it.
    // Installing inside another trial must still log full original entries,
    // including key representatives changed by remove/reinsert sequences.
    pub(super) fn install_capture(&mut self, saved: Capture<I, V>) {
        assert_eq!(
            (self.left, self.right),
            (saved.left, saved.right),
            "pair row capture universe"
        );
        for (slot, next) in saved.entries {
            let old = entry(&self.root, slot);
            if old.is_none() && next.is_none() {
                continue;
            }
            if old.zip(next.as_ref()).is_some_and(|(old, new)| {
                Arc::ptr_eq(&old.key, &new.key) && Arc::ptr_eq(&old.value, &new.value)
            }) {
                continue;
            }
            let old = std::mem::replace(&mut page_mut(&mut self.root, slot).0[slot % PAGE], next);
            if let Some(frame) = self.frames.last_mut() {
                frame.push(Change::Entry(slot, old));
            }
        }
    }
    pub(super) fn rollback(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "pair row journal order");
        let mut undo = self.frames.pop().unwrap();
        for change in undo.drain(..).rev() {
            match change {
                Change::Value(slot, value) => replace(&mut self.root, slot, value),
                Change::Entry(slot, value) => page_mut(&mut self.root, slot).0[slot % PAGE] = value,
            }
        }
        self.spare.push(undo);
    }
    pub(super) fn commit(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "pair row journal order");
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
    pub(super) fn install(&mut self, saved: Self) {
        assert_eq!(
            (self.left, self.right),
            (saved.left, saved.right),
            "pair row universe"
        );
        if self.frames.is_empty() {
            self.root = saved.root;
            return;
        }
        // Installing a saved state inside a trial must preserve that trial's
        // entire previous state, including keys that are absent in the save.
        for slot in 0..self.left * self.right {
            let old = entry(&self.root, slot);
            let new = entry(&saved.root, slot);
            if old.is_none() && new.is_none() {
                continue;
            }
            if old.zip(new).is_some_and(|(a, b)| {
                Arc::ptr_eq(&a.key, &b.key) && Arc::ptr_eq(&a.value, &b.value)
            }) {
                continue;
            }
            let next = new.cloned();
            let old = std::mem::replace(&mut page_mut(&mut self.root, slot).0[slot % PAGE], next);
            self.frames
                .last_mut()
                .unwrap()
                .push(Change::Entry(slot, old));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn contents(rows: &PairRows<(usize, usize), i64>) -> BTreeMap<(usize, usize), i64> {
        rows.iter().map(|(id, value)| (*id, *value)).collect()
    }

    #[test]
    fn sparse_pairs_cross_pages_and_restore_repeated_replacements_and_full_saves() {
        let original = SnapshotMap::from_sorted(
            [((0, 0), 1), ((0, 63), 2), ((1, 0), 3), ((572, 572), 4)].into_iter(),
        );
        let want = original
            .iter()
            .map(|(id, v)| (*id, *v))
            .collect::<BTreeMap<_, _>>();
        let mut rows = PairRows::bind(&original, 573, 573).unwrap();
        let held = rows.clone();
        let outer = rows.begin();
        rows.insert((0, 0), 10);
        rows.insert((0, 0), 11);
        rows.remove(&(0, 63));
        rows.insert((1, 63), 12);
        let saved = rows.clone();
        let inner = rows.begin();
        rows.remove(&(0, 0));
        rows.insert((0, 0), 13);
        rows.insert((572, 572), 14);
        rows.commit(inner);
        rows.install(saved.clone());
        assert_eq!(contents(&rows), contents(&saved));
        rows.clear();
        assert_eq!(rows.iter().count(), 0);
        rows.rollback(outer);
        assert_eq!(contents(&rows), want);
        assert_eq!(contents(&held), want);
        assert_eq!(contents(&saved)[&(0, 0)], 11);
        let (id, value) = rows.iter().next().unwrap();
        let (old_id, old_value) = original.iter().next().unwrap();
        assert!(std::ptr::eq(id, old_id));
        assert!(std::ptr::eq(value, old_value));
    }

    #[test]
    fn binding_rejects_invalid_domains_and_uses_no_user_clone_for_saved_states() {
        struct NoClone;
        let empty = SnapshotMap::<(usize, usize), NoClone>::new();
        let mut rows = PairRows::bind(&empty, 0, 0).unwrap();
        let saved = rows.clone();
        rows.clear();
        rows.install(saved);
        assert!(rows.iter().next().is_none());
        assert!(PairRows::bind(&empty, usize::MAX, 2).is_none());
        assert!(PairRows::bind(&empty, 1025, 1025).is_none());
        let outside = SnapshotMap::from_sorted([((1usize, 2usize), NoClone)].into_iter());
        assert!(PairRows::bind(&outside, 1, 3).is_none());
        assert!(PairRows::bind(&outside, 2, 2).is_none());
        let wrong = SnapshotMap::<usize, NoClone>::new();
        assert!(PairRows::bind(&wrong, 2, 2).is_none());
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    struct NoClone(i64, Rc<RefCell<Vec<i64>>>);
    impl Drop for NoClone {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    #[test]
    fn pair_capture_keeps_only_unique_current_entries_and_original_arc_owners() {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let original = SnapshotMap::<(usize, usize), _>::from_sorted(
            [
                ((0, 1), NoClone(1, drops.clone())),
                ((1, 0), NoClone(2, drops.clone())),
            ]
            .into_iter(),
        );
        let mut rows = PairRows::bind(&original, 3, 65).unwrap();
        let base_key = original.iter().next().unwrap().0 as *const _;
        let outer = rows.begin();
        rows.insert((0, 1), NoClone(3, drops.clone()));
        rows.remove(&(0, 1));
        rows.insert((0, 1), NoClone(4, drops.clone()));
        rows.remove(&(1, 0));
        let inner = rows.begin();
        rows.insert((2, 64), NoClone(5, drops.clone()));
        let capture = rows.capture_current(outer);
        let capture_in_trial = rows.capture_current(outer);
        assert_eq!(capture.entries.len(), 3);
        assert_eq!(
            Arc::strong_count(&rows.root),
            1,
            "capture does not share table ownership"
        );
        let slot = rows.position(&(0, 1)).unwrap();
        let captured_key = entry(&rows.root, slot).unwrap().key.as_ref() as *const _;
        let captured_value = rows.get(&(0, 1)).unwrap() as *const _;
        assert_ne!(
            base_key, captured_key,
            "remove/reinsert chooses a new key owner"
        );
        let value_owner = Arc::downgrade(&entry(&rows.root, slot).unwrap().value);
        rows.rollback(inner);
        rows.rollback(outer);
        assert!(
            value_owner.upgrade().is_some(),
            "capture owns current payload without Clone"
        );
        assert_eq!(rows.get(&(0, 1)).unwrap().0, 1);
        // A full save and a different trial may run before the chosen capture.
        let base = rows.clone();
        let other = rows.begin();
        rows.insert((0, 1), NoClone(6, drops.clone()));
        rows.remove(&(1, 0));
        rows.rollback(other);
        rows.install(base);
        let install = rows.begin();
        rows.install_capture(capture_in_trial);
        assert_eq!(rows.get(&(0, 1)).unwrap().0, 4);
        assert!(rows.get(&(1, 0)).is_none());
        assert_eq!(rows.get(&(2, 64)).unwrap().0, 5);
        assert_eq!(
            entry(&rows.root, slot).unwrap().key.as_ref() as *const _,
            captured_key
        );
        assert_eq!(rows.get(&(0, 1)).unwrap() as *const _, captured_value);
        rows.rollback(install);
        assert_eq!(
            entry(&rows.root, slot).unwrap().key.as_ref() as *const _,
            base_key
        );
        assert_eq!(rows.get(&(1, 0)).unwrap().0, 2);
        rows.install_capture(capture);
        assert_eq!(rows.get(&(0, 1)).unwrap().0, 4);
        assert_eq!(rows.get(&(2, 64)).unwrap().0, 5);
        assert!(!drops.borrow().contains(&4));
        assert!(!drops.borrow().contains(&5));
        drop(rows);
        assert!(drops.borrow().contains(&4));
        assert!(drops.borrow().contains(&5));
    }
}
