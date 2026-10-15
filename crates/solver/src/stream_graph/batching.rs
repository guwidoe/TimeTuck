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
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            owner: None,
            events: Vec::with_capacity(capacity),
        }
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
    fn discard(&mut self, nodes: &[usize]);
    fn recycle(&mut self, _id: usize, _inputs: BatchInputs) {}
}
impl BatchPendingQueue for BTreeMap<usize, BatchInputs> {
    fn push(&mut self, id: usize, input: (bool, Event)) {
        self.entry(id).or_default().push(input);
    }
    fn pop(&mut self) -> Option<(usize, BatchInputs)> {
        self.pop_first()
    }
    fn discard(&mut self, nodes: &[usize]) {
        for id in nodes {
            self.remove(id);
        }
    }
}
// Single-change rounds usually have only a few distinct pending ranks. Keep
// those ranks sorted inline; wider fan-out promotes once to the ordinary tree.
// Slots own their input vectors in rank order, including during unwinding.
#[derive(Default)]
struct SmallBatchQueue {
    entries: [Option<(usize, BatchInputs)>; 4],
    len: usize,
    spill: Option<BTreeMap<usize, BatchInputs>>,
}
impl BatchPendingQueue for SmallBatchQueue {
    fn push(&mut self, id: usize, input: (bool, Event)) {
        if let Some(spill) = &mut self.spill {
            BatchPendingQueue::push(spill, id, input);
            return;
        }
        let position =
            self.entries[..self.len].partition_point(|entry| entry.as_ref().unwrap().0 < id);
        if position < self.len && self.entries[position].as_ref().unwrap().0 == id {
            self.entries[position].as_mut().unwrap().1.push(input);
        } else if self.len < self.entries.len() {
            for index in (position..self.len).rev() {
                self.entries[index + 1] = self.entries[index].take();
            }
            self.entries[position] = Some((id, Vec::new()));
            self.len += 1;
            self.entries[position].as_mut().unwrap().1.push(input);
        } else {
            let mut spill = BTreeMap::new();
            for entry in &mut self.entries {
                let (rank, inputs) = entry.take().unwrap();
                spill.insert(rank, inputs);
            }
            self.len = 0;
            BatchPendingQueue::push(&mut spill, id, input);
            self.spill = Some(spill);
        }
    }
    fn pop(&mut self) -> Option<(usize, BatchInputs)> {
        if let Some(spill) = &mut self.spill {
            return spill.pop_first();
        }
        if self.len == 0 {
            return None;
        }
        let first = self.entries[0].take();
        for index in 1..self.len {
            self.entries[index - 1] = self.entries[index].take();
        }
        self.len -= 1;
        first
    }
    fn discard(&mut self, nodes: &[usize]) {
        if let Some(spill) = &mut self.spill {
            BatchPendingQueue::discard(spill, nodes);
            return;
        }
        // Remove/drop each requested rank immediately in caller order, as the
        // tree queue does. Remaining payloads keep their within-rank FIFO order.
        for id in nodes {
            let position =
                self.entries[..self.len].partition_point(|entry| entry.as_ref().unwrap().0 < *id);
            if position < self.len && self.entries[position].as_ref().unwrap().0 == *id {
                let removed = self.entries[position].take();
                for index in position + 1..self.len {
                    self.entries[index - 1] = self.entries[index].take();
                }
                self.len -= 1;
                drop(removed);
            }
        }
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
    fn recycle(&mut self, id: usize, inputs: BatchInputs) {
        BatchQueue::recycle(self, id, inputs);
    }
    fn discard(&mut self, nodes: &[usize]) {
        for &id in nodes {
            self.buffers[id].clear();
        }
        self.ready.retain(|id| nodes.binary_search(&id.0).is_err());
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
    fn route_batch(
        &self,
        node: usize,
        outputs: BatchOutputs,
        pending: &mut impl BatchPendingQueue,
    ) {
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
    /// structural activity rebuilds only affected connected components from their
    /// final retained sources; pending changes in other components still settle.
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
            return self.apply_prepared_batch(
                batch,
                &mut SmallBatchQueue::default(),
                &mut SmallBatchQueue::default(),
            );
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
        let mut changes = Vec::new();
        if self.graph.active.is_some() {
            let next = self.graph.active_plan();
            if self.graph.active.as_ref() != Some(&next) {
                let (nodes, reset_changes) = self.reset_activity_components(next);
                changes.extend(reset_changes.into_iter().map(|(_, old)| (old, S::ZERO)));
                pending.discard(&nodes);
                for id in nodes {
                    let events = self.graph.nodes[id].source_events();
                    self.route_batch(id, events, pending);
                }
            }
        }
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
    // Constructor-only binding. Unobserved sources still retain every row for
    // source inspection and later weight reactivation. Their sorted initial
    // rows need neither per-row AVL insertion nor empty propagation queues.
    // Observed/activity/batched paths retain their exact callback interleaving.
    pub(crate) fn bind_planning_rows(&mut self, batch: StreamBatch) -> Result<(), GraphError> {
        const MIN_ROWS: usize = 256;
        if batch.events.len() < MIN_ROWS
            || self.graph.active.is_some()
            || self.graph.batched_updates
        {
            return self.apply_updates(batch);
        }
        // Default observed-only graphs keep the ordinary loop, with no per-row
        // grouping or event ownership overhead. This checks topology, not rows.
        if !self
            .graph
            .nodes
            .iter()
            .enumerate()
            .any(|(id, node)| self.graph.edges[id].is_empty() && node.source_count() == Some(0))
        {
            return self.apply_updates(batch);
        }
        self.valid()?;
        self.failed = true;
        self.validate_batch(&batch)?;
        let mut events = batch.events.into_iter();
        while let Some((source, _)) = events.as_slice().first() {
            let source = *source;
            let count = events
                .as_slice()
                .iter()
                .take_while(|(id, _)| *id == source)
                .count();
            let bulk = count >= MIN_ROWS
                && self.graph.edges[source].is_empty()
                && self.graph.nodes[source].source_count() == Some(0);
            let initialized = if bulk {
                self.checkpoint(source);
                self.graph.nodes[source].initialize_sorted_source(&events.as_slice()[..count])
            } else {
                false
            };
            if initialized {
                for _ in events.by_ref().take(count) {}
            } else {
                for (_, event) in events.by_ref().take(count) {
                    self.propagate(source, event)?;
                }
            }
        }
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

#[cfg(test)]
mod initial_binding_tests;

#[cfg(test)]
mod small_queue_tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
    };

    #[test]
    fn inline_boundary_spill_fifo_and_reentry_match_tree() {
        for width in [0, 1, 4, 5, 12] {
            let mut small = SmallBatchQueue::default();
            let mut tree = BTreeMap::new();
            for id in (0..width).rev() {
                for serial in 0..2 {
                    let event: Event = Rc::new((id, serial));
                    small.push(id * 7, (serial == 1, event.clone()));
                    BatchPendingQueue::push(&mut tree, id * 7, (serial == 1, event));
                }
            }
            assert_eq!(small.spill.is_some(), width > 4);
            small.discard(&[7, 777]);
            BatchPendingQueue::discard(&mut tree, &[7, 777]);
            loop {
                let expected = tree.pop_first();
                let actual = small.pop();
                assert_eq!(actual.as_ref().map(|x| x.0), expected.as_ref().map(|x| x.0));
                let (Some((_, actual)), Some((_, expected))) = (actual, expected) else {
                    break;
                };
                assert_eq!(actual.len(), expected.len());
                for ((right, event), (expected_right, expected_event)) in
                    actual.iter().zip(&expected)
                {
                    assert_eq!(right, expected_right);
                    assert!(Rc::ptr_eq(event, expected_event));
                    assert_eq!(Rc::strong_count(event), 2);
                }
            }
            let event: Event = Rc::new(42usize);
            let weak = Rc::downgrade(&event);
            small.push(3, (false, event));
            drop(small.pop());
            assert!(weak.upgrade().is_none());
            assert!(small.pop().is_none());
        }
    }

    struct Tracked(usize, Rc<RefCell<Vec<usize>>>);
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    fn dropped<Q: BatchPendingQueue + Default>(width: usize, unwind: bool) -> Vec<usize> {
        let log = Rc::new(RefCell::new(Vec::new()));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut queue = Q::default();
            for id in (0..width).rev() {
                for serial in 0..2 {
                    queue.push(id, (false, Rc::new(Tracked(id * 10 + serial, log.clone()))));
                }
            }
            // Deliberately unsorted: discard order is the caller's order.
            queue.discard(&[3, 1, 99]);
            drop(queue.pop());
            if unwind {
                panic!("injected queue unwind");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        Rc::try_unwrap(log).unwrap().into_inner()
    }
    #[test]
    fn discard_and_unwind_preserve_tree_payload_drop_order() {
        for width in [4, 5, 9] {
            for unwind in [false, true] {
                let expected = dropped::<BTreeMap<usize, BatchInputs>>(width, unwind);
                assert_eq!(dropped::<SmallBatchQueue>(width, unwind), expected);
            }
        }
    }

    #[test]
    fn single_source_spill_replay_and_callback_panic_restore_graph() {
        for fanout in [2, 5] {
            let fail = Cell::new(false);
            let mut builder = StreamGraphBuilder::<crate::SimpleScore>::new();
            let source = builder.source::<usize, i64>();
            for index in 0..fanout {
                builder
                    .score(format!("score-{index}"), source.port(), |value| {
                        assert!(!fail.get(), "injected scoring panic");
                        crate::SimpleScore::new(*value)
                    })
                    .unwrap();
            }
            let mut graph = builder.build();
            let batch = |value| {
                let mut batch = StreamBatch::new();
                batch.push(&source, TupleChange::put(0, value)).unwrap();
                batch
            };
            {
                let mut tx = graph.transaction();
                tx.apply_batch(batch(3)).unwrap();
                tx.commit().unwrap();
            }
            graph.assert_consistent().unwrap();
            fail.set(true);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let mut tx = graph.transaction();
                    tx.apply_batch(batch(8)).unwrap();
                }))
                .is_err()
            );
            fail.set(false);
            assert_eq!(graph.score(), crate::SimpleScore::new(3 * fanout));
            graph.assert_consistent().unwrap();
            {
                let mut tx = graph.transaction();
                tx.apply_batch(batch(4)).unwrap();
                assert_eq!(tx.score().unwrap(), crate::SimpleScore::new(4 * fanout));
            }
            graph.assert_consistent().unwrap();
            assert_eq!(graph.score(), crate::SimpleScore::new(3 * fanout));
        }
    }
}
