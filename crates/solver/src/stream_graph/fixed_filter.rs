//! Explicit fixed-universe filtering without retained payload owners.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Membership {
    Absent,
    Rejected,
    Accepted,
}
struct Full(Vec<Membership>);
struct Frame(usize);
struct FixedFilter<'a, I, T> {
    ids: Vec<I>,
    direct: bool,
    rows: Vec<Membership>,
    undo: Vec<(usize, Membership)>,
    frames: Vec<usize>,
    checkpoint_box: CheckpointBox<Frame>,
    predicate: Box<dyn Fn(&T) -> bool + 'a>,
}
impl<'a, I: Ord + 'static, T> FixedFilter<'a, I, T> {
    fn new(mut ids: Vec<I>, predicate: impl Fn(&T) -> bool + 'a) -> Result<Self, GraphError> {
        ids.sort();
        if ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(GraphError::Dependency("duplicate fixed filter ID".into()));
        }
        let direct = std::any::TypeId::of::<I>() == std::any::TypeId::of::<usize>()
            && ids
                .iter()
                .enumerate()
                .all(|(slot, id)| (id as &dyn Any).downcast_ref::<usize>() == Some(&slot));
        Ok(Self {
            rows: vec![Membership::Absent; ids.len()],
            ids,
            direct,
            undo: Vec::new(),
            frames: Vec::new(),
            checkpoint_box: CheckpointBox::new(),
            predicate: Box::new(predicate),
        })
    }
    fn position(&self, id: &I) -> Result<usize, GraphError> {
        let slot = if self.direct {
            let slot = *(id as &dyn Any)
                .downcast_ref::<usize>()
                .expect("usize universe");
            (slot < self.ids.len()).then_some(slot)
        } else {
            self.ids.binary_search(id).ok()
        };
        slot.ok_or_else(|| {
            GraphError::Dependency("occurrence ID outside fixed filter universe".into())
        })
    }
    fn set(&mut self, slot: usize, value: Membership) {
        if self.rows[slot] != value {
            if !self.frames.is_empty() {
                self.undo.push((slot, self.rows[slot]));
            }
            self.rows[slot] = value;
        }
    }
    fn commit(&mut self, Frame(token): Frame) {
        assert_eq!(
            token + 1,
            self.frames.len(),
            "innermost fixed filter journal"
        );
        self.frames.pop();
        if self.frames.is_empty() {
            self.undo.clear();
        }
    }
    fn rollback(&mut self, Frame(token): Frame) {
        assert_eq!(
            token + 1,
            self.frames.len(),
            "innermost fixed filter journal"
        );
        let start = self.frames.pop().expect("fixed filter frame");
        while self.undo.len() > start {
            let (slot, old) = self.undo.pop().expect("fixed filter undo");
            self.rows[slot] = old;
        }
    }
}
impl<I: Ord + Clone + 'static, T: 'static> FixedFilter<'_, I, T> {
    fn apply_row(&mut self, event: &Event) -> Result<Option<Event>, GraphError> {
        let change = typed::<I, T>(event);
        let slot = self.position(&change.id)?;
        let next = match &change.value {
            None => Membership::Absent,
            Some(row) if (self.predicate)(row) => Membership::Accepted,
            Some(_) => Membership::Rejected,
        };
        // Construct any fallible user-ID clone before installing membership.
        let output = if next == Membership::Accepted {
            Some(event.clone())
        } else if self.rows[slot] == Membership::Accepted {
            Some(Event::new(TupleChange::<I, T>::retract(
                self.ids[slot].clone(),
            )))
        } else {
            None
        };
        self.set(slot, next);
        Ok(output)
    }
}
impl<I: Ord + Clone + 'static, T: 'static, S: AdditiveScore> Node<S> for FixedFilter<'_, I, T> {
    fn preserves_input_ids(&self) -> bool {
        true
    }
    fn clear(&mut self) {
        for slot in 0..self.rows.len() {
            self.set(slot, Membership::Absent);
        }
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(Full(self.rows.clone()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        let token = self.frames.len();
        self.frames.push(self.undo.len());
        self.checkpoint_box.begin(Frame(token))
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let (frame, empty) = CheckpointBox::<Frame>::unpack(saved).expect("fixed filter journal");
        self.commit(frame);
        self.checkpoint_box.recycle(empty);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<Frame>::unpack(saved) {
            Ok((frame, empty)) => {
                self.rollback(frame);
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => {
                let Full(rows) = *saved.downcast::<Full>().expect("fixed filter snapshot");
                assert_eq!(rows.len(), self.rows.len(), "fixed filter universe");
                for (slot, value) in rows.into_iter().enumerate() {
                    self.set(slot, value);
                }
            }
        }
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        Ok(self.apply_row(event)?.into_iter().collect())
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        // Reject invalid delivered IDs before predicates or membership installation.
        for (_, event) in inputs {
            self.position(&typed::<I, T>(event).id)?;
        }
        let mut outputs = Vec::new();
        for (_, event) in inputs {
            outputs.extend(self.apply_row(event)?);
        }
        Ok(outputs)
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Filter under a caller-declared immutable universe of possible input IDs.
    /// Duplicate declarations and unknown puts/retracts are errors. Declaration
    /// IDs are sorted and retained as canonical cache keys; stable Ord/Eq and
    /// pure predicates with nonpanicking destructors are required.
    ///
    /// Every delivered put calls the predicate once, every accepted put forwards
    /// the complete original immutable event (including its incoming ID owner).
    /// Rejection/removal retracts an earlier accepted row using a clone of the
    /// canonical declared ID. Absent/rejected removals publish nothing. Batches
    /// retain delivered order and intermediate updates; no final-ID coalescing
    /// is introduced. Existing upstream batch boundaries still apply.
    ///
    /// Payloads need not implement Clone. Complete source facts remain upstream;
    /// only one byte of membership per declared ID is retained here. Full saved
    /// states copy O(universe) bytes; nested trials record changed membership.
    /// Generic/sparse ID lookup is binary search; contiguous usize IDs use direct
    /// indexing. Own/downstream ID Clone/Drop schedules and representatives can
    /// differ from ordinary filter, including snapshot release order. Accepted
    /// puts retain the producer's event owner instead of cloning its payload.
    /// Existing validated source identity may propagate through this unary node;
    /// declaring IDs does not create a source or group binding capability.
    pub fn filter_fixed<I: Ord + Clone + 'static, T: 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        ids: impl IntoIterator<Item = I>,
        predicate: impl Fn(&T) -> bool + 'a,
    ) -> Result<StreamPort<I, T>, GraphError> {
        self.validate(input)?;
        let node = FixedFilter::new(ids.into_iter().collect(), predicate)?;
        let output = self.push(node);
        self.connect(input.node, output.node, false);
        Ok(output)
    }
}

#[cfg(test)]
mod tests;
