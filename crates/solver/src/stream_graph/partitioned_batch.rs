//! Coalesced rounds over equality-partitioned comparison joins.
use super::*;
struct PartitionedBatchJoin<'a, I, J, L, R, E, K>(PartitionedJoinNode<'a, I, J, L, R, E, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    E: Ord + Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for PartitionedBatchJoin<'_, I, J, L, R, E, K>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.0.restore(saved);
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
        Ok(erase(
            self.0.apply_batch(
                inputs
                    .iter()
                    .filter(|(r, _)| !r)
                    .map(|(_, e)| typed::<I, L>(e).clone()),
                inputs
                    .iter()
                    .filter(|(r, _)| *r)
                    .map(|(_, e)| typed::<J, R>(e).clone()),
            ),
        ))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Join equality partitions with an ordered comparison, publishing only final
    /// pair changes in each batch. Projections return `(equality_key,ordered_key)`;
    /// tuple equality keys support multiple equalities. Each side retains only its
    /// last update per ID. Input state is shared by equality partition with lazy
    /// side allocation; matched pair payloads are not retained by this operator.
    ///
    /// Observing the node enables batched planning updates. Outputs are sorted by
    /// pair ID. Transactions, saved candidates, activity pruning and consistency
    /// rebuilding follow the ordinary graph contracts. Pure callbacks/stable keys
    /// are required. Dense rounds can still require quadratic temporary space.
    pub fn join_partitioned_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        E: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> (E, K) + 'a,
        right_key: impl Fn(&R) -> (E, K) + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(PartitionedBatchJoin(
            PartitionedJoinNode::<I, J, L, R, E, K>::new(comparison, left_key, right_key),
        ));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}
