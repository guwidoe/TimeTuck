//! Deterministic greedy construction for unassigned required basic variables.
use crate::search_scoring::SearchDirector;
use crate::{
    AdditiveScore, Assignment, ConstraintNetwork, FiniteProblem, Move, ScoreDirector,
    ScoredAssignment, ScoringError,
};

/// When to stop scanning one variable's domain. Without an early pick, select
/// the first highest-scoring value in the evaluated prefix of the domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionPickEarly {
    Never,
    FirstNonDeterioratingScore,
    FirstFeasibleScore,
    FirstFeasibleScoreOrNonDeterioratingHard,
}
impl ConstructionPickEarly {
    /// Compare ordinary score levels, ignoring the network's structural count
    /// of required unassigned variables. "Feasible" here means hard-feasible;
    /// a partially constructed assignment is still incomplete.
    pub fn should_pick<S: AdditiveScore>(self, previous: S, candidate: S) -> bool {
        let previous = previous.with_structural(0);
        let candidate = candidate.with_structural(0);
        match self {
            Self::Never => false,
            Self::FirstNonDeterioratingScore => candidate >= previous,
            Self::FirstFeasibleScore => candidate.is_feasible(),
            Self::FirstFeasibleScoreOrNonDeterioratingHard => {
                candidate.is_feasible() || !candidate.hard_cmp(&previous).is_lt()
            }
        }
    }
}

/// How construction treats missing optional values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OptionalConstruction {
    /// Leave optional values for local search.
    #[default]
    Preserve,
    /// Evaluate legal assignments and retain the first best candidate whose score
    /// is at least the current score. Otherwise preserve unassignment. Existing
    /// assignments and pinned unassignment are never reconsidered.
    NonWorsening,
}

/// Which unassigned items compete within a construction step.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConstructionPlacement {
    /// Finish one item in model order before considering the next.
    #[default]
    Sequential,
    /// Reconsider all eligible missing items each step, installing the first best
    /// placement. With Never early-picking this is cheapest insertion. Optional
    /// candidates must be non-worsening; a pool with no accepted move ends the phase.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = FiniteProblem::new(vec![Variable::new(vec![1]), Variable::new(vec![9])])?;
    /// let mut n = ConstraintNetwork::new(2);
    /// n.add_constraint("sum", vec![0,1], |v| SimpleScore::new(v.iter().filter_map(|v| v.copied()).sum()))?;
    /// let r = greedy_construction(&p, &n, ConstructionConfig {
    ///     placement: ConstructionPlacement::Pooled, max_steps: Some(1), ..Default::default()
    /// })?;
    /// assert_eq!(r.assignment.values(), &[None, Some(9)]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    Pooled,
    /// Cycle through distinct basic values in first model/domain encounter order.
    /// For each value choose the best eligible missing variable in model order.
    /// Skip values without targets; stop after a full rotation installs nothing.
    /// List variables use their ordinary sequential value insertion. Mixed models
    /// finish each component in the configured order.
    ValueQueue,
}

