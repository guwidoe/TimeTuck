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
pub(super) fn record_replacement<K, V>(
    journal: &mut Option<Box<Journal<K, V>>>,
    path: &Path,
    old: &Arc<V>,
) {
    let Some(journal) = journal else {
        return;
    };
    let Some(Saved::Updates(updates)) = journal.frames.last_mut() else {
        return;
    };
    let mut directions = [0; 2];
    for (i, direction) in path.as_slice().iter().enumerate() {
        if *direction == Ordering::Greater {
            directions[i / usize::BITS as usize] |= 1 << (i % usize::BITS as usize);
        }
    }
    updates.push(Undo {
        directions,
        len: path.len,
        value: old.clone(),
    });
}
fn recycle<V>(spare: &mut Vec<Vec<Undo<V>>>, empty: Vec<Undo<V>>) {
    debug_assert!(empty.is_empty());
    spare.push(empty);
}

impl<K, V> SnapshotMap<K, V> {
    pub(crate) fn begin_journal(&mut self) -> usize {
        // A retained snapshot already forces copy-on-write. Keep its root for
        // constant-time rollback instead of adding a value log to the same work.
        let shared = self
            .root
            .as_ref()
            .is_some_and(|root| Arc::strong_count(root) > 1);
        if shared {
            // A nested root checkpoint must preserve its enclosing entry states
            // before it can commit changes without merging replacement records.
            self.preserve_journal();
        }
        let journal = self.journal.get_or_insert_with(|| {
            Box::new(Journal {
                frames: Vec::new(),
                spare: Vec::new(),
            })
        });
        let token = journal.frames.len();
        journal.frames.push(if shared {
            Saved::Root(self.root.clone())
        } else {
            Saved::Updates(journal.spare.pop().unwrap_or_default())
        });
        token
    }
    /// Detached view of a journal's entry state, used only by diagnostics.
    pub(crate) fn journal_snapshot(&self, token: usize) -> Self {
        let journal = self.journal.as_ref().expect("active map journal");
        assert!(token < journal.frames.len(), "active map journal token");
        let mut snapshot = self.clone();
        for frame in journal.frames[token..].iter().rev() {
            match frame {
                Saved::Root(root) => snapshot.root = root.clone(),
                Saved::Updates(updates) => {
                    for undo in updates.iter().rev() {
                        restore_value(
                            &mut snapshot.root,
                            Undo {
                                directions: undo.directions,
                                len: undo.len,
                                value: undo.value.clone(),
                            },
                        );
                    }
                }
            }
        }
        snapshot
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
    pub(crate) fn install_snapshot(&mut self, snapshot: Self) {
        self.preserve_journal();
        self.root = snapshot.root;
    }
}
// Full saved candidates keep their existing map type. Trial checkpoints carry
// only a token, so they do not add an owner to the working root.
struct JournalToken(usize);
impl<K: 'static, V: 'static> SnapshotMap<K, V> {
    pub(crate) fn begin_checkpoint(&mut self) -> Box<dyn std::any::Any> {
        Box::new(JournalToken(self.begin_journal()))
    }
    pub(crate) fn commit_checkpoint(&mut self, checkpoint: Box<dyn std::any::Any>) {
        let token = checkpoint
            .downcast::<JournalToken>()
            .expect("map journal token");
        self.commit_journal(token.0);
    }
    pub(crate) fn restore_checkpoint(&mut self, checkpoint: Box<dyn std::any::Any>) {
        match checkpoint.downcast::<JournalToken>() {
            Ok(token) => self.rollback_journal(token.0),
            Err(snapshot) => {
                self.install_snapshot(*snapshot.downcast::<Self>().expect("map snapshot"))
            }
        }
    }
    pub(crate) fn checkpoint_snapshot(&self, checkpoint: &dyn std::any::Any) -> Self {
        if let Some(token) = checkpoint.downcast_ref::<JournalToken>() {
            self.journal_snapshot(token.0)
        } else {
            checkpoint
                .downcast_ref::<Self>()
                .expect("map snapshot")
                .clone()
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
    fn shared_roots_use_snapshot_rollback_and_preserve_enclosing_value_trials() {
        let mut map = SnapshotMap::new();
        map.insert(1, 10);
        let outer = map.begin_journal();
        map.insert(1, 20);
        let held = map.clone();
        let inner = map.begin_journal();
        assert!(
            map.journal
                .as_ref()
                .unwrap()
                .frames
                .iter()
                .all(|f| matches!(f, Saved::Root(_)))
        );
        map.insert(1, 30);
        map.commit_journal(inner);
        assert_eq!(map[&1], 30);
        assert_eq!(held[&1], 20);
        map.rollback_journal(outer);
        assert_eq!(map[&1], 10);
        let held = map.clone();
        let trial = map.begin_journal();
        map.insert(1, 40);
        map.rollback_journal(trial);
        assert!(map.shares_root(&held));
    }
    #[test]
    fn diagnostic_views_preserve_nested_frames_and_detach_from_later_edits() {
        let mut map = SnapshotMap::new();
        for i in 0..32 {
            map.insert(i, i);
        }
        let original: BTreeMap<_, _> = map.iter().map(|(&k, &v)| (k, v)).collect();
        let outer = map.begin_journal();
        map.insert(0, 100);
        let middle: BTreeMap<_, _> = map.iter().map(|(&k, &v)| (k, v)).collect();
        let inner = map.begin_journal();
        map.insert(0, 200);
        map.insert(1, 300);
        let current = map.clone();
        let outer_view = map.journal_snapshot(outer);
        let inner_view = map.journal_snapshot(inner);
        assert_eq!(outer_view, original);
        assert_eq!(inner_view, middle);
        assert_eq!(map, current);
        map.remove(&2);
        map.insert(99, 999);
        assert_eq!(map.journal_snapshot(outer), original);
        assert_eq!(map.journal_snapshot(inner), middle);
        map.commit_journal(inner);
        map.insert(0, 500);
        assert_eq!(map.journal_snapshot(outer), original);
        map.rollback_journal(outer);
        assert_eq!(map, original);
        assert_eq!(inner_view, middle);
        assert_eq!(outer_view, original);
    }
    #[test]
    fn erased_checkpoints_allow_saved_snapshots_inside_an_outer_trial() {
        let mut map = SnapshotMap::new();
        map.insert(1, 10);
        let outer = map.begin_checkpoint();
        map.insert(1, 20);
        let saved = Box::new(map.clone());
        let inner = map.begin_checkpoint();
        map.insert(1, 30);
        map.commit_checkpoint(inner);
        map.restore_checkpoint(saved);
        assert_eq!(map[&1], 20);
        assert_eq!(map.checkpoint_snapshot(outer.as_ref())[&1], 10);
        map.restore_checkpoint(outer);
        assert_eq!(map[&1], 10);
        assert!(map.journal.as_ref().unwrap().frames.is_empty());
    }
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
        // Join batches now use this guard; custom keys retain SnapshotMap storage.
        let mut map = crate::primitive_rows::PrimitiveRows::new();
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
        assert!(
            map.materialized()
                .journal
                .as_ref()
                .unwrap()
                .frames
                .is_empty()
        );
    }
}
