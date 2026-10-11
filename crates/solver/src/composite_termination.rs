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
    /// Evaluated candidates across all phases, excluding initialization and
    /// diagnostic scoring. Uses the same accounting as MoveEvaluationCounts.
    /// Reconstruction publishes its candidates after each rebuild, so a rebuild
    /// may cross this cooperative threshold; use max_move_evaluations for a hard cap.
    MoveEvaluations(u64),
    /// Scored states across all phases, including every phase/rebuild initialization
    /// and bounded-search node. Diagnostics, explanations and graph preparation are
    /// excluded. Custom operations follow SolverWork's attempted-operation accounting.
    /// Polled between candidates, including inside reconstruction scans.
    ScoreEvaluations(u64),
    /// Evaluated candidates of one native family, including rejected candidates.
    /// Composite moves count once under their composite family, not per child.
    MoveEvaluationsOf {
        kind: crate::MoveEvaluationKind,
        limit: u64,
    },
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
    /// Live candidate counts since solve entry, shared across phase handoffs.
    pub move_evaluations: crate::MoveEvaluationCounts,
    pub score_evaluations: u64,
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
            Self::ScoreEvaluations(n) => (
                StepCountLimit::new(n).is_reached(progress.score_evaluations),
                StepCountLimit::new(n).progress(progress.score_evaluations),
            ),
            Self::MoveEvaluations(n) => (
                StepCountLimit::new(n).is_reached(progress.move_evaluations.total()),
                StepCountLimit::new(n).progress(progress.move_evaluations.total()),
            ),
            Self::MoveEvaluationsOf { kind, limit } => (
                StepCountLimit::new(limit).is_reached(progress.move_evaluations.get(kind)),
                StepCountLimit::new(limit).progress(progress.move_evaluations.get(kind)),
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
    pub(crate) fn needs_score_counts(&self) -> bool {
        match self {
            Self::Condition(SearchLimit::ScoreEvaluations(_)) => true,
            Self::Condition(_) => false,
            Self::All(children) | Self::Any(children) => {
                children.iter().any(Self::needs_score_counts)
            }
        }
    }
    pub(crate) fn needs_move_counts(&self) -> bool {
        match self {
            Self::Condition(
                SearchLimit::MoveEvaluations(_) | SearchLimit::MoveEvaluationsOf { .. },
            ) => true,
            Self::Condition(_) => false,
            Self::All(children) | Self::Any(children) => {
                children.iter().any(Self::needs_move_counts)
            }
        }
    }
    pub fn status(&self, progress: SearchProgress) -> Option<TerminationState> {
        self.evaluate(|limit| limit.evaluate(progress))
    }
}

/// A typed score goal or an existing solve-wide count/time condition. Score goals
/// inspect only the best complete, structurally sound state retained by the solve.
/// They remain false before such a state exists; an accepted worsening move does
/// not undo an attained goal. Native feasibility follows `Score::is_feasible`,
/// including score types without hard levels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoredSearchLimit<S = crate::SimpleScore> {
    Search(SearchLimit),
    BestScore(S),
    Feasible,
}
/// Compose score goals with count/time limits. Existing hard limits and the
/// separate `SolverLimits::termination` expression remain independent safeguards.
/// Counter leaves use the same solve-wide totals as `SearchTermination`; phase
/// handoffs never reset them. Score goals have progress 1 once reached and no
/// estimate beforehand. No initial-score interpolation is implied.
///
/// Construction retains its final state, so its current complete score contributes
/// only at initialization and committed-placement boundaries. It is not accumulated
/// into best history while optional placements can worsen it. Candidate-scan polls
/// use the best retained from earlier phases; independent count/time alternatives
/// still apply. The actual retained handoff score enters best history after the phase.
/// Local, custom, repair and exact phases publish their retained bests within the phase.
///
/// ```
/// use timetuck::*;
/// let leaves = [
///     ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::new(10))),
///     ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(100))),
/// ];
/// let stop = ScoredSearchTermination::All(&leaves);
/// let limits = SolverLimits { scored_termination: Some(&stop), ..Default::default() };
/// assert!(limits.scored_termination.is_some());
/// ```
pub type ScoredSearchTermination<'a, S = crate::SimpleScore> =
    TerminationExpression<'a, ScoredSearchLimit<S>>;
impl<S: crate::Score> ScoredSearchTermination<'_, S> {
    /// Query with the retained complete score, or None if none exists. A negative
    /// structural score is rejected even if the caller supplies it as complete.
    pub fn status(&self, progress: SearchProgress, best: Option<&S>) -> Option<TerminationState> {
        let best = best.filter(|s| s.structural() >= 0);
        self.evaluate(|limit| {
            let reached = match limit {
                ScoredSearchLimit::Search(limit) => return limit.evaluate(progress),
                ScoredSearchLimit::BestScore(target) => best.is_some_and(|score| score >= target),
                ScoredSearchLimit::Feasible => best.is_some_and(crate::Score::is_feasible),
            };
            Some(TerminationState {
                reached,
                progress: reached.then_some(1.0),
            })
        })
    }
    fn any_leaf(&self, predicate: impl Fn(&ScoredSearchLimit<S>) -> bool + Copy) -> bool {
        match self {
            Self::Condition(limit) => predicate(limit),
            Self::All(children) | Self::Any(children) => {
                children.iter().any(|c| c.any_leaf(predicate))
            }
        }
    }
    pub(crate) fn needs_best_score(&self) -> bool {
        self.any_leaf(|l| !matches!(l, ScoredSearchLimit::Search(_)))
    }
    pub(crate) fn needs_score_counts(&self) -> bool {
        self.any_leaf(|l| {
            matches!(
                l,
                ScoredSearchLimit::Search(SearchLimit::ScoreEvaluations(_))
            )
        })
    }
    pub(crate) fn needs_move_counts(&self) -> bool {
        self.any_leaf(|l| {
            matches!(
                l,
                ScoredSearchLimit::Search(
                    SearchLimit::MoveEvaluations(_) | SearchLimit::MoveEvaluationsOf { .. }
                )
            )
        })
    }
}
