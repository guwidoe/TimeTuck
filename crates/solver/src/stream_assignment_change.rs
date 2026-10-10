//! Whole-assignment transactions that retain a compiled constraint graph.
use crate::{AdditiveScore, GraphTransaction, StreamGraph};

/// A scored replacement. Drop restores the old assignment and cached graph;
/// commit retains the replacement. Neither operation replays scoring callbacks.
#[must_use = "dropping a replacement restores the previous assignment and graph"]
pub struct StreamAssignmentChange<'g, 'a, A, S: AdditiveScore> {
    assignment: &'g mut A,
    before: Option<A>,
    graph: Option<GraphTransaction<'g, 'a, S>>,
    score: S,
}
impl<'g, 'a, A, S: AdditiveScore> StreamAssignmentChange<'g, 'a, A, S> {
    pub(crate) fn new(
        assignment: &'g mut A,
        next: A,
        graph: GraphTransaction<'g, 'a, S>,
        score: S,
    ) -> Self {
        let before = Some(std::mem::replace(assignment, next));
        Self {
            assignment,
            before,
            graph: Some(graph),
            score,
        }
    }
    pub fn assignment(&self) -> &A {
        self.assignment
    }
    pub fn score(&self) -> S {
        self.score.clone()
    }
    pub fn graph(&self) -> &StreamGraph<'_, S> {
        self.graph
            .as_ref()
            .expect("active replacement")
            .graph()
            .expect("successfully scored transaction")
    }
    /// Terminal scoring callbacks invoked while installing this replacement.
    pub fn constraint_evaluations(&self) -> u128 {
        self.graph
            .as_ref()
            .expect("active replacement")
            .score_evaluations()
    }
    pub fn checkpoint_count(&self) -> usize {
        self.graph
            .as_ref()
            .expect("active replacement")
            .checkpoint_count()
    }
    pub fn commit(mut self) {
        self.graph
            .take()
            .expect("active replacement")
            .commit()
            .expect("successfully scored transaction");
        self.before = None;
    }
}
impl<A, S: AdditiveScore> Drop for StreamAssignmentChange<'_, '_, A, S> {
    fn drop(&mut self) {
        if let Some(before) = self.before.take() {
            let replaced = std::mem::replace(self.assignment, before);
            drop(self.graph.take());
            drop(replaced);
        }
    }
}
