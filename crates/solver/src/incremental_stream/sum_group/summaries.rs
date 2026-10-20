//! Bound usize summaries contain no per-update shared owners. Overflow keys and
//! all generic/unbound keys retain the persistent ordered map implementation.
use super::SumState;
use crate::SnapshotMap;
use std::{
    any::{Any, TypeId},
    sync::Arc,
};

pub(super) struct Summaries<K> {
    tree: SnapshotMap<K, SumState>,
    dense: Option<Dense<K>>,
}
type Undo = Vec<(usize, Option<SumState>)>;
struct Dense<K> {
    keys: Arc<[K]>,
    values: Box<[Option<SumState>; 256]>,
    index: fn(&K) -> usize,
    frames: Vec<Undo>,
    spare: Vec<Undo>,
}
impl<K> Clone for Summaries<K> {
    fn clone(&self) -> Self {
        Self {
            tree: self.tree.clone(),
            dense: self.dense.as_ref().map(|d| Dense {
                keys: d.keys.clone(),
                values: d.values.clone(),
                index: d.index,
                frames: Vec::new(),
                spare: Vec::new(),
            }),
        }
    }
}
impl<K> Summaries<K> {
    pub(super) fn new() -> Self {
        Self {
            tree: SnapshotMap::new(),
            dense: None,
        }
    }
    fn replace(d: &mut Dense<K>, index: usize, value: Option<SumState>) {
        let old = std::mem::replace(&mut d.values[index], value);
        if let Some(frame) = d.frames.last_mut() {
            frame.push((index, old));
        }
    }
    pub(super) fn clear(&mut self) {
        self.tree.clear();
        if let Some(d) = &mut self.dense {
            for i in 0..256 {
                Self::replace(d, i, None);
            }
        }
    }
    pub(super) fn begin_journal(&mut self) -> usize {
        let token = self.tree.begin_journal();
        if let Some(d) = &mut self.dense {
            d.frames.push(d.spare.pop().unwrap_or_default());
        }
        token
    }
    pub(super) fn commit_journal(&mut self, token: usize) {
        self.tree.commit_journal(token);
        if let Some(d) = &mut self.dense {
            let mut undo = d.frames.pop().expect("summary journal");
            if let Some(parent) = d.frames.last_mut() {
                if parent.is_empty() {
                    std::mem::swap(parent, &mut undo);
                } else {
                    parent.append(&mut undo);
                }
            }
            undo.clear();
            d.spare.push(undo);
        }
    }
    pub(super) fn rollback_journal(&mut self, token: usize) {
        self.tree.rollback_journal(token);
        if let Some(d) = &mut self.dense {
            let mut undo = d.frames.pop().expect("summary journal");
            for (i, value) in undo.drain(..).rev() {
                d.values[i] = value;
            }
            d.spare.push(undo);
        }
    }
    pub(super) fn install_snapshot(&mut self, snapshot: Self) {
        self.tree.install_snapshot(snapshot.tree);
        match (&mut self.dense, snapshot.dense) {
            (Some(d), Some(saved)) => {
                for (i, value) in (*saved.values).into_iter().enumerate() {
                    Self::replace(d, i, value);
                }
            }
            (None, None) => {}
            _ => unreachable!("summary binding is immutable"),
        }
    }
    pub(super) fn journal_transaction(&mut self) -> Transaction<'_, K> {
        let token = self.begin_journal();
        Transaction {
            rows: self,
            token: Some(token),
        }
    }
}
impl<K: Ord> Summaries<K> {
    pub(super) fn iter(&self) -> impl Iterator<Item = (&K, &SumState)> {
        self.dense
            .iter()
            .flat_map(|d| {
                d.keys
                    .iter()
                    .zip(d.values.iter())
                    .filter_map(|(key, state)| state.as_ref().map(|state| (key, state)))
            })
            .chain(self.tree.iter())
    }
    pub(super) fn get_key_value(&self, key: &K) -> Option<(&K, &SumState)> {
        if let Some(d) = &self.dense {
            let i = (d.index)(key);
            if i < 256 {
                return d.values[i].as_ref().map(|s| (&d.keys[i], s));
            }
        }
        self.tree.get_key_value(key)
    }
    pub(super) fn contains_key(&self, key: &K) -> bool {
        self.get_key_value(key).is_some()
    }
    fn insert(&mut self, key: K, state: SumState) {
        if let Some(d) = &mut self.dense {
            let i = (d.index)(&key);
            if i < 256 {
                Self::replace(d, i, Some(state));
                return;
            }
        }
        self.tree.insert(key, state);
    }
    fn remove(&mut self, key: &K) {
        if let Some(d) = &mut self.dense {
            let i = (d.index)(key);
            if i < 256 {
                Self::replace(d, i, None);
                return;
            }
        }
        self.tree.remove(key);
    }
}
impl<K: 'static> Summaries<K> {
    pub(super) fn bind(&mut self) {
        if TypeId::of::<K>() != TypeId::of::<usize>() || self.dense.is_some() {
            return;
        }
        fn index<K: 'static>(key: &K) -> usize {
            *(key as &dyn Any)
                .downcast_ref::<usize>()
                .expect("usize summary key")
        }
        let keys: Arc<[K]> = (*(Box::new((0usize..256).collect::<Vec<_>>()) as Box<dyn Any>)
            .downcast::<Vec<K>>()
            .expect("usize summary keys"))
        .into();
        let mut values = Box::new([None; 256]);
        // Move the original tree aside only after all allocation and conversion.
        // Rebuild high keys using shared owners, without cloning user values.
        let mut high = Vec::new();
        for (key, state) in self.tree.shared_entries() {
            let i = index(key.as_ref());
            if i < 256 {
                values[i] = Some(*state);
            } else {
                high.push((key, state));
            }
        }
        self.tree = SnapshotMap::from_shared_sorted(high);
        self.dense = Some(Dense {
            keys,
            values,
            index: index::<K>,
            frames: Vec::new(),
            spare: Vec::new(),
        });
    }
}
pub(super) struct Transaction<'a, K> {
    rows: &'a mut Summaries<K>,
    token: Option<usize>,
}
impl<K: Ord> Transaction<'_, K> {
    pub(super) fn insert(&mut self, key: K, state: SumState) {
        self.rows.insert(key, state);
    }
    pub(super) fn remove(&mut self, key: &K) {
        self.rows.remove(key);
    }
}
impl<K> Transaction<'_, K> {
    pub(super) fn commit(mut self) {
        self.rows.commit_journal(self.token.take().unwrap());
    }
}
impl<K> Drop for Transaction<'_, K> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.rows.rollback_journal(token);
        }
    }
}

