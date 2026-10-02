//! Bounded greedy insertion of required unassigned list values.
use crate::list_neighborhood::insertions;
use crate::{
    AdditiveScore, ConstructionConfig, ConstructionTermination, ListAssignment,
    ListConstraintNetwork, ListMove, ListProblem, ListScoreDirector, ListScoringError,
    ScoredListAssignment,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListConstructionResult<V, S> {
    pub assignment: ListAssignment<V>,
    pub score: Option<S>,
    pub complete: bool,
    pub evaluations: u64,
    pub assigned_values: usize,
    pub constraint_evaluations: u128,
    pub termination: ConstructionTermination,
}
impl<V: Clone, S: Copy> ListConstructionResult<V, S> {
    pub fn solution(&self) -> Option<ScoredListAssignment<V, S>> {
        if !self.complete {
            return None;
        }
        self.score.map(|score| ScoredListAssignment {
            assignment: self.assignment.clone(),
            score,
        })
    }
}
/// Preserve initial lists and pins. Insert missing required values in range order,
/// scanning entities and insertion indices in order. Ties retain the first best
/// candidate; the shared early-pick policy compares ordinary levels. Optional
/// missing values remain unassigned for subsequent local search to consider.
/// Initial scoring and every trial count toward the budget; installing a winner
/// reuses its saved contribution cache. Partial progress is returned on limits.
pub fn greedy_list_construction<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    config: ConstructionConfig,
) -> Result<ListConstructionResult<V, S>, ListScoringError> {
    let initial = problem.initial_assignment();
    network.validate(problem, &initial)?;
    let mut result = ListConstructionResult {
        complete: problem.is_complete(&initial),
        assignment: initial.clone(),
        score: None,
        evaluations: 0,
        assigned_values: 0,
        constraint_evaluations: 0,
        termination: ConstructionTermination::EvaluationLimit,
    };
    if !problem.can_complete(&initial) {
        result.termination = ConstructionTermination::NoCompleteAssignment;
        return Ok(result);
    }
    if config.max_evaluations == 0 {
        return Ok(result);
    }
    let mut director = ListScoreDirector::new(problem, network, initial)?;
    result.evaluations = 1;
    result.constraint_evaluations = network.constraint_count() as u128;
    if config.assert_scores {
        director.assert_consistent()?;
    }
    result.termination = ConstructionTermination::Completed;
    if !problem.allows_unassigned() {
        for value in problem.values() {
            if director.assignment().location(value).is_some() {
                continue;
            }
            let snapshot = director.assignment().clone();
            let previous = director.score();
            let moves = insertions(problem, &snapshot, 0..problem.entities().len())
                .filter(|position| problem.accepts(position.entity, value))
                .map(|destination| ListMove::Assign {
                    value: value.clone(),
                    destination,
                });
            let selection = director.select_best(
                moves,
                config.max_evaluations - result.evaluations,
                false,
                |_, _| true,
                |score| config.pick_early.should_pick(previous, score),
                config.assert_scores,
            )?;
            result.evaluations += selection.evaluations;
            result.constraint_evaluations += selection.constraint_evaluations;
            result.assigned_values += usize::from(selection.selected.is_some());
            if config.assert_scores {
                director.assert_consistent()?;
            }
            if selection.evaluation_limit {
                result.termination = ConstructionTermination::EvaluationLimit;
                break;
            }
        }
    }
    result.complete = problem.is_complete(director.assignment());
    result.assignment = director.assignment().clone();
    result.score = Some(director.score());
    Ok(result)
}
