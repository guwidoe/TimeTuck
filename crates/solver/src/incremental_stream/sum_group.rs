//! Source-ID-authoritative integer sums without value-frequency storage.
use super::{TupleChange, group_inputs::GroupInputs, join_batch};
use crate::{SnapshotMap, SumOverflow};
use std::{any::Any, collections::BTreeMap};

#[derive(Clone, Copy)]
enum Arithmetic {
    Checked,
    Wrapping,
}

#[derive(Clone, Copy, Default)]
struct SumState {
    count: usize,
    total: i64,
}

impl Arithmetic {
    fn add(self, left: i64, right: i64) -> Result<i64, SumOverflow> {
        match self {
            Self::Checked => left.checked_add(right).ok_or(SumOverflow),
            Self::Wrapping => Ok(left.wrapping_add(right)),
        }
    }

    fn sub(self, left: i64, right: i64) -> Result<i64, SumOverflow> {
        match self {
            Self::Checked => left.checked_sub(right).ok_or(SumOverflow),
            Self::Wrapping => Ok(left.wrapping_sub(right)),
        }
    }
}

struct Journal {
    inputs: usize,
    groups: usize,
}

/// Keyed signed-64-bit sums with one captured `(key, value)` per input ID.
///
/// [`Self::new`] checks arithmetic; [`Self::wrapping`] uses modular i64 arithmetic.
/// Unlike `SumCollector`, this node accepts removals by ID, so it needs no second
/// value-frequency index. Equal values and zero values remain separate captures.
/// A nonempty zero-sum group emits zero; only its last removal retracts the group.
/// Group inspection and output are ordered by key. Stable keys/IDs and
/// non-panicking destructors are required.
///
/// A single `apply` publishes immediately. `apply_batch` retains the first ID
/// representative and final value of repeated IDs, visits those IDs in sorted
/// order, and publishes one final value per affected key. Existing key
/// representatives survive temporary emptiness within a batch. New groups use
/// the first final key encountered in ID order. Equal totals still emit a put.
///
/// Checked insertion uses total+value, retraction uses total-value. A same-key
/// replacement checks new-old, then total+difference, including both intermediate
/// overflows. Other key transfers remove then insert. Batch arithmetic follows
/// sorted final-ID order; final representability alone does not guarantee success.
/// A checked graph rebuild also requires representable insertion prefixes in
/// source-ID order, as with the existing checked collector.
/// `try_` methods return `SumOverflow` without modification; other methods panic
/// on that error. Count overflow panics before installation. Ordinary key/ID
/// comparison or Clone panics leave this node unchanged, including installation.
/// Cloning shares immutable state; later updates isolate the clone.
pub struct SumGroupNode<I, K> {
    inputs: GroupInputs<I, (K, i64)>,
    groups: SnapshotMap<K, SumState>,
    arithmetic: Arithmetic,
}

impl<I, K> Clone for SumGroupNode<I, K> {
    fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.clone(),
            groups: self.groups.clone(),
            arithmetic: self.arithmetic,
        }
    }
}

impl<I, K> Default for SumGroupNode<I, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, K> SumGroupNode<I, K> {
    /// Empty checked signed-i64 aggregation.
    pub fn new() -> Self {
        Self {
            inputs: GroupInputs::new(),
            groups: SnapshotMap::new(),
            arithmetic: Arithmetic::Checked,
        }
    }

    /// Empty aggregation with wrapping signed-i64 addition/subtraction.
    pub fn wrapping() -> Self {
        Self {
            arithmetic: Arithmetic::Wrapping,
            ..Self::new()
        }
    }

    /// Remove all captured inputs and groups, preserving an established binding.
    pub fn clear(&mut self) {
        self.inputs.clear();
        self.groups.clear();
    }

    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        K: 'static,
    {
        Box::new((self.inputs.clone(), self.groups.clone()))
    }

    pub(crate) fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        Box::new(Journal {
            inputs: self.inputs.begin_journal(),
            groups: self.groups.begin_journal(),
        })
    }

    pub(crate) fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved.downcast::<Journal>().expect("sum group journal");
        self.inputs.commit_journal(saved.inputs);
        self.groups.commit_journal(saved.groups);
    }

    pub(crate) fn restore(&mut self, saved: Box<dyn Any>)
    where
        I: 'static,
        K: 'static,
    {
        match saved.downcast::<Journal>() {
            Ok(saved) => {
                self.inputs.rollback_journal(saved.inputs);
                self.groups.rollback_journal(saved.groups);
            }
            Err(saved) => {
                let (inputs, groups) = *saved
                    .downcast::<(GroupInputs<I, (K, i64)>, SnapshotMap<K, SumState>)>()
                    .expect("sum group snapshot");
                self.inputs.install_snapshot(inputs);
                self.groups.install_snapshot(groups);
            }
        }
    }
}

