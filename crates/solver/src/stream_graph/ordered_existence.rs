//! Pair-free ordered semi/anti joins in final-state graph rounds.
use super::*;
use crate::ExistsCompareNode;
struct OrderedExistence<'a, I, J, L, R, E, K>(ExistsCompareNode<'a, I, J, L, R, E, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    E: Ord + Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for OrderedExistence<'_, I, J, L, R, E, K>
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
    fn restore(&mut self, s: Box<dyn Any>) {
        self.0.restore(s);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        a: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        <Self as Node<S>>::apply_batch(self, &[(right, event.clone())], a)
    }
    fn apply_batch(
        &mut self,
        events: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(
            self.0.apply_batch(
                events
                    .iter()
                    .filter(|(r, _)| !*r)
                    .map(|(_, e)| typed::<I, L>(e).clone()),
                events
                    .iter()
                    .filter(|(r, _)| *r)
                    .map(|(_, e)| typed::<J, R>(e).clone()),
            ),
        ))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Ordered semi/anti join without retained matching pairs. Publishes final
    /// visibility per batch; surviving changed left IDs refresh their payloads.
    /// Self joins include self matches. Pure/stable key projections are required.
    pub fn existence_compare_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        mode: ExistenceMode,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        self.existence_partitioned_batched(
            left,
            right,
            mode,
            comparison,
            move |l| ((), left_key(l)),
            move |r| ((), right_key(r)),
        )
    }
    /// Equality-partitioned ordered semi/anti join retaining only input indexes
    /// and right key counts. Right boundary changes visit only crossed left keys;
    /// duplicate witnesses do not multiply state or output. Tuple equality keys
    /// allow multiple equalities. Observing the node enables batched solver updates.
    /// Transactions, pruning and saved candidates follow ordinary graph contracts.
    pub fn existence_partitioned_batched<
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
        mode: ExistenceMode,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> (E, K) + 'a,
        right_key: impl Fn(&R) -> (E, K) + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(OrderedExistence(
            ExistsCompareNode::<I, J, L, R, E, K>::new(mode, comparison, left_key, right_key),
        ));
        if mode == ExistenceMode::NotExists {
            self.activity_rules[out.node] = ActivityRule::Left;
        }
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}
