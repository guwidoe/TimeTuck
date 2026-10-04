//! Ordered construction, local, repair and exact phases under shared controls.
#[path = "solver_bounded.rs"]
mod bounded;
#[path = "solver_exact.rs"]
mod exact;
use crate::*;
use std::{error::Error, fmt, time::Duration};

/// Behavior when a phase leaves an incomplete assignment for local or repair search.
/// Earlier solve-wide stopping reasons take precedence over this policy.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2])?;
/// let n = ConstraintNetwork::<_, SimpleScore>::new(2);
/// let c = SolverConfig {
///     construction: ConstructionConfig { max_steps: Some(1), ..Default::default() },
///     limits: SolverLimits { incomplete_handoff: IncompleteHandoffPolicy::Error, ..Default::default() },
///     ..Default::default()
/// };
/// let mut initialized = false;
/// let r = solve_with_events(&p, &n, c, |event| {
///     if let SolverEvent::FirstInitialized { initialization, complete, .. } = event {
///         assert!(initialization.terminated_early);
///         assert!(!complete);
///         initialized = true;
///     }
/// });
/// assert!(initialized);
/// assert!(matches!(r, Err(SolverError::IncompleteLocalSearch { phase: 1 })));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IncompleteHandoffPolicy {
    /// Return the retained partial solution with IncompleteConstruction.
    #[default]
    ReturnPartial,
    /// Report an invalid local-search start after publishing initialization.
    Error,
}

/// Shared limits for one solve. Phase-local controls can stop a phase earlier.
/// Diagnostic callbacks are excluded from both evaluation counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverLimits<'a, S = SimpleScore> {
    pub max_evaluations: u64,
    pub max_move_evaluations: Option<u64>,
    /// Cooperative deadline shared by construction, improvement and stream graph
    /// preparation. Initial scoring is allowed at zero time. None disables it.
    pub max_time: Option<Duration>,
    /// Solve-wide stagnation, starting when the first improvement phase is ready.
    /// Construction is excluded. Later phases/handoffs do not reset the timer.
    /// Only strict retained-best improvement resets it, before expiry.
    pub max_unimproved_time: Option<Duration>,
    /// Completed local-search steps and accepted changed repairs since the last
    /// strict retained-best improvement.
    /// Counts across phases; construction and initialization do not count.
    pub max_unimproved_steps: Option<u64>,
    pub cancellation: Option<&'a CancellationToken>,
    pub best_score_limit: Option<S>,
    pub stop_when_feasible: bool,
    /// Applies to an incomplete handoff into a following local or repair phase.
    /// Final partial results and handoffs into construction/custom/exact remain allowed.
    pub incomplete_handoff: IncompleteHandoffPolicy,
}
impl<S> Default for SolverLimits<'_, S> {
    fn default() -> Self {
        Self {
            max_evaluations: 100_000,
            max_move_evaluations: None,
            max_time: None,
            max_unimproved_time: None,
            max_unimproved_steps: None,
            cancellation: None,
            best_score_limit: None,
            stop_when_feasible: false,
            incomplete_handoff: IncompleteHandoffPolicy::ReturnPartial,
        }
    }
}
/// Always construct first, then run the listed local-search phases in order.
/// Each improvement phase starts from the retained best complete assignment,
/// with fresh acceptor state and its configured random seed. Empty local_search
/// means construction only. A shared deadline can bound every phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverConfig<'a, S = SimpleScore> {
    pub limits: SolverLimits<'a, S>,
    pub construction: ConstructionConfig<'a>,
    pub local_search: Vec<LocalSearchConfig<'a, S>>,
}
impl<S> Default for SolverConfig<'_, S> {
    fn default() -> Self {
        Self {
            limits: SolverLimits::default(),
            construction: ConstructionConfig::default(),
            local_search: vec![LocalSearchConfig::default()],
        }
    }
}
/// Mixed phases additionally select construction order and joint neighborhoods.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedSolverConfig<'a, S = SimpleScore> {
    pub limits: SolverLimits<'a, S>,
    pub construction: MixedConstructionConfig<'a>,
    pub local_search: Vec<MixedLocalSearchConfig<'a, S>>,
}
impl<S> Default for MixedSolverConfig<'_, S> {
    fn default() -> Self {
        Self {
            limits: SolverLimits::default(),
            construction: MixedConstructionConfig::default(),
            local_search: vec![MixedLocalSearchConfig::default()],
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolverWork {
    /// Includes construction/local-search initialization; exact candidates need no rescore.
    pub evaluations: u64,
    /// Candidates, including exact assignments; initialization and diagnostics are excluded.
    pub move_evaluations: u64,
    pub constraint_evaluations: u128,
    /// Local-search steps and accepted changed repairs; excludes construction steps.
    pub completed_steps: u64,
    pub accepted_moves: u64,
    /// Started ruin-and-recreate attempts, including incomplete, rejected and unchanged repairs.
    /// Reconstruction candidates are counted separately in `move_evaluations`.
    pub repair_attempts: u64,
    /// Repair attempts producing a complete scored assignment, including rejected and unchanged results.
    /// Complete does not imply that every reconstruction candidate was evaluated.
    pub complete_reconstructions: u64,
}
impl SolverWork {
    fn add(&mut self, other: Self) {
        self.evaluations += other.evaluations;
        self.move_evaluations += other.move_evaluations;
        self.constraint_evaluations += other.constraint_evaluations;
        self.completed_steps += other.completed_steps;
        self.accepted_moves += other.accepted_moves;
        self.repair_attempts += other.repair_attempts;
        self.complete_reconstructions += other.complete_reconstructions;
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseTermination {
    Construction(ConstructionTermination),
    LocalSearch(LocalSearchTermination),
    RuinRecreate(RuinRecreateTermination),
    Exact(ExactPhaseTermination),
    Custom(CustomPhaseTermination),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseReport<S> {
    /// Index in the explicit sequence; converted plans prepend construction at zero.
    pub index: usize,
    pub termination: PhaseTermination,
    pub work: SolverWork,
    /// The phase's retained score, including a partial construction score.
    pub score: Option<S>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverTermination {
    /// All planned phases ended, possibly at their own limits. Not optimality.
    PlanCompleted,
    /// A complete scored solution and an exhausted exact search. Pruned branches
    /// count as exhausted only under the configured caller bound contract.
    Optimal,
    EvaluationLimit,
    MoveEvaluationLimit,
    TimeLimit,
    UnimprovedTimeLimit,
    UnimprovedStepLimit,
    Cancelled,
    BestScoreLimit,
    FeasibleScore,
    IncompleteConstruction,
    NoCompleteAssignment,
}
/// The retained complete best, partial initialization progress, or model initial
/// assignment when no scoring occurred. Equal-score phase results retain the earlier assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverResult<A, S> {
    pub assignment: A,
    pub score: Option<S>,
    pub complete: bool,
    pub work: SolverWork,
    pub phases: Vec<PhaseReport<S>>,
    pub termination: SolverTermination,
}
impl<A, S: Clone> SolverResult<A, S> {
    pub fn is_optimal(&self) -> bool {
        self.termination == SolverTermination::Optimal && self.complete && self.score.is_some()
    }
    /// Complete, scored solution; completeness does not imply hard feasibility.
    pub fn solution(&self) -> Option<(&A, S)> {
        if self.complete {
            self.score.clone().map(|s| (&self.assignment, s))
        } else {
            None
        }
    }
}
#[derive(Debug)]
pub enum SolverError<E> {
    Repair {
        phase: usize,
        error: RuinRecreateError,
    },
    InvalidScoreTrend {
        phase: usize,
        error: ScoreTrendError,
    },
    /// A visited eligible leaf exceeded a declared ancestor bound. Scores are
    /// retained as Debug strings so the scoring error type remains unchanged.
    InvalidOptimisticBound {
        phase: usize,
        score: String,
        bound: String,
    },
    /// Explicit sequences must contain at least one phase.
    EmptyPlan,
    /// The preceding phase completed with a partial retained assignment and no
    /// earlier shared stopping reason. No local/repair scoring or factory ran.
    IncompleteLocalSearch {
        phase: usize,
    },
    /// A custom phase requires a typed command callback.
    UnboundCustomPhase {
        phase: usize,
    },
    Custom {
        phase: usize,
        error: CustomPhaseError<E>,
    },
    InvalidAcceptance {
        phase: usize,
        error: AcceptanceError,
    },
    /// A global token and a distinct phase token cannot both control one phase.
    ConflictingCancellation {
        phase: usize,
    },
    Phase {
        phase: usize,
        error: E,
    },
}
impl<E: fmt::Display> fmt::Display for SolverError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScoreTrend { phase, error } => write!(f, "phase {phase}: {error}"),
            Self::InvalidOptimisticBound {
                phase,
                score,
                bound,
            } => write!(
                f,
                "phase {phase}: complete score {score} exceeds optimistic bound {bound}"
            ),
            Self::IncompleteLocalSearch { phase } => write!(
                f,
                "improvement phase {phase} needs to start from an initialized solution"
            ),
            Self::UnboundCustomPhase { phase } => {
                write!(f, "custom phase {phase} has no command callback")
            }
            Self::Custom { phase, error } => write!(f, "custom phase {phase} failed: {error}"),
            Self::EmptyPlan => write!(f, "a solve sequence must contain at least one phase"),
            Self::InvalidAcceptance { phase, error } => {
                write!(f, "invalid acceptance in phase {phase}: {error}")
            }
            Self::ConflictingCancellation { phase } => write!(
                f,
                "phase {phase} uses a different cancellation token from the solve"
            ),
            Self::Phase { phase, error } => write!(f, "phase {phase} failed: {error}"),
            Self::Repair { phase, error } => write!(f, "repair phase {phase} failed: {error}"),
        }
    }
}
impl<E: Error + 'static> Error for SolverError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidAcceptance { error, .. } => Some(error),
            Self::InvalidScoreTrend { error, .. } => Some(error),
            Self::Phase { error, .. } => Some(error),
            Self::Custom { error, .. } => Some(error),
            Self::Repair { error, .. } => Some(error),
            Self::EmptyPlan
            | Self::InvalidOptimisticBound { .. }
            | Self::IncompleteLocalSearch { .. }
            | Self::ConflictingCancellation { .. }
            | Self::UnboundCustomPhase { .. } => None,
        }
    }
}

