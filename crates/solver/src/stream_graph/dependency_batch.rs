//! An explicit boundary for evaluating final dependency-row batches.
use super::*;

struct BatchedDerived<'a, K, T, V>(crate::DependencyNode<'a, K, T, V>);
impl<
    K: Ord + Clone + fmt::Debug + 'static,
    T: Clone + 'static,
    V: Clone + Eq + 'static,
    S: AdditiveScore,
> Node<S> for BatchedDerived<'_, K, T, V>
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
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        self.0
            .apply(typed::<K, T>(event).clone())
            .map(erase)
            .map_err(|e| GraphError::Dependency(e.to_string()))
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        self.0
            .apply_batch(inputs.iter().map(|(_, event)| typed::<K, T>(event).clone()))
            .map(erase)
            .map_err(|e| GraphError::Dependency(e.to_string()))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Derive values once from final rows and dependencies in a graph batch.
    /// Like [`Self::derive_dependencies`], every present input row produces its
    /// row and a derived value under the same identity. This explicit boundary
    /// coalesces repeated row updates and evaluates coordinated rewires together,
    /// avoiding transient cycles and intermediate supplier/output observations.
    /// Dependencies and suppliers must read immutable inputs; discarded intermediate
    /// row values are never evaluated. Final cycles remain inconsistent.
    ///
    /// When this node reaches a constraint or captured dataset, basic/list/mixed
    /// stream score directors batch complete initialization and move updates.
    /// Explicit transaction `apply` stays immediate. Upstream and downstream nodes
    /// retain their own callback and checked-arithmetic semantics. Trial, saved
    /// candidates, undo, activity changes and diagnostic rebuilds include this state.
    /// See [`crate::DependencyNode::apply_batch`] for validation and output rules.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, (Option<usize>, i64)>();
    /// let derived = b.derive_dependencies_batched(source.port(), 0..2,
    ///     |row| row.0.into_iter().collect(),
    ///     |_, row, inputs| Some(row.1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0)))?;
    /// b.score("derived", &derived, |(_, v)| SimpleScore::new(v.value.unwrap_or(0)))?;
    /// let mut graph = b.build();
    /// let mut batch = StreamBatch::new();
    /// batch.push(&source, TupleChange::put(0, (Some(1), 2)))?;
    /// batch.push(&source, TupleChange::put(1, (None, 3)))?;
    /// let mut tx = graph.transaction();
    /// tx.apply_batch(batch)?;
    /// tx.commit()?;
    /// assert_eq!(graph.score(), SimpleScore::new(8));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn derive_dependencies_batched<
        K: Ord + Clone + fmt::Debug + 'static,
        T: Clone + 'static,
        V: Clone + Eq + 'static,
    >(
        &mut self,
        input: &StreamPort<K, T>,
        keys: impl IntoIterator<Item = K>,
        dependencies: impl Fn(&T) -> Vec<K> + 'a,
        supplier: impl Fn(&K, &T, &[(&K, &crate::DerivedValue<V>)]) -> Option<V> + 'a,
    ) -> Result<StreamPort<K, (T, crate::DerivedValue<V>)>, GraphError> {
        self.validate(input)?;
        let node = crate::DependencyNode::new(keys, dependencies, supplier)
            .map_err(|e| GraphError::Dependency(e.to_string()))?;
        let out = self.push(BatchedDerived(node));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}
