//! Bounded deterministic local search using incremental scores.
use crate::search_scoring::SearchDirector;
use crate::{
    AdditiveScore, ConstraintNetwork, FiniteProblem, ScoreDirector, ScoredAssignment, ScoringError,
};

pub use crate::acceptance::Acceptance;
use crate::neighborhood::{change_moves_ordered, swap_moves_ordered};
use crate::random::SplitMix64;
use crate::{AcceptanceError, Assignment, ModelError};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveOrder {
    Original,
    ShuffledVariables {
        seed: u64,
    },
    /// Shuffle candidates within a rolling window, using a reproducible seed.
    /// Each step caps lookahead by remaining evaluations (minimum one). A window
    /// covering the whole neighborhood gives a uniform permutation; smaller
    /// windows are locally shuffled, not globally uniform. One preserves order.
    /// Generation/prefetch is unscored and cooperative with phase time limits.
    ShuffledMoves {
        seed: u64,
        window: std::num::NonZeroUsize,
    },
}

#[derive(Debug)]
pub enum LocalSearchError {
    Scoring(ScoringError),
    StreamScoring(crate::StreamScoringError),
    Acceptance(AcceptanceError),
    IncompleteInitialAssignment,
}
impl fmt::Display for LocalSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoring(e) => e.fmt(f),
            Self::StreamScoring(e) => e.fmt(f),
            Self::Acceptance(e) => e.fmt(f),
            Self::IncompleteInitialAssignment => {
                f.write_str("local search requires a complete initial assignment")
            }
        }
    }
}
impl Error for LocalSearchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Scoring(e) => e,
            Self::StreamScoring(e) => e,
            Self::Acceptance(e) => e,
            Self::IncompleteInitialAssignment => return None,
        })
    }
}
impl From<crate::StreamScoringError> for LocalSearchError {
    fn from(e: crate::StreamScoringError) -> Self {
        Self::StreamScoring(e)
    }
}
impl From<ScoringError> for LocalSearchError {
    fn from(e: ScoringError) -> Self {
        Self::Scoring(e)
    }
}
impl From<ModelError> for LocalSearchError {
    fn from(e: ModelError) -> Self {
        Self::Scoring(e.into())
    }
}
impl From<AcceptanceError> for LocalSearchError {
    fn from(e: AcceptanceError) -> Self {
        Self::Acceptance(e)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Neighborhood {
    Change,
    ChangeAndSwap,
    /// For list variables, use only in-place reversals and optional cross-owner
    /// nonempty tail swaps. No list assignment/unassignment/single relocations.
    /// Basic variables retain changes/swaps; mixed joint moves use this list set.
    TwoOpt {
        cross_entity: bool,
    },
    /// Add list-span relocations/reversals and disjoint span swaps after single swaps.
    /// Each span is bounded by max_length; span swaps include singleton partners.
    /// Values below two add no spans. Basic-variable search retains changes/swaps.
    ChangeSwapAndSubList {
        max_length: usize,
    },
}
impl Neighborhood {
    pub(crate) fn includes_swaps(self) -> bool {
        !matches!(self, Self::Change)
    }
}

/// Choose the first accepted candidate, or the first highest-scoring accepted
/// candidate over the evaluated neighborhood prefix. BestAccepted can select a
/// worsening step with tabu/late acceptance, while retaining the global best.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveSelection {
    FirstAccepted,
    BestAccepted,
}

/// Behavior when a scanned neighborhood prefix has no accepted candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectedMovePolicy {
    Stop,
    /// Pick the highest-scoring rejected candidate, with first-tie ordering.
    /// Completed-step limits still bound progress when acceptance rejects all moves.
    BestScore,
}