#[derive(Clone)]
enum PhaseRequest<'a, S> {
    Construction(ConstructionConfig<'a>),
    LocalSearch(LocalSearchConfig<'a, S>),
    RuinRecreate(RuinRecreateConfig<'a, S>),
    Exact(ExactPhaseConfig<'a, S>, Option<S>),
    Custom(CustomPhaseConfig<'a>, Option<S>, bool),
}
// Clock readings are elapsed from entry to run(), including plan validation.
// Clip immediately before phase entry, after any graph factory preparation.
struct SolveControl<'c> {
    limit: Option<Duration>,
    clock: &'c mut dyn FnMut() -> Duration,
    last: Duration,
    shared_bound: bool,
    unimproved_limit: Option<Duration>,
    last_improvement: Option<Duration>,
    unimproved_expired: bool,
    unimproved_steps_limit: Option<u64>,
    steps: StepProgress,
    started: bool,
    steps_expired: bool,
}
impl SolveControl<'_> {
    fn sample(&mut self) {
        self.last = self.last.max((self.clock)());
    }
    fn stagnated(&mut self) -> bool {
        if let (Some(limit), Some(last_improvement)) =
            (self.unimproved_limit, self.last_improvement)
        {
            self.sample();
            self.unimproved_expired |= self.last - last_improvement >= limit;
        }
        self.unimproved_expired
    }
    fn step_stagnated(&mut self) -> bool {
        self.steps_expired |= self.started
            && self
                .unimproved_steps_limit
                .is_some_and(|limit| self.steps.unimproved_steps() >= limit);
        self.steps_expired
    }
    fn monitor(&mut self) -> Option<&mut dyn crate::time_limit::SearchControl> {
        if self.unimproved_limit.is_some() || self.unimproved_steps_limit.is_some() {
            Some(self)
        } else {
            None
        }
    }
    fn remaining(&mut self) -> Option<Duration> {
        self.limit.map(|limit| {
            self.sample();
            limit.saturating_sub(self.last)
        })
    }
    fn expired(&mut self) -> bool {
        self.remaining() == Some(Duration::ZERO)
    }
    fn bound(&mut self, own: &mut Option<Duration>) {
        self.shared_bound = false;
        if let Some(remaining) = self.remaining() {
            self.shared_bound = own.is_none_or(|limit| remaining <= limit);
            *own = Some(own.map_or(remaining, |limit| limit.min(remaining)));
        }
    }
}
impl crate::time_limit::SearchControl for SolveControl<'_> {
    fn start_search(&mut self) {
        self.started = true;
        if self.unimproved_limit.is_some() && self.last_improvement.is_none() {
            self.sample();
            self.last_improvement = Some(self.last);
        }
    }
    fn record_improvement(&mut self) {
        if !self.step_stagnated() {
            self.steps.record_improvement();
        }
        // Keep late improvements in the solution, but never revive an expired solve.
        if !self.stagnated() && self.last_improvement.is_some() {
            self.last_improvement = Some(self.last);
        }
    }
    fn complete_step(&mut self, improved: bool) {
        if self.unimproved_steps_limit.is_some() && !self.step_stagnated() {
            self.steps.complete_step(improved);
            self.step_stagnated();
        }
    }
    fn stop(&mut self) -> Option<LocalSearchTermination> {
        if self.stagnated() {
            Some(LocalSearchTermination::UnimprovedTimeLimit)
        } else if self.step_stagnated() {
            Some(LocalSearchTermination::UnimprovedStepLimit)
        } else {
            None
        }
    }
}
impl<S> PhaseRequest<'_, S> {
    fn with_time(mut self, time: &mut SolveControl<'_>) -> Self {
        match &mut self {
            Self::Construction(c) => time.bound(&mut c.max_time),
            Self::LocalSearch(c) => time.bound(&mut c.max_time),
            Self::RuinRecreate(c) => time.bound(&mut c.max_time),
            Self::Exact(c, _) => time.bound(&mut c.max_time),
            Self::Custom(c, ..) => time.bound(&mut c.max_time),
        }
        self
    }
}
struct PhaseOutput<A, S> {
    assignment: Option<A>,
    score: Option<S>,
    complete: bool,
    work: SolverWork,
    termination: PhaseTermination,
}
fn smaller(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
/// Native origin of a retained-best update within a solve plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverPhaseKind {
    Custom,
    Construction,
    LocalSearch,
    RuinRecreate,
    Exact,
}
/// A synchronous retained-best update from one ordered solve. Phase indices refer
/// to the explicit sequence; converted plans prepend construction at zero.
/// Equal or worse phase-initial scores are suppressed across phase boundaries.
/// Native phase metadata is not a Java event-producer identity. Borrowed state
/// must be cloned for asynchronous publication. Observers must not mutate facts.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2])])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap_or(0)))?;
/// let mut updates = Vec::new();
/// let r = solve_with_observer(&p, &n, SolverConfig::default(), |u| {
///     updates.push((u.phase, u.kind, u.score));
/// })?;
/// assert_eq!(updates.last().unwrap().2, r.score.unwrap());
/// assert!(updates.windows(2).all(|w| w[0].2 < w[1].2));
/// assert_eq!(updates[0].1, SolverPhaseKind::Construction);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct SolverUpdate<'a, A, S> {
    pub phase: usize,
    pub kind: SolverPhaseKind,
    pub assignment: &'a A,
    pub score: S,
    pub complete: bool,
}
/// Origin and completion status of the phase before the first local or repair search.
/// Early termination and assignment completeness are independent properties.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverInitialization {
    pub phase: usize,
    pub kind: SolverPhaseKind,
    pub terminated_early: bool,
}
/// Synchronous plan notifications. Initialization fires once when the phase
/// immediately before the first requested local or repair search ends successfully, even
/// if a limit left it partial or the solve stops at that boundary. It is omitted
/// when improvement has no preceding phase, is absent, or the producer fails. No rescoring is
/// performed; a zero-work producing phase may have no score. Borrowed assignments
/// must be cloned for asynchronous delivery. Callback time belongs to solve limits
/// and panics propagate. Best updates retain their usual duplicate suppression.
///
/// ```
/// use timetuck::*;
/// let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2])?;
/// let network = ConstraintNetwork::<_, SimpleScore>::new(2);
/// let mut initialized = Vec::new();
/// let result = solve_with_events(&problem, &network, SolverPlan::default(), |event| {
///     if let SolverEvent::FirstInitialized { initialization, complete, .. } = event {
///         initialized.push((initialization, complete));
///     }
/// })?;
/// assert!(result.complete);
/// assert_eq!(initialized.len(), 1);
/// assert_eq!(initialized[0], (SolverInitialization {
///     phase: 0, kind: SolverPhaseKind::Construction, terminated_early: false,
/// }, true));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub enum SolverEvent<'a, A, S> {
    Best(SolverUpdate<'a, A, S>),
    FirstInitialized {
        initialization: SolverInitialization,
        assignment: &'a A,
        score: Option<S>,
        complete: bool,
    },
}
enum PlanCallback<'a, A, S> {
    Best(&'a mut dyn FnMut(SolverUpdate<'_, A, S>)),
    Events(&'a mut dyn FnMut(SolverEvent<'_, A, S>)),
}
type PlanObserver<'a, A, S> = Option<PlanCallback<'a, A, S>>;
struct PlanObservation<'a, A, S> {
    callback: PlanObserver<'a, A, S>,
    score: Option<S>,
    complete: bool,
}
impl<'a, A, S: Score> PlanObservation<'a, A, S> {
    fn new(callback: PlanObserver<'a, A, S>) -> Self {
        Self {
            callback,
            score: None,
            complete: false,
        }
    }
    fn initialize(
        &mut self,
        phase: usize,
        first_local: Option<usize>,
        result: &SolverResult<A, S>,
        termination: PhaseTermination,
    ) {
        let Some(PlanCallback::Events(callback)) = &mut self.callback else {
            return;
        };
        if first_local != Some(phase + 1) {
            return;
        }
        let (kind, terminated_early) = match termination {
            PhaseTermination::Construction(reason) => (
                SolverPhaseKind::Construction,
                reason != ConstructionTermination::Completed,
            ),
            PhaseTermination::Exact(reason) => (
                SolverPhaseKind::Exact,
                reason != ExactPhaseTermination::Exhausted,
            ),
            PhaseTermination::Custom(reason) => (
                SolverPhaseKind::Custom,
                reason != CustomPhaseTermination::Completed,
            ),
            PhaseTermination::LocalSearch(_) | PhaseTermination::RuinRecreate(_) => {
                unreachable!("initialization precedes first local search")
            }
        };
        callback(SolverEvent::FirstInitialized {
            initialization: SolverInitialization {
                phase,
                kind,
                terminated_early,
            },
            assignment: &result.assignment,
            score: result.score.clone(),
            complete: result.complete,
        });
    }
    fn notify(&mut self, phase: usize, kind: SolverPhaseKind, update: SolutionUpdate<'_, A, S>) {
        let Some(callback) = &mut self.callback else {
            return;
        };
        if (!self.complete && update.complete)
            || self.score.as_ref().is_none_or(|best| update.score > *best)
        {
            self.score = Some(update.score.clone());
            self.complete = update.complete;
            let update = SolverUpdate {
                phase,
                kind,
                assignment: update.assignment,
                score: update.score,
                complete: update.complete,
            };
            match callback {
                PlanCallback::Best(callback) => callback(update),
                PlanCallback::Events(callback) => callback(SolverEvent::Best(update)),
            }
        }
    }
}
impl<S> PhaseRequest<'_, S> {
    fn kind(&self) -> SolverPhaseKind {
        match self {
            Self::Construction(_) => SolverPhaseKind::Construction,
            Self::LocalSearch(_) => SolverPhaseKind::LocalSearch,
            Self::RuinRecreate(_) => SolverPhaseKind::RuinRecreate,
            Self::Exact(..) => SolverPhaseKind::Exact,
            Self::Custom(..) => SolverPhaseKind::Custom,
        }
    }
}

