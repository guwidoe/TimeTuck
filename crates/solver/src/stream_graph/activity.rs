//! Conservative source-emptiness activity and optional branch scheduling.
use super::*;

// Every physical edge participates, including both inputs of multi-input nodes.
// Consequently no event can leave a component without traversing a missing edge.
// This metadata exists only for graphs that enable activity pruning.
pub(super) struct ActivityComponents {
    component_of: Vec<usize>,
    members: Vec<Vec<usize>>,
}
impl ActivityComponents {
    pub(super) fn new(edges: &[Vec<Edge>]) -> Self {
        fn root(parents: &mut [usize], mut node: usize) -> usize {
            while parents[node] != node {
                parents[node] = parents[parents[node]];
                node = parents[node];
            }
            node
        }
        let mut parents: Vec<_> = (0..edges.len()).collect();
        let mut sizes = vec![1; edges.len()];
        for (source, downstream) in edges.iter().enumerate() {
            for edge in downstream {
                let mut a = root(&mut parents, source);
                let mut b = root(&mut parents, edge.node);
                if a == b {
                    continue;
                }
                if sizes[a] < sizes[b] {
                    std::mem::swap(&mut a, &mut b);
                }
                parents[b] = a;
                sizes[a] += sizes[b];
            }
        }
        let mut ids = vec![usize::MAX; edges.len()];
        let mut component_of = Vec::with_capacity(edges.len());
        let mut members: Vec<Vec<usize>> = Vec::new();
        for node in 0..edges.len() {
            let root = root(&mut parents, node);
            if ids[root] == usize::MAX {
                ids[root] = members.len();
                members.push(Vec::new());
            }
            component_of.push(ids[root]);
            members[ids[root]].push(node);
        }
        Self {
            component_of,
            members,
        }
    }
    fn changed(&self, previous: &[bool], next: &[bool]) -> Vec<usize> {
        let mut selected = vec![false; self.members.len()];
        let mut nodes = Vec::new();
        for (node, (old, new)) in previous.iter().zip(next).enumerate() {
            let component = self.component_of[node];
            if old != new && !selected[component] {
                selected[component] = true;
                nodes.extend_from_slice(&self.members[component]);
            }
        }
        // Multiple components may be interleaved in registration order.
        nodes.sort_unstable();
        nodes
    }
}

#[derive(Clone, Copy, Default)]
pub(super) enum ActivityRule {
    #[default]
    All,
    Any,
    Left,
}

/// Structural activity after all pending updates have settled.
/// `can_produce` uses source emptiness and operator input requirements, not
/// predicates, matching keys, or actual output cardinality. `active` additionally
/// requires a path to an enabled scoring constraint or captured dataset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeActivity {
    pub can_produce: bool,
    pub active: bool,
}

