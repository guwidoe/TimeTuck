//! Original borrowed traversal controls; no upstream source-method credit.
use super::*;
use std::{cell::Cell, collections::BTreeMap, rc::Rc};
#[test]
fn inline_and_spilled_traversals_are_ordered_exact_sized_fused_and_refill() {
    for size in [0usize, 1, 7, 15, 31, 255] {
        let map = SnapshotMap::from_sorted((0..size).map(|key| (key, key * 3)));
        let expected: BTreeMap<_, _> = (0..size).map(|key| (key, key * 3)).collect();
        let mut iter = map.iter();
        assert_eq!(iter.stack.overflow.is_empty(), size <= 15);
        assert_eq!(iter.len(), size);
        let mut actual = Vec::new();
        let mut spilled = !iter.stack.overflow.is_empty();
        let mut spill_starts = usize::from(spilled);
        while let Some((&key, &value)) = iter.next() {
            actual.push((key, value));
            let remaining = size - actual.len();
            assert_eq!(iter.len(), remaining);
            assert_eq!(iter.size_hint(), (remaining, Some(remaining)));
            let now = !iter.stack.overflow.is_empty();
            if now && !spilled {
                spill_starts += 1;
            }
            spilled = now;
        }
        assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);
        if size == 255 {
            assert!(spill_starts > 1, "right subtrees must refill overflow");
        }
        let mut partial = map.iter();
        let first: Vec<_> = partial.by_ref().take(3).map(|(&k, &v)| (k, v)).collect();
        assert_eq!(
            first,
            (0..size.min(3)).map(|k| (k, k * 3)).collect::<Vec<_>>()
        );
        assert_eq!(partial.len(), size - size.min(3));
        drop(partial);
        assert_eq!(map.iter().count(), size);
    }
}
#[test]
fn deletion_replacement_and_persistent_snapshots_match_independent_ordered_maps() {
    let mut map = SnapshotMap::new();
    let mut oracle = BTreeMap::new();
    for index in 0..257 {
        let key = (index * 73) % 257;
        map.insert(key, key + 1000);
        oracle.insert(key, key + 1000);
    }
    let held = map.clone();
    let old = oracle.clone();
    for key in (0..257).step_by(3) {
        assert_eq!(map.remove(&key), oracle.remove(&key).is_some());
    }
    for key in (0..257).step_by(5) {
        map.insert(key, -key);
        oracle.insert(key, -key);
    }
    assert_eq!(
        map.iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>(),
        oracle.into_iter().collect::<Vec<_>>()
    );
    assert_eq!(
        (&held)
            .into_iter()
            .map(|(&k, &v)| (k, v))
            .collect::<Vec<_>>(),
        old.into_iter().collect::<Vec<_>>()
    );
    assert!(!held.iter().stack.overflow.is_empty());
    let mut partial = held.iter();
    assert_eq!(partial.nth(63).map(|(&k, &v)| (k, v)), Some((63, 1063)));
    assert_eq!(partial.len(), 193);
    drop(partial);
    assert_eq!(held.len(), 257);
}
struct Key(usize, Rc<Cell<bool>>, Rc<Cell<usize>>); // No Clone.
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        assert!(!self.1.get(), "iteration compared keys");
        self.0.cmp(&other.0)
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        self.2.set(self.2.get() + 1);
    }
}
struct Value(usize, Rc<Cell<usize>>); // No Clone.
impl Drop for Value {
    fn drop(&mut self) {
        self.1.set(self.1.get() + 1);
    }
}
#[test]
fn borrowed_nonclone_owners_keep_identity_without_comparisons_or_iterator_drop_effects() {
    let fail = Rc::new(Cell::new(false));
    let keys = Rc::new(Cell::new(0));
    let values = Rc::new(Cell::new(0));
    let mut map = SnapshotMap::new();
    for id in 0..128 {
        map.insert(
            Key(id, fail.clone(), keys.clone()),
            Value(id, values.clone()),
        );
    }
    let entries = map.shared_entries();
    let held = map.clone();
    fail.set(true);
    let mut iter = map.iter();
    assert!(!iter.stack.overflow.is_empty());
    for (index, (key, value)) in iter.by_ref().enumerate() {
        assert_eq!((key.0, value.0), (index, index));
        assert!(std::ptr::eq(key, entries[index].0.as_ref()));
        assert!(std::ptr::eq(value, entries[index].1.as_ref()));
    }
    drop(iter);
    let mut partial = held.iter();
    assert_eq!(partial.next().unwrap().0.0, 0);
    drop(partial);
    assert_eq!((keys.get(), values.get()), (0, 0));
    drop(held);
    drop(map);
    assert_eq!((keys.get(), values.get()), (0, 0));
    drop(entries);
    assert_eq!((keys.get(), values.get()), (128, 128));
}
