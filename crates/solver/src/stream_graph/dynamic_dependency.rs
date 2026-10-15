//! Runtime dependency membership under graph transaction and batch boundaries.
use super::*;
use crate::{DynamicDependencyInput, DynamicDependencyNode};
struct DynamicDerived<'a, K, T, V>(DynamicDependencyNode<'a, K, T, V>);
impl<
    K: Ord + Clone + fmt::Debug + 'static,
    T: Clone + 'static,
    V: Clone + Eq + 'static,
    S: AdditiveScore,
> Node<S> for DynamicDerived<'_, K, T, V>
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
    /// Derive rows whose source keys are discovered and reclaimed at runtime.
    /// Inputs preserve declared order/duplicates and distinguish absent sources
    /// from present rows with absent derived values. Row metadata/presence changes
    /// reevaluate direct consumers even when derived values compare equal.
    ///
    /// Complete graph batches evaluate final membership/edges together. Immediate
    /// transaction apply remains immediate. Undo, saved candidates, activity
    /// changes and rebuilds restore or reconstruct membership and values together.
    /// See [`DynamicDependencyNode`] for reclamation and rebuild cost boundaries.
    /// The existing fixed-universe derivation APIs retain their stricter validation.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b=StreamGraphBuilder::<SimpleScore>::new();
    /// let source=b.source::<usize,(Option<usize>,i64)>();
    /// let derived=b.derive_dynamic_dependencies(source.port(),
    ///     |row|row.0.into_iter().collect(),
    ///     |_,row,inputs|Some(row.1+inputs.first().and_then(|i|i.derived.value).unwrap_or(0)))?;
    /// b.score("sum",&derived,|(_,v)|SimpleScore::new(v.value.unwrap_or(0)))?;
    /// let mut graph=b.build();
    /// let mut tx=graph.transaction();
    /// tx.apply(&source,TupleChange::put(10,(Some(20),2)))?;
    /// assert_eq!(tx.score()?,SimpleScore::new(2));
    /// tx.apply(&source,TupleChange::put(20,(None,3)))?;
    /// assert_eq!(tx.score()?,SimpleScore::new(8));
    /// tx.commit()?;
    /// graph.assert_consistent()?;
    /// # Ok::<(),GraphError>(())
    /// ```
    pub fn derive_dynamic_dependencies<
        K: Ord + Clone + fmt::Debug + 'static,
        T: Clone + 'static,
        V: Clone + Eq + 'static,
    >(
        &mut self,
        input: &StreamPort<K, T>,
        dependencies: impl Fn(&T) -> Vec<K> + 'a,
        supplier: impl Fn(&K, &T, &[DynamicDependencyInput<'_, K, T, V>]) -> Option<V> + 'a,
    ) -> Result<StreamPort<K, (T, crate::DerivedValue<V>)>, GraphError> {
        self.validate(input)?;
        let out = self.push(DynamicDerived(DynamicDependencyNode::new(
            dependencies,
            supplier,
        )));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimpleScore, TupleChange};
    use std::cell::Cell;
    #[test]
    fn saved_candidates_restore_membership_and_declarations_without_callbacks() {
        let calls = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<i32, (Vec<i32>, i64)>();
        let derived = b
            .derive_dynamic_dependencies(
                source.port(),
                |r| r.0.clone(),
                |_, r, i| {
                    calls.set(calls.get() + 1);
                    Some(r.1 + i.iter().map(|i| i.derived.value.unwrap_or(0)).sum::<i64>())
                },
            )
            .unwrap();
        let dataset = b.dataset(&derived).unwrap();
        let mut graph = b.build();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(10, (vec![-5, -5], 1)))
            .unwrap();
        tx.apply(&source, TupleChange::put(-5, (vec![], 3)))
            .unwrap();
        let saved = tx.capture().unwrap();
        drop(tx);
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(-20, (vec![40], 99)))
            .unwrap();
        tx.commit().unwrap();
        let previous = calls.get();
        graph.install(saved);
        assert_eq!(calls.get(), previous);
        assert_eq!(
            graph
                .dataset_rows(&dataset)
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [-5, 10]
        );
        assert_eq!(graph.dataset_rows(&dataset).unwrap()[&10].1.value, Some(7));
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(-5, (vec![], 4)))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(graph.dataset_rows(&dataset).unwrap()[&10].1.value, Some(9));
        graph.assert_consistent().unwrap();
    }
}
