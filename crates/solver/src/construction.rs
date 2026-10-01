//! Deterministic greedy construction for unassigned required basic variables.
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
                candidate.is_feasible() || !candidate.hard_cmp(previous).is_lt()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstructionConfig {
    /// Includes the initial score and each candidate; zero invokes no callbacks.
    pub max_evaluations: u64,
    pub pick_early: ConstructionPickEarly,
    /// Extra diagnostic full recomputations are excluded from reported work.
    pub assert_scores: bool,
}
impl Default for ConstructionConfig {
    fn default() -> Self {
        Self {
            max_evaluations: 10_000,
            pick_early: ConstructionPickEarly::Never,
            assert_scores: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionTermination {
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
impl<V: Clone, S: Copy> ConstructionResult<V, S> {
    /// Return a scored solution only if all required variables are assigned.
    /// A complete solution can still violate hard constraints.
    pub fn solution(&self) -> Option<ScoredAssignment<V, S>> {
        if !self.complete {
            return None;
        }
        self.score.map(|score| ScoredAssignment {
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
    if network.variable_count() != problem.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        });
    }
    let initial = problem.initial_assignment();
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
    let mut director = ScoreDirector::new(problem, network, initial)?;
    result.evaluations = 1;
    result.constraint_evaluations = network.constraint_count() as u128;
    if config.assert_scores {
        director.assert_consistent()?;
    }
    result.termination = ConstructionTermination::Completed;
    for (index, variable) in problem.variables().iter().enumerate() {
        if director.assignment().values()[index].is_some() || variable.allows_unassigned() {
            continue;
        }
        let previous = director.score();
        let moves = variable.domain().iter().map(|value| Move::Change {
            variable: index,
            value: Some(value.clone()),
        });
        let selection = director.select_best(
            moves,
            config.max_evaluations - result.evaluations,
            |_, _| true,
            |score| config.pick_early.should_pick(previous, score),
            config.assert_scores,
        )?;
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        result.assigned_variables += usize::from(selection.evaluations > 0);
        if config.assert_scores {
            director.assert_consistent()?;
        }
        if selection.evaluation_limit {
            result.termination = ConstructionTermination::EvaluationLimit;
            break;
        }
    }
    result.complete = problem.is_complete(director.assignment());
    result.assignment = director.assignment().clone();
    result.score = Some(director.score());
    Ok(result)
}