/// Shared controls for basic, list and mixed construction in both scoring
/// backends. A cancellation request retains the best evaluated candidate of the
/// current scan, including incomplete progress. Initialization and diagnostics
/// are not interrupted. Validation, impossibility and zero score-budget checks
/// precede cancellation; already-observed evaluation exhaustion also takes
/// precedence. A zero move budget permits initialization, then no candidates.
/// The default token is None. Configurations borrow tokens and remain Copy.
///
/// A sequential step evaluates one missing item; a pooled step compares all
/// remaining eligible items; a queued-value step compares receivers of one value.
/// Required items are assigned; optional scans
/// may reject all candidates. A step limit preserves an entire scan, unlike a
/// candidate limit (unless another control interrupts the scan):
///
/// ```
/// use timetuck::{ConstructionConfig, ConstructionTermination, ConstraintNetwork,
///     FiniteProblem, SimpleScore, Variable, greedy_construction,
///     greedy_construction_from};
/// let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2])?;
/// let n = ConstraintNetwork::<_, SimpleScore>::new(2);
/// let config = ConstructionConfig { max_steps: Some(1), ..Default::default() };
/// let first = greedy_construction(&p, &n, config)?;
/// assert_eq!(first.termination, ConstructionTermination::StepLimit);
/// assert_eq!(first.evaluations, 3); // Initial score and both values of the first variable.
/// assert_eq!(first.assignment.values(), &[Some(1), None]);
/// let next = greedy_construction_from(&p, &n, first.assignment, config)?;
/// assert_eq!(next.assigned_variables, 1); // Fresh per-call step budget.
/// assert_eq!(next.termination, ConstructionTermination::Completed);
/// assert!(next.complete);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstructionConfig<'a> {
    /// Sequential by default; optionally compare a pool or cycle through values.
    pub placement: ConstructionPlacement,
    /// Cooperative cancellation shared with other phases; validation runs first.
    pub cancellation: Option<&'a crate::CancellationToken>,
    /// Includes the initial score and each candidate; zero invokes no callbacks.
    pub max_evaluations: u64,
    /// Candidate evaluations only; initialization is excluded. None disables it.
    pub max_move_evaluations: Option<u64>,
    /// Evaluated item, whole-pool or queued-value target scans per call.
    /// None disables this limit.
    /// Scans of optional values count even if every candidate is rejected.
    /// Initialization, existing assignments and scans with no candidates do not
    /// consume steps. A step finishes its scan unless another limit interrupts it.
    pub max_steps: Option<u64>,
    /// Whether to consider missing optional values. Defaults to Preserve.
    pub optional: OptionalConstruction,
    /// Cooperative elapsed time, including initialization and diagnostics. Zero
    /// permits initial scoring only; active callbacks cannot be interrupted.
    pub max_time: Option<std::time::Duration>,
    pub pick_early: ConstructionPickEarly,
    /// Extra diagnostic full recomputations are excluded from reported work.
    pub assert_scores: bool,
}
impl Default for ConstructionConfig<'_> {
    fn default() -> Self {
        Self {
            placement: ConstructionPlacement::Sequential,
            cancellation: None,
            max_evaluations: 10_000,
            max_move_evaluations: None,
            max_steps: None,
            optional: OptionalConstruction::Preserve,
            max_time: None,
            pick_early: ConstructionPickEarly::Never,
            assert_scores: false,
        }
    }
}

