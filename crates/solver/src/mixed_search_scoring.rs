//! Internal scoring boundary shared by mixed basic/list search algorithms.
use crate::mixed_scoring::MixedSelection;
use crate::{AdditiveScore, MixedAssignment, MixedMove, MixedScoreDirector, MixedScoringError};

pub(crate) trait MixedSearchDirector<B, L, S> {
    type Error: From<crate::MixedError>;
    fn notify_best(&mut self, _complete: impl FnOnce(&MixedAssignment<B, L>) -> bool) {}
    fn assignment(&self) -> &MixedAssignment<B, L>;
    fn score(&self) -> S;
    fn initial_constraint_evaluations(&self) -> u128;
    fn assert_consistent(&mut self) -> Result<(), Self::Error>;
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = MixedMove<B, L>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&MixedMove<B, L>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<MixedSelection<B, L>, Self::Error>;
}
impl<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore> MixedSearchDirector<B, L, S>
    for MixedScoreDirector<'_, '_, B, L, S>
{
    type Error = MixedScoringError;
    fn assignment(&self) -> &MixedAssignment<B, L> {
        self.assignment()
    }
    fn score(&self) -> S {
        self.score()
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        self.constraint_scores().count() as u128
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        MixedScoreDirector::assert_consistent(self)
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = MixedMove<B, L>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        accepts: impl FnMut(&MixedMove<B, L>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<MixedSelection<B, L>, Self::Error> {
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
