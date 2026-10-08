//! Explicit boundaries for publishing only final tuple updates in a graph batch.
use super::*;

struct CoalescedUpdatesNode<I, T> {
    visible: SnapshotMap<I, ()>,
    row: PhantomData<T>,
}
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore> Node<S>
    for CoalescedUpdatesNode<I, T>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn clear(&mut self) {
        self.visible.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(self.visible.clone())
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.visible = *saved.downcast().expect("coalesced updates checkpoint");
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        if change.value.is_some() {
            self.visible.insert(change.id.clone(), ());
            Ok(vec![event.clone()])
        } else if self.visible.remove(&change.id) {
            Ok(vec![event.clone()])
        } else {
            Ok(Vec::new())
        }
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let mut final_updates = BTreeMap::new();
        for (_, event) in inputs {
            final_updates.insert(typed::<I, T>(event).id.clone(), event.clone());
        }
        let mut out = Vec::new();
        for event in final_updates.into_values() {
            out.extend(<Self as Node<S>>::apply(self, false, &event, arithmetic)?);
        }
        Ok(out)
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Publish at most one final change per tuple identity in a graph batch.
    /// Place this after aggregation or another operator that may update one row
    /// repeatedly in a move. Downstream scoring sees the final payload once;
    /// transient absent-to-absent rows emit nothing. Visible replacements still
    /// emit an update without comparing payloads. Output order is by identity.
    /// An observed boundary enables batching in the stream score directors.
    /// Direct transaction `apply` stays immediate. Upstream callbacks and checked
    /// arithmetic keep their own per-event semantics. This node stores identities
    /// only, and its state participates in ordinary graph rollback and snapshots.
    pub fn coalesce_updates<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<I, T>, GraphError> {
        self.validate(input)?;
        let out = self.push(CoalescedUpdatesNode::<I, T> {
            visible: SnapshotMap::new(),
            row: PhantomData,
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}
