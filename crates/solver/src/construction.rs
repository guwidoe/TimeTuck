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
    /// Score legal assignments followed by one no-change candidate. Every
    /// candidate is eligible; a worse insertion can win only through early pick
    /// or interruption with KeepBestPrefix. No-change consumes a move evaluation.
    /// Existing assignments and pinned unassignment remain untouched.
    EvaluateUnassigned,
}

/// Whether an interrupted candidate scan may change the working assignment.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![])])?.allowing_unassigned();
/// let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 1);
/// let r = greedy_list_construction(&p, &n, ConstructionConfig {
///     optional: OptionalConstruction::EvaluateUnassigned,
///     interruption: ConstructionInterruption::DiscardIncompleteStep,
///     max_move_evaluations: Some(1), ..Default::default()
/// })?;
/// // The insertion was scored, but the final no-change candidate was not.
/// assert_eq!(r.move_evaluations(), 1);
/// assert_eq!(r.completed_steps, 0);
/// assert!(r.assignment.lists()[0].is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConstructionInterruption {
    /// Retain the best evaluated candidate, including an unfinished scan.
    #[default]
    KeepBestPrefix,
    /// Install a winner only after exhaustion or a deliberate early pick.
    /// Discard candidates on score/move budget, cancellation, time or shared
    /// termination. Evaluations still count; discarded scans consume no steps.
    DiscardIncompleteStep,
}

