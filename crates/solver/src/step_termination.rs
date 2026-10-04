//! Composable, exact conditions evaluated at local-search step boundaries.
use crate::{Score, StepProgress};

/// A phase-local stopping expression over retained best scores and completed work.
/// `All` requires every child; `Any` requires at least one. Empty `All` is true,
/// empty `Any` is false. Expressions borrow their children and need no allocation.
///
/// Conditions are evaluated afresh at each boundary: stagnation can reset after
/// an improvement while another `All` child is still false. These conditions do
/// not override hard resource limits, cancellation or neighborhood exhaustion.
/// They do not govern construction, exact search or solve-wide termination.
///
/// ```
/// use timetuck::{LocalSearchConfig, SimpleScore, StepTermination};
/// let children = [StepTermination::Steps(20), StepTermination::UnimprovedSteps(5)];
/// let condition = StepTermination::All(&children);
/// let config = LocalSearchConfig::<SimpleScore> {
///     step_termination: Some(&condition),
///     ..Default::default()
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepTermination<'a, S = crate::SimpleScore> {
    Steps(u64),
    UnimprovedSteps(u64),
    /// Accepted installed moves, excluding selected rejected fallback moves.
    AcceptedMoves(u64),
    /// Exact lexicographic comparison, including structural state.
    BestScore(S),
    /// Uses `Score::is_feasible`, including every hard score level.
    Feasible,
    All(&'a [Self]),
    Any(&'a [Self]),
}

impl<S: Score> StepTermination<'_, S> {
    /// Query a snapshot without mutating or latching any child condition.
    /// Counts and best score must belong to the same local-search phase.
    pub fn is_reached(&self, best: &S, accepted_moves: u64, progress: StepProgress) -> bool {
        match self {
            Self::Steps(limit) => progress.completed_steps() >= *limit,
            Self::UnimprovedSteps(limit) => progress.unimproved_steps() >= *limit,
            Self::AcceptedMoves(limit) => accepted_moves >= *limit,
            Self::BestScore(target) => best >= target,
            Self::Feasible => best.is_feasible(),
            Self::All(children) => children
                .iter()
                .all(|child| child.is_reached(best, accepted_moves, progress)),
            Self::Any(children) => children
                .iter()
                .any(|child| child.is_reached(best, accepted_moves, progress)),
        }
    }
}
