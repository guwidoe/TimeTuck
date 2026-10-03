//! Construction followed by configured improvement phases under shared controls.
#[path = "solver_exact.rs"]
mod exact;
use crate::*;
use std::{error::Error, fmt, time::Duration};

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
    /// Completed local-search steps since the last strict retained-best improvement.
    /// Counts across phases; construction and initialization do not count.
    pub max_unimproved_steps: Option<u64>,
    pub cancellation: Option<&'a CancellationToken>,
    pub best_score_limit: Option<S>,
    pub stop_when_feasible: bool,
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
    /// Local-search statistics only; construction placements are not steps.
    pub completed_steps: u64,
    pub accepted_moves: u64,
}
impl SolverWork {
    fn add(&mut self, other: Self) {
        self.evaluations += other.evaluations;
        self.move_evaluations += other.move_evaluations;
        self.constraint_evaluations += other.constraint_evaluations;
        self.completed_steps += other.completed_steps;
        self.accepted_moves += other.accepted_moves;
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseTermination {
    Construction(ConstructionTermination),
    LocalSearch(LocalSearchTermination),
    Exact(ExactPhaseTermination),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseReport<S> {
    /// Zero is construction; subsequent indices refer to phases[index-1].
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
    /// A complete scored solution and an exhausted exact assignment space.
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
/// The retained complete best, or partial construction progress if initialization
/// did not complete. Equal-score phase results retain the earlier assignment.
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
            Self::InvalidAcceptance { phase, error } => {
                write!(f, "invalid acceptance in phase {phase}: {error}")
            }
            Self::ConflictingCancellation { phase } => write!(
                f,
                "phase {phase} uses a different cancellation token from the solve"
            ),
            Self::Phase { phase, error } => write!(f, "phase {phase} failed: {error}"),
        }
    }
}
impl<E: Error + 'static> Error for SolverError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidAcceptance { error, .. } => Some(error),
            Self::Phase { error, .. } => Some(error),
            Self::ConflictingCancellation { .. } => None,
        }
    }
}

