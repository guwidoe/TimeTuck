//! Batched semi/anti joins for the transactional graph scheduler.
use super::*;
use crate::{BatchedExistsNode, ExistenceEvaluation, TupleLifecycleEvent};

struct BatchedExistence<'a, I, J, L, R, K>(BatchedExistsNode<'a, I, J, L, R, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for BatchedExistence<'_, I, J, L, R, K>
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
        a: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        <Self as Node<S>>::apply_batch(self, &[(right, event.clone())], a).map(Outputs::from)
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        for (right, event) in inputs {
            if *right {
                self.0.enqueue_right(typed::<J, R>(event).clone());
            } else {
                self.0.enqueue_left(typed::<I, L>(event).clone());
            }
        }
        Ok(erase(
            self.0
                .propagate()
                .into_iter()
                .map(TupleLifecycleEvent::into_change),
        ))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Equality-indexed semi/anti join that publishes final visibility once per
    /// graph batch. Multiple witnesses retain one left occurrence. Right-side
    /// changes within a batch cannot expose temporary downstream visibility.
    /// An observed node enables automatic batching in stream score directors.
    /// Direct `apply` remains immediate. Pure callbacks and transactional panic
    /// handling follow `existence`; intermediate predicate evaluations can still
    /// occur inside the node. Matching links can require quadratic space.
    pub fn existence_batched<
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
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(BatchedExistence(BatchedExistsNode::<I, J, L, R, K>::new(
            mode,
            ExistenceEvaluation::Deferred,
            left_key,
            right_key,
            predicate,
        )));
        if mode == ExistenceMode::NotExists {
            self.activity_rules[out.node] = ActivityRule::Left;
        }
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}
