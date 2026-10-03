//! Ordered construction, heuristic and exact phases.
use crate::*;
use std::time::Duration;

/// Enumerate complete assignments using the solve's objective and shared controls.
/// Each candidate counts as one score and one move evaluation; there is no initial
/// rescore and no local-search step. Polling is cooperative between assignments.
/// Only exhausting the full space proves optimality. Runtime can be factorial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactPhaseConfig<'a, S = SimpleScore> {
    pub max_evaluations: u64,
    pub max_move_evaluations: Option<u64>,
    pub max_time: Option<Duration>,
    pub cancellation: Option<&'a CancellationToken>,
    pub best_score_limit: Option<S>,
    pub stop_when_feasible: bool,
}
impl<S> Default for ExactPhaseConfig<'_, S> {
    fn default() -> Self {
        Self {
            max_evaluations: 10_000,
            max_move_evaluations: None,
            max_time: None,
            cancellation: None,
            best_score_limit: None,
            stop_when_feasible: false,
        }
    }
}
impl<'a, S> From<ExactSearchConfig<'a>> for ExactPhaseConfig<'a, S> {
    fn from(c: ExactSearchConfig<'a>) -> Self {
        Self {
            max_evaluations: c.max_evaluations,
            max_time: c.max_time,
            cancellation: c.cancellation,
            ..Self::default()
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactPhaseTermination {
    Exhausted,
    EvaluationLimit,
    MoveEvaluationLimit,
    TimeLimit,
    UnimprovedTimeLimit,
    UnimprovedStepLimit,
    Cancelled,
    BestScoreLimit,
    FeasibleScore,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolverPhase<'a, S = SimpleScore> {
    LocalSearch(LocalSearchConfig<'a, S>),
    Exact(ExactPhaseConfig<'a, S>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MixedSolverPhase<'a, S = SimpleScore> {
    LocalSearch(MixedLocalSearchConfig<'a, S>),
    Exact(ExactPhaseConfig<'a, S>),
}
impl<'a, S: Clone> MixedSolverPhase<'a, S> {
    pub(crate) fn common(&self) -> SolverPhase<'a, S> {
        match self {
            Self::LocalSearch(c) => SolverPhase::LocalSearch(c.search.clone()),
            Self::Exact(c) => SolverPhase::Exact(c.clone()),
        }
    }
    pub(crate) fn local(&self) -> &MixedLocalSearchConfig<'a, S> {
        match self {
            Self::LocalSearch(c) => c,
            Self::Exact(_) => unreachable!("local-search request uses a local-search phase"),
        }
    }
}
/// Construct first, then execute improvement phases in order. Old SolverConfig
/// values convert into this plan without changing their behavior. All phases must
/// use the same deterministic objective. Exact search may complete a partial
/// construction; local search requires a complete starting assignment.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap_or(0)))?;
/// let result = solve(&p, &n, SolverPlan {
///     phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
///     ..Default::default()
/// })?;
/// assert!(result.is_optimal());
/// assert_eq!(result.score, Some(SimpleScore::new(1)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverPlan<'a, S = SimpleScore> {
    pub limits: SolverLimits<'a, S>,
    pub construction: ConstructionConfig<'a>,
    pub phases: Vec<SolverPhase<'a, S>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedSolverPlan<'a, S = SimpleScore> {
    pub limits: SolverLimits<'a, S>,
    pub construction: MixedConstructionConfig<'a>,
    pub phases: Vec<MixedSolverPhase<'a, S>>,
}
impl<S> Default for SolverPlan<'_, S> {
    fn default() -> Self {
        SolverConfig::default().into()
    }
}
impl<S> Default for MixedSolverPlan<'_, S> {
    fn default() -> Self {
        MixedSolverConfig::default().into()
    }
}
impl<'a, S> From<SolverConfig<'a, S>> for SolverPlan<'a, S> {
    fn from(c: SolverConfig<'a, S>) -> Self {
        Self {
            limits: c.limits,
            construction: c.construction,
            phases: c
                .local_search
                .into_iter()
                .map(SolverPhase::LocalSearch)
                .collect(),
        }
    }
}
impl<'a, S> From<MixedSolverConfig<'a, S>> for MixedSolverPlan<'a, S> {
    fn from(c: MixedSolverConfig<'a, S>) -> Self {
        Self {
            limits: c.limits,
            construction: c.construction,
            phases: c
                .local_search
                .into_iter()
                .map(MixedSolverPhase::LocalSearch)
                .collect(),
        }
    }
}
