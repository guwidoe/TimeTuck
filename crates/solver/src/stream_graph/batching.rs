//! Topological batch scheduling and native batched join adapters.
use super::*;

/// Heterogeneous changes belonging to one graph. Adding a foreign source fails
/// without adding that change. With explicit `apply_batch`, repeated changes to
/// one source ID keep only the last value; sources settle before derived nodes.
/// No callbacks run while collecting changes. Cloning a batch shares its immutable
/// event payloads. Submission uses the ordinary graph transaction rollback contract.
#[derive(Clone, Default)]
pub struct StreamBatch {
    pub(super) owner: Option<Rc<()>>,
    pub(super) events: Vec<(usize, Event)>,
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
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Equality-indexed filtered join that coalesces inputs in graph batches.
    /// Building an observed batched join enables batched planning updates in the
    /// basic/list/mixed stream score directors. Direct transaction `apply` remains
    /// immediate; `apply_batch` settles both inputs together. Callbacks must depend
    /// only on immutable row contents; intermediate source values are not evaluated.
    /// Stores input rows/indexes and accepted pair IDs; rejected pair payloads
    /// are never cloned or retained. Accepted-ID storage can be quadratic. Pure
    /// callback and panic/rollback requirements match standalone BatchedJoinNode.
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
        self.join_compare_filtered_batched(
            left,
            right,
            JoinComparison::Equal,
            left_key,
            right_key,
            predicate,
        )
    }
}
type BatchInputs = Vec<(bool, Event)>;
trait BatchPendingQueue {
    fn push(&mut self, id: usize, input: (bool, Event));
    fn pop(&mut self) -> Option<(usize, BatchInputs)>;
    fn clear(&mut self);
    fn recycle(&mut self, _id: usize, _inputs: BatchInputs) {}
}
impl BatchPendingQueue for BTreeMap<usize, BatchInputs> {
    fn push(&mut self, id: usize, input: (bool, Event)) {
        self.entry(id).or_default().push(input);
    }
    fn pop(&mut self) -> Option<(usize, BatchInputs)> {
        self.pop_first()
    }
    fn clear(&mut self) {
        BTreeMap::clear(self);
    }
}
// Node IDs are topological ranks. Keep event buffers between batches and queue
// only nonempty ranks; sparse propagation never scans all nodes to find work.
#[derive(Default)]
struct BatchQueue {
    buffers: Vec<Vec<(bool, Event)>>,
    ready: std::collections::BinaryHeap<std::cmp::Reverse<usize>>,
}
impl BatchQueue {
    fn resize(&mut self, nodes: usize) {
        self.buffers.resize_with(nodes, Vec::new);
    }
    fn push(&mut self, id: usize, input: (bool, Event)) {
        if self.buffers[id].is_empty() {
            self.ready.push(std::cmp::Reverse(id));
        }
        self.buffers[id].push(input);
    }
    fn pop(&mut self) -> Option<(usize, Vec<(bool, Event)>)> {
        let id = self.ready.pop()?.0;
        Some((id, std::mem::take(&mut self.buffers[id])))
    }
    fn recycle(&mut self, id: usize, mut inputs: Vec<(bool, Event)>) {
        inputs.clear();
        debug_assert!(self.buffers[id].is_empty());
        self.buffers[id] = inputs;
    }
    fn clear(&mut self) {
        while let Some(std::cmp::Reverse(id)) = self.ready.pop() {
            self.buffers[id].clear();
        }
    }
}
impl BatchPendingQueue for BatchQueue {
    fn push(&mut self, id: usize, input: (bool, Event)) {
        BatchQueue::push(self, id, input);
    }
    fn pop(&mut self) -> Option<(usize, BatchInputs)> {
        BatchQueue::pop(self)
    }
    fn clear(&mut self) {
        BatchQueue::clear(self);
    }
    fn recycle(&mut self, id: usize, inputs: BatchInputs) {
        BatchQueue::recycle(self, id, inputs);
    }
}
#[derive(Default)]
pub(super) struct BatchWorkspace {
    sources: BatchQueue,
    pending: BatchQueue,
}
impl BatchWorkspace {
    fn prepare(&mut self, nodes: usize) {
        self.sources.resize(nodes);
        self.pending.resize(nodes);
    }
    fn clear(&mut self) {
        self.sources.clear();
        self.pending.clear();
    }
}
impl<S: AdditiveScore> GraphTransaction<'_, '_, S> {
    // Called only after consistency rebuilding has checkpointed and cleared all
    // nodes. Preserve the live graph's batch semantics during diagnostic replay.
    pub(super) fn replay_sources(&mut self, events: Vec<(usize, Event)>) -> Result<(), GraphError> {
        let batch = StreamBatch {
            owner: Some(self.graph.owner.clone()),
            events,
        };
        self.failed = false;
        self.apply_batch(batch)
    }
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
    fn route_batch(&self, node: usize, outputs: Outputs, pending: &mut impl BatchPendingQueue) {
        for output in outputs {
            for edge in &self.graph.edges[node] {
                pending.push(edge.node, (edge.right, output.clone()));
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
        // Reusable routing storage pays off for multi-change rounds. Keep the
        // compact local queue for single changes, which do less routing work.
        if batch.events.len() <= 1 {
            return self.apply_prepared_batch(batch, &mut BTreeMap::new(), &mut BTreeMap::new());
        }
        let mut workspace = std::mem::take(&mut self.graph.batch_workspace);
        workspace.prepare(self.graph.nodes.len());
        // On panic the local workspace drops all pending payloads; the graph
        // keeps an empty workspace and the transaction retains rollback ownership.
        let result =
            self.apply_prepared_batch(batch, &mut workspace.sources, &mut workspace.pending);
        workspace.clear();
        self.graph.batch_workspace = workspace;
        result
    }
    fn apply_prepared_batch<Q: BatchPendingQueue>(
        &mut self,
        batch: StreamBatch,
        sources: &mut Q,
        pending: &mut Q,
    ) -> Result<(), GraphError> {
        let arithmetic = NodeArithmetic {
            impact: self.arithmetic,
            totals: if self.replaying {
                crate::ScoringArithmetic::Wrapping
            } else {
                self.arithmetic
            },
        };
        for (source, event) in batch.events {
            sources.push(source, (false, event));
        }
        while let Some((source, events)) = sources.pop() {
            self.checkpoint(source);
            let outputs = self.graph.nodes[source].apply_batch(&events, arithmetic);
            sources.recycle(source, events);
            self.route_batch(source, outputs?, pending);
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
                    self.route_batch(id, events, pending);
                }
            }
        }
        let mut changes = Vec::new();
        while let Some((id, inputs)) = pending.pop() {
            if self.graph.active.as_ref().is_some_and(|active| !active[id]) {
                pending.recycle(id, inputs);
                continue;
            }
            self.checkpoint(id);
            self.score_evaluations += inputs
                .iter()
                .filter(|(_, e)| self.graph.nodes[id].evaluates_score(e))
                .count() as u128;
            let old = self.graph.nodes[id].score();
            let outputs = self.graph.nodes[id].apply_batch(&inputs, arithmetic);
            pending.recycle(id, inputs);
            let outputs = outputs?;
            let new = self.graph.nodes[id].score();
            if old != new {
                changes.push((old, new));
            }
            self.route_batch(id, outputs, pending);
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

#[cfg(test)]
mod workspace_tests {
    use super::*;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    fn empty<S: AdditiveScore>(g: &StreamGraph<'_, S>) {
        for q in [&g.batch_workspace.sources, &g.batch_workspace.pending] {
            assert!(q.ready.is_empty());
            assert!(q.buffers.iter().all(Vec::is_empty));
        }
    }
    fn storage<S: AdditiveScore>(g: &StreamGraph<'_, S>) -> Vec<(usize, usize)> {
        [&g.batch_workspace.sources, &g.batch_workspace.pending]
            .into_iter()
            .flat_map(|q| {
                q.buffers
                    .iter()
                    .map(|v| (v.as_ptr() as usize, v.capacity()))
            })
            .collect()
    }
    #[test]
    fn repeated_batches_reuse_buffers_across_commits_and_rollbacks() {
        let mut b = StreamGraphBuilder::<crate::SimpleScore>::new();
        let l = b.source::<usize, i64>();
        let r = b.source::<usize, i64>();
        let j = b
            .join_equal_batched(l.port(), r.port(), |_| 0, |_| 0)
            .unwrap();
        b.score("sum", &j, |(l, r)| crate::SimpleScore::new(l + r))
            .unwrap();
        let mut g = b.build();
        let batch = |offset| {
            let mut updates = StreamBatch::new();
            for i in 0..2 {
                updates
                    .push(&r, TupleChange::put(i, 10 * (i as i64 + 1)))
                    .unwrap();
            }
            for i in 0..2 {
                updates
                    .push(&l, TupleChange::put(i, i as i64 + 1 + offset))
                    .unwrap();
            }
            updates
        };
        {
            let mut tx = g.transaction();
            tx.apply_batch(batch(0)).unwrap();
            tx.commit().unwrap();
        }
        empty(&g);
        let allocation = storage(&g);
        assert!(allocation.iter().any(|(_, capacity)| *capacity > 0));
        assert_eq!(g.score(), crate::SimpleScore::new(66));
        for _ in 0..3 {
            {
                let mut tx = g.transaction();
                tx.apply_batch(batch(2)).unwrap();
                assert_eq!(tx.score().unwrap(), crate::SimpleScore::new(74));
            }
            empty(&g);
            assert_eq!(storage(&g), allocation);
            assert_eq!(g.score(), crate::SimpleScore::new(66));
        }
        {
            let mut tx = g.transaction();
            tx.apply_batch(batch(2)).unwrap();
            tx.commit().unwrap();
        }
        empty(&g);
        assert_eq!(storage(&g), allocation);
        assert_eq!(g.score(), crate::SimpleScore::new(74));
        // Interleave a single-change round using the compact queue. It must not
        // disturb retained workspace capacity or the surrounding transactions.
        {
            let mut single = StreamBatch::new();
            single.push(&l, TupleChange::put(0, 10)).unwrap();
            let mut tx = g.transaction();
            tx.apply_batch(single).unwrap();
            assert_eq!(tx.score().unwrap(), crate::SimpleScore::new(88));
        }
        empty(&g);
        assert_eq!(storage(&g), allocation);
        assert_eq!(g.score(), crate::SimpleScore::new(74));
        g.assert_consistent().unwrap();
        empty(&g);
    }
    #[test]
    fn panic_drops_pending_payloads_and_later_batches_recover() {
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::<crate::SimpleScore>::new();
        let s = b.source::<usize, i64>();
        b.score("first", s.port(), |v| {
            assert!(!fail.get(), "injected");
            crate::SimpleScore::new(*v)
        })
        .unwrap();
        b.score("second", s.port(), |v| crate::SimpleScore::new(*v))
            .unwrap();
        let mut g = b.build();
        let batch = |v| {
            let mut b = StreamBatch::new();
            b.push(&s, TupleChange::put(0, v)).unwrap();
            b.push(&s, TupleChange::put(1, 0)).unwrap();
            b
        };
        {
            let mut tx = g.transaction();
            tx.apply_batch(batch(3)).unwrap();
            tx.commit().unwrap();
        }
        fail.set(true);
        {
            let mut tx = g.transaction();
            assert!(catch_unwind(AssertUnwindSafe(|| tx.apply_batch(batch(9)))).is_err());
            assert!(tx.score().is_err());
        }
        empty(&g);
        assert_eq!(g.score(), crate::SimpleScore::new(6));
        assert_eq!(g.source_rows(&s).unwrap()[&0], 3);
        fail.set(false);
        {
            let mut tx = g.transaction();
            tx.apply_batch(batch(4)).unwrap();
            tx.commit().unwrap();
        }
        empty(&g);
        assert_eq!(g.score(), crate::SimpleScore::new(8));
        g.assert_consistent().unwrap();
        empty(&g);
    }
    #[test]
    fn returned_errors_clear_queued_events_before_reuse() {
        let mut b = StreamGraphBuilder::<crate::SimpleScore>::new();
        let source = b.source::<usize, i64>();
        b.score("first", source.port(), |v| crate::SimpleScore::new(*v))
            .unwrap();
        b.score("second", source.port(), |v| crate::SimpleScore::new(*v))
            .unwrap();
        let mut g = b.build();
        let batch = |v| {
            let mut b = StreamBatch::new();
            b.push(&source, TupleChange::put(0, v)).unwrap();
            b.push(&source, TupleChange::put(1, 0)).unwrap();
            b
        };
        {
            let mut tx = g.transaction();
            tx.apply_batch(batch(3)).unwrap();
            tx.commit().unwrap();
        }
        {
            let mut tx = g.transaction();
            assert!(tx.apply_batch(batch(i64::MAX)).is_err());
            assert!(tx.score().is_err());
        }
        empty(&g);
        assert_eq!(g.score(), crate::SimpleScore::new(6));
        assert_eq!(g.source_rows(&source).unwrap()[&0], 3);
        {
            let mut tx = g.transaction();
            tx.apply_batch(batch(4)).unwrap();
            tx.commit().unwrap();
        }
        empty(&g);
        assert_eq!(g.score(), crate::SimpleScore::new(8));
    }
    #[test]
    fn queue_keeps_rank_order_arrival_order_and_releases_payloads() {
        let mut q = BatchQueue::default();
        q.resize(8);
        let payload = Rc::new("payload");
        for id in [7, 3, 7, 1, 3] {
            q.push(id, (id == 3, payload.clone()));
        }
        assert_eq!(Rc::strong_count(&payload), 6);
        let (id, items) = q.pop().unwrap();
        assert_eq!(id, 1);
        assert_eq!(items.len(), 1);
        q.recycle(id, items);
        let (id, items) = q.pop().unwrap();
        assert_eq!(id, 3);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|(right, _)| *right));
        q.recycle(id, items);
        q.clear();
        assert!(q.ready.is_empty());
        assert!(q.buffers.iter().all(Vec::is_empty));
        assert_eq!(Rc::strong_count(&payload), 1);
        assert!(q.buffers[7].capacity() >= 2);
    }
}
