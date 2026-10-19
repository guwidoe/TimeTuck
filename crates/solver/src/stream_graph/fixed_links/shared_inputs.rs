//! Opt-in fixed links retaining complete shared immutable input events.
use super::*;

type SharedLinkSnapshot<I, J, L, R> = (
    SlotSnapshot<TupleChange<I, L>>,
    SlotSnapshot<TupleChange<J, R>>,
);

struct SharedFixedLinks<'a, E, I, J, L, R, U> {
    links: Vec<LinkId<E, I, J>>,
    endpoints: Vec<(usize, usize)>,
    by_left: BTreeMap<I, usize>,
    by_right: BTreeMap<J, usize>,
    direct_left: bool,
    direct_right: bool,
    left_links: Vec<Vec<usize>>,
    right_links: Vec<Vec<usize>>,
    left: Slots<TupleChange<I, L>>,
    right: Slots<TupleChange<J, R>>,
    checkpoint_box: CheckpointBox<LinkCheckpoint>,
    // Batch scratch holds only input ordinals and immutable relationship slots.
    // Retain capacities across trials; snapshots need only logical row state.
    left_changes: Vec<(usize, usize)>,
    right_changes: Vec<(usize, usize)>,
    affected: Vec<usize>,
    // Current input positions by immutable endpoint slot. Clear only the slots
    // recorded by the preceding successful indexing pass before reusing changes.
    left_inputs: Vec<usize>,
    right_inputs: Vec<usize>,
    inputs_indexed: bool,
    predicate: Box<Predicate<'a, E, L, R>>,
    projection: Box<Projection<'a, E, L, R, U>>,
}

impl<
    E: Ord + Clone + 'static,
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: 'static,
    R: 'static,
    U: 'static,
    S: AdditiveScore,
