//! Prepare final integer groups once, then install under reversible map guards.
use super::*;
use crate::stream_graph::checkpoint_box::CheckpointBox;
use std::collections::BTreeMap;

pub(super) struct GroupJournal {
    inputs: usize,
    groups: usize,
}
struct GroupCaptured<I, K> {
    inputs: group_inputs::Capture<I, (K, i64)>,
    groups: crate::primitive_rows::RowsCapture<K, IntCollector>,
}
impl<I: Ord + Clone, K: Ord + Clone> GroupNode<I, K> {
    /// Apply an atomic round, publishing one final summary per affected group.
    /// Repeated input IDs retain their first representative and last value.
    /// Existing group-key representatives survive temporary emptiness within the
    /// round. New groups use the first final key visited in input-ID order.
    /// Final output is ordered by group key; absent-to-absent groups emit nothing.
    /// All preparation and output cloning precede installation. Comparison panics
    /// during installation roll both maps back without invoking comparisons.
    /// Stable keys and non-panicking destructors are required.
    pub fn apply_batch(
        &mut self,
        changes: impl IntoIterator<Item = TupleChange<I, (K, i64)>>,
    ) -> Vec<TupleChange<K, IntSummary>> {
        healthy(self.poisoned);
        self.apply_sorted_batch(join_batch::last_updates(changes))
    }

    /// Internal publication path for already sorted, unique input IDs. The graph
    /// boundary has coalesced these before invoking its row mapping callbacks.
    pub(crate) fn apply_sorted_batch(
        &mut self,
        changes: Vec<(I, Option<(K, i64)>)>,
    ) -> Vec<TupleChange<K, IntSummary>> {
        healthy(self.poisoned);
        let mut groups = BTreeMap::<K, IntCollector>::new();
        // Preserve the stored representative even when the final row supplies a
        // different but equal key. Each touched collector is cloned only once.
        let prepare = |groups: &mut BTreeMap<K, IntCollector>, key: &K| {
            if !groups.contains_key(key) {
                let (key, value) = self
                    .groups
                    .get_key_value(key)
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .unwrap_or_else(|| (key.clone(), IntCollector::new()));
                groups.insert(key, value);
            }
        };
        for (id, new) in &changes {
            if let Some((key, value)) = self.inputs.get(id) {
                prepare(&mut groups, key);
                assert!(
                    groups.get_mut(key).unwrap().remove(*value),
                    "input occurrence"
                );
            }
            if let Some((key, value)) = new {
                prepare(&mut groups, key);
                groups.get_mut(key).unwrap().insert(*value);
            }
        }
        let output = groups
            .iter()
            .filter_map(|(key, collector)| {
                let summary = collector.summary();
                if summary.count == 0 && !self.groups.contains_key(key) {
                    return None;
                }
                Some(TupleChange {
                    id: key.clone(),
                    value: (summary.count != 0).then_some(summary),
                })
            })
            .collect();
        let mut inputs = self.inputs.journal_transaction();
        let mut summaries = self.groups.journal_transaction();
        for (id, new) in changes {
            if let Some(value) = new {
                inputs.insert(id, value);
            } else {
                inputs.remove(&id);
            }
        }
        for (key, collector) in groups {
            if collector.summary().count == 0 {
                summaries.remove(&key);
            } else {
                summaries.insert(key, collector);
            }
        }
        inputs.commit();
        summaries.commit();
        output
    }

    pub(crate) fn begin_journal_checkpoint(&mut self) -> Box<dyn Any> {
        healthy(self.poisoned);
        self.checkpoint_box.begin(GroupJournal {
            inputs: self.inputs.begin_journal(),
            groups: self.groups.begin_journal(),
        })
    }
    pub(crate) fn commit_journal_checkpoint(&mut self, checkpoint: Box<dyn Any>) {
        let (saved, empty) =
            CheckpointBox::<GroupJournal>::unpack(checkpoint).expect("group journal");
        self.inputs.commit_journal(saved.inputs);
        self.groups.commit_journal(saved.groups);
        self.checkpoint_box.recycle(empty);
    }
    pub(crate) fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any>
    where
        I: 'static,
        K: 'static,
    {
        healthy(self.poisoned);
        // Custom group keys can have observable destructor effects. Keep their
        // complete snapshot ownership even when input IDs have a fixed binding.
        if std::mem::needs_drop::<K>() {
            return self.checkpoint();
        }
        if let Some(saved) = CheckpointBox::<GroupJournal>::get(original) {
            Box::new(GroupCaptured {
                inputs: self.inputs.capture_current(saved.inputs),
                groups: self.groups.capture_current(saved.groups),
            })
        } else {
            self.checkpoint()
        }
    }
    pub(crate) fn restore_checkpoint(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        K: 'static,
    {
        match checkpoint.downcast::<GroupCaptured<I, K>>() {
            Ok(saved) => {
                self.inputs.install_capture(saved.inputs);
                self.groups.install_capture(saved.groups);
                self.poisoned = false;
            }
            Err(checkpoint) => match CheckpointBox::<GroupJournal>::unpack(checkpoint) {
                Ok((saved, empty)) => {
                    self.inputs.rollback_journal(saved.inputs);
                    self.groups.rollback_journal(saved.groups);
                    self.poisoned = false;
                    self.checkpoint_box.recycle(empty);
                }
                Err(snapshot) => self.restore(snapshot),
            },
        }
    }
}

// Only the private graph binding path supplies the checked planning universe.
// Generic standalone GroupNode methods retain their existing lifetime bounds.
impl<I: Ord + Clone + 'static, K: Ord + Clone> GroupNode<I, K> {
    pub(crate) fn bind_fixed_inputs(&mut self, count: usize) -> bool {
        if let Some(copy) = self.owned_input_copy {
            self.inputs.bind_owned_fixed(count, copy)
        } else {
            self.inputs.bind_fixed_extended(count)
        }
    }
}

