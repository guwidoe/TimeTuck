//! Incremental zero-or-one projection under the original input identity.
use crate::{SnapshotMap, TupleChange};
use std::any::Any;

type Mapping<'a, T, U> = dyn Fn(&T) -> Option<U> + 'a;

/// Fuse filtering and mapping without an intermediate tuple or separate graph
/// node. Only accepted identities are retained; input/output payloads need not
/// implement Clone. Rejected replacements retract earlier accepted outputs.
/// Retraction never calls the mapping; absent/rejected identities emit nothing.
/// Callbacks must be pure and deterministic. Output construction and persistent
/// state preparation precede installation, so callback/key panics preserve state.
pub struct FilterMapNode<'a, I, T, U> {
    accepted: SnapshotMap<I, ()>,
    mapping: Box<Mapping<'a, T, U>>,
}
impl<'a, I: Ord + Clone, T, U> FilterMapNode<'a, I, T, U> {
    pub fn new(mapping: impl Fn(&T) -> Option<U> + 'a) -> Self {
        Self {
            accepted: SnapshotMap::new(),
            mapping: Box::new(mapping),
        }
    }

    pub fn accepted_count(&self) -> usize {
        self.accepted.len()
    }

    pub fn apply(&mut self, change: TupleChange<I, T>) -> Option<TupleChange<I, U>> {
        self.apply_ref(&change)
    }

    /// Process a borrowed event without cloning its payload.
    pub fn apply_ref(&mut self, change: &TupleChange<I, T>) -> Option<TupleChange<I, U>> {
        let value = change.value.as_ref().and_then(|row| (self.mapping)(row));
        if value.is_none() && !self.accepted.contains_key(&change.id) {
            return None;
        }
        let output = TupleChange {
            id: change.id.clone(),
            value,
        };
        let mut accepted = self.accepted.clone();
        if output.value.is_some() {
            accepted.insert(change.id.clone(), ());
        } else {
            accepted.remove(&change.id);
        }
        self.accepted = accepted;
        Some(output)
    }

    pub(crate) fn clear(&mut self) {
        self.accepted.clear();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
    {
        Box::new(self.accepted.clone())
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
    {
        self.accepted = *checkpoint.downcast().expect("filter-map checkpoint type");
    }
}
