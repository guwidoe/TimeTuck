//! Reversible value replacements; structural changes fall back to shared roots.
use super::*;

struct Undo<V> {
    directions: [usize; 2],
    len: usize,
    value: Arc<V>,
}
enum Saved<K, V> {
    Updates(Vec<Undo<V>>),
    Root(Link<K, V>),
}
pub(super) struct Journal<K, V> {
    frames: Vec<Saved<K, V>>,
    spare: Vec<Vec<Undo<V>>>,
}
fn restore_value<K, V>(root: &mut Link<K, V>, undo: Undo<V>) {
    let mut path = Path {
        directions: [Ordering::Less; 2 * usize::BITS as usize],
        len: undo.len,
    };
    for (i, direction) in path.directions[..undo.len].iter_mut().enumerate() {
        if undo.directions[i / usize::BITS as usize] & (1 << (i % usize::BITS as usize)) != 0 {
            *direction = Ordering::Greater;
        }
    }
    replace(root, path.as_slice(), undo.value);
}
fn recycle<V>(spare: &mut Vec<Vec<Undo<V>>>, empty: Vec<Undo<V>>) {
    debug_assert!(empty.is_empty());
    spare.push(empty);
}

impl<K, V> SnapshotMap<K, V> {
    pub(crate) fn begin_journal(&mut self) -> usize {
        let journal = self.journal.get_or_insert_with(|| {
            Box::new(Journal {
                frames: Vec::new(),
                spare: Vec::new(),
            })
        });
        let token = journal.frames.len();
        journal
            .frames
            .push(Saved::Updates(journal.spare.pop().unwrap_or_default()));
        token
    }
    pub(crate) fn rollback_journal(&mut self, token: usize) {
        let journal = self.journal.as_mut().expect("active map journal");
        assert_eq!(journal.frames.len(), token + 1, "map journal stack order");
        match journal.frames.pop().unwrap() {
            Saved::Root(root) => self.root = root,
            Saved::Updates(mut updates) => {
                for undo in updates.drain(..).rev() {
                    restore_value(&mut self.root, undo);
                }
                recycle(&mut journal.spare, updates);
            }
        }
    }
    pub(crate) fn commit_journal(&mut self, token: usize) {
        let journal = self.journal.as_mut().expect("active map journal");
        assert_eq!(journal.frames.len(), token + 1, "map journal stack order");
        if let Saved::Updates(mut updates) = journal.frames.pop().unwrap() {
            if let Some(Saved::Updates(parent)) = journal.frames.last_mut() {
                if parent.is_empty() {
                    std::mem::swap(parent, &mut updates);
                } else {
                    parent.append(&mut updates);
                }
            }
            updates.clear();
            recycle(&mut journal.spare, updates);
        }
    }
    // Capture every enclosing original state before tree topology can change.
    // Replaying replacement paths uses only ownership operations, never Ord/Clone.
    pub(super) fn preserve_journal(&mut self) {
        let Some(mut journal) = self.journal.take() else {
            return;
        };
        if journal
            .frames
            .iter()
            .any(|f| matches!(f, Saved::Updates(_)))
        {
            let mut root = self.root.clone();
            for saved in journal.frames.iter_mut().rev() {
                match saved {
                    Saved::Root(old) => root = old.clone(),
                    Saved::Updates(updates) => {
                        for undo in updates.drain(..).rev() {
                            restore_value(&mut root, undo);
                        }
                        let empty = std::mem::take(updates);
                        *saved = Saved::Root(root.clone());
                        recycle(&mut journal.spare, empty);
                    }
                }
            }
        }
        self.journal = Some(journal);
    }
    pub(super) fn record_replacement(&mut self, path: &Path) {
        let Some(journal) = &mut self.journal else {
            return;
        };
        let Some(Saved::Updates(updates)) = journal.frames.last_mut() else {
            return;
        };
        let mut node = self.root.as_deref().expect("replacement root");
        let mut directions = [0; 2];
        for (i, direction) in path.as_slice().iter().enumerate() {
            node = if *direction == Ordering::Less {
                node.left.as_deref()
            } else {
                directions[i / usize::BITS as usize] |= 1 << (i % usize::BITS as usize);
                node.right.as_deref()
            }
            .expect("replacement path");
        }
        updates.push(Undo {
            directions,
            len: path.len,
            value: node.value.clone(),
        });
    }
    pub(crate) fn install_snapshot(&mut self, snapshot: Self) {
        self.preserve_journal();
        self.root = snapshot.root;
    }
    pub(crate) fn journal_transaction(&mut self) -> Transaction<'_, K, V> {
        let token = self.begin_journal();
        Transaction {
            map: self,
            token: Some(token),
        }
    }
}
pub(crate) struct Transaction<'a, K, V> {
    map: &'a mut SnapshotMap<K, V>,
    token: Option<usize>,
}
impl<K: Ord, V> Transaction<'_, K, V> {
    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.map.insert(key, value);
    }
}
impl<K, V> Transaction<'_, K, V> {
    pub(crate) fn commit(mut self) {
        self.map.commit_journal(self.token.take().unwrap());
    }
}
impl<K, V> Drop for Transaction<'_, K, V> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.map.rollback_journal(token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    #[test]
    fn value_trials_reuse_tree_nodes_and_journal_storage_without_clone_bounds() {
        #[derive(Eq, PartialEq, Ord, PartialOrd)]
        struct Key(usize);
        struct Value(usize);
        let mut map = SnapshotMap::new();
        for i in 0..128 {
            map.insert(Key(i), Value(i));
        }
        let root = Arc::as_ptr(map.root.as_ref().unwrap());
        let token = map.begin_journal();
        for i in 0..32 {
            map.insert(Key(i), Value(999));
        }
        assert_eq!(Arc::as_ptr(map.root.as_ref().unwrap()), root);
        let held = map.clone();
        map.insert(Key(5), Value(777));
        map.rollback_journal(token);
        for i in 0..128 {
            assert_eq!(map.get(&Key(i)).unwrap().0, i);
        }
        assert_eq!(held.get(&Key(5)).unwrap().0, 999);
        let spare = map.journal.as_ref().unwrap().spare.last().unwrap();
        let allocation = (spare.as_ptr(), spare.capacity());
        for i in 0..8 {
            let token = map.begin_journal();
            map.insert(Key(i), Value(555));
            map.rollback_journal(token);
            let j = map.journal.as_ref().unwrap();
            assert!(j.frames.is_empty());
            let spare = j.spare.last().unwrap();
            assert_eq!((spare.as_ptr(), spare.capacity()), allocation);
            assert_eq!(map.get(&Key(i)).unwrap().0, i);
        }
        let mut warmed = None;
        for round in 0..4 {
            let outer = map.begin_journal();
            map.insert(Key(0), Value(100));
            let inner = map.begin_journal();
            map.insert(Key(1), Value(200));
            map.commit_journal(inner);
            map.rollback_journal(outer);
            let pool: std::collections::BTreeSet<_> = map
                .journal
                .as_ref()
                .unwrap()
                .spare
                .iter()
                .map(|v| {
                    assert!(v.is_empty());
                    (v.as_ptr() as usize, v.capacity())
                })
                .collect();
            if round == 1 {
                warmed = Some(pool);
            } else if round > 1 {
                assert_eq!(Some(pool), warmed);
            }
        }
    }
    #[test]
    fn structural_changes_preserve_all_enclosing_journals_and_held_snapshots() {
        for operation in 0..4 {
            for commit in [false, true] {
                let mut map = SnapshotMap::new();
                for i in 0..64 {
                    map.insert(i, i);
                }
                let original: BTreeMap<_, _> = map.iter().map(|(&k, &v)| (k, v)).collect();
                let outer = map.begin_journal();
                map.insert(0, 100);
                let before_inner = map.clone();
                let inner = map.begin_journal();
                map.insert(1, 200);
                match operation {
                    0 => map.insert(200, 300),
                    1 => {
                        assert!(map.remove(&3));
                    }
                    2 => map.clear(),
                    _ => {
                        let mut other = SnapshotMap::new();
                        other.insert(500, 600);
                        map.install_snapshot(other);
                    }
                }
                let held = map.clone();
                let expected_held: BTreeMap<_, _> = held.iter().map(|(&k, &v)| (k, v)).collect();
                if commit {
                    map.commit_journal(inner);
                } else {
                    map.rollback_journal(inner);
                    assert_eq!(map, before_inner);
                }
                map.insert(999, 1000);
                map.rollback_journal(outer);
                assert_eq!(map, original);
                assert_eq!(held, expected_held);
                assert_eq!(before_inner[&0], 100);
                assert_eq!(before_inner[&1], 1);
                assert!(map.journal.as_ref().unwrap().frames.is_empty());
            }
        }
    }
    #[test]
    fn nested_commits_replay_repeated_replacements_in_reverse_order() {
        let mut map = SnapshotMap::new();
        map.insert(5, 0);
        let outer = map.begin_journal();
        map.insert(5, 1);
        let first = map.clone();
        let inner = map.begin_journal();
        map.insert(5, 2);
        let nested = map.begin_journal();
        map.insert(5, 3);
        map.rollback_journal(nested);
        assert_eq!(map[&5], 2);
        map.commit_journal(inner);
        map.insert(5, 4);
        map.rollback_journal(outer);
        assert_eq!(map[&5], 0);
        assert_eq!(first[&5], 1);
        let token = map.begin_journal();
        map.insert(5, 6);
        map.commit_journal(token);
        assert_eq!(map[&5], 6);
    }
    #[test]
    fn guard_panic_restoration_never_calls_key_comparisons() {
        struct Key(i32, Rc<Cell<bool>>);
        impl PartialEq for Key {
            fn eq(&self, r: &Self) -> bool {
                self.0 == r.0
            }
        }
        impl Eq for Key {}
        impl PartialOrd for Key {
            fn partial_cmp(&self, r: &Self) -> Option<Ordering> {
                Some(self.cmp(r))
            }
        }
        impl Ord for Key {
            fn cmp(&self, r: &Self) -> Ordering {
                assert!(!self.1.get(), "comparison during rollback");
                self.0.cmp(&r.0)
            }
        }
        let fail = Rc::new(Cell::new(false));
        let mut map = SnapshotMap::new();
        for i in 0..32 {
            map.insert(Key(i, fail.clone()), i);
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut trial = map.journal_transaction();
                trial.insert(Key(0, fail.clone()), 100);
                trial.insert(Key(31, fail.clone()), 200);
                fail.set(true);
                panic!("downstream failure");
            }))
            .is_err()
        );
        fail.set(false);
        assert_eq!(map.get(&Key(0, fail.clone())), Some(&0));
        assert_eq!(map.get(&Key(31, fail)), Some(&31));
        assert!(map.journal.as_ref().unwrap().frames.is_empty());
    }
}
