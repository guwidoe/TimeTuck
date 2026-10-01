//! Bounded deterministic first-accepted local search using incremental scores.
use crate::{
    AdditiveScore, ConstraintNetwork, FiniteProblem, Score, ScoreDirector, ScoredAssignment,
    ScoringError, change_moves, swap_moves,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acceptance {
    /// Accept only strictly better scores. Finite spaces cannot cycle.
    StrictImprovement,
    /// Accept equal or better scores, including plateaus. Limits bound cycles.
    HillClimbing,
}
impl Acceptance {
    pub fn accepts<S: Score>(self, current: S, candidate: S) -> bool {
        match self {
            Self::StrictImprovement => candidate > current,
            Self::HillClimbing => candidate >= current,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Neighborhood {
    Change,
    ChangeAndSwap,
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
    pub assert_scores: bool,
}
impl Default for LocalSearchConfig {
    fn default() -> Self {
        Self {
            max_evaluations: 10_000,
            max_accepted_moves: 1_000,
            neighborhood: Neighborhood::ChangeAndSwap,
            acceptance: Acceptance::StrictImprovement,
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
/// Then scan changes followed by swaps in deterministic order, restarting after
/// the first accepted move. Equal best scores retain the earliest best assignment.
///
/// This initial construction is first-fit, not a score-aware construction
/// heuristic. No random seed is used. Returned best solutions are complete, but
/// may be hard-infeasible. No termination here proves global optimality.
pub fn local_search<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    config: LocalSearchConfig,
) -> Result<LocalSearchResult<V, S>, ScoringError> {
    if network.variable_count() != problem.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        });
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
    let mut initial = problem.initial_assignment();
    for (i, variable) in problem.variables().iter().enumerate() {
        if initial.values[i].is_none() && !variable.allows_unassigned() {
            initial.values[i] = variable.domain().first().cloned();
        }
    }
    let mut director = ScoreDirector::new(problem, network, initial)?;
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
        let changes = change_moves(problem, &snapshot)?;
        let swaps = (config.neighborhood == Neighborhood::ChangeAndSwap)
            .then(|| swap_moves(problem, &snapshot))
            .transpose()?
            .into_iter()
            .flatten();
        let mut accepted = false;
        for movement in changes.chain(swaps) {
            if result.evaluations == config.max_evaluations {
                result.termination = LocalSearchTermination::EvaluationLimit;
                return Ok(result);
            }
            let current = director.score();
            {
                let candidate = director.apply(&movement)?;
                result.evaluations += 1;
                result.constraint_evaluations += candidate.constraint_evaluations() as u128;
                if config.assert_scores
                    && network.calculate(problem, candidate.assignment())? != candidate.score()
                {
                    return Err(ScoringError::CorruptedTotal);
                }
                if config.acceptance.accepts(current, candidate.score()) {
                    if candidate.score() > result.best.as_ref().expect("initial best exists").score
                    {
                        result.best = Some(ScoredAssignment {
                            assignment: candidate.assignment().clone(),
                            score: candidate.score(),
                        });
                    }
                    candidate.commit();
                    result.accepted_moves += 1;
                    accepted = true;
                }
            }
            if config.assert_scores {
                director.assert_consistent()?;
            }
            if accepted {
                break;
            }
        }
        if !accepted {
            result.termination = LocalSearchTermination::NoAcceptedMove;
            return Ok(result);
        }
    }
}