struct InitialState<A> {
    assignment: A,
    complete: bool,
}
fn run<A, S: AdditiveScore, E>(
    limits: SolverLimits<'_, S>,
    local: &[SolverPhase<'_, S>],
    initial: impl FnOnce() -> InitialState<A>,
    mut observation: PlanObservation<'_, A, S>,
    phase: impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveControl<'_>,
        &mut PlanObservation<'_, A, S>,
    ) -> Result<PhaseOutput<A, S>, SolverError<E>>,
) -> Result<SolverResult<A, S>, SolverError<E>>
where
    A: Clone,
{
    let mut clock = crate::time_limit::monotonic_clock(
        limits.max_time.is_some() || limits.max_unimproved_time.is_some(),
    );
    run_with_clock_observed(limits, local, initial, &mut observation, phase, &mut clock)
}
#[cfg(test)]
fn run_with_clock<A: Clone, S: AdditiveScore, E>(
    limits: SolverLimits<'_, S>,
    construction: ConstructionConfig<'_>,
    local: &[SolverPhase<'_, S>],
    initial: InitialState<A>,
    mut phase: impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveControl<'_>,
    ) -> Result<PhaseOutput<A, S>, E>,
    clock: &mut dyn FnMut() -> Duration,
) -> Result<SolverResult<A, S>, SolverError<E>> {
    let phases: Vec<_> = std::iter::once(SolverPhase::Construction(construction))
        .chain(local.iter().cloned())
        .collect();
    run_with_clock_observed(
        limits,
        &phases,
        || initial,
        &mut PlanObservation::new(None),
        |i, a, r, t, _| phase(i, a, r, t).map_err(|error| SolverError::Phase { phase: i, error }),
        clock,
    )
}
fn run_with_clock_observed<A, S: AdditiveScore, E>(
    limits: SolverLimits<'_, S>,
    local: &[SolverPhase<'_, S>],
    initial: impl FnOnce() -> InitialState<A>,
    observation: &mut PlanObservation<'_, A, S>,
    mut phase: impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveControl<'_>,
        &mut PlanObservation<'_, A, S>,
    ) -> Result<PhaseOutput<A, S>, SolverError<E>>,
    clock: &mut dyn FnMut() -> Duration,
) -> Result<SolverResult<A, S>, SolverError<E>>
where
    A: Clone,
{
    let mut time = SolveControl {
        limit: limits.max_time,
        clock,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: limits.max_unimproved_time,
        last_improvement: None,
        unimproved_expired: false,
        unimproved_steps_limit: limits.max_unimproved_steps,
        steps: StepProgress::default(),
        started: false,
        steps_expired: false,
    };
    // Validate the entire sequence before scoring, including phases that might never run.
    if local.is_empty() {
        return Err(SolverError::EmptyPlan);
    }
    for (index, token) in local
        .iter()
        .map(|c| match c {
            SolverPhase::Construction(c) => c.cancellation,
            SolverPhase::LocalSearch(c) => c.cancellation,
            SolverPhase::RuinRecreate(c) => c.cancellation,
            SolverPhase::Exact(c) => c.cancellation,
            SolverPhase::Custom(c) => c.cancellation,
        })
        .enumerate()
    {
        if let (Some(global), Some(token)) = (limits.cancellation, token)
            && global != token
        {
            return Err(SolverError::ConflictingCancellation { phase: index });
        }
    }
    for (index, c) in local.iter().enumerate() {
        if let SolverPhase::RuinRecreate(c) = c {
            if c.minimum_ruined == 0 || c.minimum_ruined > c.maximum_ruined {
                return Err(SolverError::Repair {
                    phase: index,
                    error: RuinRecreateError::InvalidRuinRange,
                });
            }
            c.acceptance
                .validate()
                .map_err(|error| SolverError::InvalidAcceptance {
                    phase: index,
                    error,
                })?;
        }
        let SolverPhase::LocalSearch(c) = c else {
            continue;
        };
        c.validate_acceptance()
            .map_err(|error| SolverError::InvalidAcceptance {
                phase: index,
                error,
            })?;
    }
    let first_local = local.iter().position(|p| {
        matches!(
            p,
            SolverPhase::LocalSearch(_) | SolverPhase::RuinRecreate(_)
        )
    });
    let initial = initial();
    let mut result = SolverResult {
        assignment: initial.assignment,
        complete: initial.complete,
        score: None,
        work: SolverWork::default(),
        phases: Vec::new(),
        termination: SolverTermination::PlanCompleted,
    };
    for index in 0..local.len() {
        let used = result.work;
        let remaining_scores = limits.max_evaluations - used.evaluations;
        let remaining_moves = limits
            .max_move_evaluations
            .map(|max| max - used.move_evaluations);
        if index > 0 {
            let reason = if remaining_scores == 0 {
                Some(SolverTermination::EvaluationLimit)
            } else if remaining_moves == Some(0) {
                Some(SolverTermination::MoveEvaluationLimit)
            } else if time.expired() {
                Some(SolverTermination::TimeLimit)
            } else if time.stagnated() {
                Some(SolverTermination::UnimprovedTimeLimit)
            } else if time.step_stagnated() {
                Some(SolverTermination::UnimprovedStepLimit)
            } else {
                None
            };
            if let Some(reason) = reason {
                let r = &mut result;
                r.termination = reason;
                break;
            }
        }
        let construction_request = if let SolverPhase::Construction(c) = &local[index] {
            Some(*c)
        } else {
            None
        };
        let (request, global_scores, global_moves) = if let Some(mut c) = construction_request {
            let owned = (
                remaining_scores <= c.max_evaluations,
                remaining_moves.is_some_and(|n| c.max_move_evaluations.is_none_or(|m| n <= m)),
            );
            c.max_evaluations = c.max_evaluations.min(remaining_scores);
            c.max_move_evaluations = smaller(c.max_move_evaluations, remaining_moves);
            c.cancellation = c.cancellation.or(limits.cancellation);
            (PhaseRequest::Construction(c), owned.0, owned.1)
        } else if let SolverPhase::RuinRecreate(mut c) = local[index].clone() {
            let owned = (
                remaining_scores <= c.max_evaluations,
                remaining_moves.is_some_and(|n| c.max_move_evaluations.is_none_or(|m| n <= m)),
            );
            c.max_evaluations = c.max_evaluations.min(remaining_scores);
            c.max_move_evaluations = smaller(c.max_move_evaluations, remaining_moves);
            c.cancellation = c.cancellation.or(limits.cancellation);
            c.best_score_limit = match (c.best_score_limit, limits.best_score_limit.clone()) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            c.stop_when_feasible |= limits.stop_when_feasible;
            (PhaseRequest::RuinRecreate(c), owned.0, owned.1)
        } else if let SolverPhase::Custom(mut c) = local[index].clone() {
            let owned = (
                remaining_scores <= c.max_evaluations,
                remaining_moves.is_some_and(|n| c.max_move_evaluations.is_none_or(|m| n <= m)),
            );
            c.max_evaluations = c.max_evaluations.min(remaining_scores);
            c.max_move_evaluations = smaller(c.max_move_evaluations, remaining_moves);
            c.cancellation = c.cancellation.or(limits.cancellation);
            (
                PhaseRequest::Custom(
                    c,
                    limits.best_score_limit.clone(),
                    limits.stop_when_feasible,
                ),
                owned.0,
                owned.1,
            )
        } else if let SolverPhase::Exact(mut c) = local[index].clone() {
            let owned = (
                remaining_scores <= c.max_evaluations,
                remaining_moves.is_some_and(|n| c.max_move_evaluations.is_none_or(|m| n <= m)),
            );
            c.max_evaluations = c.max_evaluations.min(remaining_scores);
            c.max_move_evaluations = smaller(c.max_move_evaluations, remaining_moves);
            c.cancellation = c.cancellation.or(limits.cancellation);
            c.best_score_limit = match (c.best_score_limit, limits.best_score_limit.clone()) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            c.stop_when_feasible |= limits.stop_when_feasible;
            let prior = if result.complete {
                result.score.clone()
            } else {
                None
            };
            (PhaseRequest::Exact(c, prior), owned.0, owned.1)
        } else {
            let SolverPhase::LocalSearch(mut c) = local[index].clone() else {
                unreachable!()
            };
            let owned = (
                remaining_scores <= c.max_evaluations,
                remaining_moves.is_some_and(|n| c.max_move_evaluations.is_none_or(|m| n <= m)),
            );
            c.max_evaluations = c.max_evaluations.min(remaining_scores);
            c.max_move_evaluations = smaller(c.max_move_evaluations, remaining_moves);
            c.cancellation = c.cancellation.or(limits.cancellation);
            c.best_score_limit = match (c.best_score_limit, limits.best_score_limit.clone()) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            c.stop_when_feasible |= limits.stop_when_feasible;
            (PhaseRequest::LocalSearch(c), owned.0, owned.1)
        };
        let initial = Some(result.assignment.clone());
        let mut out = phase(index, initial, request, &mut time, observation)?;
        let r = &mut result;
        if let (Some(assignment), Some(score)) = (out.assignment.take(), out.score.as_ref())
            && ((!r.complete && out.complete) || r.score.as_ref().is_none_or(|best| score > best))
        {
            r.assignment = assignment;
            r.score = Some(score.clone());
            r.complete = out.complete;
        }
        r.work.add(out.work);
        r.phases.push(PhaseReport {
            index,
            termination: out.termination,
            work: out.work,
            score: out.score,
        });
        observation.initialize(index, first_local, r, out.termination);
        use PhaseTermination::{
            Construction as C, Custom as U, Exact as X, LocalSearch as L, RuinRecreate as R,
        };
        let reason = match out.termination {
            X(ExactPhaseTermination::Exhausted) => Some(if r.complete && r.score.is_some() {
                SolverTermination::Optimal
            } else {
                SolverTermination::NoCompleteAssignment
            }),
            C(ConstructionTermination::NoCompleteAssignment) => {
                Some(SolverTermination::NoCompleteAssignment)
            }
            R(RuinRecreateTermination::Cancelled)
            | U(CustomPhaseTermination::Cancelled)
            | C(ConstructionTermination::Cancelled)
            | L(LocalSearchTermination::Cancelled)
            | X(ExactPhaseTermination::Cancelled) => Some(SolverTermination::Cancelled),
            R(RuinRecreateTermination::EvaluationLimit)
            | U(CustomPhaseTermination::EvaluationLimit)
            | C(ConstructionTermination::EvaluationLimit)
            | L(LocalSearchTermination::EvaluationLimit)
            | X(ExactPhaseTermination::EvaluationLimit)
                if global_scores =>
            {
                Some(SolverTermination::EvaluationLimit)
            }
            R(RuinRecreateTermination::MoveEvaluationLimit)
            | U(CustomPhaseTermination::MoveEvaluationLimit)
            | C(ConstructionTermination::MoveEvaluationLimit)
            | L(LocalSearchTermination::MoveEvaluationLimit)
            | X(ExactPhaseTermination::MoveEvaluationLimit)
                if global_moves =>
            {
                Some(SolverTermination::MoveEvaluationLimit)
            }
            _ if limits
                .cancellation
                .is_some_and(CancellationToken::is_cancelled) =>
            {
                Some(SolverTermination::Cancelled)
            }
            R(RuinRecreateTermination::TimeLimit)
            | U(CustomPhaseTermination::TimeLimit)
            | C(ConstructionTermination::TimeLimit)
            | L(LocalSearchTermination::TimeLimit)
            | X(ExactPhaseTermination::TimeLimit)
                if time.shared_bound =>
            {
                Some(SolverTermination::TimeLimit)
            }
            _ if time.expired() => Some(SolverTermination::TimeLimit),
            _ if time.stagnated() => Some(SolverTermination::UnimprovedTimeLimit),
            _ if time.step_stagnated() => Some(SolverTermination::UnimprovedStepLimit),
            _ if !r.complete
                && !matches!(
                    local.get(index + 1),
                    Some(
                        SolverPhase::Exact(_)
                            | SolverPhase::Construction(_)
                            | SolverPhase::Custom(_)
                    )
                ) =>
            {
                if limits.incomplete_handoff == IncompleteHandoffPolicy::Error
                    && matches!(
                        local.get(index + 1),
                        Some(SolverPhase::LocalSearch(_) | SolverPhase::RuinRecreate(_))
                    )
                {
                    // A phase-local step/other stop can coincide with exhausting
                    // the shared budget. No following local phase would start.
                    if r.work.evaluations >= limits.max_evaluations {
                        Some(SolverTermination::EvaluationLimit)
                    } else if limits
                        .max_move_evaluations
                        .is_some_and(|n| r.work.move_evaluations >= n)
                    {
                        Some(SolverTermination::MoveEvaluationLimit)
                    } else {
                        return Err(SolverError::IncompleteLocalSearch { phase: index + 1 });
                    }
                } else {
                    Some(SolverTermination::IncompleteConstruction)
                }
            }
            _ if r.complete
                && r.score.as_ref().is_some_and(|score| {
                    limits
                        .best_score_limit
                        .as_ref()
                        .is_some_and(|target| score >= target)
                }) =>
            {
                Some(SolverTermination::BestScoreLimit)
            }
            _ if r.complete
                && limits.stop_when_feasible
                && r.score.as_ref().is_some_and(Score::is_feasible) =>
            {
                Some(SolverTermination::FeasibleScore)
            }
            _ => None,
        };
        if let Some(reason) = reason {
            r.termination = reason;
            break;
        }
    }
    Ok(result)
}

