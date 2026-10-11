//! Rare explicit configuration changes rebuild live operators from retained facts.
use super::*;

// Removed edges retain their original sibling positions, including duplicate
// edges into different join sides. Fully enabled graphs allocate no extra table.
#[derive(Clone, Copy)]
pub(super) struct PrunedEdge {
    source: usize,
    position: usize,
    edge: Edge,
}
pub(super) fn prune<S: AdditiveScore>(
    nodes: &[Box<dyn Node<S> + '_>],
    edges: &mut [Vec<Edge>],
) -> Vec<PrunedEdge> {
    if !nodes
        .iter()
        .any(|node| node.constraint_weight().is_some_and(|w| *w == S::ZERO))
    {
        return Vec::new();
    }
    let mut needed: Vec<_> = nodes
        .iter()
        .map(|node| {
            node.dataset_rows().is_some()
                || (node.is_constraint() && node.constraint_weight().is_none_or(|w| *w != S::ZERO))
        })
        .collect();
    for id in (0..needed.len()).rev() {
        needed[id] |= edges[id].iter().any(|edge| needed[edge.node]);
    }
    let mut removed = Vec::new();
    for (source, outgoing) in edges.iter_mut().enumerate() {
        let mut position = 0;
        outgoing.retain(|edge| {
            let keep = needed[edge.node];
            if !keep {
                removed.push(PrunedEdge {
                    source,
                    position,
                    edge: *edge,
                });
            }
            position += 1;
            keep
        });
    }
    removed
}

struct RestoreWeights<'g, 'a, S: AdditiveScore> {
    graph: &'g mut StreamGraph<'a, S>,
    state: Option<GraphState<S>>,
    weights: Vec<(usize, S)>,
    edges: Option<Vec<Vec<Edge>>>,
    pruned: Option<Vec<PrunedEdge>>,
    components: Option<Option<Box<activity::ActivityComponents>>>,
    batched: bool,
}
impl<S: AdditiveScore> Drop for RestoreWeights<'_, '_, S> {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            for (id, weight) in self.weights.drain(..) {
                self.graph.nodes[id].replace_weight(weight);
            }
            if let Some(edges) = self.edges.take() {
                self.graph.edges = edges;
            }
            if let Some(pruned) = self.pruned.take() {
                self.graph.pruned_edges = pruned;
            }
            if let Some(components) = self.components.take() {
                self.graph.activity_components = components;
            }
            self.graph.batched_updates = self.batched;
            self.graph.install(state);
        }
    }
}
impl<S: AdditiveScore> StreamGraph<'_, S> {
    /// Current configurable weights, in constraint-name order, including zero.
    pub fn constraint_weights(&self) -> impl Iterator<Item = (&str, &S)> {
        self.constraints.iter().filter_map(|(name, &id)| {
            self.nodes[id]
                .constraint_weight()
                .map(|w| (name.as_str(), w))
        })
    }
    /// Atomically change live constraint weights while preserving source handles.
    /// Unknown names, unweighted constraints and structural weights are rejected
    /// before callbacks. Omitted names keep their effective weight. An unchanged
    /// batch returns zero without replaying. Zero disables a branch; reactivation
    /// reconstructs its joins/collectors from current retained source rows,
    /// including updates received while disabled. Captured datasets stay live.
    ///
    /// This explicit configuration boundary rebuilds the entire live graph, even
    /// unchanged constraints. Returns all replayed scoring callbacks; ordinary
    /// subsequent source updates remain incremental. Temporary storage includes
    /// full node checkpoints, source events and topology. Removed edges are retained
    /// for future reactivation; fully enabled graphs need no extra edge table.
    /// Checked final terminal and graph totals permit valid prefix cancellation.
    /// Errors and ordinary panic unwinding restore weights, scheduling, matches,
    /// datasets and score without replaying callbacks. External callback effects
    /// and panicking destructors are outside rollback guarantees. Existing graph
    /// transactions/edit sessions must finish before this exclusive operation.
    pub fn apply_weight_overrides(
        &mut self,
        overrides: &crate::ConstraintWeightOverrides<S>,
    ) -> Result<u128, GraphError> {
        let unknown: Vec<_> = overrides
            .iter()
            .filter(|(name, _)| !self.constraints.contains_key(*name))
            .map(|(name, _)| name.to_owned())
            .collect();
        if !unknown.is_empty() {
            return Err(GraphError::UnknownConstraintWeights {
                unknown,
                known: self.constraints.keys().cloned().collect(),
            });
        }
        let mut updates = Vec::new();
        for (name, weight) in overrides.iter() {
            let id = self.constraints[name];
            let current = self.nodes[id]
                .constraint_weight()
                .ok_or_else(|| GraphError::UnweightedConstraintOverride(name.to_owned()))?;
            if weight.structural() != 0 {
                return Err(GraphError::StructuralConstraintWeight(name.to_owned()));
            }
            if current != weight {
                updates.push((id, weight.clone()));
            }
        }
        if updates.is_empty() {
            return Ok(0);
        }
        updates.sort_unstable_by_key(|(id, _)| *id);
        // Finish user cloning before publishing any configuration. Keep an outer
        // full snapshot because the replay transaction sees the new configuration.
        let previous = updates
            .iter()
            .map(|(id, _)| (*id, self.nodes[*id].constraint_weight().unwrap().clone()))
            .collect();
        let state = self.capture_full();
        let events: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(id, node)| {
                node.source_events()
                    .into_iter()
                    .map(move |event| (id, event))
            })
            .collect();
        let mut edges = self.edges.clone();
        for removed in &self.pruned_edges {
            edges[removed.source].insert(removed.position, removed.edge);
        }
        let batched = self.batched_updates;
        let mut restore = RestoreWeights {
            graph: self,
            state: Some(state),
            weights: previous,
            edges: None,
            pruned: None,
            components: None,
            batched,
        };
        for (id, weight) in updates {
            restore.graph.nodes[id].replace_weight(weight);
        }
        let pruned = prune(&restore.graph.nodes, &mut edges);
        let components = restore
            .graph
            .activity_components
            .as_ref()
            .map(|_| Box::new(activity::ActivityComponents::new(&edges)));
        restore.edges = Some(std::mem::replace(&mut restore.graph.edges, edges));
        restore.pruned = Some(std::mem::replace(&mut restore.graph.pruned_edges, pruned));
        restore.components = Some(std::mem::replace(
            &mut restore.graph.activity_components,
            components,
        ));
        if restore.graph.active.is_some() {
            restore.graph.active = Some(restore.graph.active_plan());
        }
        // An earlier zero weight may have hidden the graph's only batched node.
        let mut observed: Vec<_> = restore
            .graph
            .nodes
            .iter()
            .map(|node| {
                node.dataset_rows().is_some()
                    || (node.is_constraint()
                        && node.constraint_weight().is_none_or(|w| *w != S::ZERO))
            })
            .collect();
        for id in (0..observed.len()).rev() {
            observed[id] |= restore.graph.edges[id]
                .iter()
                .any(|edge| observed[edge.node]);
        }
        restore.graph.batched_updates = restore
            .graph
            .nodes
            .iter()
            .enumerate()
            .any(|(id, node)| observed[id] && node.prefers_batch());
        let mut trial = restore.graph.transaction();
        trial.replaying = true;
        trial.failed = true;
        for id in 0..trial.graph.nodes.len() {
            trial.checkpoint(id);
        }
        for node in &mut trial.graph.nodes {
            node.clear();
        }
        trial.graph.score = S::ZERO;
        if trial.graph.batched_updates {
            trial.replay_sources(events)?;
        } else {
            for (id, event) in events {
                trial.propagate(id, event)?;
            }
        }
        for &id in trial.graph.constraints.values() {
            trial.graph.nodes[id].validate_total(trial.arithmetic)?;
        }
        let scores: Vec<_> = trial
            .graph
            .constraints
            .values()
            .map(|&id| trial.graph.nodes[id].score())
            .collect();
        trial.graph.score = trial
            .arithmetic
            .sum(&scores)
            .map_err(|_| GraphError::ScoreOverflow { constraint: None })?;
        trial.failed = false;
        let work = trial.score_evaluations();
        trial.commit()?;
        restore.state = None;
        Ok(work)
    }
}
