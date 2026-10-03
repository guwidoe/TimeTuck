//! Bounded greedy insertion of required unassigned list values.
use crate::list_neighborhood::insertions;
use crate::list_search_scoring::ListSearchDirector;
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
impl<V, S> ListConstructionResult<V, S> {
    /// Candidate evaluations, excluding the initial score and diagnostics.
    pub fn move_evaluations(&self) -> u64 {
        self.evaluations.saturating_sub(1)
    }
}
impl<V: Clone, S: Clone> ListConstructionResult<V, S> {
    pub fn solution(&self) -> Option<ScoredListAssignment<V, S>> {
        if !self.complete {
            return None;
        }
        self.score.clone().map(|score| ScoredListAssignment {
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
    construction_impl(problem, initial, config, |initial| {
        ListScoreDirector::new(problem, network, initial)
    })
}
pub(crate) fn construction_impl<V: Eq + Clone, S: AdditiveScore, D: ListSearchDirector<V, S>>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
) -> Result<ListConstructionResult<V, S>, D::Error> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    construction_impl_with_clock(problem, initial, config, make_director, clock)
}
pub(crate) fn construction_impl_with_clock<
    V: Eq + Clone,
    S: AdditiveScore,
    D: ListSearchDirector<V, S>,
>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<ListConstructionResult<V, S>, D::Error> {
    let mut time = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    problem.validate(&initial).map_err(D::Error::from)?;
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
    if config.cancelled() {
        result.termination = ConstructionTermination::Cancelled;
        return Ok(result);
    }
    let mut director = make_director(initial)?;
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_constraint_evaluations();
    if config.assert_scores {
        director.assert_consistent()?;
    }
    result.termination = ConstructionTermination::Completed;
    if !problem.allows_unassigned() {
        for value in problem.values() {
            if config.cancelled() {
                result.termination = ConstructionTermination::Cancelled;
                break;
            }
            if time.expired() {
                result.termination = ConstructionTermination::TimeLimit;
                break;
            }
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
            result.assigned_values += usize::from(selection.selected.is_some());
            if config.assert_scores {
                director.assert_consistent()?;
            }
            if selection.evaluation_limit {
                result.termination = budget.construction_reason();
                break;
            }
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
