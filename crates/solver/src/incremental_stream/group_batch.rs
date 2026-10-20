//! Prepare final integer groups once, then install under reversible map guards.
use super::*;
use std::collections::BTreeMap;

struct GroupJournal {
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
        Box::new(GroupJournal {
            inputs: self.inputs.begin_journal(),
            groups: self.groups.begin_journal(),
        })
    }
    pub(crate) fn commit_journal_checkpoint(&mut self, checkpoint: Box<dyn Any>) {
        let saved = checkpoint
            .downcast::<GroupJournal>()
            .expect("group journal");
        self.inputs.commit_journal(saved.inputs);
        self.groups.commit_journal(saved.groups);
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
        if let Some(saved) = original.downcast_ref::<GroupJournal>() {
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
            Err(checkpoint) => match checkpoint.downcast::<GroupJournal>() {
                Ok(saved) => {
                    self.inputs.rollback_journal(saved.inputs);
                    self.groups.rollback_journal(saved.groups);
                    self.poisoned = false;
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
        self.inputs.bind_fixed_extended(count)
    }
}