#[cfg(test)]
mod carrier_tests {
    use super::*;
    fn contents<K: Ord + Clone>(node: &GroupNode<usize, K>) -> Vec<(K, usize, i64)> {
        node.groups()
            .map(|(k, s)| (k.clone(), s.count, s.sum))
            .collect()
    }
    fn address(saved: &dyn Any) -> usize {
        CheckpointBox::<GroupJournal>::get(saved).unwrap() as *const GroupJournal as usize
    }
    #[test]
    fn group_checkpoint_carrier_reuse_keeps_nested_captures_full_saves_and_public_modes() {
        for mode in 0..3 {
            let mut node = GroupNode::<usize, usize>::new();
            if mode > 0 {
                assert!(node.bind_primitive_group_keys());
                assert!(node.bind_primitive_inputs());
            }
            if mode == 2 {
                node.enable_consumed_collectors();
            }
            node.apply(TupleChange::put(0, (0, 1)));
            node.apply(TupleChange::put(1, (0, 2)));
            let baseline = vec![(0, 2, 3)];
            assert_eq!(contents(&node), baseline);
            let outer = node.begin_journal_checkpoint();
            let pointer = address(outer.as_ref());
            node.apply(TupleChange::put(0, (1, 5)));
            let full = node.checkpoint();
            let inner = node.begin_journal_checkpoint();
            assert_ne!(pointer, address(inner.as_ref()));
            node.apply(TupleChange::retract(1));
            let captured = node.capture_checkpoint(outer.as_ref());
            assert_eq!(contents(&node), vec![(1, 1, 5)]);
            node.commit_journal_checkpoint(inner);
            node.restore_checkpoint(outer);
            assert_eq!(contents(&node), baseline);
            let restore = node.begin_journal_checkpoint();
            assert_eq!(address(restore.as_ref()), pointer);
            node.restore_checkpoint(full);
            assert_eq!(contents(&node), vec![(0, 1, 2), (1, 1, 5)]);
            node.restore_checkpoint(restore);
            assert_eq!(contents(&node), baseline);
            let restore = node.begin_journal_checkpoint();
            assert_eq!(address(restore.as_ref()), pointer);
            node.restore_checkpoint(captured);
            assert_eq!(contents(&node), vec![(1, 1, 5)]);
            node.restore_checkpoint(restore);
            assert_eq!(contents(&node), baseline);
            let committed = node.begin_journal_checkpoint();
            assert_eq!(address(committed.as_ref()), pointer);
            node.apply(TupleChange::put(1, (0, 3)));
            node.commit_journal_checkpoint(committed);
            assert_eq!(contents(&node), vec![(0, 2, 4)]);
            let empty = node.begin_journal_checkpoint();
            assert_eq!(address(empty.as_ref()), pointer);
            node.restore_checkpoint(empty);
            assert_eq!(contents(&node), vec![(0, 2, 4)]);
        }
        let mut node = GroupNode::<usize, String>::new();
        node.apply(TupleChange::put(0, ("old".into(), 7)));
        let outer = node.begin_journal_checkpoint();
        let pointer = address(outer.as_ref());
        node.apply(TupleChange::put(0, ("new".into(), 9)));
        let captured = node.capture_checkpoint(outer.as_ref());
        node.restore_checkpoint(outer);
        assert_eq!(contents(&node), vec![("old".into(), 1, 7)]);
        let restore = node.begin_journal_checkpoint();
        assert_eq!(address(restore.as_ref()), pointer);
        node.restore_checkpoint(captured);
        assert_eq!(contents(&node), vec![("new".into(), 1, 9)]);
        node.restore_checkpoint(restore);
        assert_eq!(contents(&node), vec![("old".into(), 1, 7)]);
    }
}
