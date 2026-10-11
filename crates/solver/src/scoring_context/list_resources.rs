//! Ordered-list scoring resources use the same runtime context lifecycle.
use crate::{AdditiveScore, ContextScoreResource};
impl<V: Eq + Clone, S: AdditiveScore> ContextScoreResource
    for crate::ListScoreDirector<'_, '_, V, S>
{
    type Score = S;
    type Error = crate::ListScoringError;
    fn context_score(&self) -> S {
        self.score()
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn context_totals(&self) -> Vec<(String, S)> {
        self.constraint_scores()
            .map(|(n, s)| (n.into(), s))
            .collect()
    }
}
impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static> ContextScoreResource
    for crate::ListStreamScoreDirector<'_, '_, V, S>
{
    type Score = S;
    type Error = crate::ListStreamScoringError;
    fn context_score(&self) -> S {
        self.score()
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn context_totals(&self) -> Vec<(String, S)> {
        self.constraint_scores()
            .map(|(n, s)| (n.into(), s))
            .collect()
    }
}

impl<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore> ContextScoreResource
    for crate::MixedScoreDirector<'_, '_, B, L, S>
{
    type Score = S;
    type Error = crate::MixedScoringError;
    fn context_score(&self) -> S {
        self.score()
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn context_totals(&self) -> Vec<(String, S)> {
        self.constraint_scores()
            .map(|(n, s)| (n.into(), s))
            .collect()
    }
}
impl<B: Eq + Clone + 'static, L: Eq + Clone + 'static, S: AdditiveScore + 'static>
    ContextScoreResource for crate::MixedStreamScoreDirector<'_, '_, B, L, S>
{
    type Score = S;
    type Error = crate::MixedStreamScoringError;
    fn context_score(&self) -> S {
        self.score()
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn context_totals(&self) -> Vec<(String, S)> {
        self.constraint_scores()
            .map(|(n, s)| (n.into(), s))
            .collect()
    }
}
