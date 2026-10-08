//! Graph integration for stable duplicate occurrence expansion.
use super::*;
use crate::StableFlatMapNode;
struct StableExpansion<'a, I, T, U, K>(StableFlatMapNode<'a, I, T, U, K>);
impl<
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    U: 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for StableExpansion<'_, I, T, U, K>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        Ok(erase(self.0.apply(typed::<I, T>(event).clone())))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Expand while preserving duplicate occurrence IDs by a caller-supplied key.
    /// Equal-key occurrences are paired in their previous order. Values refresh
    /// even when their keys are unchanged. Dataset order follows identity, not
    /// the latest expansion order. Normal graph transactions and pruning apply.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let s = b.source::<usize, Vec<char>>();
    /// let expanded = b.flat_map_stable(s.port(), Clone::clone, |v| *v)?;
    /// let data = b.dataset(&expanded)?;
    /// let mut g = b.build();
    /// let mut tx = g.transaction();
    /// tx.apply(&s, TupleChange::put(0, vec!['A','B']))?;
    /// tx.commit()?;
    /// let mut tx = g.transaction();
    /// tx.apply(&s, TupleChange::put(0, vec!['B','X','B']))?;
    /// tx.commit()?;
    /// assert_eq!(g.dataset_rows(&data)?[&(0,1)], 'B');
    /// assert_eq!(g.dataset_rows(&data)?.len(), 3);
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn flat_map_stable<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        U: Clone + 'static,
        K: Ord + Clone + 'static,
        R: IntoIterator<Item = U>,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> R + 'a,
        key: impl Fn(&U) -> K + 'a,
    ) -> Result<StreamPort<(I, usize), U>, GraphError> {
        self.validate(input)?;
        let out = self.push(StableExpansion(StableFlatMapNode::<I, T, U, K>::new(
            mapping, key,
        )));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}