#[derive(Clone)]
enum PhaseRequest<'a, S> {
    Construction(ConstructionConfig<'a>),
    LocalSearch(LocalSearchConfig<'a, S>),
    Exact(ExactPhaseConfig<'a, S>, Option<S>),
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
            Self::Exact(c, _) => time.bound(&mut c.max_time),
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
    Construction,
    LocalSearch,
    Exact,
}
/// A synchronous retained-best update from one ordered solve. Phase zero is
/// construction; later indices address the requested improvement phases plus one.
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
type PlanObserver<'a, A, S> = Option<&'a mut dyn FnMut(SolverUpdate<'_, A, S>)>;
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
    fn notify(&mut self, phase: usize, kind: SolverPhaseKind, update: SolutionUpdate<'_, A, S>) {
        let Some(callback) = &mut self.callback else {
            return;
        };
        if (!self.complete && update.complete)
            || self.score.as_ref().is_none_or(|best| update.score > *best)
        {
            self.score = Some(update.score.clone());
            self.complete = update.complete;
            callback(SolverUpdate {
                phase,
                kind,
                assignment: update.assignment,
                score: update.score,
                complete: update.complete,
            });
        }
    }
}
impl<S> PhaseRequest<'_, S> {
    fn kind(&self) -> SolverPhaseKind {
        match self {
            Self::Construction(_) => SolverPhaseKind::Construction,
            Self::LocalSearch(_) => SolverPhaseKind::LocalSearch,
            Self::Exact(..) => SolverPhaseKind::Exact,
        }
    }
}

fn run<A, S: AdditiveScore, E>(
    limits: SolverLimits<'_, S>,
    construction: ConstructionConfig<'_>,
    local: &[SolverPhase<'_, S>],
    phase: impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveControl<'_>,
    ) -> Result<PhaseOutput<A, S>, E>,
) -> Result<SolverResult<A, S>, SolverError<E>>
where
    A: Clone,
{
    let mut clock = crate::time_limit::monotonic_clock(
        limits.max_time.is_some() || limits.max_unimproved_time.is_some(),
    );
    run_with_clock(limits, construction, local, phase, &mut clock)
}
fn run_with_clock<A, S: AdditiveScore, E>(
    limits: SolverLimits<'_, S>,
    construction: ConstructionConfig<'_>,
    local: &[SolverPhase<'_, S>],
    mut phase: impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveControl<'_>,
    ) -> Result<PhaseOutput<A, S>, E>,
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
    // Validate the entire plan before scoring, including phases that might never run.
    for (index, token) in std::iter::once(construction.cancellation)
        .chain(local.iter().map(|c| match c {
            SolverPhase::LocalSearch(c) => c.cancellation,
            SolverPhase::Exact(c) => c.cancellation,
        }))
        .enumerate()
    {
        if let (Some(global), Some(token)) = (limits.cancellation, token)
            && global != token
        {
            return Err(SolverError::ConflictingCancellation { phase: index });
        }
    }
    for (index, c) in local.iter().enumerate() {
        let SolverPhase::LocalSearch(c) = c else {
            continue;
        };
        c.acceptance
            .validate()
            .map_err(|error| SolverError::InvalidAcceptance {
                phase: index + 1,
                error,
            })?;
    }
    let mut result: Option<SolverResult<A, S>> = None;
    for index in 0..=local.len() {
        let used = result.as_ref().map_or(SolverWork::default(), |r| r.work);
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
                let r = result.as_mut().unwrap();
                r.termination = reason;
                break;
            }
        }
        let (request, global_scores, global_moves) = if index == 0 {
            let mut c = construction;
            let owned = (
                remaining_scores <= c.max_evaluations,
                remaining_moves.is_some_and(|n| c.max_move_evaluations.is_none_or(|m| n <= m)),
            );
            c.max_evaluations = c.max_evaluations.min(remaining_scores);
            c.max_move_evaluations = smaller(c.max_move_evaluations, remaining_moves);
            c.cancellation = c.cancellation.or(limits.cancellation);
            (PhaseRequest::Construction(c), owned.0, owned.1)
        } else if let SolverPhase::Exact(mut c) = local[index - 1].clone() {
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
            let prior = result
                .as_ref()
                .filter(|r| r.complete)
                .and_then(|r| r.score.clone());
            (PhaseRequest::Exact(c, prior), owned.0, owned.1)
        } else {
            let SolverPhase::LocalSearch(mut c) = local[index - 1].clone() else {
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
        let initial = result.as_ref().map(|r| r.assignment.clone());
        let mut out =
            phase(index, initial, request, &mut time).map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })?;
        let r = result.get_or_insert_with(|| SolverResult {
            assignment: out
                .assignment
                .take()
                .expect("construction always returns an assignment"),
            score: out.score.clone(),
            complete: out.complete,
            work: SolverWork::default(),
            phases: Vec::new(),
            termination: SolverTermination::PlanCompleted,
        });
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
        use PhaseTermination::{Construction as C, Exact as X, LocalSearch as L};
        let reason = match out.termination {
            X(ExactPhaseTermination::Exhausted) => Some(if r.complete && r.score.is_some() {
                SolverTermination::Optimal
            } else {
                SolverTermination::NoCompleteAssignment
            }),
            C(ConstructionTermination::NoCompleteAssignment) => {
                Some(SolverTermination::NoCompleteAssignment)
            }
            C(ConstructionTermination::Cancelled)
            | L(LocalSearchTermination::Cancelled)
            | X(ExactPhaseTermination::Cancelled) => Some(SolverTermination::Cancelled),
            C(ConstructionTermination::EvaluationLimit)
            | L(LocalSearchTermination::EvaluationLimit)
            | X(ExactPhaseTermination::EvaluationLimit)
                if global_scores =>
            {
                Some(SolverTermination::EvaluationLimit)
            }
            C(ConstructionTermination::MoveEvaluationLimit)
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
            C(ConstructionTermination::TimeLimit)
            | L(LocalSearchTermination::TimeLimit)
            | X(ExactPhaseTermination::TimeLimit)
                if time.shared_bound =>
            {
                Some(SolverTermination::TimeLimit)
            }
            _ if time.expired() => Some(SolverTermination::TimeLimit),
            _ if time.stagnated() => Some(SolverTermination::UnimprovedTimeLimit),
            _ if time.step_stagnated() => Some(SolverTermination::UnimprovedStepLimit),
            _ if !r.complete && !matches!(local.get(index), Some(SolverPhase::Exact(_))) => {
                Some(SolverTermination::IncompleteConstruction)
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
    Ok(result.expect("construction phase always runs"))
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
pub fn solve<'c, V: Eq + Clone, S: AdditiveScore>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverPlan<'c, S>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(p, n, c, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_with_observer<'c, V: Eq + Clone, S: AdditiveScore>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverPlan<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_impl(p, n, c, Some(&mut observer))
}
fn solve_impl<'c, V: Eq + Clone, S: AdditiveScore>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: impl Into<SolverPlan<'c, S>>,
    observer: PlanObserver<'_, Assignment<V>, S>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let c = c.into();
    let mut observation = PlanObservation::new(observer);
    run(
        c.limits,
        c.construction,
        &c.phases,
        |index, a, request, time| {
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            Ok(match request {
                PhaseRequest::Exact(c, prior) => exact::run_observed(
                    c,
                    prior,
                    time,
                    |visit| crate::search::visit_assignments(p, visit),
                    |a| {
                        n.calculate(p, a)
                            .map(|score| (score, n.constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )?,
                PhaseRequest::Construction(c) => constructed!(if observed {
                    greedy_construction_with_observer(p, n, c, &mut notify)
                } else {
                    greedy_construction(p, n, c)
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
        },
    )
}
/// Construct and improve ordered lists with shared solve budgets.
pub fn solve_list<'c, V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverPlan<'c, S>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(p, n, c, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_list_with_observer<'c, V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverPlan<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_impl(p, n, c, Some(&mut observer))
}
fn solve_list_impl<'c, V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: impl Into<SolverPlan<'c, S>>,
    observer: PlanObserver<'_, ListAssignment<V>, S>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let c = c.into();
    let mut observation = PlanObservation::new(observer);
    run(
        c.limits,
        c.construction,
        &c.phases,
        |index, a, request, time| {
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            Ok(match request {
                PhaseRequest::Exact(c, prior) => exact::run_observed(
                    c,
                    prior,
                    time,
                    |visit| crate::list_search::visit_list_assignments(p, visit),
                    |a| {
                        n.calculate(p, a)
                            .map(|score| (score, n.constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )?,
                PhaseRequest::Construction(c) => constructed!(if observed {
                    greedy_list_construction_with_observer(p, n, c, &mut notify)
                } else {
                    greedy_list_construction(p, n, c)
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
        },
    )
}
/// Construct and improve coupled basic/list models. Each phase retains its own
/// construction order or joint-neighborhood setting.
pub fn solve_mixed<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverPlan<'c, S>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(p, n, c, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_mixed_with_observer<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverPlan<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_impl(p, n, c, Some(&mut observer))
}
fn solve_mixed_impl<'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: impl Into<MixedSolverPlan<'c, S>>,
    observer: PlanObserver<'_, MixedAssignment<B, L>, S>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let c = c.into();
    let mut observation = PlanObservation::new(observer);
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    run(
        c.limits,
        c.construction.construction,
        &phases,
        |index, a, request, time| {
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                observation.notify(index, kind, update)
            };
            Ok(match request {
                PhaseRequest::Exact(c, prior) => exact::run_observed(
                    c,
                    prior,
                    time,
                    |visit| exact::visit_mixed(p, visit),
                    |a| {
                        n.calculate(p, a)
                            .map(|score| (score, n.constraint_count() as u128))
                    },
                    observed.then_some(&mut notify),
                )?,
                PhaseRequest::Construction(config) => constructed!(if observed {
                    greedy_mixed_construction_with_observer(
                        p,
                        n,
                        MixedConstructionConfig {
                            construction: config,
                            ..c.construction
                        },
                        &mut notify,
                    )
                } else {
                    greedy_mixed_construction(
                        p,
                        n,
                        MixedConstructionConfig {
                            construction: config,
                            ..c.construction
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
                            ..c.phases[index - 1].local().clone()
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
                            ..c.phases[index - 1].local().clone()
                        },
                    )
                }?),
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
    c: impl Into<SolverPlan<'c, S>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(p, graph, c, None)
}
/// Observe retained-best progress throughout construction, local search and
/// exact phases. Shares all validation, work limits and result semantics with
/// the ordinary solve. Notifications are synchronous, suppress cross-phase
/// duplicates and do not rescore. Observer time belongs to solve/phase limits;
/// panics propagate. See [`SolverUpdate`] for native phase metadata.
pub fn solve_stream_with_observer<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverPlan<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_stream_impl(p, graph, c, Some(&mut observer))
}
fn solve_stream_impl<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: impl Into<SolverPlan<'c, S>>,
    observer: PlanObserver<'_, Assignment<V>, S>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let c = c.into();
    let mut observation = PlanObservation::new(observer);
    run(
        c.limits,
        c.construction,
        &c.phases,
        |index, a, request, time| {
            let (mut g, s) = graph().map_err(StreamScoringError::from)?;
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            Ok(match request {
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
                        greedy_stream_construction_with_observer(p, g, s, c, &mut notify)
                    } else {
                        greedy_stream_construction(p, g, s, c)
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
        },
    )
}
/// Ordered-list stream solve; the factory has the same contract as solve_stream.
pub fn solve_list_stream<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverPlan<'c, S>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(p, graph, c, None)
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
    c: impl Into<SolverPlan<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_stream_impl(p, graph, c, Some(&mut observer))
}
fn solve_list_stream_impl<'c, 'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: impl Into<SolverPlan<'c, S>>,
    observer: PlanObserver<'_, ListAssignment<V>, S>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let c = c.into();
    let mut observation = PlanObservation::new(observer);
    run(
        c.limits,
        c.construction,
        &c.phases,
        |index, a, request, time| {
            let (mut g, s) = graph().map_err(ListStreamScoringError::from)?;
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            Ok(match request {
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
                        greedy_list_stream_construction_with_observer(p, g, s, c, &mut notify)
                    } else {
                        greedy_list_stream_construction(p, g, s, c)
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
    c: impl Into<MixedSolverPlan<'c, S>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(p, graph, c, None)
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
    c: impl Into<MixedSolverPlan<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_stream_impl(p, graph, c, Some(&mut observer))
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
    c: impl Into<MixedSolverPlan<'c, S>>,
    observer: PlanObserver<'_, MixedAssignment<B, L>, S>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let c = c.into();
    let mut observation = PlanObservation::new(observer);
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    run(
        c.limits,
        c.construction.construction,
        &phases,
        |index, a, request, time| {
            let (mut g, s) = graph().map_err(MixedStreamScoringError::from)?;
            let request = request.with_time(time);
            let observed = observation.callback.is_some();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                observation.notify(index, kind, update)
            };
            Ok(match request {
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
                        greedy_mixed_stream_construction_with_observer(
                            p,
                            g,
                            s,
                            MixedConstructionConfig {
                                construction: config,
                                ..c.construction
                            },
                            &mut notify,
                        )
                    } else {
                        greedy_mixed_stream_construction(
                            p,
                            g,
                            s,
                            MixedConstructionConfig {
                                construction: config,
                                ..c.construction
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
                            ..c.phases[index - 1].local().clone()
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
                            ..c.phases[index - 1].local().clone()
                        },
                    )
                }?),
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
