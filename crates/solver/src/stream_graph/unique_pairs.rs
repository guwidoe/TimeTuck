//! Single-input pair routing through the ordinary graph journal.
use super::*;
use crate::UniquePairNode;

struct UniquePairs<'a, I, T, K>(UniquePairNode<'a, I, T, K>);
impl<I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static, S: AdditiveScore>
    Node<S> for UniquePairs<'_, I, T, K>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, checkpoint: Box<dyn Any>) {
        self.0.restore(checkpoint);
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        Ok(erase(self.0.apply(typed::<I, T>(event).clone())))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Compare each pair of distinct input occurrences once, in stable ID order.
    /// Input filtering is explicit; unassigned facts are not implicitly removed.
    pub fn unique_pairs<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
    ) -> Result<JoinedStreamPort<I, I, T, T>, GraphError> {
        self.unique_pairs_equal(input, |_| ())
    }
    /// Compare distinct occurrences only inside equal-key buckets. Tuple keys
    /// express multiple equalities; add a downstream filter for other predicates.
    /// Each source update reaches this node once, including in shared branches.
    /// Orientation follows native occurrence IDs, unlike eager stream position order.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let jobs = b.source::<usize, u32>();
    /// let conflicts = b.unique_pairs_equal(jobs.port(), |machine| *machine)?;
    /// b.score("conflicts", &conflicts, |_| SimpleScore::new(-1))?;
    /// let mut graph = b.build();
    /// let mut tx = graph.transaction();
    /// for id in [2, 0, 1] { tx.apply(&jobs, TupleChange::put(id, 7))?; }
    /// assert_eq!(tx.score()?, SimpleScore::new(-3));
    /// tx.commit()?;
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn unique_pairs_equal<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        key: impl Fn(&T) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, I, T, T>, GraphError> {
        self.validate(input)?;
        let out = self.push(UniquePairs(UniquePairNode::<I, T, K>::new(key)));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}
