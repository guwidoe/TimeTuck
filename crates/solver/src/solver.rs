//! Construction followed by configured improvement phases under shared controls.
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
    /// Includes a fresh initial score in every phase that performs scoring.
    pub evaluations: u64,
    /// Candidates only; phase initialization and diagnostics are excluded.
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
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseReport<S> {
    /// Zero is construction; subsequent indices refer to local_search[index-1].
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
    EvaluationLimit,
    MoveEvaluationLimit,
    TimeLimit,
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
impl<A, S: Copy> SolverResult<A, S> {
    /// Complete, scored solution; completeness does not imply hard feasibility.
    pub fn solution(&self) -> Option<(&A, S)> {
        if self.complete {
            self.score.map(|s| (&self.assignment, s))
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

#[derive(Clone, Copy)]
enum PhaseRequest<'a, S> {
    Construction(ConstructionConfig<'a>),
    LocalSearch(LocalSearchConfig<'a, S>),
}
// Clock readings are elapsed from entry to run(), including plan validation.
// Clip immediately before phase entry, after any graph factory preparation.
struct SolveTime<'c> {
    limit: Option<Duration>,
    clock: &'c mut dyn FnMut() -> Duration,
    last: Duration,
    shared_bound: bool,
}
impl SolveTime<'_> {
    fn remaining(&mut self) -> Option<Duration> {
        self.limit.map(|limit| {
            self.last = self.last.max((self.clock)());
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
impl<S> PhaseRequest<'_, S> {
    fn with_time(mut self, time: &mut SolveTime<'_>) -> Self {
        match &mut self {
            Self::Construction(c) => time.bound(&mut c.max_time),
            Self::LocalSearch(c) => time.bound(&mut c.max_time),
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
fn run<A, S: AdditiveScore, E>(
    limits: SolverLimits<'_, S>,
    construction: ConstructionConfig<'_>,
    local: &[LocalSearchConfig<'_, S>],
    phase: impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveTime<'_>,
    ) -> Result<PhaseOutput<A, S>, E>,
) -> Result<SolverResult<A, S>, SolverError<E>>
where
    A: Clone,
{
    let mut clock = crate::time_limit::monotonic_clock(limits.max_time.is_some());
    run_with_clock(limits, construction, local, phase, &mut clock)
}
fn run_with_clock<A, S: AdditiveScore, E>(
    limits: SolverLimits<'_, S>,
    construction: ConstructionConfig<'_>,
    local: &[LocalSearchConfig<'_, S>],
    mut phase: impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveTime<'_>,
    ) -> Result<PhaseOutput<A, S>, E>,
    clock: &mut dyn FnMut() -> Duration,
) -> Result<SolverResult<A, S>, SolverError<E>>
where
    A: Clone,
{
    let mut time = SolveTime {
        limit: limits.max_time,
        clock,
        last: Duration::ZERO,
        shared_bound: false,
    };
    // Validate the entire plan before scoring, including phases that might never run.
    for (index, token) in std::iter::once(construction.cancellation)
        .chain(local.iter().map(|c| c.cancellation))
        .enumerate()
    {
        if let (Some(global), Some(token)) = (limits.cancellation, token)
            && global != token
        {
            return Err(SolverError::ConflictingCancellation { phase: index });
        }
    }
    for (index, c) in local.iter().enumerate() {
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
        } else {
            let mut c = local[index - 1];
            let owned = (
                remaining_scores <= c.max_evaluations,
                remaining_moves.is_some_and(|n| c.max_move_evaluations.is_none_or(|m| n <= m)),
            );
            c.max_evaluations = c.max_evaluations.min(remaining_scores);
            c.max_move_evaluations = smaller(c.max_move_evaluations, remaining_moves);
            c.cancellation = c.cancellation.or(limits.cancellation);
            c.best_score_limit = match (c.best_score_limit, limits.best_score_limit) {
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
            score: out.score,
            complete: out.complete,
            work: SolverWork::default(),
            phases: Vec::new(),
            termination: SolverTermination::PlanCompleted,
        });
        if let (Some(assignment), Some(score)) = (out.assignment.take(), out.score)
            && r.score.is_none_or(|best| score > best)
        {
            r.assignment = assignment;
            r.score = Some(score);
            r.complete = out.complete;
        }
        r.work.add(out.work);
        r.phases.push(PhaseReport {
            index,
            termination: out.termination,
            work: out.work,
            score: out.score,
        });
        use PhaseTermination::{Construction as C, LocalSearch as L};
        let reason = match out.termination {
            C(ConstructionTermination::NoCompleteAssignment) => {
                Some(SolverTermination::NoCompleteAssignment)
            }
            C(ConstructionTermination::Cancelled) | L(LocalSearchTermination::Cancelled) => {
                Some(SolverTermination::Cancelled)
            }
            C(ConstructionTermination::EvaluationLimit)
            | L(LocalSearchTermination::EvaluationLimit)
                if global_scores =>
            {
                Some(SolverTermination::EvaluationLimit)
            }
            C(ConstructionTermination::MoveEvaluationLimit)
            | L(LocalSearchTermination::MoveEvaluationLimit)
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
            C(ConstructionTermination::TimeLimit) | L(LocalSearchTermination::TimeLimit)
                if time.shared_bound =>
            {
                Some(SolverTermination::TimeLimit)
            }
            _ if time.expired() => Some(SolverTermination::TimeLimit),
            _ if !r.complete => Some(SolverTermination::IncompleteConstruction),
            _ if r.score.is_some_and(|score| {
                limits
                    .best_score_limit
                    .is_some_and(|target| score >= target)
            }) =>
            {
                Some(SolverTermination::BestScoreLimit)
            }
            _ if limits.stop_when_feasible && r.score.is_some_and(Score::is_feasible) => {
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
/// No result of these heuristic phases proves global optimality.
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
pub fn solve<V: Eq + Clone, S: AdditiveScore>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    c: SolverConfig<'_, S>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    run(
        c.limits,
        c.construction,
        &c.local_search,
        |_, a, request, time| {
            Ok(match request.with_time(time) {
                PhaseRequest::Construction(c) => constructed!(greedy_construction(p, n, c)?),
                PhaseRequest::LocalSearch(c) => {
                    improved!(local_search_from(p, n, a.expect("handoff"), c)?)
                }
            })
        },
    )
}
/// Construct and improve ordered lists with shared solve budgets.
pub fn solve_list<V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    c: SolverConfig<'_, S>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    run(
        c.limits,
        c.construction,
        &c.local_search,
        |_, a, request, time| {
            Ok(match request.with_time(time) {
                PhaseRequest::Construction(c) => constructed!(greedy_list_construction(p, n, c)?),
                PhaseRequest::LocalSearch(c) => {
                    improved!(list_local_search_from(p, n, a.expect("handoff"), c)?)
                }
            })
        },
    )
}
/// Construct and improve coupled basic/list models. Each phase retains its own
/// construction order or joint-neighborhood setting.
pub fn solve_mixed<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    c: MixedSolverConfig<'_, S>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let phases: Vec<_> = c.local_search.iter().map(|c| c.search).collect();
    run(
        c.limits,
        c.construction.construction,
        &phases,
        |index, a, request, time| {
            Ok(match request.with_time(time) {
                PhaseRequest::Construction(config) => constructed!(greedy_mixed_construction(
                    p,
                    n,
                    MixedConstructionConfig {
                        construction: config,
                        ..c.construction
                    }
                )?),
                PhaseRequest::LocalSearch(config) => improved!(mixed_local_search_from(
                    p,
                    n,
                    a.expect("handoff"),
                    MixedLocalSearchConfig {
                        search: config,
                        ..c.local_search[index - 1]
                    }
                )?),
            })
        },
    )
}

/// Stream counterpart of solve. The factory must provide a fresh graph with empty
/// planning sources and the same deterministic objective for each started phase.
/// Graph preparation is not a solve score evaluation. Phase binding/rescoring is.
/// Skipped improvement phases do not call the factory. Factory errors identify
/// their phase; previously completed work is not returned on an error.
pub fn solve_stream<'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    c: SolverConfig<'_, S>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    run(
        c.limits,
        c.construction,
        &c.local_search,
        |_, a, request, time| {
            let (g, s) = graph().map_err(StreamScoringError::from)?;
            Ok(match request.with_time(time) {
                PhaseRequest::Construction(c) => {
                    constructed!(greedy_stream_construction(p, g, s, c)?)
                }
                PhaseRequest::LocalSearch(c) => {
                    improved!(stream_local_search_from(p, g, s, a.expect("handoff"), c)?)
                }
            })
        },
    )
}
/// Ordered-list stream solve; the factory has the same contract as solve_stream.
pub fn solve_list_stream<'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    c: SolverConfig<'_, S>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    run(
        c.limits,
        c.construction,
        &c.local_search,
        |_, a, request, time| {
            let (g, s) = graph().map_err(ListStreamScoringError::from)?;
            Ok(match request.with_time(time) {
                PhaseRequest::Construction(c) => {
                    constructed!(greedy_list_stream_construction(p, g, s, c)?)
                }
                PhaseRequest::LocalSearch(c) => improved!(list_stream_local_search_from(
                    p,
                    g,
                    s,
                    a.expect("handoff"),
                    c
                )?),
            })
        },
    )
}
/// Mixed stream solve; each phase uses fresh state with the same objective.
pub fn solve_mixed_stream<
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    c: MixedSolverConfig<'_, S>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let phases: Vec<_> = c.local_search.iter().map(|c| c.search).collect();
    run(
        c.limits,
        c.construction.construction,
        &phases,
        |index, a, request, time| {
            let (g, s) = graph().map_err(MixedStreamScoringError::from)?;
            Ok(match request.with_time(time) {
                PhaseRequest::Construction(config) => {
                    constructed!(greedy_mixed_stream_construction(
                        p,
                        g,
                        s,
                        MixedConstructionConfig {
                            construction: config,
                            ..c.construction
                        }
                    )?)
                }
                PhaseRequest::LocalSearch(config) => improved!(mixed_stream_local_search_from(
                    p,
                    g,
                    s,
                    a.expect("handoff"),
                    MixedLocalSearchConfig {
                        search: config,
                        ..c.local_search[index - 1]
                    }
                )?),
            })
        },
    )
}

#[cfg(test)]
#[path = "solver_time_tests.rs"]
mod time_tests;