/// Which unassigned items compete within a construction step.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConstructionPlacement {
    /// Finish one item in model order before considering the next.
    #[default]
    Sequential,
    /// Reconsider all eligible missing items each step, installing the first best
    /// placement. With Never early-picking this is cheapest insertion. Optional
    /// candidates use the configured optional policy; a pool with no installed
    /// assignment ends the phase.
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
/// backends. By default cancellation retains the best evaluated candidate of the
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
/// candidate limit (unless another control interrupts the scan). Set
/// interruption to DiscardIncompleteStep to retain only finished scans.
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
    /// KeepBestPrefix by default; optionally discard an interrupted scan.
    pub interruption: ConstructionInterruption,
    /// Cooperative cancellation shared with other phases; validation runs first.
    pub cancellation: Option<&'a crate::CancellationToken>,
    /// Includes the initial score and each candidate; zero invokes no callbacks.
    pub max_evaluations: u64,
    /// Candidate evaluations only; initialization is excluded. None disables it.
    pub max_move_evaluations: Option<u64>,
    /// Evaluated item, whole-pool or queued-value target scans per call.
    /// None disables this limit.
    /// Finished optional scans count even if every candidate is rejected.
    /// DiscardIncompleteStep excludes interrupted scans from this count.
    /// Initialization, existing assignments and scans with no candidates do not
    /// consume steps. A step finishes its scan unless another limit interrupts it.
    pub max_steps: Option<u64>,
    /// Preserve, accept only non-worsening insertions, or score no-change last.
    pub optional: OptionalConstruction,
    /// Cooperative elapsed time, including initialization and diagnostics. Zero
    /// permits initial scoring only; active callbacks cannot be interrupted.
    pub max_time: Option<std::time::Duration>,
    pub pick_early: ConstructionPickEarly,
    /// Extra diagnostic full recomputations are excluded from reported work.
    /// Verify scoring at initialization, on candidate states, after each
    /// candidate rollback (including early selection), and at step boundaries.
    /// Diagnostic calculations do not increase search evaluation counters.
    pub assert_scores: bool,
    /// Check initialization and completed scan/placement boundaries without
    /// candidate-state or rollback rescoring. Full assert_scores takes precedence.
    /// Diagnostic work is excluded from evaluation counters but consumes time.
    pub assert_step_scores: bool,
}
impl Default for ConstructionConfig<'_> {
    fn default() -> Self {
        Self {
            placement: ConstructionPlacement::Sequential,
            interruption: ConstructionInterruption::KeepBestPrefix,
            cancellation: None,
            max_evaluations: 10_000,
            max_move_evaluations: None,
            max_steps: None,
            optional: OptionalConstruction::Preserve,
            max_time: None,
            pick_early: ConstructionPickEarly::Never,
            assert_scores: false,
            assert_step_scores: false,
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
    /// Elapsed time expired; the interruption policy controls retained progress.
    TimeLimit,
    /// An unevaluated candidate remains after the candidate-only budget is spent.
    MoveEvaluationLimit,
    /// Cancellation observed; the interruption policy controls retained progress.
    Cancelled,
    /// All eligible items were visited and their scans finished or picked early.
    /// Optional values may remain unassigned; empty scans consume no steps.
    /// This does not imply hard feasibility or optimality.
    Completed,
    /// An unevaluated candidate remains after the score budget.
    /// The interruption policy controls retained progress.
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
    /// Nonempty candidate scans, including optional rejection. Interrupted scans
    /// count only under KeepBestPrefix. Resets on each call.
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
/// by default, or considered under the configured optional construction policy.
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
    if config.assert_scores || config.assert_step_scores {
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
                    || config.optional != OptionalConstruction::Preserve)
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
        let no_change = config.optional == OptionalConstruction::EvaluateUnassigned
            && indices
                .iter()
                .any(|&i| problem.variables()[i].allows_unassigned());
        let moves = indices
            .into_iter()
            .flat_map(|index| {
                problem.variables()[index]
                    .domain()
                    .iter()
                    .filter(move |value| queued_value.is_none_or(|queued| *value == queued))
                    .map(move |value| Move::Change {
                        variable: index,
                        value: Some(value.clone()),
                    })
            })
            .chain(no_change.then(|| Move::Composite(vec![])));
        let budget = crate::count_limit::CandidateBudget::new(
            config.max_evaluations,
            config.max_move_evaluations,
            result.evaluations,
        );
        let scan = crate::construction::ConstructionScan::new(config);
        if let Some(shared) = &mut shared {
            shared.step_started();
        }
        let selection = director.select_best(
            moves.take_while(|_| scan.proceed(&mut time, &mut shared)),
            budget.remaining,
            false,
            |movement, score| {
                if config.optional == OptionalConstruction::EvaluateUnassigned {
                    return true;
                }
                let Move::Change { variable, .. } = movement else {
                    unreachable!("construction change")
                };
                !problem.variables()[*variable].allows_unassigned() || score >= previous
            },
            scan.policy(previous.clone()),
            config.assert_scores,
        )?;
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        let completed = selection.evaluations > 0 && !scan.discarded(selection.evaluation_limit);
        result.completed_steps += u64::from(completed);
        if let Some(shared) = &mut shared {
            shared.step_ended(completed);
        }
        if completed && let Some(shared) = &mut shared {
            shared.complete_construction_step();
        }
        let installed = matches!(selection.selected, Some(Move::Change { .. }));
        result.assigned_variables += usize::from(installed);
        if config.assert_scores || config.assert_step_scores {
            director.assert_consistent()?;
        }
        if installed {
            if let Some(queue) = &mut value_queue {
                queue.installed();
            }
            director.notify_best(|assignment| problem.is_complete(assignment));
        }
        if selection.evaluation_limit {
            result.termination = budget.construction_reason();
            break;
        }
        if let Some(reason) = scan.reason() {
            result.termination = reason;
            break;
        }
        if config.placement == ConstructionPlacement::Pooled && !installed {
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

// Closures retain the existing local-search/director behavior. Construction adds
// a commit guard without changing the public forager or replaying score callbacks.
pub(crate) trait SelectionPolicy<S> {
    fn should_pick(&self, score: S) -> bool;
    fn can_commit(&self, _evaluation_limit: bool) -> bool {
        true
    }
}
impl<S, F: Fn(S) -> bool> SelectionPolicy<S> for F {
    fn should_pick(&self, score: S) -> bool {
        self(score)
    }
}

pub(crate) struct ConstructionScan<'a> {
    config: ConstructionConfig<'a>,
    stopped: std::cell::Cell<Option<ConstructionTermination>>,
}
impl<'a> ConstructionScan<'a> {
    pub(crate) fn new(config: ConstructionConfig<'a>) -> Self {
        Self {
            config,
            stopped: std::cell::Cell::new(None),
        }
    }
    pub(crate) fn proceed<C: FnMut() -> std::time::Duration>(
        &self,
        time: &mut crate::time_limit::TimeBudget<'_, C>,
        shared: &mut Option<&mut dyn crate::time_limit::SearchControl>,
    ) -> bool {
        let reason = self.stopped.get().or_else(|| {
            if self.config.cancelled() {
                Some(ConstructionTermination::Cancelled)
            } else {
                construction_stop(time, shared)
            }
        });
        self.stopped.set(reason);
        reason.is_none()
    }
    pub(crate) fn policy<'s, S: AdditiveScore + 's>(
        &'s self,
        previous: S,
    ) -> impl SelectionPolicy<S> + 's {
        ConstructionSelection {
            scan: self,
            previous,
        }
    }
    pub(crate) fn discarded(&self, evaluation_limit: bool) -> bool {
        self.config.interruption == ConstructionInterruption::DiscardIncompleteStep
            && (evaluation_limit || self.stopped.get().is_some())
    }
    pub(crate) fn reason(&self) -> Option<ConstructionTermination> {
        self.stopped.get()
    }
}
struct ConstructionSelection<'s, 'a, S> {
    scan: &'s ConstructionScan<'a>,
    previous: S,
}
impl<S: AdditiveScore> SelectionPolicy<S> for ConstructionSelection<'_, '_, S> {
    fn should_pick(&self, score: S) -> bool {
        self.scan
            .config
            .pick_early
            .should_pick(self.previous.clone(), score)
    }
    fn can_commit(&self, evaluation_limit: bool) -> bool {
        !self.scan.discarded(evaluation_limit)
    }
}