impl<I: Ord, K: Ord> SumGroupNode<I, K> {
    /// Inspect the captured value at an input ID without recomputing its mapping.
    pub fn input(&self, id: &I) -> Option<&(K, i64)> {
        self.inputs.get(id)
    }

    /// Inspect all nonempty groups, including those with total zero.
    pub fn groups(&self) -> impl Iterator<Item = (&K, i64)> {
        self.groups.iter().map(|(key, state)| (key, state.total))
    }
}

impl<I: Ord + Clone, K: Ord + Clone> SumGroupNode<I, K> {
    /// Apply and publish one input immediately. Panics on checked sum overflow.
    pub fn apply(&mut self, change: TupleChange<I, (K, i64)>) -> Vec<TupleChange<K, i64>> {
        self.try_apply(change).expect("sum group overflow")
    }

    /// Apply one input atomically; a missing-ID removal is an empty no-op.
    pub fn try_apply(
        &mut self,
        change: TupleChange<I, (K, i64)>,
    ) -> Result<Vec<TupleChange<K, i64>>, SumOverflow> {
        self.try_apply_sorted_batch(vec![(change.id, change.value)])
    }

    /// Apply an atomic final-publication round. Panics on checked sum overflow.
    pub fn apply_batch(
        &mut self,
        changes: impl IntoIterator<Item = TupleChange<I, (K, i64)>>,
    ) -> Vec<TupleChange<K, i64>> {
        self.try_apply_batch(changes).expect("sum group overflow")
    }

    /// Coalesce final inputs before checked arithmetic and group publication.
    pub fn try_apply_batch(
        &mut self,
        changes: impl IntoIterator<Item = TupleChange<I, (K, i64)>>,
    ) -> Result<Vec<TupleChange<K, i64>>, SumOverflow> {
        self.try_apply_sorted_batch(join_batch::last_updates(changes))
    }

    // Graph mapping already coalesced borrowed payloads in input-ID order.
    pub(crate) fn try_apply_sorted_batch(
        &mut self,
        changes: Vec<(I, Option<(K, i64)>)>,
    ) -> Result<Vec<TupleChange<K, i64>>, SumOverflow> {
        let mut groups = BTreeMap::<K, SumState>::new();
        let prepare = |groups: &mut BTreeMap<K, SumState>, key: &K| {
            if !groups.contains_key(key) {
                let (representative, state) = self
                    .groups
                    .get_key_value(key)
                    .map(|(key, state)| (key.clone(), *state))
                    .unwrap_or_else(|| (key.clone(), SumState::default()));
                groups.insert(representative, state);
            }
        };
        for (id, new) in &changes {
            let old = self.inputs.get(id);
            if let (Some((old_key, old_value)), Some((key, value))) = (old, new)
                && old_key == key
            {
                prepare(&mut groups, old_key);
                let state = groups.get_mut(old_key).expect("captured sum group");
                assert_ne!(state.count, 0, "captured sum occurrence");
                let difference = self.arithmetic.sub(*value, *old_value)?;
                state.total = self.arithmetic.add(state.total, difference)?;
                continue;
            }
            if let Some((key, value)) = old {
                prepare(&mut groups, key);
                let state = groups.get_mut(key).expect("captured sum group");
                assert_ne!(state.count, 0, "captured sum occurrence");
                state.total = self.arithmetic.sub(state.total, *value)?;
                state.count -= 1;
            }
            if let Some((key, value)) = new {
                prepare(&mut groups, key);
                let state = groups.get_mut(key).expect("prepared sum group");
                let count = state
                    .count
                    .checked_add(1)
                    .expect("sum group count overflow");
                state.total = self.arithmetic.add(state.total, *value)?;
                state.count = count;
            }
        }
        // All user comparisons and output-key clones finish before installation.
        let output = groups
            .iter()
            .filter_map(|(key, state)| {
                if state.count == 0 && !self.groups.contains_key(key) {
                    None
                } else {
                    Some(TupleChange {
                        id: key.clone(),
                        value: (state.count != 0).then_some(state.total),
                    })
                }
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
        for (key, state) in groups {
            if state.count == 0 {
                summaries.remove(&key);
            } else {
                summaries.insert(key, state);
            }
        }
        inputs.commit();
        summaries.commit();
        Ok(output)
    }
}

impl<I: 'static, K> SumGroupNode<I, K> {
    pub(crate) fn bind_fixed_inputs(&mut self, count: usize) -> bool {
        self.inputs.bind_fixed(count)
    }
}

#[cfg(test)]
mod tests;
