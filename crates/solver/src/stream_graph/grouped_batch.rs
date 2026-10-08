//! Explicit final-publication boundary for integer groups.
use super::*;

struct BatchedGroup<'a, I, T, K>(Group<'a, I, T, K>);
impl<I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static, S: AdditiveScore>
    Node<S> for BatchedGroup<'_, I, T, K>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.node.checkpoint()
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.0.node.begin_journal_checkpoint()
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        self.0.node.commit_journal_checkpoint(saved);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.0.node.restore_checkpoint(saved);
    }
    fn clear(&mut self) {
        self.0.node.clear();
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        <Group<'_, I, T, K> as Node<S>>::apply(&mut self.0, right, event, arithmetic)
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let mut latest = BTreeMap::new();
        for (_, event) in inputs {
            let change = typed::<I, T>(event);
            latest.insert(change.id.clone(), change);
        }
        // Project only retained final payloads, before any group state changes.
        let mapped = latest
            .into_iter()
            .map(|(id, change)| TupleChange {
                id,
                value: change.value.as_ref().map(|row| (self.0.mapping)(row)),
            })
            .collect::<Vec<_>>();
        Ok(erase(self.0.node.apply_batch(mapped).into_iter().map(
            |change| {
                let key = change.id.clone();
                change.map(|summary| (key, summary))
            },
        )))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Aggregate integer values once per affected group in each graph batch.
    /// Each input identity contributes its final payload only; mapping runs once
    /// for each surviving put. Downstream operators see one final summary or
    /// retraction per group, in key order. No transient empty group is published.
    /// Observing this boundary enables batched score-director updates. Direct
    /// transaction `apply` remains immediate. Pure mapping and stable keys are
    /// required; ordinary group_collect keeps its per-event behavior.
    pub fn group_collect_batched<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, i64) + 'a,
    ) -> Result<StreamPort<K, (K, IntSummary)>, GraphError> {
        self.validate(input)?;
        let out = self.push(BatchedGroup(Group {
            node: GroupNode::<I, K>::new(),
            mapping: Box::new(mapping),
        }));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}