// Batches affecting bound summaries are small (at most the bound input set).
// A sorted contiguous scratch buffer avoids allocating a tree node per key.
pub(super) enum Prepared<K> {
    Tree(std::collections::BTreeMap<K, SumState>),
    Dense(Vec<(K, SumState)>),
}
impl<K> Summaries<K> {
    pub(super) fn prepare_batch(&self) -> Prepared<K> {
        if self.dense.is_some() {
            Prepared::Dense(Vec::new())
        } else {
            Prepared::Tree(std::collections::BTreeMap::new())
        }
    }
}
impl<K: Ord> Prepared<K> {
    pub(super) fn contains_key(&self, key: &K) -> bool {
        match self {
            Self::Tree(rows) => rows.contains_key(key),
            Self::Dense(rows) => rows.binary_search_by(|(k, _)| k.cmp(key)).is_ok(),
        }
    }
    pub(super) fn get_mut(&mut self, key: &K) -> Option<&mut SumState> {
        match self {
            Self::Tree(rows) => rows.get_mut(key),
            Self::Dense(rows) => rows
                .binary_search_by(|(k, _)| k.cmp(key))
                .ok()
                .map(|i| &mut rows[i].1),
        }
    }
    pub(super) fn insert(&mut self, key: K, state: SumState) {
        match self {
            Self::Tree(rows) => {
                rows.insert(key, state);
            }
            Self::Dense(rows) => match rows.binary_search_by(|(k, _)| k.cmp(&key)) {
                Ok(i) => rows[i].1 = state,
                Err(i) => rows.insert(i, (key, state)),
            },
        }
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&K, &SumState)> {
        let (tree, dense) = match self {
            Self::Tree(rows) => (Some(rows), None),
            Self::Dense(rows) => (None, Some(rows)),
        };
        tree.into_iter().flat_map(|rows| rows.iter()).chain(
            dense
                .into_iter()
                .flat_map(|rows| rows.iter().map(|(k, s)| (k, s))),
        )
    }
    pub(super) fn into_entries(self) -> impl Iterator<Item = (K, SumState)> {
        let (tree, dense) = match self {
            Self::Tree(rows) => (Some(rows), None),
            Self::Dense(rows) => (None, Some(rows)),
        };
        tree.into_iter()
            .flat_map(|rows| rows.into_iter())
            .chain(dense.into_iter().flat_map(|rows| rows.into_iter()))
    }
}