/// An immutable activity snapshot. It remains valid for its captured state even
/// after the graph changes. Node identities are scoped to their originating graph.
pub struct StreamActivity {
    owner: Rc<()>,
    nodes: Vec<NodeActivity>,
}
impl StreamActivity {
    pub fn get<I, T>(&self, port: &StreamPort<I, T>) -> Result<NodeActivity, GraphError> {
        if !Rc::ptr_eq(&self.owner, &port.owner) {
            return Err(GraphError::ForeignPort);
        }
        Ok(self.nodes[port.node])
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn active_count(&self) -> usize {
        self.nodes.iter().filter(|node| node.active).count()
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Enable source-emptiness pruning. Facts in every source are still retained.
    /// Updates skip branches unable to reach a live constraint or dataset. When
    /// source emptiness changes the active plan, only connected operator components
    /// whose activity changed are cleared and rebuilt from retained facts. All
    /// declared edges (including both join inputs) connect their operators;
    /// unrelated components retain their caches and invoke no replay callbacks.
    /// Other updates remain incremental. Rebuild callbacks count as ordinary work
    /// and can fail/roll back the transaction. The activity-plan scan remains
    /// global; component membership adds O(nodes) metadata only when enabled.
    /// This trades transition cost for less work in persistently empty branches.
    /// Predicates must be pure; skipped branches do not validate their payloads.
    /// Disabled by default. Graph topology and source handles never change.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    /// let jobs = b.source::<usize, i32>();
    /// let resources = b.source::<usize, i32>();
    /// let pairs = b.join_equal(jobs.port(), resources.port(), |_| (), |_| ())?;
    /// b.score("pairs", &pairs, |_| SimpleScore::new(-1))?;
    /// let mut g = b.build();
    /// let mut tx = g.transaction();
    /// tx.apply(&jobs, TupleChange::put(0, 10))?;
    /// tx.commit()?;
    /// assert!(!g.activity().get(jobs.port())?.active);
    /// let mut tx = g.transaction();
    /// tx.apply(&resources, TupleChange::put(0, 20))?;
    /// tx.commit()?;
    /// assert!(g.activity().get(jobs.port())?.active);
    /// assert_eq!(g.score(), SimpleScore::new(-1));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn with_activity_pruning(mut self) -> Self {
        self.activity_pruning = true;
        self
    }
}
impl<S: AdditiveScore> StreamGraph<'_, S> {
    /// Inspect potential activity without invoking callbacks or changing state.
    /// Available with or without activity pruning; when pruning is disabled this
    /// describes structural liveness, not which nodes receive update events.
    pub fn activity(&self) -> StreamActivity {
        let size = self.nodes.len();
        let mut seen = vec![false; size];
        let mut all = vec![true; size];
        let mut any = vec![false; size];
        let mut left = vec![false; size];
        let mut can = vec![false; size];
        for id in 0..size {
            can[id] = self.nodes[id].source_count().map_or_else(
                || match self.activity_rules[id] {
                    ActivityRule::All => seen[id] && all[id],
                    ActivityRule::Any => any[id],
                    ActivityRule::Left => left[id],
                },
                |count| count != 0,
            );
            for edge in &self.edges[id] {
                seen[edge.node] = true;
                all[edge.node] &= can[id];
                any[edge.node] |= can[id];
                if !edge.right {
                    left[edge.node] |= can[id];
                }
            }
        }
        let mut active = vec![false; size];
        for id in (0..size).rev() {
            let node = &self.nodes[id];
            let observed = node.dataset_rows().is_some()
                || (node.is_constraint()
                    && node
                        .constraint_weight()
                        .is_none_or(|weight| *weight != S::ZERO));
            active[id] =
                can[id] && (observed || self.edges[id].iter().any(|edge| active[edge.node]));
        }
        StreamActivity {
            owner: self.owner.clone(),
            nodes: can
                .into_iter()
                .zip(active)
                .map(|(can_produce, active)| NodeActivity {
                    can_produce,
                    active,
                })
                .collect(),
        }
    }
    pub(super) fn active_plan(&self) -> Vec<bool> {
        self.activity()
            .nodes
            .into_iter()
            .map(|node| node.active)
            .collect()
    }
}
impl<S: AdditiveScore> GraphTransaction<'_, '_, S> {
    pub(super) fn reset_activity_components(
        &mut self,
        next: Vec<bool>,
    ) -> (Vec<usize>, Vec<(usize, S)>) {
        let nodes = self
            .graph
            .activity_components
            .as_ref()
            .expect("pruning components")
            .changed(self.graph.active.as_ref().expect("pruning plan"), &next);
        self.graph.active = Some(next);
        let mut changes = Vec::new();
        for &id in &nodes {
            self.checkpoint(id);
            let node = &mut self.graph.nodes[id];
            if node.is_constraint() {
                changes.push((id, node.score()));
            }
            if node.source_count().is_none() {
                node.clear();
            }
        }
        (nodes, changes)
    }
    pub(super) fn apply_with_activity(
        &mut self,
        source: usize,
        event: Event,
    ) -> Result<(), GraphError> {
        self.checkpoint(source);
        let was_empty = self.graph.nodes[source].source_count() == Some(0);
        let outputs = self.graph.nodes[source].apply(
            false,
            &event,
            NodeArithmetic {
                impact: self.arithmetic,
                totals: self.arithmetic,
            },
        )?;
        let is_empty = self.graph.nodes[source].source_count() == Some(0);
        if was_empty != is_empty {
            let next = self.graph.active_plan();
            if self.graph.active.as_ref() != Some(&next) {
                let (nodes, original_scores) = self.reset_activity_components(next);
                let events: Vec<_> = nodes
                    .iter()
                    .flat_map(|&id| {
                        self.graph.nodes[id]
                            .source_events()
                            .into_iter()
                            .map(move |event| (id, event))
                    })
                    .collect();
                for (id, event) in events {
                    self.drain_nodes(VecDeque::from([(
                        Edge {
                            node: id,
                            right: false,
                        },
                        event,
                    )]))?;
                }
                // Keep the old total until every selected component has settled:
                // removing old contributions alone may overflow a valid total.
                let changes: Vec<_> = original_scores
                    .into_iter()
                    .map(|(id, old)| (old, self.graph.nodes[id].score()))
                    .collect();
                self.graph.score = self
                    .arithmetic
                    .adjust(
                        &self.graph.score,
                        changes.iter().map(|(old, new)| (old, new)),
                    )
                    .map_err(|_| GraphError::ScoreOverflow { constraint: None })?;
                return Ok(());
            }
        }
        let pending = outputs
            .into_iter()
            .flat_map(|event| {
                self.graph.edges[source]
                    .iter()
                    .map(move |&edge| (edge, event.clone()))
            })
            .collect();
        self.drain(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    #[test]
    fn saved_candidate_installs_activity_after_trial_rollback() {
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let left = b.source::<usize, i64>();
        let right = b.source::<usize, i64>();
        let joined = b
            .join_equal(left.port(), right.port(), |_| (), |_| ())
            .unwrap();
        b.score("joined", &joined, |(a, c)| SimpleScore::new(a + c))
            .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::put(0, 2)).unwrap();
        tx.commit().unwrap();
        let mut trial = g.transaction();
        trial.apply(&right, TupleChange::put(0, 3)).unwrap();
        let saved = trial.capture().unwrap();
        drop(trial);
        assert!(!g.activity().get(&joined).unwrap().active);
        g.install(saved);
        assert!(g.activity().get(&joined).unwrap().active);
        assert_eq!(g.score(), SimpleScore::new(5));
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::put(0, 4)).unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(7));
        g.assert_consistent().unwrap();
    }
    #[test]
    fn component_candidate_restore_is_callback_free_and_preserves_other_caches() {
        use std::cell::Cell;
        let calls = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let left = b.source::<usize, i64>();
        let stable = b.source::<usize, i64>();
        let right = b.source::<usize, i64>();
        let mapped = b
            .map(stable.port(), |v| {
                calls.set(calls.get() + 1);
                *v
            })
            .unwrap();
        let data = b.dataset(&mapped).unwrap();
        b.score("stable", &mapped, |v| SimpleScore::new(*v))
            .unwrap();
        let pairs = b
            .join_equal(left.port(), right.port(), |_| (), |_| ())
            .unwrap();
        b.score("pairs", &pairs, |(a, c)| SimpleScore::new(a + c))
            .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::put(0, 3)).unwrap();
        tx.apply(&stable, TupleChange::put(0, 9)).unwrap();
        tx.commit().unwrap();
        let before = calls.get();
        let mut tx = g.transaction();
        tx.apply(&right, TupleChange::put(0, 4)).unwrap();
        assert!(tx.checkpoint_count() < tx.graph.nodes.len());
        let saved = tx.capture().unwrap();
        drop(tx);
        assert_eq!(g.score(), SimpleScore::new(9));
        for value in [8, 10, 12] {
            let mut tx = g.transaction();
            tx.apply(&right, TupleChange::put(0, value)).unwrap();
            drop(tx);
        }
        assert_eq!(calls.get(), before);
        g.install(saved);
        assert_eq!(calls.get(), before);
        assert_eq!(g.dataset_rows(&data).unwrap()[&0], 9);
        assert_eq!(g.score(), SimpleScore::new(16));
        assert!(g.activity().get(&pairs).unwrap().active);
        g.assert_consistent().unwrap();
        let b = StreamGraphBuilder::<SimpleScore>::new();
        assert!(b.build().activity_components.is_none());
    }
}