/// Evaluation budget includes the initial score and every candidate score.
/// The accepted-move limit is separate. Optional diagnostic recomputations are
/// excluded from both budgets and from the constraint-evaluation statistic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSearchConfig<'a, S = crate::SimpleScore> {
    /// Optional external cancellation. Borrowing keeps this configuration Copy.
    /// Model/configuration validation precedes cancellation checks.
    pub cancellation: Option<&'a crate::CancellationToken>,
    pub max_evaluations: u64,
    pub max_accepted_moves: u64,
    pub max_steps: u64,
    /// Cooperative phase time limit, including initial scoring. Checked between
    /// candidates; an active scoring callback cannot be interrupted. None disables.
    pub max_time: Option<std::time::Duration>,
    /// Cooperative time since initialization or the last strict best improvement.
    /// Initial scoring is excluded. None disables; zero stops before any move.
    pub max_unimproved_time: Option<std::time::Duration>,
    /// Completed steps since the last strict global-best improvement. None disables.
    pub max_unimproved_steps: Option<u64>,
    /// Stop at a step boundary when the retained best score is at least this value.
    pub best_score_limit: Option<S>,
    /// Stop at a step boundary when the retained best satisfies Score::is_feasible.
    pub stop_when_feasible: bool,
    pub neighborhood: Neighborhood,
    pub acceptance: Acceptance<S>,
    pub move_order: MoveOrder,
    pub selection: MoveSelection,
    pub rejected_moves: RejectedMovePolicy,
    pub assert_scores: bool,
}
impl<S> Default for LocalSearchConfig<'_, S> {
    fn default() -> Self {
        Self {
            cancellation: None,
            max_evaluations: 10_000,
            max_accepted_moves: 1_000,
            max_steps: 1_000,
            max_time: None,
            max_unimproved_time: None,
            max_unimproved_steps: None,
            best_score_limit: None,
            stop_when_feasible: false,
            neighborhood: Neighborhood::ChangeAndSwap,
            acceptance: Acceptance::StrictImprovement,
            move_order: MoveOrder::Original,
            selection: MoveSelection::FirstAccepted,
            rejected_moves: RejectedMovePolicy::Stop,
            assert_scores: false,
        }
    }
}
impl<S: crate::Score> LocalSearchConfig<'_, S> {
    pub(crate) fn cancelled(&self) -> bool {
        self.cancellation
            .is_some_and(crate::CancellationToken::is_cancelled)
    }
    // Preserve existing count-limit precedence; new optional criteria are checked
    // only at step boundaries, never midway through candidate evaluation.
    pub(crate) fn step_termination(
        &self,
        best: S,
        accepted: u64,
        progress: crate::StepProgress,
    ) -> Option<LocalSearchTermination> {
        use LocalSearchTermination::*;
        if accepted >= self.max_accepted_moves {
            Some(AcceptedMoveLimit)
        } else if crate::StepCountLimit::new(self.max_steps).is_reached(progress.completed_steps())
        {
            Some(StepLimit)
        } else if self.best_score_limit.is_some_and(|target| best >= target) {
            Some(BestScoreLimit)
        } else if self.stop_when_feasible && best.is_feasible() {
            Some(FeasibleScore)
        } else if self.max_unimproved_steps.is_some_and(|limit| {
            crate::StepCountLimit::new(limit).is_reached(progress.unimproved_steps())
        }) {
            Some(UnimprovedStepLimit)
        } else {
            None
        }
    }
}
/// Reason a local-search phase ended. None of these reasons proves optimality.
///
/// At step boundaries, limits are checked in this order: accepted moves, completed
/// steps, best score, feasibility, unimproved steps. Zero evaluation budget ends
/// before initial scoring. If candidate selection exhausts its budget, it installs
/// any selected move and reports `EvaluationLimit` before checking a new boundary.
/// Exhaustion is detected when attempting another candidate: consuming the last
/// evaluation on the final candidate alone does not mark that selection exhausted.
/// An enabled time limit is polled after boundary criteria, before each emitted
/// candidate, and after selection/diagnostics. A timed-out iterator preserves the
/// evaluated prefix and returns `TimeLimit` or `UnimprovedTimeLimit` before another boundary or no-move
/// decision. An evaluation limit already observed by selection takes precedence;
/// a time check withholding the next candidate can prevent that observation.
/// Unimproved timing starts after initialization and is also polled before a strict
/// best improvement resets it. Expiry latches; total time wins simultaneous expiry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalSearchTermination {
    /// The external cancellation token was observed. Does not prove optimality.
    Cancelled,
    EvaluationLimit,
    TimeLimit,
    UnimprovedTimeLimit,
    AcceptedMoveLimit,
    StepLimit,
    UnimprovedStepLimit,
    BestScoreLimit,
    FeasibleScore,
    /// No move in the configured neighborhood satisfied the acceptance rule.
    /// This is not proof of global optimality or hard feasibility.
    NoAcceptedMove,
    /// At least one required variable has an empty domain.
    NoCompleteAssignment,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSearchResult<V, S> {
    pub best: Option<ScoredAssignment<V, S>>,
    pub evaluations: u64,
    pub accepted_moves: u64,
    /// Includes selected rejected fallback moves.
    pub completed_steps: u64,
    /// Completed steps since the last strict improvement of the retained best.
    pub unimproved_steps: u64,
    pub constraint_evaluations: u128,
    pub termination: LocalSearchTermination,
}

