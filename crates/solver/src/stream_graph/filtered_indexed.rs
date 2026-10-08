//! Compact filtered joins: final predicates precede pair-payload cloning.
use super::*;

type PairPredicate<'a, L, R> = dyn Fn(&L, &R) -> bool + 'a;
struct FilteredIndexedJoin<'a, I, J, L, R, K> {
    join: JoinNode<'a, I, J, L, R, K>,
    visible: SnapshotMap<(I, J), ()>,
    predicate: Box<PairPredicate<'a, L, R>>,
}
struct FilteredCheckpoint {
    join: Box<dyn Any>,
    visible: Box<dyn Any>,
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
            visible: Box::new(self.visible.clone()),
        })
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        Box::new(FilteredCheckpoint {
            join: self.join.begin_journal_checkpoint(),
            visible: self.visible.begin_checkpoint(),
        })
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<FilteredCheckpoint>()
            .expect("filtered join checkpoint");
        self.visible.commit_checkpoint(saved.visible);
        self.join.commit_journal_checkpoint(saved.join);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<FilteredCheckpoint>()
            .expect("filtered join checkpoint");
        self.visible.restore_checkpoint(saved.visible);
        self.join.restore_checkpoint(saved.join);
    }
    fn clear(&mut self) {
        self.join.clear();
        self.visible.clear();
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
        let predicate = &self.predicate;
        let changes = self.join.apply_batch_projected(
            inputs
                .iter()
                .filter(|(right, _)| !right)
                .map(|(_, event)| typed::<I, L>(event).clone()),
            inputs
                .iter()
                .filter(|(right, _)| *right)
                .map(|(_, event)| typed::<J, R>(event).clone()),
            |left, right| predicate(left, right).then(|| (left.clone(), right.clone())),
        );
        // The graph's outer checkpoint covers join installation and this ID set.
        // Stable accepted IDs need no map replacement; their payload is downstream.
        let mut output = Vec::with_capacity(changes.len());
        for change in changes {
            if change.value.is_some() {
                if !self.visible.contains_key(&change.id) {
                    self.visible.insert(change.id.clone(), ());
                }
                output.push(Rc::new(change) as Event);
            } else if self.visible.remove(&change.id) {
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
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(FilteredIndexedJoin {
            join: JoinNode::<I, J, L, R, K>::comparing(comparison, left_key, right_key),
            visible: SnapshotMap::new(),
            predicate: Box::new(predicate),
        });
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}
