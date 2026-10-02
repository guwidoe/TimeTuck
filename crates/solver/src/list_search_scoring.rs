//! Internal scoring boundary shared by ordered-list search algorithms.
use crate::list_scoring::ListSelection;
use crate::{AdditiveScore, ListAssignment, ListMove, ListScoreDirector, ListScoringError};

pub(crate) trait ListSearchDirector<V, S> {
    type Error: From<crate::ListError>;
    fn assignment(&self) -> &ListAssignment<V>;
    fn score(&self) -> S;
    fn initial_constraint_evaluations(&self) -> u128;
    fn assert_consistent(&mut self) -> Result<(), Self::Error>;
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        allow_rejected: bool,
        accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
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
        allow_rejected: bool,
        accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, Self::Error> {
        self.select_best(
            moves,
            max_evaluations,
            allow_rejected,
            accepts,
            quit_early,
            assert_scores,
        )
    }
}