#[cfg(test)]
mod tests {
    use crate::incremental_stream::{SumGroupNode, TupleChange};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn view(node: &SumGroupNode<usize, usize>) -> Vec<(usize, i64)> {
        node.groups().map(|(key, value)| (*key, value)).collect()
    }

    #[test]
    fn bound_upgrade_retains_every_capture_and_sorted_overflow_group() {
        let mut node = SumGroupNode::new();
        for (id, key, value) in [(0, 255, 7), (1, 0, 0), (2, 256, 11), (3, usize::MAX, -2)] {
            node.apply(TupleChange::put(id, (key, value)));
        }
        let before = view(&node);
        assert!(node.bind_fixed_inputs(4));
        assert!(node.groups.dense.is_some());
        assert_eq!(view(&node), before);
        assert_eq!(node.input(&3), Some(&(usize::MAX, -2)));
        let output = node.apply_batch([
            TupleChange::put(0, (0, 5)),
            TupleChange::put(2, (256, 0)),
            TupleChange::put(3, (255, -5)),
        ]);
        assert_eq!(
            output.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![0, 255, 256, usize::MAX]
        );
        assert_eq!(view(&node), vec![(0, 5), (255, -5), (256, 0)]);
        assert_eq!(node.input(&1), Some(&(0, 0)));
        let mut generic = SumGroupNode::<usize, String>::new();
        assert!(generic.bind_fixed_inputs(1));
        assert!(generic.groups.dense.is_none());
        let mut unbound = SumGroupNode::<usize, usize>::new();
        assert!(!unbound.bind_fixed_inputs(257));
        assert!(unbound.groups.dense.is_none());
    }

