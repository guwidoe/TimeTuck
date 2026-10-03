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
    /// Fill remaining required values, preserving the preceding phase's assignment.
    Construction(ConstructionConfig<'a>),
    LocalSearch(LocalSearchConfig<'a, S>),
    Exact(ExactPhaseConfig<'a, S>),
    /// Run the typed command callback bound to this phase index.
    Custom(CustomPhaseConfig<'a>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MixedSolverPhase<'a, S = SimpleScore> {
    /// Resume construction in the configured component order.
    Construction(MixedConstructionConfig<'a>),
    LocalSearch(MixedLocalSearchConfig<'a, S>),
    Exact(ExactPhaseConfig<'a, S>),
    /// Run the typed command callback bound to this phase index.
    Custom(CustomPhaseConfig<'a>),
}
impl<'a, S: Clone> MixedSolverPhase<'a, S> {
    pub(crate) fn common(&self) -> SolverPhase<'a, S> {
        match self {
            Self::Construction(c) => SolverPhase::Construction(c.construction),
            Self::LocalSearch(c) => SolverPhase::LocalSearch(c.search.clone()),
            Self::Exact(c) => SolverPhase::Exact(c.clone()),
            Self::Custom(c) => SolverPhase::Custom(*c),
        }
    }
    pub(crate) fn construction(&self) -> MixedConstructionConfig<'a> {
        match self {
            Self::Construction(c) => *c,
            _ => unreachable!("construction request uses a construction phase"),
        }
    }
    pub(crate) fn local(&self) -> &MixedLocalSearchConfig<'a, S> {
        match self {
            Self::LocalSearch(c) => c,
            Self::Construction(_) | Self::Exact(_) | Self::Custom(_) => {
                unreachable!("local-search request uses a local-search phase")
            }
        }
    }
}
/// Construct first, then execute subsequent phases in order. Old SolverConfig
/// values convert into this plan without changing their behavior. All phases must
/// use the same deterministic objective. Exact search may complete a partial
/// construction, as may another construction phase; local search requires a
/// complete starting assignment. Each construction phase preserves values already
/// supplied and has its own budget and scoring initialization.
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
///
/// A later construction phase can finish a locally limited first phase:
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3])?;
/// let n = ConstraintNetwork::<_, SimpleScore>::new(3);
/// let result = solve(&p, &n, SolverPlan {
///     construction: ConstructionConfig { max_evaluations: 2, ..Default::default() },
///     phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
///     ..Default::default()
/// })?;
/// assert!(result.complete);
/// assert_eq!(result.assignment.values(), &[Some(1), Some(1), Some(1)]);
/// assert_eq!(result.phases.len(), 2);
/// assert_eq!(result.phases[1].work.move_evaluations, 4);
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

/// An explicit, nonempty phase sequence. Unlike [`SolverPlan`], this does not
/// insert construction. A first local-search phase requires a complete model
/// initial assignment; exact search can start from an uninitialized model.
/// Phase indices refer directly to this vector. Empty sequences are rejected
/// before scoring or stream graph creation. The default retains construction
/// followed by local search; existing configurations convert without changes.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap_or(0)))?;
/// let result = solve(&p, &n, SolverSequence {
///     phases: vec![SolverPhase::LocalSearch(LocalSearchConfig::default())],
///     ..Default::default()
/// })?;
/// assert_eq!(result.score, Some(SimpleScore::new(1)));
/// assert_eq!(result.phases.len(), 1);
/// assert_eq!(result.phases[0].index, 0);
/// assert_eq!(result.work.evaluations, 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverSequence<'a, S = SimpleScore> {
    pub limits: SolverLimits<'a, S>,
    pub phases: Vec<SolverPhase<'a, S>>,
}
/// Explicit mixed-model phases, with the same initial-state and validation rules
/// as [`SolverSequence`]. Each construction/local phase retains its own settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedSolverSequence<'a, S = SimpleScore> {
    pub limits: SolverLimits<'a, S>,
    pub phases: Vec<MixedSolverPhase<'a, S>>,
}
impl<S> Default for SolverSequence<'_, S> {
    fn default() -> Self {
        SolverPlan::default().into()
    }
}
impl<S> Default for MixedSolverSequence<'_, S> {
    fn default() -> Self {
        MixedSolverPlan::default().into()
    }
}
impl<'a, S> From<SolverPlan<'a, S>> for SolverSequence<'a, S> {
    fn from(c: SolverPlan<'a, S>) -> Self {
        Self {
            limits: c.limits,
            phases: std::iter::once(SolverPhase::Construction(c.construction))
                .chain(c.phases)
                .collect(),
        }
    }
}
impl<'a, S> From<MixedSolverPlan<'a, S>> for MixedSolverSequence<'a, S> {
    fn from(c: MixedSolverPlan<'a, S>) -> Self {
        Self {
            limits: c.limits,
            phases: std::iter::once(MixedSolverPhase::Construction(c.construction))
                .chain(c.phases)
                .collect(),
        }
    }
}
impl<'a, S> From<SolverConfig<'a, S>> for SolverSequence<'a, S> {
    fn from(c: SolverConfig<'a, S>) -> Self {
        SolverPlan::from(c).into()
    }
}
impl<'a, S> From<MixedSolverConfig<'a, S>> for MixedSolverSequence<'a, S> {
    fn from(c: MixedSolverConfig<'a, S>) -> Self {
        MixedSolverPlan::from(c).into()
    }
}
