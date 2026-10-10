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
    /// Nonempty scans, including optional rejection. Interrupted scans count
    /// only under KeepBestPrefix.
    pub completed_steps: u64,
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
/// candidate; the shared early-pick policy compares ordinary levels.
/// Pooled placement compares all missing values and legal insertion positions
/// each step, preserving registry/entity/index order for ties. Optional
/// missing values remain unassigned by default; NonWorsening also considers them
/// and preserves unassignment when every evaluated insertion is worse.
/// EvaluateUnassigned instead appends a scored no-change candidate last; early
/// picks and KeepBestPrefix interruptions may retain a worsening insertion.
/// ValueQueue uses this same sequential list-value insertion order.
/// Initial scoring and every trial count toward the budget; installing a winner
/// reuses its saved contribution cache. Partial progress is returned on limits.
pub fn greedy_list_construction<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    config: ConstructionConfig,
) -> Result<ListConstructionResult<V, S>, ListScoringError> {
    greedy_list_construction_from(problem, network, problem.initial_assignment(), config)
}

/// Resume construction from a validated assignment, preserving supplied values and pins.
/// Missing required and configured optional values are considered; each call
/// initializes its own scoring cache.
/// Validation runs even with zero budget or a cancelled token.
pub fn greedy_list_construction_from<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
) -> Result<ListConstructionResult<V, S>, ListScoringError> {
    greedy_list_construction_from_monitored(None, problem, network, initial, config)
}

pub(crate) fn greedy_list_construction_from_monitored<V: Eq + Clone, S: AdditiveScore>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
) -> Result<ListConstructionResult<V, S>, ListScoringError> {
    network.validate(problem, &initial)?;
    construction_impl_monitored(shared, problem, initial, config, |initial| {
        ListScoreDirector::new(problem, network, initial)
    })
}

pub(crate) fn construction_impl_with_clock_monitored<
    V: Eq + Clone,
    S: AdditiveScore,
    D: ListSearchDirector<V, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<ListConstructionResult<V, S>, D::Error> {
    let mut shared = shared;
    let move_counter = crate::move_metrics::from_control(&shared);
    let mut time = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    problem.validate(&initial).map_err(D::Error::from)?;
    let mut result = ListConstructionResult {
        complete: problem.is_complete(&initial),
        assignment: initial.clone(),
        score: None,
        evaluations: 0,
        assigned_values: 0,
        completed_steps: 0,
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
    if config.assert_scores || config.assert_step_scores {
        director.assert_consistent()?;
    }
    director.notify_best(|assignment| problem.is_complete(assignment));
    result.termination = ConstructionTermination::Completed;
    if !problem.allows_unassigned() || config.optional != crate::OptionalConstruction::Preserve {
        for value in problem.values() {
            if config.cancelled() {
                result.termination = ConstructionTermination::Cancelled;
                break;
            }
            if let Some(reason) = crate::construction::construction_stop(&mut time, &mut shared) {
                result.termination = reason;
                break;
            }
            let values: Vec<_> = if config.placement == crate::ConstructionPlacement::Pooled {
                problem
                    .values()
                    .iter()
                    .filter(|v| {
                        director.assignment().location(v).is_none()
                            && insertions(
                                problem,
                                director.assignment(),
                                0..problem.entities().len(),
                            )
                            .any(|position| problem.accepts(position.entity, v))
                    })
                    .collect()
            } else if director.assignment().location(value).is_none() {
                vec![value]
            } else {
                continue;
            };
            if values.is_empty() {
                break;
            }
            if config.step_limit_reached(result.completed_steps) {
                result.termination = ConstructionTermination::StepLimit;
                break;
            }
            let snapshot = director.assignment().clone();
            let previous = director.score();
            let moves = values
                .into_iter()
                .flat_map(|value| {
                    insertions(problem, &snapshot, 0..problem.entities().len())
                        .filter(move |position| problem.accepts(position.entity, value))
                        .map(move |destination| ListMove::Assign {
                            value: value.clone(),
                            destination,
                        })
                })
                .chain(
                    (problem.allows_unassigned()
                        && config.optional == crate::OptionalConstruction::EvaluateUnassigned)
                        .then(|| ListMove::Composite(vec![])),
                );
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
                    crate::move_metrics::record(&move_counter, movement);
                    config.optional == crate::OptionalConstruction::EvaluateUnassigned
                        || !problem.allows_unassigned()
                        || score >= previous
                },
                scan.policy(previous.clone()),
                config.assert_scores,
            )?;
            result.evaluations += selection.evaluations;
            result.constraint_evaluations += selection.constraint_evaluations;
            let installed = matches!(selection.selected, Some(ListMove::Assign { .. }));
            result.assigned_values += usize::from(installed);
            let completed =
                selection.evaluations > 0 && !scan.discarded(selection.evaluation_limit);
            result.completed_steps += u64::from(completed);
            if let Some(shared) = &mut shared {
                shared.step_ended(completed);
            }
            if completed && let Some(shared) = &mut shared {
                shared.complete_construction_step();
            }
            if config.assert_scores || config.assert_step_scores {
                director.assert_consistent()?;
            }
            if installed {
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
            if config.placement == crate::ConstructionPlacement::Pooled && !installed {
                break;
            }
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
    D: ListSearchDirector<V, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
) -> Result<ListConstructionResult<V, S>, D::Error> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    construction_impl_with_clock_monitored(shared, problem, initial, config, make_director, clock)
}
