//! Internal scoring boundary shared by ordered-list search algorithms.
use crate::list_scoring::ListSelection;
use crate::{AdditiveScore, ListAssignment, ListMove, ListScoreDirector, ListScoringError};

pub(crate) trait ListSearchDirector<V, S> {
    fn capture_constraint_metrics(
        &self,
        _capture: &crate::solver_metrics::ConstraintMetricCapture<S>,
    ) {
    }
    type Error: From<crate::ListError> + From<crate::MoveProviderError>;
    fn notify_best(&mut self, _complete: impl FnOnce(&ListAssignment<V>) -> bool) {}
    fn assignment(&self) -> &ListAssignment<V>;
    fn assigned_registry(&self, problem: &crate::ListProblem<V>) -> Option<Vec<bool>>
    where
        V: Eq + Clone,
    {
        self.assignment().retained_assigned_mask(problem)
    }
    fn score(&self) -> S;
    fn initial_constraint_evaluations(&self) -> u128;
    fn assert_consistent(&mut self) -> Result<(), Self::Error>;
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, Self::Error>;
}
impl<V: Eq + Clone, S: AdditiveScore> ListSearchDirector<V, S> for ListScoreDirector<'_, '_, V, S> {
    type Error = ListScoringError;
    fn assignment(&self) -> &ListAssignment<V> {
        self.assignment()
    }
    fn score(&self) -> S {
        self.score()
    }
    fn capture_constraint_metrics(
        &self,
        capture: &crate::solver_metrics::ConstraintMetricCapture<S>,
    ) {
        self.capture_constraint_metrics(capture);
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        self.constraint_scores().count() as u128
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        ListScoreDirector::assert_consistent(self)
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, Self::Error> {
        self.select_best(
            moves,
            max_evaluations,
            forager,
            accepts,
            quit_early,
            assert_scores,
        )
    }
}
