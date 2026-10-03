//! Incremental one-to-many expansion with positional occurrence identities.
use crate::{SnapshotMap, TupleChange};
use std::any::Any;

type Expansion<'a, T, U> = dyn Fn(&T) -> Vec<U> + 'a;

/// Expands each input into zero or more rows, preserving duplicates and iterator
/// order. Each child is identified by `(input_id, zero_based_position)`.
/// Replacing an input replaces its surviving positions and retracts its old tail.
/// Retraction never invokes the mapping; retracting an absent input is a no-op.
///
/// Mapping, iterator collection, output creation and persistent state preparation
/// precede installation. Ordinary callback, comparison and clone panics preserve
/// this node. External callback side effects cannot be rolled back. The iterator
/// must be finite and deterministic; IDs must have stable ordering.
///
/// Only expansion lengths are cached here. Downstream nodes capture payloads.
/// Updates cost O(old length + new length + log(input count)); checkpoints share
/// persistent roots. Positional IDs are not stable value identities across reorder.
///
/// ```
/// use timetuck::{FlatMapNode, TupleChange};
/// let mut node = FlatMapNode::new(|n: &usize| 0..*n);
/// assert_eq!(node.apply(TupleChange::put(7, 2)), vec![
///     TupleChange::put((7, 0), 0), TupleChange::put((7, 1), 1)]);
/// assert_eq!(node.apply(TupleChange::put(7, 1)), vec![
///     TupleChange::retract((7, 1)), TupleChange::put((7, 0), 0)]);
/// ```
pub struct FlatMapNode<'a, I, T, U> {
    lengths: SnapshotMap<I, usize>,
    mapping: Box<Expansion<'a, T, U>>,
}
impl<'a, I: Ord + Clone, T, U> FlatMapNode<'a, I, T, U> {
    pub fn new<R: IntoIterator<Item = U>>(mapping: impl Fn(&T) -> R + 'a) -> Self {
        Self {
            lengths: SnapshotMap::new(),
            mapping: Box::new(move |value| mapping(value).into_iter().collect()),
        }
    }
    pub fn apply(&mut self, change: TupleChange<I, T>) -> Vec<TupleChange<(I, usize), U>> {
        let rows = change.value.as_ref().map(|value| (self.mapping)(value));
        let old_length = self.lengths.get(&change.id).copied().unwrap_or(0);
        let new_length = rows.as_ref().map_or(0, Vec::len);
        let mut output = Vec::new();
        for position in new_length..old_length {
            output.push(TupleChange::retract((change.id.clone(), position)));
        }
        if let Some(rows) = rows {
            for (position, row) in rows.into_iter().enumerate() {
                output.push(TupleChange::put((change.id.clone(), position), row));
            }
        }
        let mut lengths = self.lengths.clone();
        if change.value.is_some() {
            lengths.insert(change.id, new_length);
        } else {
            lengths.remove(&change.id);
        }
        self.lengths = lengths;
        output
    }
    pub(crate) fn clear(&mut self) {
        self.lengths.clear();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
    {
        Box::new(self.lengths.clone())
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
    {
        self.lengths = *checkpoint.downcast().expect("flat-map checkpoint type");
    }
}
