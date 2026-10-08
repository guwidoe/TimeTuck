//! Topological batch scheduling and native batched join adapters.
use super::*;
use crate::{BatchedJoinNode, JoinEvaluation, TupleLifecycleEvent};

/// Heterogeneous changes belonging to one graph. Adding a foreign source fails
/// without adding that change. With explicit `apply_batch`, repeated changes to
/// one source ID keep only the last value; sources settle before derived nodes.
/// No callbacks run while collecting changes. Cloning a batch shares its immutable
/// event payloads. Submission uses the ordinary graph transaction rollback contract.
#[derive(Clone, Default)]
pub struct StreamBatch {
    owner: Option<Rc<()>>,
    events: Vec<(usize, Event)>,
}
impl StreamBatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        source: &StreamSource<I, T>,
        change: TupleChange<I, T>,
    ) -> Result<(), GraphError> {
        if self
            .owner
            .as_ref()
            .is_some_and(|o| !Rc::ptr_eq(o, &source.0.owner))
        {
            return Err(GraphError::ForeignPort);
        }
        self.events.push((source.0.node, Rc::new(change)));
        self.owner = Some(source.0.owner.clone());
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
struct BatchedJoin<'a, I, J, L, R, K>(BatchedJoinNode<'a, I, J, L, R, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for BatchedJoin<'_, I, J, L, R, K>
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
        <Self as Node<S>>::apply_batch(self, &[(right, event.clone())], a)
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
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
    /// Equality-indexed filtered join that coalesces inputs in graph batches.
    /// Building an observed batched join enables batched planning updates in the
    /// basic/list/mixed stream score directors. Direct transaction `apply` remains
    /// immediate; `apply_batch` settles both inputs together. Callbacks must depend
    /// only on immutable row contents; intermediate source values are not evaluated.
    /// Accepted-pair storage can be quadratic. Pure callback and panic/rollback
    /// requirements are the same as for standalone BatchedJoinNode.
    pub fn join_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(BatchedJoin(BatchedJoinNode::<I, J, L, R, K>::new(
            JoinEvaluation::Deferred,
            left_key,
            right_key,
            predicate,
        )));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}
type Pending = BTreeMap<usize, Vec<(bool, Event)>>;
impl<S: AdditiveScore> GraphTransaction<'_, '_, S> {
    fn validate_batch(&self, batch: &StreamBatch) -> Result<(), GraphError> {
        if batch
            .owner
            .as_ref()
            .is_some_and(|owner| !Rc::ptr_eq(owner, &self.graph.owner))
        {
            Err(GraphError::ForeignPort)
        } else {
            Ok(())
        }
    }
    fn route_batch(&self, node: usize, outputs: Outputs, pending: &mut Pending) {
        for output in outputs {
            for edge in &self.graph.edges[node] {
                pending
                    .entry(edge.node)
                    .or_default()
                    .push((edge.right, output.clone()));
            }
        }
    }
    /// Apply a complete set of source changes atomically within this transaction.
    /// Each source ID keeps its last update; affected nodes run in topological
    /// order. Batched join nodes see both final inputs together. Other operators
    /// keep their per-event semantics and checked arithmetic rules. A change in
    /// structural activity triggers one rebuild from all final retained sources.
    /// Any error/panic poisons the transaction; dropping restores all prior state.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b=StreamGraphBuilder::<SimpleScore>::new();
    /// let l=b.source::<usize,i64>();let r=b.source::<usize,i64>();
    /// let joined=b.join_batched(l.port(),r.port(),|_|(),|_|(),|a,b|a<b)?;
    /// b.score("pair",&joined,|(a,b)|SimpleScore::new(b-a))?;
    /// let mut g=b.build();let mut changes=StreamBatch::new();
    /// changes.push(&l,TupleChange::put(0,2))?;
    /// changes.push(&r,TupleChange::put(0,5))?;
    /// let mut tx=g.transaction();tx.apply_batch(changes)?;tx.commit()?;
    /// assert_eq!(g.score(),SimpleScore::new(3));
    /// # Ok::<(),GraphError>(())
    /// ```
    pub fn apply_batch(&mut self, batch: StreamBatch) -> Result<(), GraphError> {
        self.valid()?;
        self.failed = true;
        self.validate_batch(&batch)?;
        let arithmetic = NodeArithmetic {
            impact: self.arithmetic,
            totals: if self.replaying {
                crate::ScoringArithmetic::Wrapping
            } else {
                self.arithmetic
            },
        };
        let mut sources = Pending::new();
        for (source, event) in batch.events {
            sources.entry(source).or_default().push((false, event));
        }
        let mut pending = Pending::new();
        for (source, events) in sources {
            self.checkpoint(source);
            let outputs = self.graph.nodes[source].apply_batch(&events, arithmetic)?;
            self.route_batch(source, outputs, &mut pending);
        }
        if self.graph.active.is_some() {
            let next = self.graph.active_plan();
            if self.graph.active.as_ref() != Some(&next) {
                self.graph.active = Some(next);
                for id in 0..self.graph.nodes.len() {
                    self.checkpoint(id);
                }
                for node in &mut self.graph.nodes {
                    if node.source_count().is_none() {
                        node.clear();
                    }
                }
                self.graph.score = S::ZERO;
                pending.clear();
                for id in 0..self.graph.nodes.len() {
                    let events = self.graph.nodes[id].source_events();
                    self.route_batch(id, events, &mut pending);
                }
            }
        }
        let mut changes = Vec::new();
        while let Some((id, inputs)) = pending.pop_first() {
            if self.graph.active.as_ref().is_some_and(|active| !active[id]) {
                continue;
            }
            self.checkpoint(id);
            self.score_evaluations += inputs
                .iter()
                .filter(|(_, e)| self.graph.nodes[id].evaluates_score(e))
                .count() as u128;
            let old = self.graph.nodes[id].score();
            let outputs = self.graph.nodes[id].apply_batch(&inputs, arithmetic)?;
            let new = self.graph.nodes[id].score();
            if old != new {
                changes.push((old, new));
            }
            self.route_batch(id, outputs, &mut pending);
        }
        self.graph.score = arithmetic
            .totals
            .adjust(&self.graph.score, changes.iter().map(|(a, b)| (a, b)))
            .map_err(|_| GraphError::ScoreOverflow { constraint: None })?;
        self.failed = false;
        Ok(())
    }
    // Directors select batching only for graphs with an observed batched join.
    // Preserve legacy per-source ordering and scoring for ordinary graphs.
    pub(crate) fn apply_updates(&mut self, batch: StreamBatch) -> Result<(), GraphError> {
        if self.graph.batched_updates {
            return self.apply_batch(batch);
        }
        self.valid()?;
        self.failed = true;
        self.validate_batch(&batch)?;
        for (source, event) in batch.events {
            if self.graph.active.is_some() {
                self.apply_with_activity(source, event)?;
            } else {
                self.propagate(source, event)?;
            }
        }
        self.failed = false;
        Ok(())
    }
}
