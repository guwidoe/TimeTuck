//! A fused zero-or-one projection participates in the ordinary graph lifecycle.
use super::*;
struct FilterMap<'a, I, T, U>(crate::FilterMapNode<'a, I, T, U>);
impl<I: Ord + Clone + 'static, T: 'static, U: 'static, S: AdditiveScore> Node<S>
    for FilterMap<'_, I, T, U>
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
        Ok(erase(self.0.apply_ref(typed::<I, T>(event))))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Filter and map through one physical node, preserving the input identity.
    /// Returning None rejects a row and retracts any earlier accepted output.
    /// Retracts never call the projection. The graph owns rollback, saved-state,
    /// activity and rebuild behavior just as for other incremental operators.
    /// Activity uses source capability, never the predicate's current result.
    pub fn filter_map<I: Ord + Clone + 'static, T: 'static, U: 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> Option<U> + 'a,
    ) -> Result<StreamPort<I, U>, GraphError> {
        self.validate(input)?;
        let output = self.push(FilterMap(crate::FilterMapNode::<I, T, U>::new(mapping)));
        self.connect(input.node, output.node, false);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    #[test]
    fn updates_batches_trials_and_saved_states_match_independent_rows() {
        for prune in [false, true] {
            for batched in [false, true] {
                let mut builder = StreamGraphBuilder::<SimpleScore>::new();
                if prune {
                    builder = builder.with_activity_pruning();
                }
                let source = builder.source::<usize, i64>();
                let filtered = builder
                    .filter_map(source.port(), |v| (v % 3 != 0).then_some(v * 2))
                    .unwrap();
                let dataset = builder.dataset(&filtered).unwrap();
                builder
                    .score("sum", &filtered, |v| SimpleScore::new(*v))
                    .unwrap();
                let mut graph = builder.build();
                let mut rows = BTreeMap::new();
                for step in 0..100 {
                    let mut next = rows.clone();
                    let changes: Vec<_> = (0..3)
                        .map(|offset| {
                            let id = (step + offset * 2) % 11;
                            let value = ((step + offset) % 7 != 0)
                                .then_some((step * 3 + offset) as i64 - 30);
                            if let Some(v) = value {
                                next.insert(id, v);
                            } else {
                                next.remove(&id);
                            }
                            TupleChange { id, value }
                        })
                        .collect();
                    let expected: BTreeMap<_, _> = next
                        .iter()
                        .filter_map(|(&id, &v)| (v % 3 != 0).then_some((id, v * 2)))
                        .collect();
                    let before = graph.dataset_rows(&dataset).unwrap().clone();
                    let old_score = graph.score();
                    let mut tx = graph.transaction();
                    if batched {
                        let mut batch = crate::StreamBatch::new();
                        for change in changes {
                            batch.push(&source, change).unwrap();
                        }
                        tx.apply_batch(batch).unwrap();
                    } else {
                        for change in changes {
                            tx.apply(&source, change).unwrap();
                        }
                    }
                    let actual: BTreeMap<_, _> = tx
                        .graph()
                        .unwrap()
                        .dataset_rows(&dataset)
                        .unwrap()
                        .iter()
                        .map(|(&k, &v)| (k, v))
                        .collect();
                    assert_eq!(actual, expected);
                    assert_eq!(
                        tx.score().unwrap(),
                        SimpleScore::new(expected.values().sum())
                    );
                    if step % 5 == 0 {
                        let saved = tx.capture().unwrap();
                        drop(tx);
                        assert_eq!(graph.score(), old_score);
                        assert_eq!(graph.dataset_rows(&dataset).unwrap(), &before);
                        graph.install(saved);
                        rows = next;
                    } else if step % 3 == 0 {
                        drop(tx);
                        assert_eq!(graph.score(), old_score);
                        assert_eq!(graph.dataset_rows(&dataset).unwrap(), &before);
                    } else {
                        tx.commit().unwrap();
                        rows = next;
                    }
                    graph.assert_consistent().unwrap();
                }
            }
        }
    }
}
