//! Ordinary graph filtering needs cached acceptance only. Public standalone
//! FilterNode row inspection and all observable destruction paths keep owners.
use super::*;
use crate::primitive_rows::membership::{Capture, Membership};
use std::any::TypeId;

#[cfg(test)]
mod tests;

// Exact Copy rows only. Same reuses the producer's existing private arena cell;
// Retract is needed when a previously accepted put becomes rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InlineForward {
    None,
    Same,
    Retract,
}
fn inline_shape<I: 'static, T: 'static>() -> bool {
    type Triple = (usize, usize, usize);
    (TypeId::of::<I>() == TypeId::of::<usize>()
        && (TypeId::of::<T>() == TypeId::of::<Triple>()
            || TypeId::of::<T>() == TypeId::of::<(usize, Option<usize>)>()))
        || (TypeId::of::<I>() == TypeId::of::<(usize, usize)>()
            && TypeId::of::<T>() == TypeId::of::<(Triple, Triple)>())
}

enum Bounds<I> {
    Open,
    Fixed {
        count: usize,
        index: fn(&I) -> usize,
    },
    Pair {
        left: usize,
        right: usize,
        index: fn(&I) -> (usize, usize),
    },
}
impl<I> Bounds<I> {
    fn validate(&self, id: &I) {
        match self {
            Self::Open => {}
            Self::Fixed { count, index } => {
                assert!(index(id) < *count, "validated bound join row ID")
            }
            Self::Pair { left, right, index } => {
                let (a, b) = index(id);
                assert!(a < *left && b < *right, "validated bound pair row ID");
            }
        }
    }
}
enum Saved<I> {
    Full(Membership<I>),
    Delta(Capture),
}
pub(super) struct MembershipFilter<'a, I, T, F = Box<dyn Fn(&T) -> bool + 'a>> {
    bits: Membership<I>,
    checkpoint_box: CheckpointBox<usize>,
    predicate: F,
    marker: PhantomData<&'a T>,
    bounds: Bounds<I>,
    rejected_universe_incomplete: bool,
    poisoned: bool,
}
#[cfg(test)]
impl<'a, I, T: 'a> MembershipFilter<'a, I, T> {
    pub(super) fn new(bits: Membership<I>, predicate: impl Fn(&T) -> bool + 'a) -> Self {
        Self::new_inline(bits, Box::new(predicate))
    }
}
impl<'a, I, T, F: Fn(&T) -> bool + 'a> MembershipFilter<'a, I, T, F> {
    // Keep the callback's concrete type through the Node vtable, allowing its
    // body to inline in apply_row without a second erased Fn dispatch.
    pub(super) fn new_inline(bits: Membership<I>, predicate: F) -> Self {
        Self {
            bits,
            checkpoint_box: CheckpointBox::new(),
            predicate,
            marker: PhantomData,
            bounds: Bounds::Open,
            rejected_universe_incomplete: false,
            poisoned: false,
        }
    }
}
impl<I: Ord + Clone + 'static, T: Clone + 'static, F: Fn(&T) -> bool>
    MembershipFilter<'_, I, T, F>
{
    fn apply_row(&mut self, event: &Event) -> Option<TupleChange<I, T>> {
        // The ordinary graph adapter clones the delivered tuple before entering
        // FilterNode::apply (including before its poison assertion).
        let change = typed::<I, T>(event).clone();
        assert!(
            !self.poisoned,
            "incremental stream node is poisoned; rebuild the pipeline"
        );
        let new = change.value.map(|row| ((self.predicate)(&row), row));
        let (prepared, was_accepted) = self.bits.prepare(&change.id);
        let output = match &new {
            Some((true, row)) => Some(TupleChange::put(change.id.clone(), row.clone())),
            _ if was_accepted => Some(TupleChange::retract(change.id.clone())),
            _ => None,
        };
        self.poisoned = true;
        if new.is_some() {
            self.bounds.validate(&change.id);
        }
        let accepted = matches!(&new, Some((true, _)));
        if matches!(&new, Some((false, _))) && matches!(self.bounds, Bounds::Open) {
            self.rejected_universe_incomplete = true;
        }
        prepared.set(accepted);
        self.poisoned = false;
        output
    }
    fn bind_fixed(&mut self, count: usize) -> bool {
        if self.rejected_universe_incomplete
            || !matches!(self.bounds, Bounds::Open)
            || TypeId::of::<I>() != TypeId::of::<usize>()
            || count > 4096
            || !self.bits.complete_fixed(count)
        {
            return false;
        }
        fn index<I: 'static>(id: &I) -> usize {
            *(id as &dyn Any)
                .downcast_ref::<usize>()
                .expect("bound usize ID")
        }
        self.bounds = Bounds::Fixed {
            count,
            index: index::<I>,
        };
        true
    }
    fn bind_pair(&mut self, left: usize, right: usize) -> bool {
        if self.rejected_universe_incomplete
            || !matches!(self.bounds, Bounds::Open)
            || TypeId::of::<I>() != TypeId::of::<(usize, usize)>()
            || left
                .checked_mul(right)
                .is_none_or(|slots| slots > 1_048_576)
            || !self.bits.fits_pair(left, right)
        {
            return false;
        }
        fn index<I: 'static>(id: &I) -> (usize, usize) {
            *(id as &dyn Any)
                .downcast_ref::<(usize, usize)>()
                .expect("bound pair ID")
        }
        self.bounds = Bounds::Pair {
            left,
            right,
            index: index::<I>,
        };
        true
    }
}
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore, F: Fn(&T) -> bool> Node<S>
    for MembershipFilter<'_, I, T, F>
{
    fn has_inline_forward(&self) -> bool {
        inline_shape::<I, T>()
    }
    fn apply_inline_forward(&mut self, event: &Event) -> InlineForward {
        debug_assert!(inline_shape::<I, T>());
        // Share the unchanged original algorithm, including output preparation,
        // poison/bounds/journal order and all predicate calls. Only exact Copy
        // tuple output representation is reduced after complete apply returns.
        match self.apply_row(event) {
            None => InlineForward::None,
            Some(row) if row.value.is_some() || typed::<I, T>(event).value.is_none() => {
                InlineForward::Same
            }
            Some(_) => InlineForward::Retract,
        }
    }
    fn preserves_input_ids(&self) -> bool {
        true
    }
    fn bind_fixed_input(&mut self, right: bool, count: usize) {
        if !right {
            self.bind_fixed(count);
        }
    }
    fn bind_fixed_pair_input(&mut self, right: bool, left: usize, right_count: usize) {
        if !right {
            self.bind_pair(left, right_count);
        }
    }
    fn clear(&mut self) {
        self.bits.clear();
        self.poisoned = false;
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        assert!(
            !self.poisoned,
            "incremental stream node is poisoned; rebuild the pipeline"
        );
        Box::new(Saved::Full(self.bits.clone()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        assert!(
            !self.poisoned,
            "incremental stream node is poisoned; rebuild the pipeline"
        );
        let token = self.bits.begin();
        self.checkpoint_box.begin(token)
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<usize>::unpack(saved) {
            Ok((token, empty)) => {
                self.bits.commit(token);
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => {
                drop(
                    saved
                        .downcast::<Saved<I>>()
                        .expect("membership filter checkpoint"),
                );
            }
        }
    }
    fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any> {
        assert!(
            !self.poisoned,
            "incremental stream node is poisoned; rebuild the pipeline"
        );
        if let Some(token) = CheckpointBox::<usize>::get(original) {
            Box::new(Saved::<I>::Delta(self.bits.capture(*token)))
        } else {
            <Self as Node<S>>::checkpoint(self)
        }
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<usize>::unpack(saved) {
            Ok((token, empty)) => {
                self.bits.rollback(token);
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => match *saved
                .downcast::<Saved<I>>()
                .expect("membership filter checkpoint")
            {
                Saved::Full(snapshot) => self.bits.install(snapshot),
                Saved::Delta(capture) => self.bits.install_capture(capture),
            },
        }
        self.poisoned = false;
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        Ok(erase(self.apply_row(event)))
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        Ok(erase(
            inputs.iter().filter_map(|(_, event)| self.apply_row(event)),
        ))
    }
}

#[cfg(test)]
mod concrete_callback_tests;

#[cfg(test)]
mod inline_forward_tests;
