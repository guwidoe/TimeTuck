//! Narrow scalar routing cells; only exact NoDrop inline events enter the arena.
use super::*;

pub(super) enum QueuedEvent {
    Inline(usize),
    Shared(Rc<dyn Any>),
}
fn enqueue(
    edges: &[Edge],
    output: Event,
    pending: &mut VecDeque<(Edge, QueuedEvent)>,
    arena: &mut Vec<Event>,
    shared: bool,
) {
    let output = if shared { output.promote() } else { output };
    let Some((last, prefix)) = edges.split_last() else {
        return;
    };
    match output {
        Event::Shared(owner) => {
            for &edge in prefix {
                pending.push_back((edge, QueuedEvent::Shared(owner.clone())));
            }
            pending.push_back((*last, QueuedEvent::Shared(owner)));
        }
        inline @ (Event::Pair(_) | Event::Triple(_) | Event::Planning(_)) => {
            let index = arena.len();
            arena.push(inline);
            for &edge in edges {
                pending.push_back((edge, QueuedEvent::Inline(index)));
            }
        }
    }
}

impl<S: AdditiveScore> GraphTransaction<'_, '_, S> {
    pub(super) fn propagate_arena(
        &mut self,
        source: usize,
        event: Event,
    ) -> Result<(), GraphError> {
        // Validated sources only retain facts; all scoring nodes are downstream.
        // An unconnected source cannot change any contribution. Preserve its
        // checkpoint, payload Clone/Drop and error behavior without a workspace
        // round trip, node-score probes or an empty total-score adjustment.
        if self.graph.edges[source].is_empty() {
            self.checkpoint(source);
            return self.graph.nodes[source].apply_source(&event);
        }
        let event = if self.graph.shared_event_producers[source] {
            event.promote()
        } else {
            event
        };
        // Taking the buffers leaves an empty slot for nested diagnostics. On an
        // unwind this local owner immediately drops pending events and deltas;
        // transaction checkpoints retain sole ownership of graph rollback.
        let mut workspace = std::mem::take(&mut self.graph.propagation_workspace);
        // All callers provide a validated source: a source handle/batch entry,
        // or a row emitted by source_events during replay. Handle the root in
        // the original node-loop order, without allocating its singleton output
        // Vec or sending the same event through the queue's root round trip.
        {
            self.checkpoint(source);
            if self
                .graph
                .score_probe_nodes
                .as_ref()
                .is_some_and(|nodes| !nodes[source])
            {
                self.graph.nodes[source].apply_source(&event)?;
                enqueue(
                    &self.graph.edges[source],
                    event.clone(),
                    &mut workspace.arena_pending,
                    &mut workspace.arena,
                    false,
                );
            } else {
                self.score_evaluations +=
                    u128::from(self.graph.nodes[source].evaluates_score(&event));
                let old = self.graph.nodes[source].score();
                self.graph.nodes[source].apply_source(&event)?;
                let new = self.graph.nodes[source].score();
                if old != new {
                    workspace.changes.push((old, new));
                }
                enqueue(
                    &self.graph.edges[source],
                    event.clone(),
                    &mut workspace.arena_pending,
                    &mut workspace.arena,
                    false,
                );
            }
        }
        // Match the old root iteration: release its event before downstream
        // callbacks (or total adjustment when no outgoing edge exists).
        drop(event);
        let result = self
            .drain_arena_nodes_into(
                &mut workspace.arena_pending,
                &mut workspace.arena,
                &mut workspace.changes,
            )
            .and_then(|()| {
                let totals = if self.replaying {
                    crate::ScoringArithmetic::Wrapping
                } else {
                    self.arithmetic
                };
                self.graph.score = totals
                    .adjust(
                        &self.graph.score,
                        workspace.changes.iter().map(|(old, new)| (old, new)),
                    )
                    .map_err(|_| GraphError::ScoreOverflow { constraint: None })?;
                Ok(())
            });
        workspace.clear();
        self.graph.propagation_workspace = workspace;
        result
    }
    #[inline(always)]
    fn drain_arena_nodes_into(
        &mut self,
        pending: &mut VecDeque<(Edge, QueuedEvent)>,
        arena: &mut Vec<Event>,
        changes: &mut Vec<(S, S)>,
    ) -> Result<(), GraphError> {
        let arithmetic = NodeArithmetic {
            impact: self.arithmetic,
            totals: if self.replaying {
                crate::ScoringArithmetic::Wrapping
            } else {
                self.arithmetic
            },
        };
        while let Some((edge, queued)) = pending.pop_front() {
            // Keep custom owners local until the original loop's release phase.
            // Inline borrows end after apply, before output publication can grow
            // the arena. No reference into the arena survives that preparation.
            let shared;
            let event = match queued {
                QueuedEvent::Inline(index) => &arena[index],
                QueuedEvent::Shared(owner) => {
                    shared = Event::Shared(owner);
                    &shared
                }
            };
            let id = edge.node;
            if self.graph.active.as_ref().is_some_and(|active| !active[id])
                && self.graph.nodes[id].source_count().is_none()
            {
                continue;
            }
            self.checkpoint(id);
            if self
                .graph
                .score_probe_nodes
                .as_ref()
                .is_some_and(|nodes| !nodes[id])
            {
                let outputs = {
                    let _count = ScoreEvaluationDelta::new(
                        self.graph.score_evaluation_counters[id].as_ref(),
                        &mut self.score_evaluations,
                    );
                    self.graph.nodes[id].apply(edge.right, event, arithmetic)
                }?;
                outputs.for_each(|output| {
                    enqueue(
                        &self.graph.edges[id],
                        output,
                        pending,
                        arena,
                        self.graph.shared_event_producers[id],
                    );
                });
                continue;
            }
            self.score_evaluations += u128::from(self.graph.nodes[id].evaluates_score(event));
            let old = self.graph.nodes[id].score();
            let outputs = {
                let _count = ScoreEvaluationDelta::new(
                    self.graph.score_evaluation_counters[id].as_ref(),
                    &mut self.score_evaluations,
                );
                self.graph.nodes[id].apply(edge.right, event, arithmetic)
            }?;
            let new = self.graph.nodes[id].score();
            if old != new {
                changes.push((old, new));
            }
            outputs.for_each(|output| {
                enqueue(
                    &self.graph.edges[id],
                    output,
                    pending,
                    arena,
                    self.graph.shared_event_producers[id],
                );
            });
        }
        Ok(())
    }
}

#[cfg(test)]
thread_local! {
    static FORCE_WIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(super) fn wide_mode() -> bool {
    FORCE_WIDE.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn with_wide<T>(wide: bool, operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            FORCE_WIDE.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(FORCE_WIDE.with(|flag| flag.replace(wide)));
    operation()
}

#[cfg(test)]
mod tests;
