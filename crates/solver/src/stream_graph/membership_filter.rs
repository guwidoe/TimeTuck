//! Ordinary graph filtering needs cached acceptance only. Public standalone
//! FilterNode row inspection and all observable destruction paths keep owners.
use super::*;
use crate::primitive_rows::membership::{Capture, Membership};
use std::any::TypeId;

#[cfg(test)]
mod tests;

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
    Frame(usize),
    Full(Membership<I>),
    Delta(Capture),
}
pub(super) struct MembershipFilter<'a, I, T> {
    bits: Membership<I>,
    predicate: Box<dyn Fn(&T) -> bool + 'a>,
    bounds: Bounds<I>,
    rejected_universe_incomplete: bool,
    poisoned: bool,
}
impl<'a, I, T> MembershipFilter<'a, I, T> {
    pub(super) fn new(bits: Membership<I>, predicate: impl Fn(&T) -> bool + 'a) -> Self {
        Self {
            bits,
            predicate: Box::new(predicate),
            bounds: Bounds::Open,
            rejected_universe_incomplete: false,
            poisoned: false,
        }
    }
}
impl<I: Ord + Clone + 'static, T: Clone + 'static> MembershipFilter<'_, I, T> {
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
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore> Node<S>
    for MembershipFilter<'_, I, T>
{
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
        Box::new(Saved::<I>::Frame(self.bits.begin()))
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        if let Saved::Frame(token) = *saved
            .downcast::<Saved<I>>()
            .expect("membership filter checkpoint")
        {
            self.bits.commit(token);
        }
    }
    fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any> {
        assert!(
            !self.poisoned,
            "incremental stream node is poisoned; rebuild the pipeline"
        );
        if let Some(Saved::Frame(token)) = original.downcast_ref::<Saved<I>>() {
            Box::new(Saved::<I>::Delta(self.bits.capture(*token)))
        } else {
            <Self as Node<S>>::checkpoint(self)
        }
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match *saved
            .downcast::<Saved<I>>()
            .expect("membership filter checkpoint")
        {
            Saved::Frame(token) => self.bits.rollback(token),
            Saved::Full(snapshot) => self.bits.install(snapshot),
            Saved::Delta(capture) => self.bits.install_capture(capture),
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