macro_rules! repaired {
    ($r:expr) => {{
        let r = $r;
        let work = SolverWork {
            evaluations: r.evaluations,
            move_evaluations: r.repair_move_evaluations,
            constraint_evaluations: r.constraint_evaluations,
            completed_steps: r.accepted_moves,
            accepted_moves: r.accepted_moves,
            repair_attempts: r.attempts,
            complete_reconstructions: r.complete_reconstructions,
        };
        let (assignment, score) = r.best.map_or((None, None), |best| {
            (Some(best.assignment), Some(best.score))
        });
        PhaseOutput {
            complete: assignment.is_some(),
            assignment,
            score,
            work,
            termination: PhaseTermination::RuinRecreate(r.termination),
        }
    }};
}

macro_rules! constructed {
    ($r:expr) => {{
        let r = $r;
        let work = SolverWork {
            evaluations: r.evaluations,
            move_evaluations: r.move_evaluations(),
            constraint_evaluations: r.constraint_evaluations,
            ..Default::default()
        };
        PhaseOutput {
            assignment: Some(r.assignment),
            score: r.score,
            complete: r.complete,
            work,
            termination: PhaseTermination::Construction(r.termination),
        }
    }};
}
macro_rules! improved {
    ($r:expr) => {{
        let r = $r;
        let work = SolverWork {
            evaluations: r.evaluations,
            move_evaluations: r.move_evaluations(),
            constraint_evaluations: r.constraint_evaluations,
            completed_steps: r.completed_steps,
            accepted_moves: r.accepted_moves,
            ..Default::default()
        };
        let (assignment, score) = r.best.map_or((None, None), |best| {
            (Some(best.assignment), Some(best.score))
        });
        PhaseOutput {
            complete: assignment.is_some(),
            assignment,
            score,
            work,
            termination: PhaseTermination::LocalSearch(r.termination),
        }
    }};
}