impl ConstructionConfig<'_> {
    pub(crate) fn step_limit_reached(&self, completed: u64) -> bool {
        self.max_steps.is_some_and(|limit| completed >= limit)
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.cancellation
            .is_some_and(crate::CancellationToken::is_cancelled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionTermination {
    /// The solve-wide composed condition stopped this scan.
    Condition,
    /// Another eligible unassigned item remains after the step budget is spent.
    /// The preceding scan's winner is retained without truncating that scan.
    StepLimit,
    /// Elapsed time expired; retain the best scored prefix, possibly incomplete.
    TimeLimit,
    /// An unevaluated candidate remains after the candidate-only budget is spent.
    MoveEvaluationLimit,
    /// Cancellation observed; the best evaluated prefix of the current scan is
    /// installed. The returned assignment may be partial or complete.
    Cancelled,
    /// All eligible items were visited and their scans finished or picked early.
    /// Optional values may remain unassigned; empty scans consume no steps.
    /// This does not imply hard feasibility or optimality.
    Completed,
    /// An unevaluated candidate remains. The best evaluated value is retained;
    /// the resulting assignment may be either partial or complete.
    EvaluationLimit,
    NoCompleteAssignment,
    /// A bound construction provider supplied no accepted initialization.
    /// This does not prove that completing the assignment is impossible.
    NoCandidates,
    /// A bound provider reached its raw-proposal limit, including rejected moves.
    ScanLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionResult<V, S> {
    /// Includes partial progress when the budget runs out.
    pub assignment: Assignment<V>,
    /// None if no scoring work was performed.
    pub score: Option<S>,
    pub complete: bool,
    pub evaluations: u64,
    pub assigned_variables: usize,
    /// Scans with at least one evaluated candidate, including an interrupted scan
    /// and optional scans that reject every candidate. Resets on each call.
    pub completed_steps: u64,
    pub constraint_evaluations: u128,
    pub termination: ConstructionTermination,
}
impl<V, S> ConstructionResult<V, S> {
    /// Candidate evaluations, excluding the initial score and diagnostics.
    pub fn move_evaluations(&self) -> u64 {
        self.evaluations.saturating_sub(1)
    }
}
impl<V: Clone, S: Clone> ConstructionResult<V, S> {
    /// Return a scored solution only if all required variables are assigned.
    /// A complete solution can still violate hard constraints.
    pub fn solution(&self) -> Option<ScoredAssignment<V, S>> {
        if !self.complete {
            return None;
        }
        self.score.clone().map(|score| ScoredAssignment {
            assignment: self.assignment.clone(),
            score,
        })
    }
}

/// Visit missing variables in model order and evaluate domain values in domain
/// order. Preserve supplied values and pins. Optional None values are preserved
/// by default, or considered under [`OptionalConstruction::NonWorsening`].
/// Constraints must support partial assignments (None in scoped arguments).
/// Each variable's candidates use the same prior assignment; ties retain the
/// first candidate. Pooled placement instead compares all eligible missing items
/// each step, retaining the same model/domain tie order. ValueQueue cycles distinct
/// values in first domain encounter order and compares eligible receivers. A full
/// rotation without a selected move ends construction; each call restarts the queue.
/// Cached contributions
/// install without rescoring the winner.
/// This greedy initializer neither backtracks nor proves optimality.
pub fn greedy_construction<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    config: ConstructionConfig,
) -> Result<ConstructionResult<V, S>, ScoringError> {
    greedy_construction_from(problem, network, problem.initial_assignment(), config)
}

/// Resume construction from a validated assignment, preserving supplied values and pins.
/// Missing required and configured optional values are considered; each call
/// initializes its own scoring cache.
/// Validation runs even with zero budget or a cancelled token.
pub fn greedy_construction_from<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: ConstructionConfig,
) -> Result<ConstructionResult<V, S>, ScoringError> {
    greedy_construction_from_monitored(None, problem, network, initial, config)
}

pub(crate) fn greedy_construction_from_monitored<V: Eq + Clone, S: AdditiveScore>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: ConstructionConfig,
) -> Result<ConstructionResult<V, S>, ScoringError> {
    if network.variable_count() != problem.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        });
    }
    problem.validate(&initial)?;
    construction_impl_monitored(shared, problem, initial, config, |initial| {
        ScoreDirector::new(problem, network, initial)
    })
}

