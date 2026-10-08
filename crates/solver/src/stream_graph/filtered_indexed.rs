//! Compact filtered joins: final predicates precede pair-payload cloning.
use super::*;

type PairPredicate<'a, L, R> = dyn Fn(&L, &R) -> bool + 'a;
struct FilteredIndexedJoin<'a, I, J, L, R, K> {
    join: JoinNode<'a, I, J, L, R, K>,
    visible: Option<SnapshotMap<(I, J), ()>>,
    predicate: Box<PairPredicate<'a, L, R>>,
}
struct FilteredCheckpoint {
    join: Box<dyn Any>,
    visible: Option<Box<dyn Any>>,
}
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for FilteredIndexedJoin<'_, I, J, L, R, K>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(FilteredCheckpoint {
            join: self.join.checkpoint(),
            visible: self
                .visible
                .as_ref()
                .map(|m| Box::new(m.clone()) as Box<dyn Any>),
        })
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        Box::new(FilteredCheckpoint {
            join: self.join.begin_journal_checkpoint(),
            visible: self.visible.as_mut().map(|m| m.begin_checkpoint()),
        })
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<FilteredCheckpoint>()
            .expect("filtered join checkpoint");
        if let Some(saved) = saved.visible {
            self.visible
                .as_mut()
                .expect("retained membership")
                .commit_checkpoint(saved);
        }
        self.join.commit_journal_checkpoint(saved.join);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<FilteredCheckpoint>()
            .expect("filtered join checkpoint");
        if let Some(saved) = saved.visible {
            self.visible
                .as_mut()
                .expect("retained membership")
                .restore_checkpoint(saved);
        }
        self.join.restore_checkpoint(saved.join);
    }
    fn clear(&mut self) {
        self.join.clear();
        if let Some(visible) = &mut self.visible {
            visible.clear();
        }
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        <Self as Node<S>>::apply_batch(self, &[(right, event.clone())], arithmetic)
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let left = inputs
            .iter()
            .filter(|(right, _)| !right)
            .map(|(_, event)| typed::<I, L>(event).clone());
        let right = inputs
            .iter()
            .filter(|(right, _)| *right)
            .map(|(_, event)| typed::<J, R>(event).clone());
        let Some(visible) = &mut self.visible else {
            let changes = self
                .join
                .apply_batch_recomputed(left, right, &self.predicate);
            return Ok(changes
                .into_iter()
                .map(|change| Rc::new(change) as Event)
                .collect());
        };
        let predicate = &self.predicate;
        let changes = self.join.apply_batch_projected(left, right, |left, right| {
            predicate(left, right).then(|| (left.clone(), right.clone()))
        });
        // Retained mode preserves its once-per-final-pair predicate contract.
        let mut output = Vec::with_capacity(changes.len());
        for change in changes {
            if change.value.is_some() {
                if !visible.contains_key(&change.id) {
                    visible.insert(change.id.clone(), ());
                }
                output.push(Rc::new(change) as Event);
            } else if visible.remove(&change.id) {
                output.push(Rc::new(change) as Event);
            }
        }
        Ok(output)
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Indexed equality/range join with one residual predicate evaluation per
    /// affected final matching pair. Only accepted pair payloads are cloned;
    /// the join retains accepted IDs, input rows and indexes, not pair payloads.
    /// Outputs are ordered by pair ID. Rejected-to-rejected pairs emit nothing;
    /// surviving accepted pairs emit replacements even for unchanged payloads.
    /// Observing this node enables batched planning updates. Direct `apply`
    /// handles one input immediately. Pure callbacks and stable keys are required.
    /// Graph rollback, saved candidates and consistency/activity rebuilds retain
    /// their usual contracts. Dense accepted-ID sets may still be quadratic.
    pub fn join_compare_filtered_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.filtered_indexed_join(
            left, right, comparison, left_key, right_key, predicate, true,
        )
    }
    /// Batched equality/range join retaining only input rows and their indexes,
    /// with no accepted-pair table. Persistent join state is linear in input rows;
    /// output/candidate batches and downstream state may still be quadratic.
    ///
    /// Each final matching affected pair evaluates the predicate once. If it is
    /// rejected or disappears, the predicate may also run on its previously
    /// matching rows to decide whether to retract. Predicates must be pure and
    /// depend solely on row contents. This trades repeated predicate work for
    /// reduced retained state; prefer the retained variant for expensive predicates.
    /// Accepted replacements, output ordering, transactions, saved candidates and
    /// activity/consistency rebuilding follow `join_compare_filtered_batched`.
    pub fn join_compare_recomputed_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.filtered_indexed_join(
            left, right, comparison, left_key, right_key, predicate, false,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn filtered_indexed_join<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
        retained: bool,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(FilteredIndexedJoin {
            join: JoinNode::<I, J, L, R, K>::comparing(comparison, left_key, right_key),
            visible: retained.then(SnapshotMap::new),
            predicate: Box::new(predicate),
        });
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}