type CommandCallback<'a, A, M, S, E> = Option<
    &'a mut dyn FnMut(
        usize,
        &mut CustomPhaseContext<'_, A, M, S, E>,
    ) -> Result<(), CustomPhaseError<E>>,
>;
fn custom_error<E, F: From<E>>(phase: usize, error: CustomPhaseError<E>) -> SolverError<F> {
    let error = match error {
        CustomPhaseError::Scoring(e) => CustomPhaseError::Scoring(e.into()),
        CustomPhaseError::Command(e) => CustomPhaseError::Command(e),
        CustomPhaseError::IgnoredOperationError => CustomPhaseError::IgnoredOperationError,
    };
    SolverError::Custom { phase, error }
}
fn custom_output<A, S>(r: CustomPhaseResult<A, S>) -> PhaseOutput<A, S> {
    PhaseOutput {
        assignment: Some(r.assignment),
        score: r.score,
        complete: r.complete,
        work: SolverWork {
            evaluations: r.work.evaluations,
            move_evaluations: r.work.move_evaluations,
            constraint_evaluations: r.work.constraint_evaluations,
            ..Default::default()
        },
        termination: PhaseTermination::Custom(r.termination),
    }
}

/// Solve a basic model using one scoped objective. All acceptance/token settings
/// validate before callbacks. Model/network validation also runs at zero budget.
/// Heuristic phases alone cannot prove global optimality; an exhausted exact phase can.
///
/// ```
/// use timetuck::{FiniteProblem, Variable, ConstraintNetwork, SimpleScore,
///     SolverConfig, SolverTermination, solve};
/// let problem = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1])])?;
/// let mut network = ConstraintNetwork::new(1);
/// network.add_constraint("value", vec![0], |values| {
///     SimpleScore::new(values[0].copied().unwrap_or(0))
/// })?;
/// let result = solve(&problem, &network, SolverConfig::default())?;
/// assert_eq!(result.score, Some(SimpleScore::new(1)));
/// assert_eq!(result.termination, SolverTermination::PlanCompleted);
/// assert_eq!(result.phases.len(), 2);
/// assert_eq!(result.work.evaluations, 5); // Both phase initializations count.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn solve<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(p, n, c, None, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_with_observer<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(p, n, c, Some(PlanCallback::Best(&mut observer)), None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_with_events<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(p, n, c, Some(PlanCallback::Events(&mut observer)), None)
}
fn solve_impl<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, Assignment<V>, S>,
    mut command: CommandCallback<'_, Assignment<V>, Move<V>, S, ScoringError>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let c = c.into();
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let observation = PlanObservation::new(observer);
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.callback.is_some();
                let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = crate::ruin_recreate::ruin_recreate_search_from_observed(
                    p,
                    n,
                    a.expect("repair handoff"),
                    config,
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                n.validate(p, &initial)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| ScoreDirector::new(p, n, a),
                    |context| command.as_mut().expect("validated custom callback")(index, context),
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && matches!(config.strategy, ExactSearchStrategy::BranchAndBound(_))
            {
                n.validate(p, a.as_ref().expect("initial state"))
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error: LocalSearchError::from(error),
                    })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || crate::branch_tree::BasicTree::new(p),
                    |a| {
                        n.calculate(p, a)
                            .map(|score| (score, n.constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, LocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        n.validate(p, a.as_ref().expect("initial state"))?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| crate::search::visit_assignments(p, visit),
                            |a| {
                                n.calculate(p, a)
                                    .map(|score| (score, n.constraint_count() as u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => constructed!(if observed {
                        greedy_construction_from_with_observer(
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            &mut notify,
                        )
                    } else {
                        greedy_construction_from(
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                        )
                    }?),
                    PhaseRequest::LocalSearch(c) => {
                        improved!(if observed {
                            crate::solution_observation::local_search_from_observed(
                                time.monitor(),
                                p,
                                n,
                                a.expect("handoff"),
                                c,
                                &mut notify,
                            )
                        } else {
                            crate::local_search::local_search_from_monitored(
                                time.monitor(),
                                p,
                                n,
                                a.expect("handoff"),
                                c,
                            )
                        }?)
                    }
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}
/// Run ordered list phases with shared solve budgets.
pub fn solve_list<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(p, n, c, None, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_list_with_observer<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(p, n, c, Some(PlanCallback::Best(&mut observer)), None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_list_with_events<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(p, n, c, Some(PlanCallback::Events(&mut observer)), None)
}
fn solve_list_impl<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, ListAssignment<V>, S>,
    mut command: CommandCallback<'_, ListAssignment<V>, ListMove<V>, S, ListScoringError>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let c = c.into();
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let observation = PlanObservation::new(observer);
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.callback.is_some();
                let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = crate::list_ruin_recreate::ruin_recreate_list_search_from_observed(
                    p,
                    n,
                    a.expect("repair handoff"),
                    config,
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                n.validate(p, &initial)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| ListScoreDirector::new(p, n, a),
                    |context| command.as_mut().expect("validated custom callback")(index, context),
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && matches!(config.strategy, ExactSearchStrategy::BranchAndBound(_))
            {
                n.validate(p, a.as_ref().expect("initial state"))
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error: ListLocalSearchError::from(error),
                    })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || crate::branch_tree::ListTree::new(p),
                    |a| {
                        n.calculate(p, a)
                            .map(|score| (score, n.constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, ListLocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        n.validate(p, a.as_ref().expect("initial state"))?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| crate::list_search::visit_list_assignments(p, visit),
                            |a| {
                                n.calculate(p, a)
                                    .map(|score| (score, n.constraint_count() as u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => constructed!(if observed {
                        greedy_list_construction_from_with_observer(
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            &mut notify,
                        )
                    } else {
                        greedy_list_construction_from(
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                        )
                    }?),
                    PhaseRequest::LocalSearch(c) => {
                        improved!(if observed {
                            crate::solution_observation::list_local_search_from_observed(
                                time.monitor(),
                                p,
                                n,
                                a.expect("handoff"),
                                c,
                                &mut notify,
                            )
                        } else {
                            crate::list_local_search::list_local_search_from_monitored(
                                time.monitor(),
                                p,
                                n,
                                a.expect("handoff"),
                                c,
                            )
                        }?)
                    }
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}
/// Construct and improve coupled basic/list models. Each phase retains its own
/// construction order or joint-neighborhood setting.
pub fn solve_mixed<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(p, n, c, None, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_mixed_with_observer<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(p, n, c, Some(PlanCallback::Best(&mut observer)), None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_mixed_with_events<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(p, n, c, Some(PlanCallback::Events(&mut observer)), None)
}
fn solve_mixed_impl<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    observer: PlanObserver<'_, MixedAssignment<B, L>, S>,
    mut command: CommandCallback<'_, MixedAssignment<B, L>, MixedMove<B, L>, S, MixedScoringError>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let c = c.into();
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, MixedSolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let observation = PlanObservation::new(observer);
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    run(
        c.limits,
        &phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.callback.is_some();
                let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = crate::mixed_ruin_recreate::ruin_recreate_mixed_search_from_observed(
                    p,
                    n,
                    a.expect("repair handoff"),
                    MixedRuinRecreateConfig {
                        search: config,
                        order: c.phases[index].repair().order,
                    },
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                n.validate(p, &initial)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| MixedScoreDirector::new(p, n, a),
                    |context| command.as_mut().expect("validated custom callback")(index, context),
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && matches!(config.strategy, ExactSearchStrategy::BranchAndBound(_))
            {
                n.validate(p, a.as_ref().expect("initial state"))
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error: MixedLocalSearchError::from(error),
                    })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || crate::branch_tree::MixedTree::new(p),
                    |a| {
                        n.calculate(p, a)
                            .map(|score| (score, n.constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, MixedLocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        n.validate(p, a.as_ref().expect("initial state"))?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| exact::visit_mixed(p, visit),
                            |a| {
                                n.calculate(p, a)
                                    .map(|score| (score, n.constraint_count() as u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(config) => constructed!(if observed {
                        greedy_mixed_construction_from_with_observer(
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            MixedConstructionConfig {
                                construction: config,
                                ..c.phases[index].construction()
                            },
                            &mut notify,
                        )
                    } else {
                        greedy_mixed_construction_from(
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            MixedConstructionConfig {
                                construction: config,
                                ..c.phases[index].construction()
                            },
                        )
                    }?),
                    PhaseRequest::LocalSearch(config) => improved!(if observed {
                        crate::solution_observation::mixed_local_search_from_observed(
                            time.monitor(),
                            p,
                            n,
                            a.expect("handoff"),
                            MixedLocalSearchConfig {
                                search: config,
                                ..c.phases[index].local().clone()
                            },
                            &mut notify,
                        )
                    } else {
                        crate::mixed_local_search::mixed_local_search_from_monitored(
                            time.monitor(),
                            p,
                            n,
                            a.expect("handoff"),
                            MixedLocalSearchConfig {
                                search: config,
                                ..c.phases[index].local().clone()
                            },
                        )
                    }?),
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}

/// Stream counterpart of solve. The factory must provide a fresh graph with empty
/// planning sources and the same deterministic objective for each started phase.
/// Graph preparation is not a solve score evaluation. Phase binding/rescoring is.
/// Skipped improvement phases do not call the factory. Factory errors identify
/// their phase; previously completed work is not returned on an error.
pub fn solve_stream<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(p, graph, c, None, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_stream_with_observer<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(p, graph, c, Some(PlanCallback::Best(&mut observer)), None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_stream_with_events<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(p, graph, c, Some(PlanCallback::Events(&mut observer)), None)
}
fn solve_stream_impl<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, Assignment<V>, S>,
    mut command: CommandCallback<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let c = c.into();
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let observation = PlanObservation::new(observer);
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.callback.is_some();
                let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = crate::ruin_recreate::ruin_recreate_stream_search_from_observed(
                    p,
                    &mut graph,
                    a.expect("repair handoff"),
                    config,
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let (mut g, s) = graph().map_err(|e| SolverError::Phase {
                phase: index,
                error: LocalSearchError::from(StreamScoringError::from(e)),
            })?;
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                crate::stream_scoring::validate_source(&g, &s)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                p.validate(&initial).map_err(|e| {
                    custom_error(
                        index,
                        CustomPhaseError::Scoring(StreamScoringError::from(e)),
                    )
                })?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| StreamScoreDirector::new(p, g, s, a),
                    |context| command.as_mut().expect("validated custom callback")(index, context),
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && matches!(config.strategy, ExactSearchStrategy::BranchAndBound(_))
            {
                crate::stream_scoring::validate_source(&g, &s).map_err(|error| {
                    SolverError::Phase {
                        phase: index,
                        error: LocalSearchError::from(error),
                    }
                })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || crate::branch_tree::BasicTree::new(p),
                    |a| exact::score_basic(p, &mut g, &s, a),
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, LocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        crate::stream_scoring::validate_source(&g, &s)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| crate::search::visit_assignments(p, visit),
                            |a| exact::score_basic(p, &mut g, &s, a),
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => {
                        constructed!(if observed {
                            greedy_stream_construction_from_with_observer(
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                c,
                                &mut notify,
                            )
                        } else {
                            greedy_stream_construction_from(
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                c,
                            )
                        }?)
                    }
                    PhaseRequest::LocalSearch(c) => {
                        improved!(if observed {
                            crate::solution_observation::stream_local_search_from_observed(
                                time.monitor(),
                                p,
                                g,
                                s,
                                a.expect("handoff"),
                                c,
                                &mut notify,
                            )
                        } else {
                            crate::stream_scoring::stream_local_search_from_monitored(
                                time.monitor(),
                                p,
                                g,
                                s,
                                a.expect("handoff"),
                                c,
                            )
                        }?)
                    }
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}
/// Ordered-list stream solve; the factory has the same contract as solve_stream.
pub fn solve_list_stream<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(p, graph, c, None, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_list_stream_with_observer<
    'c,
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(p, graph, c, Some(PlanCallback::Best(&mut observer)), None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_list_stream_with_events<
    'c,
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(p, graph, c, Some(PlanCallback::Events(&mut observer)), None)
}
fn solve_list_stream_impl<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, ListAssignment<V>, S>,
    mut command: CommandCallback<'_, ListAssignment<V>, ListMove<V>, S, ListStreamScoringError>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let c = c.into();
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let observation = PlanObservation::new(observer);
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.callback.is_some();
                let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = crate::list_ruin_recreate::ruin_recreate_list_stream_search_from_observed(
                    p,
                    &mut graph,
                    a.expect("repair handoff"),
                    config,
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let (mut g, s) = graph().map_err(|e| SolverError::Phase {
                phase: index,
                error: ListLocalSearchError::from(ListStreamScoringError::from(e)),
            })?;
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                s.validate(&g)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                p.validate(&initial).map_err(|e| {
                    custom_error(
                        index,
                        CustomPhaseError::Scoring(ListStreamScoringError::from(e)),
                    )
                })?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| ListStreamScoreDirector::new(p, g, s, a),
                    |context| command.as_mut().expect("validated custom callback")(index, context),
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && matches!(config.strategy, ExactSearchStrategy::BranchAndBound(_))
            {
                s.validate(&g).map_err(|error| SolverError::Phase {
                    phase: index,
                    error: ListLocalSearchError::from(error),
                })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || crate::branch_tree::ListTree::new(p),
                    |a| exact::score_list(p, &mut g, &s, a),
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, ListLocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        s.validate(&g)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| crate::list_search::visit_list_assignments(p, visit),
                            |a| exact::score_list(p, &mut g, &s, a),
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => {
                        constructed!(if observed {
                            greedy_list_stream_construction_from_with_observer(
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                c,
                                &mut notify,
                            )
                        } else {
                            greedy_list_stream_construction_from(
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                c,
                            )
                        }?)
                    }
                    PhaseRequest::LocalSearch(c) => improved!(if observed {
                        crate::solution_observation::list_stream_local_search_from_observed(
                            time.monitor(),
                            p,
                            g,
                            s,
                            a.expect("handoff"),
                            c,
                            &mut notify,
                        )
                    } else {
                        crate::list_stream_scoring::list_stream_local_search_from_monitored(
                            time.monitor(),
                            p,
                            g,
                            s,
                            a.expect("handoff"),
                            c,
                        )
                    }?),
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}
/// Mixed stream solve; each phase uses fresh state with the same objective.
pub fn solve_mixed_stream<
    'c,
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: impl Into<MixedSolverSequence<'c, S>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(p, graph, c, None, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_mixed_stream_with_observer<
    'c,
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(p, graph, c, Some(PlanCallback::Best(&mut observer)), None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_mixed_stream_with_events<
    'c,
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(p, graph, c, Some(PlanCallback::Events(&mut observer)), None)
}
fn solve_mixed_stream_impl<
    'c,
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    observer: PlanObserver<'_, MixedAssignment<B, L>, S>,
    mut command: CommandCallback<
        '_,
        MixedAssignment<B, L>,
        MixedMove<B, L>,
        S,
        MixedStreamScoringError,
    >,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let c = c.into();
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, MixedSolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let observation = PlanObservation::new(observer);
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    run(
        c.limits,
        &phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.callback.is_some();
                let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r =
                    crate::mixed_ruin_recreate::ruin_recreate_mixed_stream_search_from_observed(
                        p,
                        &mut graph,
                        a.expect("repair handoff"),
                        MixedRuinRecreateConfig {
                            search: config,
                            order: c.phases[index].repair().order,
                        },
                        observed.then_some(&mut notify),
                        time.monitor(),
                    )
                    .map_err(|error| SolverError::Repair {
                        phase: index,
                        error,
                    })?;
                return Ok(repaired!(r));
            }

            let (mut g, s) = graph().map_err(|e| SolverError::Phase {
                phase: index,
                error: MixedLocalSearchError::from(MixedStreamScoringError::from(e)),
            })?;
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                s.validate(&g)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                p.validate(&initial).map_err(|e| {
                    custom_error(
                        index,
                        CustomPhaseError::Scoring(MixedStreamScoringError::from(e)),
                    )
                })?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| MixedStreamScoreDirector::new(p, g, s, a),
                    |context| command.as_mut().expect("validated custom callback")(index, context),
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && matches!(config.strategy, ExactSearchStrategy::BranchAndBound(_))
            {
                s.validate(&g).map_err(|error| SolverError::Phase {
                    phase: index,
                    error: MixedLocalSearchError::from(error),
                })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || crate::branch_tree::MixedTree::new(p),
                    |a| exact::score_mixed(p, &mut g, &s, a),
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, MixedLocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        s.validate(&g)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| exact::visit_mixed(p, visit),
                            |a| exact::score_mixed(p, &mut g, &s, a),
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(config) => {
                        constructed!(if observed {
                            greedy_mixed_stream_construction_from_with_observer(
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                MixedConstructionConfig {
                                    construction: config,
                                    ..c.phases[index].construction()
                                },
                                &mut notify,
                            )
                        } else {
                            greedy_mixed_stream_construction_from(
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                MixedConstructionConfig {
                                    construction: config,
                                    ..c.phases[index].construction()
                                },
                            )
                        }?)
                    }
                    PhaseRequest::LocalSearch(config) => improved!(if observed {
                        crate::solution_observation::mixed_stream_local_search_from_observed(
                            time.monitor(),
                            p,
                            g,
                            s,
                            a.expect("handoff"),
                            MixedLocalSearchConfig {
                                search: config,
                                ..c.phases[index].local().clone()
                            },
                            &mut notify,
                        )
                    } else {
                        crate::mixed_stream_scoring::mixed_stream_local_search_from_monitored(
                            time.monitor(),
                            p,
                            g,
                            s,
                            a.expect("handoff"),
                            MixedLocalSearchConfig {
                                search: config,
                                ..c.phases[index].local().clone()
                            },
                        )
                    }?),
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}

#[cfg(test)]
#[path = "solver_time_tests.rs"]
mod time_tests;

#[cfg(test)]
#[path = "solver_observation_tests.rs"]
mod observation_tests;

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap()))?;
/// let sequence = SolverSequence {
///     phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
///     ..Default::default()
/// };
/// let r = solve_with_commands(&p, &n, sequence, |phase, c| {
///     assert_eq!(phase, 0);
///     c.execute(&Move::Change { variable: 0, value: Some(2) })?;
///     c.execute(&Move::Change { variable: 0, value: Some(0) })?;
///     Ok(())
/// })?;
/// assert_eq!(r.assignment.values(), &[Some(2)]);
/// assert_eq!(r.phases[0].termination, PhaseTermination::Custom(CustomPhaseTermination::Completed));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn solve_with_commands<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, ScoringError>,
    ) -> Result<(), CustomPhaseError<ScoringError>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(p, n, c, None, Some(&mut command))
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
/// Also observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_with_commands_and_events<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, ScoringError>,
    ) -> Result<(), CustomPhaseError<ScoringError>>,
    mut observer: impl FnMut(SolverEvent<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(
        p,
        n,
        c,
        Some(PlanCallback::Events(&mut observer)),
        Some(&mut command),
    )
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
pub fn solve_stream_with_commands<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
    ) -> Result<(), CustomPhaseError<StreamScoringError>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(p, graph, c, None, Some(&mut command))
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
/// Also observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_stream_with_commands_and_events<
    'c,
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
    ) -> Result<(), CustomPhaseError<StreamScoringError>>,
    mut observer: impl FnMut(SolverEvent<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Events(&mut observer)),
        Some(&mut command),
    )
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
pub fn solve_list_with_commands<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListScoringError>,
    ) -> Result<(), CustomPhaseError<ListScoringError>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(p, n, c, None, Some(&mut command))
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
/// Also observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_list_with_commands_and_events<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListScoringError>,
    ) -> Result<(), CustomPhaseError<ListScoringError>>,
    mut observer: impl FnMut(SolverEvent<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(
        p,
        n,
        c,
        Some(PlanCallback::Events(&mut observer)),
        Some(&mut command),
    )
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
pub fn solve_list_stream_with_commands<
    'c,
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListStreamScoringError>,
    ) -> Result<(), CustomPhaseError<ListStreamScoringError>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(p, graph, c, None, Some(&mut command))
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
/// Also observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_list_stream_with_commands_and_events<
    'c,
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListStreamScoringError>,
    ) -> Result<(), CustomPhaseError<ListStreamScoringError>>,
    mut observer: impl FnMut(SolverEvent<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Events(&mut observer)),
        Some(&mut command),
    )
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
pub fn solve_mixed_with_commands<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, MixedAssignment<B, L>, MixedMove<B, L>, S, MixedScoringError>,
    ) -> Result<(), CustomPhaseError<MixedScoringError>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(p, n, c, None, Some(&mut command))
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
/// Also observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_mixed_with_commands_and_events<
    'c,
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore + 'c,
>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<'_, MixedAssignment<B, L>, MixedMove<B, L>, S, MixedScoringError>,
    ) -> Result<(), CustomPhaseError<MixedScoringError>>,
    mut observer: impl FnMut(SolverEvent<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(
        p,
        n,
        c,
        Some(PlanCallback::Events(&mut observer)),
        Some(&mut command),
    )
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
pub fn solve_mixed_stream_with_commands<
    'c,
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<
            '_,
            MixedAssignment<B, L>,
            MixedMove<B, L>,
            S,
            MixedStreamScoringError,
        >,
    ) -> Result<(), CustomPhaseError<MixedStreamScoringError>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(p, graph, c, None, Some(&mut command))
}

/// Execute custom phases using a typed callback dispatched by phase index.
/// The callback runs once per entered custom phase and cooperates with limits
/// through its context. All other phases retain their usual behavior.
/// Also observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_mixed_stream_with_commands_and_events<
    'c,
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut command: impl FnMut(
        usize,
        &mut CustomPhaseContext<
            '_,
            MixedAssignment<B, L>,
            MixedMove<B, L>,
            S,
            MixedStreamScoringError,
        >,
    ) -> Result<(), CustomPhaseError<MixedStreamScoringError>>,
    mut observer: impl FnMut(SolverEvent<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Events(&mut observer)),
        Some(&mut command),
    )
}