> Node<S> for SharedFixedLinks<'_, E, I, J, L, R, U>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((self.left.snapshot(), self.right.snapshot()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        let checkpoint = LinkCheckpoint {
            left: self.left.begin(),
            right: self.right.begin(),
        };
        self.checkpoint_box.begin(checkpoint)
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let (saved, empty) =
            CheckpointBox::<LinkCheckpoint>::unpack(saved).expect("fixed-link checkpoint");
        self.left.commit(saved.left);
        self.right.commit(saved.right);
        self.checkpoint_box.recycle(empty);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<LinkCheckpoint>::unpack(saved) {
            Ok((saved, empty)) => {
                self.left.rollback(saved.left);
                self.right.rollback(saved.right);
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => {
                let (left, right) = *saved
                    .downcast::<SharedLinkSnapshot<I, J, L, R>>()
                    .expect("fixed-link snapshot");
                self.left.install(left);
                self.right.install(right);
            }
        }
    }
    fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        <Self as Node<S>>::apply_batch(self, &[(right, event.clone())], arithmetic)
            .map(Outputs::from)
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        // Invalidate positions before latest_slots can clear/rewrite its scratch
        // or panic in user ordering. A failed indexing pass leaves both arrays
        // empty; callback/output-ID-clone failures retain valid cleanup slot lists.
        if self.inputs_indexed {
            for &(slot, _) in &self.left_changes {
                self.left_inputs[slot] = usize::MAX;
            }
            for &(slot, _) in &self.right_changes {
                self.right_inputs[slot] = usize::MAX;
            }
            self.inputs_indexed = false;
        }
        latest_slots::<I, L>(
            inputs,
            false,
            &self.by_left,
            self.direct_left,
            &mut self.left_changes,
        );
        latest_slots::<J, R>(
            inputs,
            true,
            &self.by_right,
            self.direct_right,
            &mut self.right_changes,
        );
        // Both changed-slot lists are now valid cleanup lists, including if
        // affected-link scratch growth fails before every position is filled.
        self.inputs_indexed = true;
        self.affected.clear();
        for &(i, input) in &self.left_changes {
            self.left_inputs[i] = input;
            self.affected.extend_from_slice(&self.left_links[i]);
        }
        for &(j, input) in &self.right_changes {
            self.right_inputs[j] = input;
            self.affected.extend_from_slice(&self.right_links[j]);
        }
        self.affected.sort_unstable();
        self.affected.dedup();
        let mut out = Vec::with_capacity(self.affected.len());
        for &index in &self.affected {
            let edge = &self.links[index].0.0;
            let (i, j) = self.endpoints[index];
            let old_l = self.left.get(i).and_then(|row| row.value.as_ref());
            let old_r = self.right.get(j).and_then(|row| row.value.as_ref());
            let value = after::<I, L>(self.left_inputs[i], inputs, old_l)
                .zip(after::<J, R>(self.right_inputs[j], inputs, old_r))
                .and_then(|(l, r)| {
                    (self.predicate)(edge, l, r).then(|| (self.projection)(edge, l, r))
                });
            let retract = value.is_none()
                && old_l
                    .zip(old_r)
                    .is_some_and(|(l, r)| (self.predicate)(edge, l, r));
            if value.is_some() || retract {
                out.push(TupleChange {
                    id: self.links[index].clone(),
                    value,
                });
            }
        }
        // Complete callbacks/output IDs before retaining input owners. Rc clones
        // and checked Any downcasts share each producer allocation; no L/R Clone
        // or fresh per-endpoint row allocation occurs in this installation path.
        for &(index, input) in &self.left_changes {
            let row = inputs[input]
                .1
                .clone()
                .downcast::<TupleChange<I, L>>()
                .expect("shared fixed-link left event");
            self.left
                .set_shared(index, row.value.is_some().then_some(row));
        }
        for &(index, input) in &self.right_changes {
            let row = inputs[input]
                .1
                .clone()
                .downcast::<TupleChange<J, R>>()
                .expect("shared fixed-link right event");
            self.right
                .set_shared(index, row.value.is_some().then_some(row));
        }
        Ok(erase(out))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Explicit shared-input counterpart of `join_fixed_links_batched`.
    ///
    /// Each linked endpoint retains its complete immutable producer TupleChange,
    /// including the input ID, by sharing that event's Rc owner. This declaration
    /// requires neither endpoint payload to implement Clone. Endpoint installation
    /// does not clone L/R or allocate a separate Rc row owner; source operators,
    /// output publication, journals and snapshots keep their ordinary costs.
    ///
    /// This changes ownership observables: endpoint payload Clone calls from the
    /// ordinary fixed-link cache are omitted. Complete input IDs and payloads can
    /// remain alive in rows, undo journals and saved candidates until their last
    /// shared event owner is released. Their Clone/Drop counts and release order
    /// may differ from the ordinary declaration. No old ownership trace is promised.
    /// IDs and payload contents must stay logically immutable, including through
    /// shared interiors, and callbacks must be pure. Full source facts remain in
    /// upstream nodes; sharing does not make their retention optional.
    ///
    /// Fixed links must have unique relationship IDs; parallel/self links are
    /// allowed. Missing/retracted inputs produce no output. Both sides coalesce
    /// each batch to the final event per ID, then publish affected relationships
    /// once in sorted relationship-ID order. Predicates/projection, retractions,
    /// direct updates, activity replay, nested journals and full saved states use
    /// ordinary graph semantics. Rebuild the graph to change relationship topology.
    pub fn join_fixed_links_batched_shared<
        E: Ord + Clone + 'static,
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: 'static,
        R: 'static,
        U: 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        links: impl IntoIterator<Item = (E, I, J)>,
        predicate: impl Fn(&E, &L, &R) -> bool + 'a,
        projection: impl Fn(&E, &L, &R) -> U + 'a,
    ) -> Result<FixedLinkedStreamPort<E, I, J, U>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let mut links: Vec<_> = links.into_iter().map(|(e, i, j)| ((e, i), j)).collect();
        links.sort_by(|a, b| a.0.0.cmp(&b.0.0));
        if links.windows(2).any(|w| w[0].0.0 == w[1].0.0) {
            return Err(GraphError::Dependency(
                "duplicate fixed relationship ID".into(),
            ));
        }
        // Slots follow key order so coalesced source changes remain sorted by slot.
        let left_keys: BTreeSet<_> = links.iter().map(|((_, i), _)| i.clone()).collect();
        let right_keys: BTreeSet<_> = links.iter().map(|(_, j)| j.clone()).collect();
        let by_left: BTreeMap<_, _> = left_keys
            .into_iter()
            .enumerate()
            .map(|(n, i)| (i, n))
            .collect();
        let by_right: BTreeMap<_, _> = right_keys
            .into_iter()
            .enumerate()
            .map(|(n, j)| (j, n))
            .collect();
        let mut left_links = vec![Vec::new(); by_left.len()];
        let mut right_links = vec![Vec::new(); by_right.len()];
        let endpoints: Vec<_> = links
            .iter()
            .enumerate()
            .map(|(n, ((_, i), j))| {
                let (i, j) = (by_left[i], by_right[j]);
                left_links[i].push(n);
                right_links[j].push(n);
                (i, j)
            })
            .collect();
        let out = self.push(SharedFixedLinks {
            left: Slots::new(by_left.len()),
            right: Slots::new(by_right.len()),
            checkpoint_box: CheckpointBox::new(),
            left_changes: Vec::new(),
            right_changes: Vec::new(),
            affected: Vec::new(),
            left_inputs: vec![usize::MAX; by_left.len()],
            right_inputs: vec![usize::MAX; by_right.len()],
            inputs_indexed: false,
            links,
            endpoints,
            direct_left: contiguous_slots(&by_left),
            direct_right: contiguous_slots(&by_right),
            by_left,
            by_right,
            left_links,
            right_links,
            predicate: Box::new(predicate),
            projection: Box::new(projection),
        });
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}

#[cfg(test)]
mod tests;

mod fused_score;
