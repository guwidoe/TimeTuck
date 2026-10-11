//! Ordered construction, local, repair and exact phases under shared controls.
#[path = "solver_bounded.rs"]
mod bounded;
#[path = "solver_exact.rs"]
mod exact;
#[path = "solver_phase_assertions.rs"]
mod phase_assertions;
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
    /// Reject a missing required pinned basic variable after plan validation,
    /// before score factories, provider events, budgets or ordinary phase work.
    /// Optional pinned None remains valid. Applies to basic and mixed plans.
    /// Disabled by default to retain native partial construction workflows.
    pub reject_unassigned_pins: bool,
    /// Recalculate each scored phase output using a fresh backend before handoff
    /// and initialization notification, including initial-only immovable solves.
    /// Diagnostic scoring is outside work counters/budgets; its time belongs to
    /// the solve deadline. No providers, commands or progress events are replayed.
    /// Stream/calculator factories must return the same objective on each call.
    /// Disabled by default; independent extension checks remain separately available.
    pub assert_phase_scores: bool,
    /// Composed solve-wide limits, polled within search/custom phases and at
    /// phase boundaries and within construction candidate scans.
    pub termination: Option<&'a SearchTermination<'a>>,
    pub max_evaluations: u64,
    pub max_move_evaluations: Option<u64>,
    /// Cooperative deadline shared by construction, improvement and stream graph
    /// preparation. Initial scoring is allowed at zero time. None disables it.
    pub max_time: Option<Duration>,
    /// Solve-wide stagnation, starting when the first improvement phase is ready.
    /// Construction is excluded. Later phases/handoffs do not reset the timer.
    /// Only strict retained-best improvement resets it, before expiry.
    pub max_unimproved_time: Option<Duration>,
    /// Solve-wide score-threshold stagnation. The first scored improvement phase
    /// arms the timer; initial construction and preparation are excluded. Later
    /// phases and their preparation do not reset it. A retained-best improvement
    /// resets it only when its accumulated difference reaches the threshold.
    /// Equality at the deadline continues; a late improvement is retained but
    /// cannot revive expiry. Exact search without a prior score arms on its first
    /// complete candidate. Phase-local policies remain independent.
    pub significant_improvement: Option<&'a SignificantImprovementConfig<S>>,
    /// Completed local-search steps and accepted changed repair/fine moves since the last
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
            reject_unassigned_pins: false,
            assert_phase_scores: false,
            termination: None,
            max_evaluations: 100_000,
            max_move_evaluations: None,
            max_time: None,
            max_unimproved_time: None,
            significant_improvement: None,
            max_unimproved_steps: None,
            cancellation: None,
            best_score_limit: None,
            stop_when_feasible: false,
            incomplete_handoff: IncompleteHandoffPolicy::ReturnPartial,
        }
    }
}
/// For movable models, construct first, then run local-search phases in order.
/// Models with no unpinned basic variables or list entities only score their
/// initial state; ordinary phases, commands and providers are skipped. Explicit
/// exact-search plans still execute to retain their exhaustion/optimality contract.
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
    /// Includes initial-only scoring and phase initialization; exact candidates need no rescore.
    pub evaluations: u64,
    /// Candidates, including exact assignments; initialization and diagnostics are excluded.
    pub move_evaluations: u64,
    pub constraint_evaluations: u128,
    /// Local-search steps and accepted changed repair/fine moves; excludes construction steps.
    pub completed_steps: u64,
    /// Completed construction placements, including scored no-change and retained
    /// prefixes; excludes discarded scans. Separate from search stagnation.
    pub construction_steps: u64,
    pub accepted_moves: u64,
    /// Started ruin-and-recreate attempts, including incomplete, rejected and unchanged repairs.
    /// Reconstruction candidates are counted separately in `move_evaluations`.
    pub repair_attempts: u64,
    /// Sampled change/swap attempts interleaved with repair phases.
    pub fine_move_attempts: u64,
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
        self.construction_steps += other.construction_steps;
        self.accepted_moves += other.accepted_moves;
        self.repair_attempts += other.repair_attempts;
        self.fine_move_attempts += other.fine_move_attempts;
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
    /// The configured solve-wide expression became true.
    Condition,
    /// All planned phases ended, or were skipped for a model without movable
    /// entities. This does not establish optimality.
    PlanCompleted,
    /// A complete scored solution and an exhausted exact search. Pruned branches
    /// count as exhausted only under the configured caller bound contract.
    Optimal,
    EvaluationLimit,
    MoveEvaluationLimit,
    TimeLimit,
    UnimprovedTimeLimit,
    SignificantImprovementTimeLimit,
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
    /// Startup policy rejected a permanently missing required basic variable.
    /// The index is in the basic model, including for a mixed solution.
    PinnedUnassignedVariable {
        variable: usize,
    },
    /// An independent phase check disagreed with the phase's returned score.
    PhaseScoreMismatch {
        phase: usize,
        kind: SolverPhaseKind,
        actual: String,
        expected: String,
    },
    /// The independent phase checker could not produce an expected score.
    PhaseScoreCheckFailed {
        phase: usize,
        kind: SolverPhaseKind,
        reason: String,
    },
    /// Scoring or graph preparation failed before any configured phase ran.
    InitialScoring {
        error: E,
    },
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
    /// A provider binding must target an existing local-search phase.
    InvalidMoveProviderPhase {
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
            Self::PinnedUnassignedVariable { variable } => write!(
                f,
                "required basic variable {variable} is pinned with an unassigned value"
            ),
            Self::PhaseScoreMismatch {
                phase,
                kind,
                actual,
                expected,
            } => write!(
                f,
                "phase {phase} ({kind:?}) score corruption: actual {actual}, expected {expected}"
            ),
            Self::PhaseScoreCheckFailed {
                phase,
                kind,
                reason,
            } => write!(f, "phase {phase} ({kind:?}) score check failed: {reason}"),
            Self::InitialScoring { error } => write!(f, "initial scoring failed: {error}"),
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
            Self::InvalidMoveProviderPhase { phase } => write!(
                f,
                "move provider targets a missing phase or one that is neither construction nor local search: {phase}"
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
            Self::InitialScoring { error } => Some(error),
            Self::InvalidAcceptance { error, .. } => Some(error),
            Self::InvalidScoreTrend { error, .. } => Some(error),
            Self::Phase { error, .. } => Some(error),
            Self::Custom { error, .. } => Some(error),
            Self::Repair { error, .. } => Some(error),
            Self::EmptyPlan
            | Self::PhaseScoreMismatch { .. }
            | Self::PhaseScoreCheckFailed { .. }
            | Self::InvalidOptimisticBound { .. }
            | Self::IncompleteLocalSearch { .. }
            | Self::ConflictingCancellation { .. }
            | Self::InvalidMoveProviderPhase { .. }
            | Self::UnboundCustomPhase { .. }
            | Self::PinnedUnassignedVariable { .. } => None,
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
struct SolveControl<'c, S = crate::SimpleScore> {
    move_counter: Option<crate::move_metrics::MoveCounter>,
    score_counter: Option<crate::score_evaluations::ScoreCounter>,
    step_metrics: Option<crate::solver_metrics::MetricWriter<S>>,
    metric_phase: Option<(usize, SolverPhaseKind)>,
    metric_step: u64,
    lifecycle: crate::solver_lifecycle::Lifecycle<'c>,
    condition: Option<&'c SearchTermination<'c>>,
    condition_expired: bool,
    condition_uses_moves: bool,
    limit: Option<Duration>,
    clock: &'c mut dyn FnMut() -> Duration,
    last: Duration,
    shared_bound: bool,
    significant: Option<SignificantImprovement<S>>,
    unimproved_limit: Option<Duration>,
    last_improvement: Option<Duration>,
    unimproved_expired: bool,
    unimproved_steps_limit: Option<u64>,
    steps: StepProgress,
    construction_steps: u64,
    started: bool,
    steps_expired: bool,
}
impl<S: Score> SolveControl<'_, S> {
    fn condition_reached(&mut self) -> bool {
        if let Some(condition) = self.condition {
            self.sample();
            let progress = SearchProgress {
                elapsed: self.last,
                completed_steps: self.steps.completed_steps(),
                construction_steps: self.construction_steps,
                score_evaluations: self.score_counter.as_ref().map_or(0, |c| c.total()),
                move_evaluations: if self.condition_uses_moves {
                    self.move_counter
                        .as_ref()
                        .expect("move condition counter")
                        .snapshot()
                } else {
                    MoveEvaluationCounts::default()
                },
                unimproved_steps: self.started.then(|| self.steps.unimproved_steps()),
                unimproved_time: self.last_improvement.map(|last| self.last - last),
            };
            self.condition_expired |= condition.status(progress).is_some_and(|s| s.reached);
        }
        self.condition_expired
    }

    fn sample(&mut self) {
        self.last = self.last.max((self.clock)());
    }
    fn significant_stagnated(&mut self) -> bool {
        if self
            .significant
            .as_ref()
            .is_some_and(SignificantImprovement::is_started)
        {
            self.sample();
            self.significant.as_mut().unwrap().expired(self.last)
        } else {
            false
        }
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
    fn construction_monitor(&mut self) -> Option<&mut dyn crate::time_limit::SearchControl<S>> {
        (self.condition.is_some() || self.lifecycle.enabled() || self.move_counter.is_some())
            .then_some(self)
    }
    fn monitor(&mut self) -> Option<&mut dyn crate::time_limit::SearchControl<S>> {
        if self.move_counter.is_some()
            || self.lifecycle.enabled()
            || self.condition.is_some()
            || self.significant.is_some()
            || self.unimproved_limit.is_some()
            || self.unimproved_steps_limit.is_some()
        {
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
impl<S: Score> crate::time_limit::SearchControl<S> for SolveControl<'_, S> {
    fn score_counter(&self) -> Option<crate::score_evaluations::ScoreCounter> {
        self.score_counter.clone()
    }
    fn move_counter(&self) -> Option<crate::move_metrics::MoveCounter> {
        self.move_counter.clone()
    }
    fn monitors_step_scores(&self) -> bool {
        self.step_metrics.is_some()
    }
    fn record_step_score(&mut self, score: S, complete: bool) {
        if let (Some(writer), Some((phase, kind))) = (&self.step_metrics, self.metric_phase) {
            writer.step(crate::SolverStepMetric {
                phase,
                kind,
                step: self.metric_step,
                score,
                complete,
            });
        }
    }
    fn step_started(&mut self) {
        self.lifecycle.start_step();
    }
    fn step_ended(&mut self, completed: bool) {
        self.lifecycle.end_step(completed);
        self.metric_step = self.metric_step.saturating_add(1);
    }
    fn complete_construction_step(&mut self) {
        self.construction_steps = self.construction_steps.saturating_add(1);
        self.condition_reached();
    }
    fn construction_stopped(&mut self) -> bool {
        self.condition_reached()
    }
    fn observe_best_score(&mut self, score: &S) {
        if self.started && self.significant.is_some() {
            self.sample();
            let limit = self.significant.as_mut().unwrap();
            if limit.is_started() {
                limit.is_terminated(self.last, score);
            } else {
                limit.start(self.last, score.clone());
            }
        }
    }
    fn start_search(&mut self) {
        self.started = true;
        if (self.condition.is_some() || self.unimproved_limit.is_some())
            && self.last_improvement.is_none()
        {
            self.sample();
            self.last_improvement = Some(self.last);
        }
    }
    fn record_improvement(&mut self) {
        self.condition_reached();
        if !self.step_stagnated() {
            self.steps.record_improvement();
        }
        // Keep late improvements in the solution, but never revive an expired solve.
        if !self.stagnated() && !self.condition_expired && self.last_improvement.is_some() {
            self.last_improvement = Some(self.last);
        }
    }
    fn complete_step(&mut self, improved: bool) {
        if (self.condition.is_some() || self.unimproved_steps_limit.is_some())
            && !self.step_stagnated()
        {
            self.steps.complete_step(improved);
            self.step_stagnated();
            self.condition_reached();
        }
    }
    fn stop(&mut self) -> Option<LocalSearchTermination> {
        if self.stagnated() {
            Some(LocalSearchTermination::UnimprovedTimeLimit)
        } else if self.significant_stagnated() {
            Some(LocalSearchTermination::SignificantImprovementTimeLimit)
        } else if self.step_stagnated() {
            Some(LocalSearchTermination::UnimprovedStepLimit)
        } else if self.condition_reached() {
            Some(LocalSearchTermination::StepCondition)
        } else {
            None
        }
    }
}
impl<S: Score> PhaseRequest<'_, S> {
    fn with_time(mut self, time: &mut SolveControl<'_, S>) -> Self {
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
    /// Initial score only: no configured phase ran because every entity is pinned
    /// or absent. The accompanying phase index is zero.
    Initial,
    Custom,
    Construction,
    LocalSearch,
    RuinRecreate,
    Exact,
}
/// A synchronous retained-best update from one ordered solve. Phase indices refer
/// to the explicit sequence; converted plans prepend construction at zero.
/// Initial-only scoring uses index zero and kind Initial, with no phase report.
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
    Checked {
        events: Option<&'a mut crate::SolverEventCallback<'a, A, S>>,
        check: Option<&'a mut crate::PhaseScoreCheck<'a, A, S>>,
        lifecycle: Option<&'a mut crate::SolverLifecycleCallback<'a>>,
        metrics: Option<&'a crate::SolverMetrics<S>>,
    },
}
impl<'a, A, S> PlanCallback<'a, A, S> {
    fn extensions(
        events: Option<&'a mut crate::SolverEventCallback<'a, A, S>>,
        check: Option<&'a mut crate::PhaseScoreCheck<'a, A, S>>,
        lifecycle: Option<&'a mut crate::SolverLifecycleCallback<'a>>,
        metrics: Option<&'a crate::SolverMetrics<S>>,
    ) -> Option<Self> {
        if check.is_some() || lifecycle.is_some() || metrics.is_some() {
            Some(Self::Checked {
                events,
                check,
                lifecycle,
                metrics,
            })
        } else {
            events.map(Self::Events)
        }
    }
}
type PlanObserver<'a, A, S> = Option<PlanCallback<'a, A, S>>;
struct PlanObservation<'a, A, S> {
    callback: PlanObserver<'a, A, S>,
    score: Option<S>,
    complete: bool,
    initial_only: bool,
    repaired_initial: bool,
    metric_writer: Option<crate::solver_metrics::MetricWriter<S>>,
}
impl<'a, A, S: Score> PlanObservation<'a, A, S> {
    fn new(callback: PlanObserver<'a, A, S>) -> Self {
        Self {
            callback,
            score: None,
            complete: false,
            initial_only: false,
            repaired_initial: false,
            metric_writer: None,
        }
    }
    fn take_lifecycle(&mut self) -> Option<&'a mut crate::SolverLifecycleCallback<'a>> {
        match &mut self.callback {
            Some(PlanCallback::Checked { lifecycle, .. }) => lifecycle.take(),
            _ => None,
        }
    }
    fn has_events(&self) -> bool {
        self.metric_writer.is_some()
            || matches!(
                &self.callback,
                Some(
                    PlanCallback::Best(_)
                        | PlanCallback::Events(_)
                        | PlanCallback::Checked {
                            events: Some(_),
                            ..
                        }
                )
            )
    }
    fn check_score<E>(
        &mut self,
        phase: usize,
        kind: SolverPhaseKind,
        assignment: &A,
        actual: Option<&S>,
    ) -> Result<(), SolverError<E>> {
        let (
            Some(PlanCallback::Checked {
                check: Some(check), ..
            }),
            Some(actual),
        ) = (&mut self.callback, actual)
        else {
            return Ok(());
        };
        let expected = check(phase, kind, assignment).map_err(|reason| {
            SolverError::PhaseScoreCheckFailed {
                phase,
                kind,
                reason,
            }
        })?;
        if let Some(expected) = expected
            && &expected != actual
        {
            return Err(SolverError::PhaseScoreMismatch {
                phase,
                kind,
                actual: format!("{actual:?}"),
                expected: format!("{expected:?}"),
            });
        }
        Ok(())
    }
    fn initialize(
        &mut self,
        phase: usize,
        initialization_phase: Option<usize>,
        result: &SolverResult<A, S>,
        termination: PhaseTermination,
    ) {
        let callback = match &mut self.callback {
            Some(
                PlanCallback::Events(callback)
                | PlanCallback::Checked {
                    events: Some(callback),
                    ..
                },
            ) => callback,
            _ => return,
        };
        if initialization_phase != Some(phase) {
            return;
        }
        let (kind, terminated_early) = match termination {
            PhaseTermination::Construction(reason) => (
                SolverPhaseKind::Construction,
                reason != ConstructionTermination::Completed,
            ),
            PhaseTermination::Exact(reason) => (
                SolverPhaseKind::Exact,
                !matches!(
                    reason,
                    ExactPhaseTermination::Exhausted | ExactPhaseTermination::CompletionExhausted
                ),
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
        if !self.has_events() {
            return;
        }
        let Some(callback) = &mut self.callback else {
            return;
        };
        if (!self.complete && update.complete)
            || self.score.as_ref().is_none_or(|best| update.score > *best)
        {
            let repair_baseline =
                self.repaired_initial && self.score.is_none() && kind != SolverPhaseKind::Exact;
            self.score = Some(update.score.clone());
            self.complete = update.complete;
            if let Some(metrics) = &self.metric_writer {
                metrics.best(&update.score, update.complete);
            }
            if repair_baseline {
                return;
            }
            let update = SolverUpdate {
                phase,
                kind: if self.initial_only {
                    SolverPhaseKind::Initial
                } else {
                    kind
                },
                assignment: update.assignment,
                score: update.score,
                complete: update.complete,
            };
            match callback {
                PlanCallback::Best(callback) => callback(update),
                PlanCallback::Events(callback)
                | PlanCallback::Checked {
                    events: Some(callback),
                    ..
                } => callback(SolverEvent::Best(update)),
                PlanCallback::Checked { events: None, .. } => {}
            }
        }
    }
}
impl<S: Score> PhaseRequest<'_, S> {
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

fn required_pin_startup_issue<V: Eq>(
    reject: bool,
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
) -> Option<usize> {
    if reject {
        crate::local_search::pinned_unassigned_variable(p, a)
    } else {
        None
    }
}
struct InitialState<A> {
    required_unassigned_pin: Option<usize>,
    assignment: A,
    complete: bool,
    movable: bool,
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
        &mut SolveControl<'_, S>,
        &mut PlanObservation<'_, A, S>,
    ) -> Result<PhaseOutput<A, S>, SolverError<E>>,
) -> Result<SolverResult<A, S>, SolverError<E>>
where
    A: Clone,
{
    let mut clock = crate::time_limit::monotonic_clock(
        limits.termination.is_some()
            || limits.max_time.is_some()
            || limits.max_unimproved_time.is_some()
            || limits.significant_improvement.is_some(),
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
        &mut SolveControl<'_, S>,
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
        &mut SolveControl<'_, S>,
        &mut PlanObservation<'_, A, S>,
    ) -> Result<PhaseOutput<A, S>, SolverError<E>>,
    clock: &mut dyn FnMut() -> Duration,
) -> Result<SolverResult<A, S>, SolverError<E>>
where
    A: Clone,
{
    // Reborrow through a local adapter so callback storage does not tie the
    // solve clock and termination references to the caller's callback lifetime.
    let mut callback = observation.take_lifecycle();
    let lifecycle_enabled = callback.is_some();
    let mut lifecycle_callback = |event| callback.as_mut().expect("enabled lifecycle")(event);
    let mut time = SolveControl {
        move_counter: None,
        score_counter: limits
            .termination
            .is_some_and(SearchTermination::needs_score_counts)
            .then(Default::default),
        step_metrics: None,
        metric_phase: None,
        metric_step: 0,
        lifecycle: crate::solver_lifecycle::Lifecycle::new(
            lifecycle_enabled.then_some(&mut lifecycle_callback),
        ),
        condition: limits.termination,
        condition_expired: false,
        condition_uses_moves: limits
            .termination
            .is_some_and(SearchTermination::needs_move_counts),
        limit: limits.max_time,
        clock,
        last: Duration::ZERO,
        shared_bound: false,
        significant: limits
            .significant_improvement
            .cloned()
            .map(SignificantImprovement::new),
        unimproved_limit: limits.max_unimproved_time,
        last_improvement: None,
        unimproved_expired: false,
        unimproved_steps_limit: limits.max_unimproved_steps,
        steps: StepProgress::default(),
        construction_steps: 0,
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
            if !c.has_moves() {
                return Err(SolverError::Repair {
                    phase: index,
                    error: RuinRecreateError::InvalidMoveWeights,
                });
            }
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
    let initialization_phase =
        crate::phase_metadata::initialization_index(local.iter().map(SolverPhase::kind));
    let initial = initial();
    if limits.reject_unassigned_pins
        && let Some(variable) = initial.required_unassigned_pin
    {
        return Err(SolverError::PinnedUnassignedVariable { variable });
    }
    let mut metrics = match &observation.callback {
        Some(PlanCallback::Checked {
            metrics: Some(metrics),
            ..
        }) => Some(metrics.start()),
        _ => None,
    };
    time.move_counter = metrics
        .as_ref()
        .map(|m| m.moves.clone())
        .or_else(|| time.condition_uses_moves.then(Default::default));
    time.step_metrics = metrics.as_ref().map(|m| m.writer.clone());
    observation.metric_writer = metrics.as_ref().map(|m| m.writer.clone());
    let outcome = (|| {
        time.lifecycle.start();
        let mut result = SolverResult {
            assignment: initial.assignment,
            complete: initial.complete,
            score: None,
            work: SolverWork::default(),
            phases: Vec::new(),
            termination: SolverTermination::PlanCompleted,
        };
        if !initial.movable && !local.iter().any(|p| matches!(p, SolverPhase::Exact(_))) {
            // Reuse the score-director initialization kernel with no user command.
            // This is not an entered custom phase and therefore produces no phase
            // report or FirstInitialized event. All plan validation above still runs.
            if time.condition_reached() {
                result.termination = SolverTermination::Condition;
                return Ok(result);
            }
            if result.complete && limits.max_evaluations == 0 {
                result.termination = SolverTermination::EvaluationLimit;
                return Ok(result);
            }
            if result.complete
                && limits
                    .cancellation
                    .is_some_and(CancellationToken::is_cancelled)
            {
                result.termination = SolverTermination::Cancelled;
                return Ok(result);
            }
            observation.initial_only = true;
            let out = phase(
                0,
                Some(result.assignment.clone()),
                PhaseRequest::Custom(
                    CustomPhaseConfig {
                        // A frozen incomplete assignment cannot become complete.
                        // Still validate the score binding, before budget/cancel gates,
                        // using the director kernel's unscored entry path.
                        max_evaluations: if result.complete {
                            limits.max_evaluations
                        } else {
                            0
                        },
                        max_move_evaluations: limits.max_move_evaluations,
                        cancellation: limits.cancellation,
                        max_time: None,
                    },
                    limits.best_score_limit.clone(),
                    limits.stop_when_feasible,
                ),
                &mut time,
                observation,
            )
            .map_err(|error| match error {
                SolverError::Custom {
                    error: CustomPhaseError::Scoring(error),
                    ..
                }
                | SolverError::Phase { error, .. } => SolverError::InitialScoring { error },
                other => other,
            })?;
            if limits.assert_phase_scores {
                phase_assertions::verify(
                    0,
                    SolverPhaseKind::Initial,
                    out.assignment
                        .as_ref()
                        .expect("initial scoring retains assignment"),
                    out.score.as_ref(),
                    &mut phase,
                )?;
            }
            observation.check_score(
                0,
                SolverPhaseKind::Initial,
                out.assignment
                    .as_ref()
                    .expect("initial scoring retains assignment"),
                out.score.as_ref(),
            )?;
            result.assignment = out.assignment.expect("initial scoring retains assignment");
            result.complete = out.complete;
            result.score = out.score;
            result.work = out.work;
            if !result.complete {
                result.termination = SolverTermination::NoCompleteAssignment;
                return Ok(result);
            }
            let PhaseTermination::Custom(reason) = out.termination else {
                unreachable!("initial scoring uses the custom director kernel")
            };
            use CustomPhaseTermination as C;
            result.termination = match reason {
                C::Completed if result.complete => SolverTermination::PlanCompleted,
                C::Completed => SolverTermination::NoCompleteAssignment,
                C::EvaluationLimit => SolverTermination::EvaluationLimit,
                C::MoveEvaluationLimit => SolverTermination::MoveEvaluationLimit,
                C::TimeLimit => SolverTermination::TimeLimit,
                C::Cancelled => SolverTermination::Cancelled,
                C::UnimprovedTimeLimit => SolverTermination::UnimprovedTimeLimit,
                C::SignificantImprovementTimeLimit => {
                    SolverTermination::SignificantImprovementTimeLimit
                }
                C::UnimprovedStepLimit => SolverTermination::UnimprovedStepLimit,
                C::Condition => SolverTermination::Condition,
                C::BestScoreLimit => SolverTermination::BestScoreLimit,
                C::FeasibleScore => SolverTermination::FeasibleScore,
            };
            return Ok(result);
        }
        for index in 0..local.len() {
            if time.condition_reached() {
                result.termination = SolverTermination::Condition;
                break;
            }
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
                } else if time.significant_stagnated() {
                    Some(SolverTermination::SignificantImprovementTimeLimit)
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
            let kind = request.kind();
            time.metric_phase = Some((index, kind));
            time.metric_step = 0;
            time.lifecycle.start_phase(index, kind);
            let output = phase(index, initial, request, &mut time, observation);
            if let (Some(metrics), Ok(out)) = (&observation.metric_writer, &output) {
                let mut work = result.work;
                work.add(out.work);
                metrics.phase(index, work);
            }
            time.lifecycle.end_phase();
            let mut out = output?;
            if limits.assert_phase_scores {
                phase_assertions::verify(
                    index,
                    kind,
                    out.assignment.as_ref().unwrap_or(&result.assignment),
                    out.score.as_ref(),
                    &mut phase,
                )?;
            }
            observation.check_score(
                index,
                kind,
                out.assignment.as_ref().unwrap_or(&result.assignment),
                out.score.as_ref(),
            )?;
            let r = &mut result;
            if let (Some(assignment), Some(score)) = (out.assignment.take(), out.score.as_ref())
                && ((!r.complete && out.complete)
                    || r.score.as_ref().is_none_or(|best| score > best))
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
            observation.initialize(index, initialization_phase, r, out.termination);
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
                _ if time.significant_stagnated() => {
                    Some(SolverTermination::SignificantImprovementTimeLimit)
                }
                _ if time.step_stagnated() => Some(SolverTermination::UnimprovedStepLimit),
                _ if time.condition_reached() => Some(SolverTermination::Condition),
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
    })();
    // Include errors/panics in solve-end lifecycle callbacks in the metric outcome.
    drop(time);
    if let Some(metrics) = &mut metrics {
        metrics.finish(&outcome);
    }
    observation.metric_writer = None;
    outcome
}

macro_rules! repaired {
    ($r:expr) => {{
        let r = $r;
        let work = SolverWork {
            evaluations: r.evaluations,
            move_evaluations: r.repair_move_evaluations + r.fine_move_evaluations,
            constraint_evaluations: r.constraint_evaluations,
            completed_steps: r.accepted_moves,
            construction_steps: 0,
            accepted_moves: r.accepted_moves,
            repair_attempts: r.attempts,
            fine_move_attempts: r.fine_move_attempts,
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
            construction_steps: r.completed_steps,
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
// Bind construction providers without changing existing phase ordering or controls.
fn provider_construction_config(
    c: ConstructionConfig<'_>,
    max_scanned_moves: u64,
) -> ProviderConstructionConfig<'_> {
    ProviderConstructionConfig {
        interruption: c.interruption,
        max_evaluations: c.max_evaluations,
        max_move_evaluations: c.max_move_evaluations,
        max_steps: c.max_steps,
        max_scanned_moves,
        optional: c.optional,
        pick_early: c.pick_early,
        cancellation: c.cancellation,
        max_time: c.max_time,
        assert_scores: c.assert_scores,
        assert_step_scores: c.assert_step_scores,
    }
}
fn provider_constructed<A, S>(r: ProviderConstructionResult<A, S>) -> PhaseOutput<A, S> {
    use ConstructionTermination as C;
    use ProviderConstructionTermination as P;
    let reason = match r.termination {
        P::Completed => C::Completed,
        P::Condition => C::Condition,
        P::NoCandidates => C::NoCandidates,
        P::ScanLimit => C::ScanLimit,
        P::EvaluationLimit => C::EvaluationLimit,
        P::MoveEvaluationLimit => C::MoveEvaluationLimit,
        P::StepLimit => C::StepLimit,
        P::TimeLimit => C::TimeLimit,
        P::Cancelled => C::Cancelled,
    };
    PhaseOutput {
        assignment: Some(r.assignment),
        score: r.score,
        complete: r.complete,
        work: SolverWork {
            evaluations: r.evaluations,
            move_evaluations: r.evaluations.saturating_sub(1),
            constraint_evaluations: r.constraint_evaluations,
            construction_steps: r.completed_steps,
            ..Default::default()
        },
        termination: PhaseTermination::Construction(reason),
    }
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

#[path = "solver_retained.rs"]
pub(crate) mod retained;

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
    solve_impl(p, n, c, None, None, None)
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
    solve_impl(p, n, c, Some(PlanCallback::Best(&mut observer)), None, None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_with_events<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(
        p,
        n,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}
fn solve_impl<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, Assignment<V>, S>,
    mut command: CommandCallback<'_, Assignment<V>, Move<V>, S, ScoringError>,
    providers: Option<&mut PhaseMoveProviders<'_, FiniteProblem<V>, Assignment<V>, Move<V>>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                SolverPhase::LocalSearch(_) | SolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_repair_candidates(&c.phases, p)?;
    validate_basic_phase_neighborhoods(
        p,
        &c.phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        LocalSearchError::from,
    )?;
    let observation = PlanObservation::new(observer);
    let reject_unassigned_pins = c.limits.reject_unassigned_pins;
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            required_unassigned_pin: required_pin_startup_issue(
                reject_unassigned_pins,
                p,
                &assignment,
            ),
            movable: p.variables().iter().any(|v| !v.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
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
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
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
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
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
                            .map(|score| (score, n.active_constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, LocalSearchError> = (|| {
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(crate::provider_construction::basic(
                            p,
                            n,
                            a.expect("construction provider handoff"),
                            provider_construction_config(*config, max_scanned_moves),
                            context,
                        )?));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(crate::provider_search::basic(
                        p,
                        n,
                        a.expect("provider phase handoff"),
                        config.clone(),
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        n.validate(p, a.as_ref().expect("initial state"))?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_basic_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| {
                                n.calculate(p, a)
                                    .map(|score| (score, n.active_constraint_count() as u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => constructed!(if observed {
                        crate::solution_observation::greedy_construction_from_with_observer_monitored(
                            time.construction_monitor(),
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            &mut notify,
                        )
                    } else {
                        crate::construction::greedy_construction_from_monitored(
                            time.construction_monitor(),
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
fn validate_repair_candidates<P: crate::ruin_engine::RepairModel, S, E>(
    phases: &[SolverPhase<'_, S>],
    problem: &P,
) -> Result<(), SolverError<E>> {
    for (phase, p) in phases.iter().enumerate() {
        if let SolverPhase::RuinRecreate(c) = p {
            c.validate_candidates(problem.candidate_count())
                .map_err(|error| SolverError::Repair { phase, error })?;
            for &(n, w) in c.additional_neighborhoods {
                if w > 0 {
                    problem
                        .validate_neighborhood(n)
                        .map_err(|error| SolverError::Repair { phase, error })?;
                }
            }
        }
    }
    Ok(())
}
fn validate_basic_phase_neighborhoods<V: Eq, S, E>(
    problem: &FiniteProblem<V>,
    phases: &[SolverPhase<'_, S>],
    bound: impl Fn(usize) -> bool,
    error: impl Fn(ModelError) -> E,
) -> Result<(), SolverError<E>> {
    for (phase, config) in phases.iter().enumerate() {
        if let SolverPhase::LocalSearch(config) = config
            && !bound(phase)
        {
            for (neighborhood, _) in config.neighborhoods() {
                neighborhood
                    .validate_basic(problem)
                    .map_err(|e| SolverError::Phase {
                        phase,
                        error: error(e),
                    })?;
            }
        }
    }
    Ok(())
}
fn validate_list_phase_neighborhoods<V: Eq + Clone, S, E>(
    problem: &ListProblem<V>,
    phases: &[SolverPhase<'_, S>],
    bound: impl Fn(usize) -> bool,
    error: impl Fn(ListError) -> E,
) -> Result<(), SolverError<E>> {
    for (phase, config) in phases.iter().enumerate() {
        if let SolverPhase::LocalSearch(config) = config
            && !bound(phase)
        {
            for (neighborhood, _) in config.neighborhoods() {
                neighborhood
                    .validate_list(problem)
                    .map_err(|e| SolverError::Phase {
                        phase,
                        error: error(e),
                    })?;
            }
        }
    }
    Ok(())
}

/// Run ordered list phases with shared solve budgets.
pub fn solve_list<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(p, n, c, None, None, None)
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
    solve_list_impl(p, n, c, Some(PlanCallback::Best(&mut observer)), None, None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_list_with_events<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(
        p,
        n,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}
fn solve_list_impl<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, ListAssignment<V>, S>,
    mut command: CommandCallback<'_, ListAssignment<V>, ListMove<V>, S, ListScoringError>,
    providers: Option<&mut PhaseMoveProviders<'_, ListProblem<V>, ListAssignment<V>, ListMove<V>>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                SolverPhase::LocalSearch(_) | SolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_repair_candidates(&c.phases, p)?;
    validate_list_phase_neighborhoods(
        p,
        &c.phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| ListLocalSearchError::from(ListScoringError::from(e)),
    )?;
    let raw_initial = p.initial_assignment();
    let prepared =
        p.prepare_assignment(&raw_initial)
            .map_err(|error| SolverError::InitialScoring {
                error: ListScoringError::from(error).into(),
            })?;
    let mut observation = PlanObservation::new(observer);
    observation.repaired_initial = prepared != raw_initial;
    let initial = || {
        let assignment = prepared;
        InitialState {
            required_unassigned_pin: None,
            movable: p.entities().iter().any(|e| !e.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
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
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
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
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
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
                            .map(|score| (score, n.active_constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, ListLocalSearchError> = (|| {
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(crate::provider_construction::list(
                            p,
                            n,
                            a.expect("construction provider handoff"),
                            provider_construction_config(*config, max_scanned_moves),
                            context,
                        )?));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(crate::provider_search::list(
                        p,
                        n,
                        a.expect("provider phase handoff"),
                        config.clone(),
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        n.validate(p, a.as_ref().expect("initial state"))?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_list_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| {
                                n.calculate(p, a)
                                    .map(|score| (score, n.active_constraint_count() as u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => constructed!(if observed {
                        crate::solution_observation::greedy_list_construction_from_with_observer_monitored(
                            time.construction_monitor(),
                            p,
                            n,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            &mut notify,
                        )
                    } else {
                        crate::list_construction::greedy_list_construction_from_monitored(
                            time.construction_monitor(),
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
    solve_mixed_impl(p, n, c, None, None, None)
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
    solve_mixed_impl(p, n, c, Some(PlanCallback::Best(&mut observer)), None, None)
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_mixed_with_events<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(
        p,
        n,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}
fn solve_mixed_impl<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    observer: PlanObserver<'_, MixedAssignment<B, L>, S>,
    mut command: CommandCallback<'_, MixedAssignment<B, L>, MixedMove<B, L>, S, MixedScoringError>,
    providers: Option<&mut crate::solver_extensions::MixedPhaseProviders<'_, B, L>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                MixedSolverPhase::LocalSearch(_) | MixedSolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, MixedSolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let raw_initial = p.initial_assignment();
    let prepared =
        p.prepare_assignment(&raw_initial)
            .map_err(|error| SolverError::InitialScoring {
                error: MixedScoringError::from(error).into(),
            })?;
    let mut observation = PlanObservation::new(observer);
    observation.repaired_initial = prepared != raw_initial;
    let reject_unassigned_pins = c.limits.reject_unassigned_pins;
    let initial = || {
        let assignment = prepared;
        InitialState {
            required_unassigned_pin: required_pin_startup_issue(
                reject_unassigned_pins,
                p.basic(),
                assignment.basic(),
            ),
            movable: p.basic().variables().iter().any(|v| !v.is_pinned())
                || p.lists().entities().iter().any(|e| !e.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    validate_repair_candidates(&phases, p)?;
    validate_basic_phase_neighborhoods(
        p.basic(),
        &phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Basic(e))),
    )?;
    validate_list_phase_neighborhoods(
        p.lists(),
        &phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Lists(e))),
    )?;
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
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
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
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
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
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
                            .map(|score| (score, n.active_constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, MixedLocalSearchError> = (|| {
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(crate::provider_construction::mixed(
                            p,
                            n,
                            a.expect("construction provider handoff"),
                            provider_construction_config(*config, max_scanned_moves),
                            context,
                        )?));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(crate::provider_search::mixed(
                        p,
                        n,
                        a.expect("provider phase handoff"),
                        MixedLocalSearchConfig {
                            search: config.clone(),
                            ..c.phases[index].local().clone()
                        },
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        n.validate(p, a.as_ref().expect("initial state"))?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_mixed_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| {
                                n.calculate(p, a)
                                    .map(|score| (score, n.active_constraint_count() as u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(config) => constructed!(if observed {
                        crate::solution_observation::greedy_mixed_construction_from_with_observer_monitored(
                            time.construction_monitor(),
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
                        crate::mixed_construction::greedy_mixed_construction_from_monitored(
                            time.construction_monitor(),
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
    solve_stream_impl(p, graph, c, None, None, None)
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
    solve_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Best(&mut observer)),
        None,
        None,
    )
}
/// Observe retained bests and the initialization boundary. See [`SolverEvent`].
pub fn solve_stream_with_events<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}
fn solve_stream_impl<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, Assignment<V>, S>,
    mut command: CommandCallback<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
    providers: Option<&mut PhaseMoveProviders<'_, FiniteProblem<V>, Assignment<V>, Move<V>>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                SolverPhase::LocalSearch(_) | SolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_repair_candidates(&c.phases, p)?;
    validate_basic_phase_neighborhoods(
        p,
        &c.phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        LocalSearchError::from,
    )?;
    let observation = PlanObservation::new(observer);
    let reject_unassigned_pins = c.limits.reject_unassigned_pins;
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            required_unassigned_pin: required_pin_startup_issue(
                reject_unassigned_pins,
                p,
                &assignment,
            ),
            movable: p.variables().iter().any(|v| !v.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
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
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
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
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
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
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(
                            crate::provider_construction::basic_stream(
                                p,
                                g,
                                s,
                                a.expect("construction provider handoff"),
                                provider_construction_config(*config, max_scanned_moves),
                                context,
                            )?,
                        ));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(crate::provider_search::basic_stream(
                        p,
                        g,
                        s,
                        a.expect("provider phase handoff"),
                        config.clone(),
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        crate::stream_scoring::validate_source(&g, &s)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_basic_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| exact::score_basic(p, &mut g, &s, a),
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => {
                        constructed!(if observed {
                            crate::solution_observation::greedy_stream_construction_from_with_observer_monitored(
                                time.construction_monitor(),
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                c,
                                &mut notify,
                            )
                        } else {
                            crate::stream_scoring::greedy_stream_construction_from_monitored(
                                time.construction_monitor(),
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
    solve_list_stream_impl(p, graph, c, None, None, None)
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
    solve_list_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Best(&mut observer)),
        None,
        None,
    )
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
    solve_list_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}
fn solve_list_stream_impl<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, ListAssignment<V>, S>,
    mut command: CommandCallback<'_, ListAssignment<V>, ListMove<V>, S, ListStreamScoringError>,
    providers: Option<&mut PhaseMoveProviders<'_, ListProblem<V>, ListAssignment<V>, ListMove<V>>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                SolverPhase::LocalSearch(_) | SolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_repair_candidates(&c.phases, p)?;
    validate_list_phase_neighborhoods(
        p,
        &c.phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| ListLocalSearchError::from(ListScoringError::from(e)),
    )?;
    let raw_initial = p.initial_assignment();
    let prepared =
        p.prepare_assignment(&raw_initial)
            .map_err(|error| SolverError::InitialScoring {
                error: ListStreamScoringError::from(error).into(),
            })?;
    let mut observation = PlanObservation::new(observer);
    observation.repaired_initial = prepared != raw_initial;
    let initial = || {
        let assignment = prepared;
        InitialState {
            required_unassigned_pin: None,
            movable: p.entities().iter().any(|e| !e.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
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
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
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
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
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
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(
                            crate::provider_construction::list_stream(
                                p,
                                g,
                                s,
                                a.expect("construction provider handoff"),
                                provider_construction_config(*config, max_scanned_moves),
                                context,
                            )?,
                        ));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(crate::provider_search::list_stream(
                        p,
                        g,
                        s,
                        a.expect("provider phase handoff"),
                        config.clone(),
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        s.validate(&g)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_list_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| exact::score_list(p, &mut g, &s, a),
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => {
                        constructed!(if observed {
                            crate::solution_observation::greedy_list_stream_construction_from_with_observer_monitored(
                                time.construction_monitor(),
                                p,
                                g,
                                s,
                                a.unwrap_or_else(|| p.initial_assignment()),
                                c,
                                &mut notify,
                            )
                        } else {
                            crate::list_stream_scoring::greedy_list_stream_construction_from_monitored(
                                time.construction_monitor(),
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
    solve_mixed_stream_impl(p, graph, c, None, None, None)
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
    solve_mixed_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Best(&mut observer)),
        None,
        None,
    )
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
    solve_mixed_stream_impl(
        p,
        graph,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
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
    providers: Option<&mut crate::solver_extensions::MixedPhaseProviders<'_, B, L>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                MixedSolverPhase::LocalSearch(_) | MixedSolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, MixedSolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let raw_initial = p.initial_assignment();
    let prepared =
        p.prepare_assignment(&raw_initial)
            .map_err(|error| SolverError::InitialScoring {
                error: MixedStreamScoringError::from(error).into(),
            })?;
    let mut observation = PlanObservation::new(observer);
    observation.repaired_initial = prepared != raw_initial;
    let reject_unassigned_pins = c.limits.reject_unassigned_pins;
    let initial = || {
        let assignment = prepared;
        InitialState {
            required_unassigned_pin: required_pin_startup_issue(
                reject_unassigned_pins,
                p.basic(),
                assignment.basic(),
            ),
            movable: p.basic().variables().iter().any(|v| !v.is_pinned())
                || p.lists().entities().iter().any(|e| !e.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    validate_repair_candidates(&phases, p)?;
    validate_basic_phase_neighborhoods(
        p.basic(),
        &phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Basic(e))),
    )?;
    validate_list_phase_neighborhoods(
        p.lists(),
        &phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Lists(e))),
    )?;
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
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
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
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
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
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
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(
                            crate::provider_construction::mixed_stream(
                                p,
                                g,
                                s,
                                a.expect("construction provider handoff"),
                                provider_construction_config(*config, max_scanned_moves),
                                context,
                            )?,
                        ));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(crate::provider_search::mixed_stream(
                        p,
                        g,
                        s,
                        a.expect("provider phase handoff"),
                        MixedLocalSearchConfig {
                            search: config.clone(),
                            ..c.phases[index].local().clone()
                        },
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        s.validate(&g)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_mixed_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| exact::score_mixed(p, &mut g, &s, a),
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(config) => {
                        constructed!(if observed {
                            crate::solution_observation::greedy_mixed_stream_construction_from_with_observer_monitored(
                                time.construction_monitor(),
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
                            crate::mixed_stream_scoring::greedy_mixed_stream_construction_from_monitored(
                                time.construction_monitor(),
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
    solve_impl(p, n, c, None, Some(&mut command), None)
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
        None,
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
    solve_stream_impl(p, graph, c, None, Some(&mut command), None)
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
        None,
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
    solve_list_impl(p, n, c, None, Some(&mut command), None)
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
        None,
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
    solve_list_stream_impl(p, graph, c, None, Some(&mut command), None)
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
        None,
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
    solve_mixed_impl(p, n, c, None, Some(&mut command), None)
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
        None,
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
    solve_mixed_stream_impl(p, graph, c, None, Some(&mut command), None)
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
        None,
    )
}

/// Run a complete plan with per-phase move providers, commands and events.
pub fn solve_with_extensions<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut extensions: SolverExtensions<'_, FiniteProblem<V>, Assignment<V>, Move<V>, S, ScoringError>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(
        p,
        n,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

/// Run a complete plan with per-phase move providers, commands and events.
pub fn solve_list_with_extensions<'c, V: Eq + Clone, S: AdditiveScore + 'c>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverSequence<'c, S>>,
    mut extensions: SolverExtensions<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
        ListScoringError,
    >,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(
        p,
        n,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

/// Run a complete plan with per-phase move providers, commands and events.
pub fn solve_mixed_with_extensions<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore + 'c>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut extensions: MixedSolverExtensions<'_, B, L, S, MixedScoringError>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(
        p,
        n,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

/// Run a complete plan with per-phase move providers, commands and events.
pub fn solve_stream_with_extensions<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut extensions: SolverExtensions<
        '_,
        FiniteProblem<V>,
        Assignment<V>,
        Move<V>,
        S,
        StreamScoringError,
    >,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(
        p,
        graph,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

/// Run a complete plan with per-phase move providers, commands and events.
pub fn solve_list_stream_with_extensions<
    'c,
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverSequence<'c, S>>,
    mut extensions: SolverExtensions<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
        ListStreamScoringError,
    >,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(
        p,
        graph,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

/// Run a complete plan with per-phase move providers, commands and events.
pub fn solve_mixed_stream_with_extensions<
    'c,
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut extensions: MixedSolverExtensions<'_, B, L, S, MixedStreamScoringError>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(
        p,
        graph,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

#[path = "calculator_solver.rs"]
mod calculator;
pub use calculator::{
    solve_calculator, solve_calculator_with_events, solve_calculator_with_extensions,
    solve_calculator_with_observer, solve_list_calculator, solve_list_calculator_with_events,
    solve_list_calculator_with_extensions, solve_list_calculator_with_observer,
    solve_mixed_calculator, solve_mixed_calculator_with_events,
    solve_mixed_calculator_with_extensions, solve_mixed_calculator_with_observer,
};

#[cfg(test)]
#[path = "solver_phase_check_tests.rs"]
mod phase_check_tests;