/// Start from the supplied initial values, filling unassigned required variables
/// with their first domain value. Optional `None` values and pins are preserved.
/// Then scan changes followed by swaps in deterministic order. Choose the first
/// accepted move by default, or the best accepted candidate when configured. Equal best scores retain the earliest best assignment.
///
/// This initial construction is first-fit, not a score-aware construction
/// heuristic. Original order is deterministic; shuffled-variable order uses an
/// explicit reproducible seed and preserves value order within each variable.
/// ShuffledMoves instead reorders candidates within a bounded rolling window.
/// Returned best solutions are complete, but
/// may be hard-infeasible. No termination here proves global optimality.
pub fn local_search<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    local_search_impl(problem, network, None, config)
}

/// Improve an explicit complete assignment, such as `ConstructionResult::solution`.
/// Validates domains, pins and completeness even with a zero evaluation budget.
/// The supplied assignment is scored once and that work counts toward this
/// phase's budget. Add construction and search statistics for combined work;
/// no score or cache from a different session is trusted.
pub fn local_search_from<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    local_search_impl(problem, network, Some(initial), config)
}

fn local_search_impl<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Option<Assignment<V>>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.acceptance.validate()?;
    if network.variable_count() != problem.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        }
        .into());
    }
    search_impl(problem, initial, config, |initial| {
        ScoreDirector::new(problem, network, initial)
    })
}

pub(crate) fn search_impl<V: Eq + Clone, S: AdditiveScore, D: SearchDirector<V, S>>(
    problem: &FiniteProblem<V>,
    initial: Option<Assignment<V>>,
    config: LocalSearchConfig<S>,
    create: impl FnOnce(Assignment<V>) -> Result<D, D::Error>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError>
where
    LocalSearchError: From<D::Error>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.max_time.is_some() || config.max_unimproved_time.is_some(),
    );
    search_impl_with_clock(problem, initial, config, create, clock)
}

