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

/// Shared controls for basic, list and mixed construction in both scoring
/// backends. A cancellation request retains the best evaluated candidate of the
/// current scan, including incomplete progress. Initialization and diagnostics
/// are not interrupted. Validation, impossibility and zero score-budget checks
/// precede cancellation; already-observed evaluation exhaustion also takes
/// precedence. A zero move budget permits initialization, then no candidates.
/// The default token is None. Configurations borrow tokens and remain Copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstructionConfig<'a> {
    /// Cooperative cancellation shared with other phases; validation runs first.
    pub cancellation: Option<&'a crate::CancellationToken>,
    /// Includes the initial score and each candidate; zero invokes no callbacks.
    pub max_evaluations: u64,
    /// Candidate evaluations only; initialization is excluded. None disables it.
    pub max_move_evaluations: Option<u64>,
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
            cancellation: None,
            max_evaluations: 10_000,
            max_move_evaluations: None,
            max_time: None,
            pick_early: ConstructionPickEarly::Never,
            assert_scores: false,
        }
    }
}

impl ConstructionConfig<'_> {
    pub(crate) fn cancelled(&self) -> bool {
        self.cancellation
            .is_some_and(crate::CancellationToken::is_cancelled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionTermination {
    /// Elapsed time expired; retain the best scored prefix, possibly incomplete.
    TimeLimit,
    /// An unevaluated candidate remains after the candidate-only budget is spent.
    MoveEvaluationLimit,
    /// Cancellation observed; the best evaluated prefix of the current scan is
    /// installed. The returned assignment may be partial or complete.
    Cancelled,
    /// Every required variable is assigned and its scan finished or picked early.
    /// This does not imply hard feasibility or optimality.
    Completed,
    /// An unevaluated candidate remains. The best evaluated value is retained;
    /// the resulting assignment may be either partial or complete.
    EvaluationLimit,
    NoCompleteAssignment,
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

/// Visit unassigned required variables in model order and evaluate domain values
/// in domain order. Preserve supplied values, pins and optional None values.
/// Constraints must support partial assignments (None in scoped arguments).
/// Each variable's candidates use the same prior assignment; ties retain the
/// first candidate. Cached contributions install without rescoring the winner.
/// This greedy initializer neither backtracks nor proves optimality.
pub fn greedy_construction<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    config: ConstructionConfig,
) -> Result<ConstructionResult<V, S>, ScoringError> {
    greedy_construction_from(problem, network, problem.initial_assignment(), config)
}

/// Resume construction from a validated assignment, preserving supplied values and pins.
/// Only missing required values are filled; each call initializes its own scoring cache.
/// Validation runs even with zero budget or a cancelled token.
pub fn greedy_construction_from<V: Eq + Clone, S: AdditiveScore>(
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
    construction_impl(problem, initial, config, |initial| {
        ScoreDirector::new(problem, network, initial)
    })
}

pub(crate) fn construction_impl<V: Eq + Clone, S: AdditiveScore, D: SearchDirector<V, S>>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    config: ConstructionConfig,
    create: impl FnOnce(Assignment<V>) -> Result<D, D::Error>,
) -> Result<ConstructionResult<V, S>, D::Error> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    construction_impl_with_clock(problem, initial, config, create, clock)
}
pub(crate) fn construction_impl_with_clock<
    V: Eq + Clone,
    S: AdditiveScore,
    D: SearchDirector<V, S>,
>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    config: ConstructionConfig,
    create: impl FnOnce(Assignment<V>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<ConstructionResult<V, S>, D::Error> {
    let mut time = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    let mut result = ConstructionResult {
        complete: problem.is_complete(&initial),
        assignment: initial.clone(),
        score: None,
        evaluations: 0,
        assigned_variables: 0,
        constraint_evaluations: 0,
        termination: ConstructionTermination::EvaluationLimit,
    };
    if problem.search_space_size() == Some(0) {
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
    for (index, variable) in problem.variables().iter().enumerate() {
        if config.cancelled() {
            result.termination = ConstructionTermination::Cancelled;
            break;
        }
        if time.expired() {
            result.termination = ConstructionTermination::TimeLimit;
            break;
        }
        if director.assignment().values()[index].is_some() || variable.allows_unassigned() {
            continue;
        }
        let previous = director.score();
        let moves = variable.domain().iter().map(|value| Move::Change {
            variable: index,
            value: Some(value.clone()),
        });
        let budget = crate::count_limit::CandidateBudget::new(
            config.max_evaluations,
            config.max_move_evaluations,
            result.evaluations,
        );
        let selection = director.select_best(
            moves.take_while(|_| !config.cancelled() && !time.expired()),
            budget.remaining,
            false,
            |_, _| true,
            |score| config.pick_early.should_pick(previous.clone(), score),
            config.assert_scores,
        )?;
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        result.assigned_variables += usize::from(selection.evaluations > 0);
        if config.assert_scores {
            director.assert_consistent()?;
        }
        if selection.selected.is_some() {
            director.notify_best(|assignment| problem.is_complete(assignment));
        }
        if selection.evaluation_limit {
            result.termination = budget.construction_reason();
            break;
        }
    }
    if result.termination == ConstructionTermination::Completed && config.cancelled() {
        result.termination = ConstructionTermination::Cancelled;
    }
    if result.termination == ConstructionTermination::Completed && time.expired() {
        result.termination = ConstructionTermination::TimeLimit;
    }
    result.complete = problem.is_complete(director.assignment());
    result.assignment = director.assignment().clone();
    result.score = Some(director.score());
    Ok(result)
}
