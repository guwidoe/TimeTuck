//! Inspect an entry and prepare its update with one comparison traversal.
//! The exclusive borrow keeps the path valid. Preparation does not acquire or
//! release any user-owned key/value; callers can finish fallible output cloning
//! before committing. Stored equal-key representatives remain unchanged.
use super::*;

pub(crate) struct PreparedChange<'a, K, V> {
    map: &'a mut SnapshotMap<K, V>,
    path: Path,
    found: bool,
}

impl<K: Ord, V> SnapshotMap<K, V> {
    pub(crate) fn prepare_change<R>(
        &mut self,
        key: &K,
        inspect: impl FnOnce(Option<&V>) -> R,
    ) -> (PreparedChange<'_, K, V>, R) {
        let (path, value) = locate(&self.root, key);
        let found = value.is_some();
        let observed = inspect(value.map(Arc::as_ref));
        (
            PreparedChange {
                map: self,
                path,
                found,
            },
            observed,
        )
    }
}

impl<K, V> PreparedChange<'_, K, V> {
    pub(crate) fn insert(self, key: K, value: V) {
        // Match ordinary insertion's ownership order: equal incoming keys drop
        // before allocating/installing the new value, then the old value drops.
        let key = if self.found {
            drop(key);
            None
        } else {
            Some(Arc::new(key))
        };
        commit_insert(self.map, &self.path, key, Arc::new(value));
    }
    pub(crate) fn remove(self) {
        if self.found {
            self.map.preserve_journal();
            erase(&mut self.map.root, self.path.as_slice());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    struct Key(usize, Rc<Cell<usize>>);
    impl PartialEq for Key {
        fn eq(&self, rhs: &Self) -> bool {
            self.0 == rhs.0
        }
    }
    impl Eq for Key {}
    impl PartialOrd for Key {
        fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
            Some(self.cmp(rhs))
        }
    }
    impl Ord for Key {
        fn cmp(&self, rhs: &Self) -> Ordering {
            self.1.set(self.1.get() + 1);
            self.0.cmp(&rhs.0)
        }
    }

    #[test]
    fn prepared_change_uses_one_search_and_abandoning_it_keeps_the_map() {
        let calls = Rc::new(Cell::new(0));
        let key = |id| Key(id, calls.clone());
        let mut map = SnapshotMap::new();
        for id in 0..127 {
            map.insert(key(id), id);
        }
        calls.set(0);
        let old = map.get(&key(7)).copied();
        let search_calls = calls.replace(0);
        let (prepared, value) = map.prepare_change(&key(7), |v| v.copied());
        assert_eq!(value, old);
        assert_eq!(calls.get(), search_calls);
        prepared.insert(key(7), 700);
        assert_eq!(calls.get(), search_calls);
        {
            let (_prepared, value) = map.prepare_change(&key(7), |v| v.copied());
            assert_eq!(value, Some(700));
        }
        assert_eq!(map.get(&key(7)), Some(&700));
    }

    #[test]
    fn replacements_new_entries_and_removals_preserve_nested_and_held_snapshots() {
        let mut map = SnapshotMap::from_sorted([(1, 10), (3, 30), (5, 50)].into_iter());
        let original = map.clone();
        let outer = map.begin_journal();
        map.prepare_change(&3, |_| ()).0.insert(3, 31);
        let inner = map.begin_journal();
        map.prepare_change(&2, |_| ()).0.insert(2, 20);
        map.prepare_change(&1, |_| ()).0.remove();
        map.prepare_change(&3, |_| ()).0.insert(3, 32);
        let held = map.clone();
        map.commit_journal(inner);
        map.prepare_change(&2, |_| ()).0.remove();
        map.rollback_journal(outer);
        assert_eq!(map, original);
        assert_eq!(
            held.iter().map(|(i, v)| (*i, *v)).collect::<Vec<_>>(),
            [(2, 20), (3, 32), (5, 50)]
        );
    }
}