pub(crate) fn search_impl_with_clock<V: Eq + Clone, S: AdditiveScore, D: SearchDirector<V, S>>(
    problem: &FiniteProblem<V>,
    initial: Option<Assignment<V>>,
    config: LocalSearchConfig<S>,
    create: impl FnOnce(Assignment<V>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<LocalSearchResult<V, S>, LocalSearchError>
where
    LocalSearchError: From<D::Error>,
{
    let mut time_budget =
        crate::time_limit::TimeBudget::new(config.max_time, config.max_unimproved_time, clock);
    config.acceptance.validate()?;
    if let Some(initial) = &initial {
        problem.validate(initial)?;
        if !problem.is_complete(initial) {
            return Err(LocalSearchError::IncompleteInitialAssignment);
        }
    }
    let mut result = LocalSearchResult {
        best: None,
        evaluations: 0,
        accepted_moves: 0,
        completed_steps: 0,
        unimproved_steps: 0,
        constraint_evaluations: 0,
        termination: LocalSearchTermination::EvaluationLimit,
    };
    if problem.search_space_size() == Some(0) {
        result.termination = LocalSearchTermination::NoCompleteAssignment;
        return Ok(result);
    }
    if config.max_evaluations == 0 {
        return Ok(result);
    }
    if config.cancelled() {
        result.termination = LocalSearchTermination::Cancelled;
        return Ok(result);
    }
    let initial = initial.unwrap_or_else(|| {
        let mut assignment = problem.initial_assignment();
        for (i, variable) in problem.variables().iter().enumerate() {
            if assignment.values[i].is_none() && !variable.allows_unassigned() {
                assignment.values[i] = variable.domain().first().cloned();
            }
        }
        assignment
    });
    let mut director = create(initial)?;
    let mut acceptor = config.acceptance.start(director.score())?;
    let mut random = match config.move_order {
        MoveOrder::Original => None,
        MoveOrder::ShuffledVariables { seed } | MoveOrder::ShuffledMoves { seed, .. } => {
            Some(SplitMix64::new(seed))
        }
    };
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_constraint_evaluations();
    result.best = Some(ScoredAssignment {
        assignment: director.assignment().clone(),
        score: director.score(),
    });
    if config.assert_scores {
        director.assert_consistent()?;
    }
    time_budget.start_search();
    let mut progress = crate::StepProgress::default();
    loop {
        if config.cancelled() {
            result.termination = LocalSearchTermination::Cancelled;
            return Ok(result);
        }
        if let Some(reason) = config.step_termination(
            result.best.as_ref().expect("initial best").score,
            result.accepted_moves,
            progress,
        ) {
            result.termination = reason;
            return Ok(result);
        }
        if let Some(reason) = time_budget.stop() {
            result.termination = reason;
            return Ok(result);
        }
        // Snapshot decouples lazy enumeration from the director's transaction.
        // Original order stays lazy; optional candidate shuffling has bounded lookahead.
        let snapshot = director.assignment().clone();
        let mut order: Vec<_> = (0..problem.variables().len()).collect();
        if matches!(config.move_order, MoveOrder::ShuffledVariables { .. })
            && let Some(random) = &mut random
        {
            random.shuffle(&mut order);
        }
        let changes = change_moves_ordered(problem, &snapshot, order.iter().copied());
        let swaps = config
            .neighborhood
            .includes_swaps()
            .then(|| swap_moves_ordered(problem, &snapshot, order.iter().copied()))
            .into_iter()
            .flatten();
        let current = director.score();
        let tracks_variables = matches!(config.acceptance, Acceptance::VariableTabu { .. });
        let selection = director.select_best(
            crate::random::order_moves(
                changes.chain(swaps),
                config.move_order,
                random.as_mut(),
                config.max_evaluations - result.evaluations,
            )
            .take_while(|_| !config.cancelled() && !time_budget.expired()),
            config.max_evaluations - result.evaluations,
            config.rejected_moves == RejectedMovePolicy::BestScore,
            |movement, score| {
                let variables = if tracks_variables {
                    movement.touched_variables()
                } else {
                    Vec::new()
                };
                acceptor.accepts(current, score, &variables)
            },
            |_| config.selection == MoveSelection::FirstAccepted,
            config.assert_scores,
        )?;
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        if let Some(movement) = &selection.selected {
            let improved =
                director.score() > result.best.as_ref().expect("initial best exists").score;
            if improved {
                time_budget.record_improvement();
                result.best = Some(ScoredAssignment {
                    assignment: director.assignment().clone(),
                    score: director.score(),
                });
            }
            let variables = if tracks_variables {
                movement.touched_variables()
            } else {
                Vec::new()
            };
            acceptor.step_ended(director.score(), &variables);
            result.accepted_moves += u64::from(selection.selected_accepted);
            progress.complete_step(improved);
            result.completed_steps = progress.completed_steps();
            result.unimproved_steps = progress.unimproved_steps();
        }
        if config.assert_scores {
            director.assert_consistent()?;
        }
        if selection.evaluation_limit {
            result.termination = LocalSearchTermination::EvaluationLimit;
            return Ok(result);
        }
        if config.cancelled() {
            result.termination = LocalSearchTermination::Cancelled;
            return Ok(result);
        }
        if let Some(reason) = time_budget.stop() {
            result.termination = reason;
            return Ok(result);
        }
        if selection.selected.is_none() {
            result.termination = LocalSearchTermination::NoAcceptedMove;
            return Ok(result);
        }
    }
}
