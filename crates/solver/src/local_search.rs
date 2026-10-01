//! Bounded deterministic local search using incremental scores.
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
    ShuffledVariables { seed: u64 },
}

#[derive(Debug)]
pub enum LocalSearchError {
    Scoring(ScoringError),
    Acceptance(AcceptanceError),
    IncompleteInitialAssignment,
}
impl fmt::Display for LocalSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoring(e) => e.fmt(f),
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
            Self::Acceptance(e) => e,
            Self::IncompleteInitialAssignment => return None,
        })
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
}

/// Choose the first accepted candidate, or the first highest-scoring accepted
/// candidate over the evaluated neighborhood prefix. BestAccepted can select a
/// worsening step with tabu/late acceptance, while retaining the global best.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveSelection {
    FirstAccepted,
    BestAccepted,
}

/// Evaluation budget includes the initial score and every candidate score.
/// The accepted-move limit is separate. Optional diagnostic recomputations are
/// excluded from both budgets and from the constraint-evaluation statistic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSearchConfig {
    pub max_evaluations: u64,
    pub max_accepted_moves: u64,
    pub neighborhood: Neighborhood,
    pub acceptance: Acceptance,
    pub move_order: MoveOrder,
    pub selection: MoveSelection,
    pub assert_scores: bool,
}
impl Default for LocalSearchConfig {
    fn default() -> Self {
        Self {
            max_evaluations: 10_000,
            max_accepted_moves: 1_000,
            neighborhood: Neighborhood::ChangeAndSwap,
            acceptance: Acceptance::StrictImprovement,
            move_order: MoveOrder::Original,
            selection: MoveSelection::FirstAccepted,
            assert_scores: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalSearchTermination {
    EvaluationLimit,
    AcceptedMoveLimit,
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
/// explicit reproducible seed and preserves value order within each variable. Returned best solutions are complete, but
/// may be hard-infeasible. No termination here proves global optimality.
pub fn local_search<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    config: LocalSearchConfig,
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
    config: LocalSearchConfig,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    local_search_impl(problem, network, Some(initial), config)
}

fn local_search_impl<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Option<Assignment<V>>,
    config: LocalSearchConfig,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.acceptance.validate()?;
    if network.variable_count() != problem.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        }
        .into());
    }
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
    let initial = initial.unwrap_or_else(|| {
        let mut assignment = problem.initial_assignment();
        for (i, variable) in problem.variables().iter().enumerate() {
            if assignment.values[i].is_none() && !variable.allows_unassigned() {
                assignment.values[i] = variable.domain().first().cloned();
            }
        }
        assignment
    });
    let mut director = ScoreDirector::new(problem, network, initial)?;
    let mut acceptor = config.acceptance.start(director.score())?;
    let mut random = match config.move_order {
        MoveOrder::Original => None,
        MoveOrder::ShuffledVariables { seed } => Some(SplitMix64::new(seed)),
    };
    result.evaluations = 1;
    result.constraint_evaluations = network.constraint_count() as u128;
    result.best = Some(ScoredAssignment {
        assignment: director.assignment().clone(),
        score: director.score(),
    });
    if config.assert_scores {
        director.assert_consistent()?;
    }
    loop {
        if result.accepted_moves == config.max_accepted_moves {
            result.termination = LocalSearchTermination::AcceptedMoveLimit;
            return Ok(result);
        }
        // Snapshot decouples lazy enumeration from the director's transaction.
        // No neighborhood-sized list of cloned moves is allocated.
        let snapshot = director.assignment().clone();
        let mut order: Vec<_> = (0..problem.variables().len()).collect();
        if let Some(random) = &mut random {
            random.shuffle(&mut order);
        }
        let changes = change_moves_ordered(problem, &snapshot, order.iter().copied());
        let swaps = (config.neighborhood == Neighborhood::ChangeAndSwap)
            .then(|| swap_moves_ordered(problem, &snapshot, order.iter().copied()))
            .into_iter()
            .flatten();
        let current = director.score();
        let tracks_variables = matches!(config.acceptance, Acceptance::VariableTabu { .. });
        let selection = director.select_best(
            changes.chain(swaps),
            config.max_evaluations - result.evaluations,
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
            if director.score() > result.best.as_ref().expect("initial best exists").score {
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
            result.accepted_moves += 1;
        }
        if config.assert_scores {
            director.assert_consistent()?;
        }
        if selection.evaluation_limit {
            result.termination = LocalSearchTermination::EvaluationLimit;
            return Ok(result);
        }
        if selection.selected.is_none() {
            result.termination = LocalSearchTermination::NoAcceptedMove;
            return Ok(result);
        }
    }
}