    #[test]
    fn dense_nested_journals_saved_snapshots_clear_and_failed_install_are_atomic() {
        let mut node = SumGroupNode::<usize, usize>::new();
        assert!(node.bind_fixed_inputs(3));
        node.apply(TupleChange::put(0, (0, i64::MAX)));
        node.apply(TupleChange::put(1, (300, 9)));
        let original = node.checkpoint();
        let outer = node.begin_checkpoint();
        node.apply(TupleChange::put(0, (255, 4)));
        let middle = node.checkpoint();
        let inner = node.begin_checkpoint();
        node.apply(TupleChange::put(1, (255, -4)));
        assert_eq!(view(&node), vec![(255, 0)]);
        node.commit_checkpoint(inner);
        node.restore(middle);
        assert_eq!(view(&node), vec![(255, 4), (300, 9)]);
        node.clear();
        node.restore(outer);
        assert_eq!(view(&node), vec![(0, i64::MAX), (300, 9)]);
        assert!(node.try_apply(TupleChange::put(2, (0, 1))).is_err());
        assert!(node.input(&2).is_none());
        // First input installs, then the invalid bound ID panics; the input
        // guard must undo it before any prepared summaries become visible.
        assert!(
            catch_unwind(AssertUnwindSafe(|| node.apply_batch([
                TupleChange::put(0, (1, 5)),
                TupleChange::put(3, (255, 6)),
            ])))
            .is_err()
        );
        assert_eq!(node.input(&0), Some(&(0, i64::MAX)));
        assert_eq!(view(&node), vec![(0, i64::MAX), (300, 9)]);
        let held = node.clone();
        node.apply(TupleChange::put(0, (255, 12)));
        assert_eq!(view(&held), vec![(0, i64::MAX), (300, 9)]);
        node.restore(original);
        assert_eq!(view(&node), view(&held));
    }
    #[test]
    fn bound_and_tree_match_deterministic_batches_and_nested_history() {
        let mut zero = SumGroupNode::<usize, usize>::new();
        assert!(zero.bind_fixed_inputs(0));
        assert!(zero.groups.dense.is_some());
        assert!(zero.try_apply_batch([]).unwrap().is_empty());
        assert!(zero.groups().next().is_none());
        let mut too_large = SumGroupNode::<usize, usize>::new();
        assert!(!too_large.bind_fixed_inputs(257));
        let mut custom_id = SumGroupNode::<String, usize>::new();
        assert!(!custom_id.bind_fixed_inputs(256));
        assert!(custom_id.groups.dense.is_none());
        let mut custom_key = SumGroupNode::<usize, String>::new();
        assert!(custom_key.bind_fixed_inputs(256));
        assert!(custom_key.groups.dense.is_none());

        for wrapping in [false, true] {
            let mut dense = if wrapping {
                SumGroupNode::wrapping()
            } else {
                SumGroupNode::new()
            };
            let mut tree = dense.clone();
            assert!(dense.bind_fixed_inputs(256));
            assert!(dense.groups.dense.is_some());
            assert!(tree.groups.dense.is_none());
            let mut rng = 0x3729_a57d_c483_b102u64;
            let mut next = || {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                rng
            };
            let mut journals = Vec::new();
            let mut snapshots = Vec::new();
            let keys = [0, 1, 255, 256, usize::MAX];
            for step in 0..640 {
                match next() % 12 {
                    0 if journals.len() < 4 => {
                        journals.push((dense.begin_checkpoint(), tree.begin_checkpoint()))
                    }
                    1 if !journals.is_empty() => {
                        let (left, right) = journals.pop().unwrap();
                        dense.commit_checkpoint(left);
                        tree.commit_checkpoint(right);
                    }
                    2 if !journals.is_empty() => {
                        let (left, right) = journals.pop().unwrap();
                        dense.restore(left);
                        tree.restore(right);
                    }
                    3 => snapshots.push((dense.checkpoint(), tree.checkpoint())),
                    4 if !snapshots.is_empty() => {
                        let (left, right) = snapshots.pop().unwrap();
                        dense.restore(left);
                        tree.restore(right);
                    }
                    5 => {
                        dense.clear();
                        tree.clear();
                    }
                    _ => {
                        let first_id = (next() % 256) as usize;
                        let mut changes = Vec::new();
                        for row in 0..6 {
                            // Include repeated IDs even when random IDs differ.
                            let id = if row == 0 || row == 5 {
                                first_id
                            } else {
                                (next() % 256) as usize
                            };
                            let value = if next() % 4 == 0 {
                                None
                            } else {
                                let key = keys[(next() % keys.len() as u64) as usize];
                                let amount = if wrapping {
                                    [i64::MIN, i64::MAX, -1, 0, 1][(next() % 5) as usize]
                                } else {
                                    (next() % 101) as i64 - 50
                                };
                                Some((key, amount))
                            };
                            changes.push(TupleChange { id, value });
                        }
                        assert_eq!(
                            dense.try_apply_batch(changes.clone()),
                            tree.try_apply_batch(changes),
                            "batch {step}, wrapping={wrapping}"
                        );
                    }
                }
                assert_eq!(
                    view(&dense),
                    view(&tree),
                    "view {step}, wrapping={wrapping}"
                );
                for id in 0..256 {
                    assert_eq!(
                        dense.input(&id),
                        tree.input(&id),
                        "input {id} at {step}, wrapping={wrapping}"
                    );
                }
            }
            while let Some((left, right)) = journals.pop() {
                dense.restore(left);
                tree.restore(right);
                assert_eq!(view(&dense), view(&tree));
                for id in 0..256 {
                    assert_eq!(dense.input(&id), tree.input(&id));
                }
            }
        }
    }
}
