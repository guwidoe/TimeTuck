//! Persistent duplicate elimination with deterministic representatives.
use crate::{SnapshotMap, TupleChange};
use std::any::Any;

type Groups<I, T> = SnapshotMap<T, SnapshotMap<I, T>>;

/// One output per equivalence class under `T::cmp`. Output identity is the value
/// class; output payload is the occurrence with the lowest input ID. IDs and
/// ordering must be stable, with equality consistent with ordering. Include a
/// stable fact ID in `T` when otherwise equal facts must remain distinct.
///
/// Equal values may carry different non-key data. Changing the representative's
/// payload or removing it emits an update to the surviving class. Adding/removing
/// a nonrepresentative duplicate emits nothing. Retraction of the last occurrence
/// emits a removal. The input value is captured, so removal needs only its ID.
///
/// State and output are prepared before installation; ordinary clone/ordering
/// panics leave the node unchanged. Persistent checkpoints share roots. Storage
/// is linear in occurrences and classes, with logarithmic map updates.
/// Panicking destructors and interior mutation have the usual transaction limits.
///
/// ```
/// use timetuck::{DistinctNode, TupleChange};
/// let mut node = DistinctNode::<usize, &str>::new();
/// assert_eq!(node.apply(TupleChange::put(0, "A")),
///     vec![TupleChange::put("A", "A")]);
/// assert!(node.apply(TupleChange::put(1, "A")).is_empty());
/// assert_eq!(node.occurrences(&"A"), 2);
/// assert!(node.apply(TupleChange::retract(1)).is_empty());
/// assert_eq!(node.apply(TupleChange::retract(0)),
///     vec![TupleChange::retract("A")]);
/// ```
pub struct DistinctNode<I, T> {
    inputs: SnapshotMap<I, T>,
    groups: Groups<I, T>,
}
impl<I: Ord + Clone, T: Ord + Clone> Default for DistinctNode<I, T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<I: Ord + Clone, T: Ord + Clone> DistinctNode<I, T> {
    pub fn new() -> Self {
        Self {
            inputs: SnapshotMap::new(),
            groups: SnapshotMap::new(),
        }
    }
    pub fn rows(&self) -> impl Iterator<Item = (&T, &T)> {
        self.groups
            .iter()
            .map(|(key, members)| (key, members.first_key_value().expect("nonempty class").1))
    }
    pub fn occurrences(&self, value: &T) -> usize {
        self.groups.get(value).map_or(0, SnapshotMap::len)
    }
    pub(crate) fn clear(&mut self) {
        self.inputs.clear();
        self.groups.clear();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        T: 'static,
    {
        Box::new((self.inputs.clone(), self.groups.clone()))
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        T: 'static,
    {
        (self.inputs, self.groups) = *checkpoint.downcast().expect("distinct checkpoint type");
    }
    pub fn apply(&mut self, change: TupleChange<I, T>) -> Vec<TupleChange<T, T>> {
        let old = self.inputs.get(&change.id).cloned();
        let mut affected = Vec::new();
        if let Some(old) = &old {
            affected.push(old.clone());
        }
        if let Some(new) = &change.value
            && old.as_ref() != Some(new)
        {
            affected.push(new.clone());
        }
        let mut inputs = self.inputs.clone();
        let mut groups = self.groups.clone();
        if let Some(old) = &old {
            let mut members = groups[old].clone();
            members.remove(&change.id);
            if members.is_empty() {
                groups.remove(old);
            } else {
                groups.insert(old.clone(), members);
            }
        }
        inputs.remove(&change.id);
        if let Some(new) = &change.value {
            let mut members = groups.get(new).cloned().unwrap_or_default();
            members.insert(change.id.clone(), new.clone());
            groups.insert(new.clone(), members);
            inputs.insert(change.id.clone(), new.clone());
        }
        let mut output = Vec::new();
        for key in affected {
            let before = self.groups.get(&key).and_then(SnapshotMap::first_key_value);
            let after = groups.get(&key).and_then(SnapshotMap::first_key_value);
            match (before, after) {
                (Some(_), None) => output.push(TupleChange::retract(key)),
                (None, Some((_, row))) => output.push(TupleChange::put(key, row.clone())),
                (Some((old_id, _)), Some((new_id, row)))
                    if old_id != new_id || (new_id == &change.id && change.value.is_some()) =>
                {
                    output.push(TupleChange::put(key, row.clone()));
                }
                _ => {}
            }
        }
        self.inputs = inputs;
        self.groups = groups;
        output
    }
}
