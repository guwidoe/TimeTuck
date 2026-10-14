//! Internal scoring boundary shared by the basic-variable search algorithms.
use crate::scoring::Selection;
use crate::{AdditiveScore, Assignment, Move, ScoreDirector, ScoringError};

pub(crate) trait SearchDirector<V, S> {
    fn capture_constraint_metrics(
        &self,
        _capture: &crate::solver_metrics::ConstraintMetricCapture<S>,
    ) {
    }
    type Error: From<crate::MoveProviderError>;
    fn notify_best(&mut self, _complete: impl FnOnce(&Assignment<V>) -> bool) {}
    fn assignment(&self) -> &Assignment<V>;
    fn score(&self) -> S;
    fn initial_constraint_evaluations(&self) -> u128;
    fn assert_consistent(&mut self) -> Result<(), Self::Error>;
    fn pooled_contribution_cache(
        &self,
    ) -> Option<crate::pooled_contributions::PooledContributionCache<S>> {
        None
    }
    #[allow(clippy::too_many_arguments)]
    fn select_best_pooled(
        &mut self,
        moves: impl IntoIterator<Item = (Move<V>, usize)>,
        _cache: &mut crate::pooled_contributions::PooledContributionCache<S>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<Selection<V>, Self::Error> {
        self.select_best(
            moves.into_iter().map(|(movement, _)| movement),
            max_evaluations,
            forager,
            accepts,
            quit_early,
            assert_scores,
        )
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<Selection<V>, Self::Error>;
}
impl<V: Eq + Clone, S: AdditiveScore> SearchDirector<V, S> for ScoreDirector<'_, '_, V, S> {
    type Error = ScoringError;
    fn assignment(&self) -> &Assignment<V> {
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
        self.active_constraint_count() as u128
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        ScoreDirector::assert_consistent(self)
    }
    fn pooled_contribution_cache(
        &self,
    ) -> Option<crate::pooled_contributions::PooledContributionCache<S>> {
        ScoreDirector::pooled_contribution_cache(self)
    }
    #[allow(clippy::too_many_arguments)]
    fn select_best_pooled(
        &mut self,
        moves: impl IntoIterator<Item = (Move<V>, usize)>,
        cache: &mut crate::pooled_contributions::PooledContributionCache<S>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<Selection<V>, Self::Error> {
        ScoreDirector::select_best_pooled(
            self,
            moves,
            cache,
            max_evaluations,
            forager,
            accepts,
            quit_early,
            assert_scores,
        )
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<Selection<V>, Self::Error> {
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
