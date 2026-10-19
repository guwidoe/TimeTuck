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
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        // Retain the old per-event behavior. This is not an ID-coalescing boundary.
        Ok(erase(inputs.iter().filter_map(|(_, event)| {
            self.0.apply_ref(typed::<I, T>(event))
        })))
    }
}

type OptionalMapping<'a, T, U> = dyn Fn(&T) -> Option<U> + 'a;

struct BatchedFilterMap<'a, I, T, U> {
    // Cache rejected source facts too, so the complete planning universe can
    // reuse FilterNode's existing fixed storage and transaction journals.
    membership: crate::FilterNode<'a, I, bool>,
    mapping: Box<OptionalMapping<'a, T, U>>,
}
impl<'a, I: Ord + Clone + 'static, T, U> BatchedFilterMap<'a, I, T, U> {
    fn new(mapping: impl Fn(&T) -> Option<U> + 'a) -> Self {
        Self {
            membership: crate::FilterNode::new(|accepted| *accepted),
            mapping: Box::new(mapping),
        }
    }
    fn apply_row(&mut self, id: &I, row: Option<&T>) -> Option<TupleChange<I, U>> {
        let value = row.and_then(|row| (self.mapping)(row));
        let membership = TupleChange {
            id: id.clone(),
            value: row.map(|_| value.is_some()),
        };
        self.membership.apply(membership).map(|change| TupleChange {
            id: change.id,
            value,
        })
    }
}
impl<I: Ord + Clone + 'static, T: 'static, U: 'static, S: AdditiveScore> Node<S>
    for BatchedFilterMap<'_, I, T, U>
{
    fn preserves_input_ids(&self) -> bool {
        true
    }
    fn prefers_batch(&self) -> bool {
        true
    }
    fn bind_fixed_input(&mut self, right: bool, count: usize) {
        if !right {
            self.membership.bind_fixed_inputs(count);
        }
    }
    fn clear(&mut self) {
        self.membership.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.membership.checkpoint()
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.membership.begin_checkpoint()
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        self.membership.commit_checkpoint(saved);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.membership.restore(saved);
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        Ok(erase(self.apply_row(&change.id, change.value.as_ref())))
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        let mut latest = BTreeMap::new();
        for (_, event) in inputs {
            let change = typed::<I, T>(event);
            latest.insert(change.id.clone(), change);
        }
        Ok(erase(latest.into_iter().filter_map(|(id, change)| {
            self.apply_row(&id, change.value.as_ref())
        })))
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

    /// Explicit final-publication zero-or-one projection under source identity.
    ///
    /// In a batch, retain the first representative and last value per input ID,
    /// then map each final surviving put once in sorted ID order. Final removals
    /// retract captured accepted membership without calling the projection.
    /// Every final accepted put is published, including an equal-valued update.
    /// Direct transaction apply remains immediate. Pure mapping/stable IDs are
    /// required; projection order/count can differ from ordinary filter_map.
    ///
    /// This declaration enables batched planning updates and forwards validated
    /// source identity to eligible downstream fixed-input storage. That storage
    /// can change captured key Clone/Drop schedules, including snapshot release
    /// order. Its own ID Clone/Drop schedule may also differ. No old physical trace
    /// is promised. Full source rows stay upstream; this node caches acceptance
    /// for every known fact, including rejected facts, without cloning payloads.
    /// Validated complete planning inputs reuse the existing bounded filter
    /// cache/journals; ordinary or ineligible inputs retain map storage. Normal
    /// graph snapshots and rollback apply to either representation.
    /// Neither input nor output payloads gain a Clone requirement.
    pub fn filter_map_batched<I: Ord + Clone + 'static, T: 'static, U: 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> Option<U> + 'a,
    ) -> Result<StreamPort<I, U>, GraphError> {
        self.validate(input)?;
        let output = self.push(BatchedFilterMap::<I, T, U>::new(mapping));
        self.connect(input.node, output.node, false);
        Ok(output)
    }
}

#[cfg(test)]
mod batched_tests;

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
