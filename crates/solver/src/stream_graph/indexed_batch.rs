//! Batch-only publication using the compact indexed join state.
use super::*;
struct IndexedBatchJoin<'a, I, J, L, R, K>(JoinNode<'a, I, J, L, R, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for IndexedBatchJoin<'_, I, J, L, R, K>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn clear(&mut self) {
        self.0.clear();
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.0.restore(saved);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0.apply_left(typed::<I, L>(event).clone())
        }))
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
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
    /// Equality join publishing one final change per affected pair in a graph
    /// batch. Input rows/indexes are retained, but matched pairs are not duplicated
    /// in a join-owned cache. Observing this node enables batched score-director
    /// updates; direct transaction `apply` retains immediate event semantics.
    /// Use `join_batched` when the join itself needs a residual pair predicate.
    pub fn join_equal_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.join_compare_batched(left, right, JoinComparison::Equal, left_key, right_key)
    }
    /// Ordered-key join with final batch publication and compact input/index
    /// storage. Relation orientation is left versus right. Range traversal skips
    /// nonmatching keys; affected pair/output buffers can be quadratic for dense
    /// rounds. Intermediate input payloads are coalesced before projection.
    /// Graph checkpoint, activity rebuild, diagnostic replay and rollback apply
    /// normally. Pure/stable projections and keys are required.
    pub fn join_compare_batched<
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
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(IndexedBatchJoin(JoinNode::<I, J, L, R, K>::comparing(
            comparison, left_key, right_key,
        )));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}
