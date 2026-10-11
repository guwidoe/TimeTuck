//! Internal scoring boundary shared by the basic-variable search algorithms.
use crate::scoring::Selection;
use crate::{AdditiveScore, Assignment, Move, ScoreDirector, ScoringError};

pub(crate) trait SearchDirector<V, S> {
    type Error: From<crate::MoveProviderError>;
    fn notify_best(&mut self, _complete: impl FnOnce(&Assignment<V>) -> bool) {}
    fn assignment(&self) -> &Assignment<V>;
    fn score(&self) -> S;
    fn initial_constraint_evaluations(&self) -> u128;
    fn assert_consistent(&mut self) -> Result<(), Self::Error>;
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
    fn initial_constraint_evaluations(&self) -> u128 {
        self.constraint_scores().count() as u128
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        ScoreDirector::assert_consistent(self)
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
