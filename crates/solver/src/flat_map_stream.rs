//! Incremental one-to-many expansion with positional occurrence identities.
use crate::{
    TupleChange,
    primitive_rows::{PrimitiveRows, RowsCapture},
};
use std::any::Any;

type Expansion<'a, T, U> = dyn Fn(&T) -> Vec<U> + 'a;
struct Journal(usize);
struct Capture<I>(RowsCapture<I, usize>);

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
    lengths: PrimitiveRows<I, usize>,
    mapping: Box<Expansion<'a, T, U>>,
    omit_equal_lengths: bool,
}
impl<'a, I: Ord + Clone, T, U> FlatMapNode<'a, I, T, U> {
    pub fn new<R: IntoIterator<Item = U>>(mapping: impl Fn(&T) -> R + 'a) -> Self {
        Self {
            lengths: PrimitiveRows::new(),
            omit_equal_lengths: false,
            mapping: Box::new(move |value| mapping(value).into_iter().collect()),
        }
    }
    pub fn apply(&mut self, change: TupleChange<I, T>) -> Vec<TupleChange<(I, usize), U>> {
        let rows = change.value.as_ref().map(|value| (self.mapping)(value));
        if self.lengths.is_primitive() {
            let omit_equal_lengths = self.omit_equal_lengths;
            let (prepared, old_length) =
                self.lengths.prepare_change(&change.id, |old| old.copied());
            let new_length = rows.as_ref().map_or(0, Vec::len);
            let mut output = Vec::new();
            for position in new_length..old_length.unwrap_or(0) {
                output.push(TupleChange::retract((change.id.clone(), position)));
            }
            if let Some(rows) = rows {
                for (position, row) in rows.into_iter().enumerate() {
                    output.push(TupleChange::put((change.id.clone(), position), row));
                }
            }
            // Mapper/iterator work and every output identity clone complete
            // before publishing the prepared, primitive-ID cache change.
            if change.value.is_some() {
                // Only the private graph cache may omit an existing equal usize
                // write. No public length/value owner or historical version is
                // exposed; downstream events above are still fully regenerated.
                // None differs from Some(0): an empty expansion is present.
                if omit_equal_lengths && old_length == Some(new_length) {
                    return output;
                }
                prepared.insert(change.id, new_length);
            } else {
                prepared.remove();
            }
            return output;
        }
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
        // Standalone, borrowed and custom IDs keep their staged map update,
        // including the original comparison/Clone/panic preparation order.
        let mut lengths = self.lengths.clone();
        if change.value.is_some() {
            lengths.insert(change.id, new_length);
        } else {
            lengths.remove(&change.id);
        }
        self.lengths = lengths;
        output
    }
    pub(crate) fn bind_primitive_inputs(&mut self) -> bool
    where
        I: 'static,
    {
        self.lengths.bind_primitive()
    }
    pub(crate) fn bind_graph_inputs(&mut self) -> bool
    where
        I: 'static,
    {
        self.omit_equal_lengths = self.bind_primitive_inputs();
        self.omit_equal_lengths
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
    pub(crate) fn begin_checkpoint(&mut self) -> Box<dyn Any>
    where
        I: 'static,
    {
        if self.lengths.is_primitive() {
            Box::new(Journal(self.lengths.begin_journal()))
        } else {
            self.checkpoint()
        }
    }
    pub(crate) fn commit_checkpoint(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
    {
        if let Ok(saved) = checkpoint.downcast::<Journal>() {
            self.lengths.commit_journal(saved.0);
        }
    }
    pub(crate) fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any>
    where
        I: 'static,
    {
        if let Some(saved) = original.downcast_ref::<Journal>() {
            Box::new(Capture(self.lengths.capture_current(saved.0)))
        } else {
            self.checkpoint()
        }
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
    {
        match checkpoint.downcast::<Journal>() {
            Ok(saved) => self.lengths.rollback_journal(saved.0),
            Err(checkpoint) => match checkpoint.downcast::<Capture<I>>() {
                Ok(saved) => self.lengths.install_capture(saved.0),
                Err(checkpoint) => {
                    let saved = *checkpoint
                        .downcast::<PrimitiveRows<I, usize>>()
                        .expect("flat-map checkpoint type");
                    self.lengths.install_snapshot(saved);
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod equal_length_tests;
