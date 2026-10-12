//! Best-step-index cache for late-acceptance reset levels.
use crate::Score;

/// Score baseline associated with an explicit best-step index. Indices need
/// equality, not monotonic order: pre-step sentinels and wrapping epochs work.
/// Repeated indices do not read/clone the score. Different indices refresh the
/// baseline; checks use only hard/reset levels, including medium for HMS.
///
/// Callers must change the index whenever the retained best changes and refresh
/// at step boundaries. Reusing an index for a different score skips the read.
/// The initial index is explicit; no Java scope or implicit index origin is used.
#[derive(Clone, Debug)]
pub struct LateBestLevelState<S, I = i64> {
    score: S,
    index: I,
}
impl<S: Score, I: Eq> LateBestLevelState<S, I> {
    pub fn at_best_step(score: S, index: I) -> Self {
        Self { score, index }
    }
    #[inline]
    pub fn baseline_score(&self) -> &S {
        &self.score
    }
    #[inline]
    pub fn baseline_index(&self) -> &I {
        &self.index
    }
    /// Prepare the score before changing cached fields. A failed factory leaves
    /// the old baseline/index intact. Repeated indices do not call the factory.
    #[inline]
    pub fn refresh_best_step(&mut self, index: I, score: impl FnOnce() -> S) {
        if index != self.index {
            let replacement = score();
            self.score = replacement;
            self.index = index;
        }
    }
    #[inline]
    pub fn higher_reset_levels<'a>(&self, index: &I, score: impl FnOnce() -> &'a S) -> bool
    where
        S: 'a,
    {
        *index != self.index && score().late_acceptance_cmp(&self.score).is_gt()
    }
}
