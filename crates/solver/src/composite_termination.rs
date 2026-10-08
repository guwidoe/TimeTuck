//! Scoped termination composition and solve-wide search limits.
use std::time::Duration;

/// An exact stop decision and an optional approximate fraction of progress.
/// Missing, negative and non-finite fractions are unsupported. Fractions above
/// one are capped. The decision never depends on floating-point rounding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminationState {
    pub reached: bool,
    pub progress: Option<f64>,
}

/// Borrowed AND/OR expression. A leaf evaluator returns None when that condition
/// does not apply to the current scope. Applicable conditions with no progress
/// estimate still participate in the exact stop decision.
///
/// AND uses the minimum supported fraction, OR the maximum. Their identities are
/// respectively (true, 1) and (false, 0), including empty/inapplicable children.
/// Every applicable leaf is evaluated once per occurrence, without latching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminationExpression<'a, C> {
    Condition(C),
    All(&'a [Self]),
    Any(&'a [Self]),
}
impl<C> TerminationExpression<'_, C> {
    pub fn evaluate(
        &self,
        mut leaf: impl FnMut(&C) -> Option<TerminationState>,
    ) -> Option<TerminationState> {
        self.evaluate_with(&mut leaf)
    }
    fn evaluate_with(
        &self,
        leaf: &mut impl FnMut(&C) -> Option<TerminationState>,
    ) -> Option<TerminationState> {
        match self {
            Self::Condition(condition) => leaf(condition).map(|mut state| {
                state.progress = state
                    .progress
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .map(|v| v.min(1.0));
                state
            }),
            Self::All(children) | Self::Any(children) => {
                let all = matches!(self, Self::All(_));
                let mut reached = all;
                let mut progress: f64 = if all { 1.0 } else { 0.0 };
                for child in *children {
                    if let Some(state) = child.evaluate_with(leaf) {
                        reached = if all {
                            reached && state.reached
                        } else {
                            reached || state.reached
                        };
                        if let Some(value) = state.progress {
                            progress = if all {
                                progress.min(value)
                            } else {
                                progress.max(value)
                            };
                        }
                    }
                }
                Some(TerminationState {
                    reached,
                    progress: Some(progress),
                })
            }
        }
    }
}

/// Limits composed by SolverLimits::termination. Search steps span local and
/// repair phases. Construction steps are counted separately; CombinedSteps sums
/// both counters. Initialization, custom commands and exact candidates are not
/// steps. Elapsed time spans the entire solve. Stagnation begins at the first search phase and resets
/// only on strict improvement. Before search starts, stagnation leaves are not reached and have no
/// progress estimate. Hard budgets and cancellation remain independent safeguards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchLimit {
    /// Completed local steps and accepted changed repair/fine moves.
    Steps(u64),
    /// Completed construction placements across all construction phases. Includes
    /// scored no-change placements and retained interrupted prefixes; excludes
    /// discarded incomplete scans and placements with no evaluated candidate.
    ConstructionSteps(u64),
    /// Sum of construction and search steps; neither counter resets at handoff.
    CombinedSteps(u64),
    UnimprovedSteps(u64),
    Time(Duration),
    UnimprovedTime(Duration),
}
/// Combine shared step and time limits without allocating an expression tree.
/// ```
/// use timetuck::{SearchLimit, SearchTermination, SolverLimits, SimpleScore};
/// use std::time::Duration;
/// let leaves = [SearchTermination::Condition(SearchLimit::Steps(100)),
///               SearchTermination::Condition(SearchLimit::Time(Duration::from_secs(1)))];
/// let condition = SearchTermination::Any(&leaves);
/// let limits = SolverLimits::<SimpleScore> { termination: Some(&condition), ..Default::default() };
/// assert!(limits.termination.is_some());
/// ```
pub type SearchTermination<'a> = TerminationExpression<'a, SearchLimit>;

/// Snapshot used to query a termination expression without running a solver.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchProgress {
    pub elapsed: Duration,
    pub completed_steps: u64,
    pub construction_steps: u64,
    pub unimproved_steps: Option<u64>,
    pub unimproved_time: Option<Duration>,
}
impl SearchLimit {
    pub fn evaluate(self, progress: SearchProgress) -> Option<TerminationState> {
        use crate::{ElapsedTimeLimit, StepCountLimit};
        let (reached, fraction) = match self {
            Self::Steps(n) => (
                StepCountLimit::new(n).is_reached(progress.completed_steps),
                StepCountLimit::new(n).progress(progress.completed_steps),
            ),
            Self::ConstructionSteps(n) => (
                StepCountLimit::new(n).is_reached(progress.construction_steps),
                StepCountLimit::new(n).progress(progress.construction_steps),
            ),
            Self::CombinedSteps(n) => {
                let steps = progress
                    .completed_steps
                    .saturating_add(progress.construction_steps);
                (
                    StepCountLimit::new(n).is_reached(steps),
                    StepCountLimit::new(n).progress(steps),
                )
            }
            Self::UnimprovedSteps(n) => {
                let Some(steps) = progress.unimproved_steps else {
                    return Some(TerminationState {
                        reached: false,
                        progress: None,
                    });
                };
                (
                    StepCountLimit::new(n).is_reached(steps),
                    StepCountLimit::new(n).progress(steps),
                )
            }
            Self::Time(n) => (
                ElapsedTimeLimit::new(n).is_reached(progress.elapsed),
                ElapsedTimeLimit::new(n).progress(progress.elapsed),
            ),
            Self::UnimprovedTime(n) => {
                let Some(elapsed) = progress.unimproved_time else {
                    return Some(TerminationState {
                        reached: false,
                        progress: None,
                    });
                };
                (
                    ElapsedTimeLimit::new(n).is_reached(elapsed),
                    ElapsedTimeLimit::new(n).progress(elapsed),
                )
            }
        };
        Some(TerminationState {
            reached,
            progress: Some(fraction),
        })
    }
}
impl SearchTermination<'_> {
    pub fn status(&self, progress: SearchProgress) -> Option<TerminationState> {
        self.evaluate(|limit| limit.evaluate(progress))
    }
}