pub(crate) fn construction_impl_with_clock_monitored<
    V: Eq + Clone,
    S: AdditiveScore,
    D: SearchDirector<V, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    config: ConstructionConfig,
    create: impl FnOnce(Assignment<V>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<ConstructionResult<V, S>, D::Error> {
    let mut shared = shared;
    let mut time = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    let mut result = ConstructionResult {
        complete: problem.is_complete(&initial),
        assignment: initial.clone(),
        score: None,
        evaluations: 0,
        assigned_variables: 0,
        completed_steps: 0,
        constraint_evaluations: 0,
        termination: ConstructionTermination::EvaluationLimit,
    };
    if problem.has_empty_movable_required_domain() {
        result.termination = ConstructionTermination::NoCompleteAssignment;
        return Ok(result);
    }
    if config.max_evaluations == 0 {
        return Ok(result);
    }
    if config.cancelled() {
        result.termination = ConstructionTermination::Cancelled;
        return Ok(result);
    }
    let mut director = create(initial)?;
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_constraint_evaluations();
    if config.assert_scores {
        director.assert_consistent()?;
    }
    director.notify_best(|assignment| problem.is_complete(assignment));
    result.termination = ConstructionTermination::Completed;
    let mut indices_in_order = 0..problem.variables().len();
    let mut value_queue = (config.placement == ConstructionPlacement::ValueQueue)
        .then(|| crate::value_construction_queue::ValueConstructionQueue::new(problem));
    loop {
        // Sequential and pooled construction install at most one item per pass.
        let index = if value_queue.is_some() {
            0
        } else {
            let Some(index) = indices_in_order.next() else {
                break;
            };
            index
        };
        if config.cancelled() {
            result.termination = ConstructionTermination::Cancelled;
            break;
        }
        if let Some(reason) = crate::construction::construction_stop(&mut time, &mut shared) {
            result.termination = reason;
            break;
        }
        let eligible = |i: usize| {
            let variable = &problem.variables()[i];
            director.assignment().values()[i].is_none()
                && !variable.is_pinned()
                && (!variable.allows_unassigned()
                    || config.optional == OptionalConstruction::NonWorsening)
        };
        let mut queued_value = None;
        let indices: Vec<_> = if let Some(queue) = &mut value_queue {
            let Some((value, indices)) =
                queue.next(problem, director.assignment(), config.optional)
            else {
                break;
            };
            queued_value = Some(value);
            indices
        } else if config.placement == ConstructionPlacement::Pooled {
            (0..problem.variables().len())
                .filter(|&i| eligible(i) && !problem.variables()[i].domain().is_empty())
                .collect()
        } else if eligible(index) {
            vec![index]
        } else {
            continue;
        };
        if indices.is_empty() {
            break;
        }
        if config.step_limit_reached(result.completed_steps) {
            result.termination = ConstructionTermination::StepLimit;
            break;
        }
        let previous = director.score();
        let moves = indices.into_iter().flat_map(|index| {
            problem.variables()[index]
                .domain()
                .iter()
                .filter(move |value| queued_value.is_none_or(|queued| *value == queued))
                .map(move |value| Move::Change {
                    variable: index,
                    value: Some(value.clone()),
                })
        });
        let budget = crate::count_limit::CandidateBudget::new(
            config.max_evaluations,
            config.max_move_evaluations,
            result.evaluations,
        );
        let selection = director.select_best(
            moves.take_while(|_| {
                !config.cancelled()
                    && crate::construction::construction_stop(&mut time, &mut shared).is_none()
            }),
            budget.remaining,
            false,
            |movement, score| {
                let Move::Change { variable, .. } = movement else {
                    unreachable!("construction change")
                };
                !problem.variables()[*variable].allows_unassigned() || score >= previous
            },
            |score| config.pick_early.should_pick(previous.clone(), score),
            config.assert_scores,
        )?;
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        result.completed_steps += u64::from(selection.evaluations > 0);
        result.assigned_variables += usize::from(selection.selected.is_some());
        if config.assert_scores {
            director.assert_consistent()?;
        }
        if selection.selected.is_some() {
            if let Some(queue) = &mut value_queue {
                queue.installed();
            }
            director.notify_best(|assignment| problem.is_complete(assignment));
        }
        if selection.evaluation_limit {
            result.termination = budget.construction_reason();
            break;
        }
        if config.placement == ConstructionPlacement::Pooled && selection.selected.is_none() {
            break;
        }
    }
    if result.termination == ConstructionTermination::Completed && config.cancelled() {
        result.termination = ConstructionTermination::Cancelled;
    }
    if result.termination == ConstructionTermination::Completed
        && let Some(reason) = crate::construction::construction_stop(&mut time, &mut shared)
    {
        result.termination = reason;
    }
    result.complete = problem.is_complete(director.assignment());
    result.assignment = director.assignment().clone();
    result.score = Some(director.score());
    Ok(result)
}

pub(crate) fn construction_impl_monitored<
    V: Eq + Clone,
    S: AdditiveScore,
    D: SearchDirector<V, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    config: ConstructionConfig,
    create: impl FnOnce(Assignment<V>) -> Result<D, D::Error>,
) -> Result<ConstructionResult<V, S>, D::Error> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    construction_impl_with_clock_monitored(shared, problem, initial, config, create, clock)
}

/// Preserve independent phase time limits while polling only construction-applicable
/// shared controls. Never start/reset a search clock or increment search steps.
pub(crate) fn construction_stop<C: FnMut() -> std::time::Duration>(
    time: &mut crate::time_limit::TimeBudget<'_, C>,
    shared: &mut Option<&mut dyn crate::time_limit::SearchControl>,
) -> Option<ConstructionTermination> {
    if time.expired() {
        Some(ConstructionTermination::TimeLimit)
    } else if shared.as_mut().is_some_and(|s| s.construction_stopped()) {
        Some(ConstructionTermination::Condition)
    } else {
        None
    }
}
