//! Greedy construction of partial mixed models using one contribution cache.
use crate::list_neighborhood::insertions;
use crate::mixed_search_scoring::MixedSearchDirector;
use crate::{
    AdditiveScore, ConstructionConfig, ConstructionTermination, ListMove, MixedAssignment,
    MixedConstraintNetwork, MixedMove, MixedProblem, MixedScoreDirector, MixedScoringError, Move,
    ScoredMixedAssignment,
};

/// Construction order can affect quality when constraints span both components.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MixedConstructionOrder {
    #[default]
    BasicThenLists,
    ListsThenBasic,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MixedConstructionConfig<'a> {
    pub construction: ConstructionConfig<'a>,
    pub order: MixedConstructionOrder,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedConstructionResult<B, L, S> {
    /// Includes partial progress when the shared evaluation budget runs out.
    pub assignment: MixedAssignment<B, L>,
    pub score: Option<S>,
    pub complete: bool,
    pub evaluations: u64,
    pub assigned_basic_variables: usize,
    pub assigned_list_values: usize,
    /// Nonempty scans across both components, including optional rejection.
    /// Interrupted scans count only under KeepBestPrefix.
    pub completed_steps: u64,
    pub constraint_evaluations: u128,
    pub termination: ConstructionTermination,
}
impl<B, L, S> MixedConstructionResult<B, L, S> {
    /// Candidate evaluations, excluding the initial score and diagnostics.
    pub fn move_evaluations(&self) -> u64 {
        self.evaluations.saturating_sub(1)
    }
}
impl<B: Clone, L: Clone, S: Clone> MixedConstructionResult<B, L, S> {
    /// Completeness and a calculated score are required; hard feasibility is not.
    pub fn solution(&self) -> Option<ScoredMixedAssignment<B, L, S>> {
        if !self.complete {
            return None;
        }
        self.score.clone().map(|score| ScoredMixedAssignment {
            assignment: self.assignment.clone(),
            score,
        })
    }
}
#[derive(Clone, Copy)]
enum Phase {
    Basic,
    Lists,
}

/// Preserve every supplied value, pin and initial list order. Fill missing required
/// basics in model/domain order and required list values in registry/entity/index
/// order, following the explicit component order. Optional unassignment follows
/// ConstructionConfig::optional. Pooled placement compares eligible assignments
/// across both components; component order then determines only encounter order
/// for ties and early picks. ValueQueue instead completes each component in order,
/// cycling basic values and using sequential list insertion. Rejected optional
/// basics are retried after a successful basic placement, not after later list work.
/// Constraints must handle partial combined states.
///
/// One budget includes the initial score and every candidate in both phases.
/// Winners reuse saved caches without callback replay. A truncated scan follows
/// the configured interruption policy and reports the applicable limit.
/// Exactly exhausting the final scan reports Completed. Empty required domains or
/// unreachable required list values report NoCompleteAssignment before callbacks,
/// including at zero budget. Validating model/network shape also precedes scoring.
/// This greedy initializer neither backtracks nor proves feasibility or optimality.
///
/// ```
/// use timetuck::{FiniteProblem, ListEntity, ListProblem, MixedConstraintNetwork,
///     MixedConstructionConfig, MixedProblem, SimpleScore, Variable,
///     greedy_mixed_construction};
/// let problem = MixedProblem::new(
///     FiniteProblem::new(vec![Variable::new(vec![1,2])])?,
///     ListProblem::new(vec![7], vec![ListEntity::new(vec![])])?,
/// );
/// let network = MixedConstraintNetwork::<_,_,SimpleScore>::new(1,1,1);
/// let result = greedy_mixed_construction(&problem, &network, MixedConstructionConfig::default())?;
/// assert!(result.complete);
/// assert_eq!(result.evaluations, 4);
/// assert_eq!(result.assigned_basic_variables, 1);
/// assert_eq!(result.assigned_list_values, 1);
/// assert_eq!(result.solution().unwrap().score, SimpleScore::ZERO);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn greedy_mixed_construction<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    settings: MixedConstructionConfig,
) -> Result<MixedConstructionResult<B, L, S>, MixedScoringError> {
    greedy_mixed_construction_from(problem, network, problem.initial_assignment(), settings)
}

/// Resume construction from a validated assignment, preserving supplied values and pins.
/// Missing required and configured optional values are considered; each call
/// initializes its own scoring cache.
/// Validation runs even with zero budget or a cancelled token.
pub fn greedy_mixed_construction_from<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
) -> Result<MixedConstructionResult<B, L, S>, MixedScoringError> {
    greedy_mixed_construction_from_monitored(None, problem, network, initial, settings)
}

pub(crate) fn greedy_mixed_construction_from_monitored<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
) -> Result<MixedConstructionResult<B, L, S>, MixedScoringError> {
    network.validate(problem, &initial)?;
    construction_impl_monitored(shared, problem, initial, settings, |initial| {
        MixedScoreDirector::new(problem, network, initial)
    })
}

pub(crate) fn construction_impl_with_clock_monitored<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<MixedConstructionResult<B, L, S>, D::Error> {
    let mut shared = shared;
    let move_counter = crate::move_metrics::from_control(&shared);
    let score_counter = crate::score_evaluations::from_control(&shared);
    let mut time = crate::time_limit::TimeBudget::new(settings.construction.max_time, None, clock);
    let config = settings.construction;
    problem.validate(&initial).map_err(D::Error::from)?;
    let mut result = MixedConstructionResult {
        complete: problem.is_complete(&initial),
        assignment: initial.clone(),
        score: None,
        evaluations: 0,
        assigned_basic_variables: 0,
        assigned_list_values: 0,
        completed_steps: 0,
        constraint_evaluations: 0,
        termination: ConstructionTermination::EvaluationLimit,
    };
    if problem.basic().has_empty_movable_required_domain()
        || !problem.lists().can_complete(initial.lists())
    {
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
    crate::score_evaluations::record(&score_counter);
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_constraint_evaluations();
    if config.assert_scores || config.assert_step_scores {
        director.assert_consistent()?;
    }
    crate::time_limit::capture_constraint_metrics(&shared, |c| {
        director.capture_constraint_metrics(c)
    });
    director.notify_best(|assignment| problem.is_complete(assignment));
    crate::time_limit::observe_construction_score(
        &mut shared,
        || director.score(),
        || problem.is_complete(director.assignment()),
    );
    result.termination = ConstructionTermination::Completed;
    let phases = match settings.order {
        MixedConstructionOrder::BasicThenLists => [Phase::Basic, Phase::Lists],
        MixedConstructionOrder::ListsThenBasic => [Phase::Lists, Phase::Basic],
    };
    'phases: for phase in phases {
        let count = match phase {
            Phase::Basic => problem.basic().variables().len(),
            Phase::Lists => problem.lists().values().len(),
        };
        let mut indices_in_order = 0..count;
        let mut value_queue = (matches!(phase, Phase::Basic)
            && config.placement == crate::ConstructionPlacement::ValueQueue)
            .then(|| crate::value_construction_queue::ValueConstructionQueue::new(problem.basic()));
        loop {
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
                break 'phases;
            }
            if let Some(reason) = crate::construction::construction_stop(&mut time, &mut shared) {
                result.termination = reason;
                break 'phases;
            }
            let previous = director.score();
            let budget = crate::count_limit::CandidateBudget::new(
                config.max_evaluations,
                config.max_move_evaluations,
                result.evaluations,
            );
            let scan = crate::construction::ConstructionScan::new(config);
            let selection = if let Some(queue) = &mut value_queue {
                let Some((value, indices)) = queue.next(
                    problem.basic(),
                    director.assignment().basic(),
                    config.optional,
                ) else {
                    break;
                };
                if config.step_limit_reached(result.completed_steps) {
                    result.termination = ConstructionTermination::StepLimit;
                    break 'phases;
                }
                let no_change = config.optional == crate::OptionalConstruction::EvaluateUnassigned
                    && indices
                        .iter()
                        .any(|&i| problem.basic().variables()[i].allows_unassigned());
                let moves = indices
                    .into_iter()
                    .map(|variable| MixedMove {
                        basic: Move::Change {
                            variable,
                            value: Some(value.clone()),
                        },
                        lists: ListMove::Composite(vec![]),
                    })
                    .chain(no_change.then(|| MixedMove {
                        basic: Move::Composite(vec![]),
                        lists: ListMove::Composite(vec![]),
                    }));
                if let Some(shared) = &mut shared {
                    shared.step_started();
                }
                director.select_best(
                    moves.take_while(|_| scan.proceed(&mut time, &mut shared)),
                    budget.remaining,
                    false,
                    |movement, score| {
                        crate::score_evaluations::record(&score_counter);
                        crate::move_metrics::record(&move_counter, movement);
                        if config.optional == crate::OptionalConstruction::EvaluateUnassigned {
                            return true;
                        }
                        let Move::Change { variable, .. } = &movement.basic else {
                            unreachable!("queued change")
                        };
                        !problem.basic().variables()[*variable].allows_unassigned()
                            || score >= previous
                    },
                    scan.policy(previous.clone()),
                    config.assert_scores,
                )?
            } else if config.placement == crate::ConstructionPlacement::Pooled {
                let snapshot = director.assignment().clone();
                let no_change = config.optional == crate::OptionalConstruction::EvaluateUnassigned
                    && (problem
                        .basic()
                        .variables()
                        .iter()
                        .enumerate()
                        .any(|(i, v)| {
                            v.allows_unassigned()
                                && !v.is_pinned()
                                && snapshot.basic().values()[i].is_none()
                        })
                        || (problem.lists().allows_unassigned()
                            && problem
                                .lists()
                                .values()
                                .iter()
                                .any(|v| snapshot.lists().location(v).is_none())));
                let mut moves = pooled_moves(
                    problem,
                    &snapshot,
                    settings.order,
                    config.optional,
                    config.list_order,
                )
                .chain(no_change.then(|| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Composite(vec![]),
                }))
                .peekable();
                if moves.peek().is_none() {
                    break 'phases;
                }
                if config.step_limit_reached(result.completed_steps) {
                    result.termination = ConstructionTermination::StepLimit;
                    break 'phases;
                }
                if let Some(shared) = &mut shared {
                    shared.step_started();
                }
                director.select_best(
                    moves.take_while(|_| scan.proceed(&mut time, &mut shared)),
                    budget.remaining,
                    false,
                    |movement, score| {
                        crate::score_evaluations::record(&score_counter);
                        crate::move_metrics::record(&move_counter, movement);
                        if config.optional == crate::OptionalConstruction::EvaluateUnassigned {
                            return true;
                        }
                        let optional = match &movement.basic {
                            Move::Change { variable, .. } => {
                                problem.basic().variables()[*variable].allows_unassigned()
                            }
                            _ => problem.lists().allows_unassigned(),
                        };
                        !optional || score >= previous
                    },
                    scan.policy(previous.clone()),
                    config.assert_scores,
                )?
            } else {
                match phase {
                    Phase::Basic => {
                        let variable = &problem.basic().variables()[index];
                        if (variable.allows_unassigned()
                            && config.optional == crate::OptionalConstruction::Preserve)
                            || variable.is_pinned()
                            || director.assignment().basic().values()[index].is_some()
                        {
                            continue;
                        }
                        if config.step_limit_reached(result.completed_steps) {
                            result.termination = ConstructionTermination::StepLimit;
                            break 'phases;
                        }
                        let moves = variable
                            .domain()
                            .iter()
                            .map(|value| MixedMove {
                                basic: Move::Change {
                                    variable: index,
                                    value: Some(value.clone()),
                                },
                                lists: ListMove::Composite(vec![]),
                            })
                            .chain(
                                (variable.allows_unassigned()
                                    && config.optional
                                        == crate::OptionalConstruction::EvaluateUnassigned)
                                    .then(|| MixedMove {
                                        basic: Move::Composite(vec![]),
                                        lists: ListMove::Composite(vec![]),
                                    }),
                            );
                        if let Some(shared) = &mut shared {
                            shared.step_started();
                        }
                        director.select_best(
                            moves.take_while(|_| scan.proceed(&mut time, &mut shared)),
                            budget.remaining,
                            false,
                            |movement, score| {
                                crate::score_evaluations::record(&score_counter);
                                crate::move_metrics::record(&move_counter, movement);
                                config.optional == crate::OptionalConstruction::EvaluateUnassigned
                                    || !variable.allows_unassigned()
                                    || score >= previous
                            },
                            scan.policy(previous.clone()),
                            config.assert_scores,
                        )?
                    }
                    Phase::Lists => {
                        let lists = problem.lists();
                        let value = &lists.values()[index];
                        if (lists.allows_unassigned()
                            && config.optional == crate::OptionalConstruction::Preserve)
                            || director.assignment().lists().location(value).is_some()
                        {
                            continue;
                        }
                        if config.step_limit_reached(result.completed_steps) {
                            result.termination = ConstructionTermination::StepLimit;
                            break 'phases;
                        }
                        let snapshot = director.assignment().clone();
                        let moves = crate::list_neighborhood::construction_insertions(
                            lists,
                            snapshot.lists(),
                            insertions(lists, snapshot.lists(), 0..lists.entities().len())
                                .filter(|position| lists.accepts(position.entity, value)),
                            value,
                            config.list_order,
                        )
                        .map(|destination| MixedMove {
                            basic: Move::Composite(vec![]),
                            lists: ListMove::Assign {
                                value: value.clone(),
                                destination,
                            },
                        })
                        .chain(
                            (lists.allows_unassigned()
                                && config.optional
                                    == crate::OptionalConstruction::EvaluateUnassigned)
                                .then(|| MixedMove {
                                    basic: Move::Composite(vec![]),
                                    lists: ListMove::Composite(vec![]),
                                }),
                        );
                        if let Some(shared) = &mut shared {
                            shared.step_started();
                        }
                        director.select_best(
                            moves.take_while(|_| scan.proceed(&mut time, &mut shared)),
                            budget.remaining,
                            false,
                            |movement, score| {
                                crate::score_evaluations::record(&score_counter);
                                crate::move_metrics::record(&move_counter, movement);
                                config.optional == crate::OptionalConstruction::EvaluateUnassigned
                                    || !lists.allows_unassigned()
                                    || score >= previous
                            },
                            scan.policy(previous.clone()),
                            config.assert_scores,
                        )?
                    }
                }
            };
            for _ in 0..selection.replay_evaluations {
                crate::score_evaluations::record(&score_counter);
            }
            result.evaluations += selection.evaluations;
            let completed =
                selection.evaluations > 0 && !scan.discarded(selection.evaluation_limit);
            result.completed_steps += u64::from(completed);
            if let Some(shared) = &mut shared {
                crate::time_limit::finish_step(
                    *shared,
                    completed,
                    || director.score(),
                    || problem.is_complete(director.assignment()),
                );
            }
            if completed && let Some(shared) = &mut shared {
                shared.complete_construction_step();
            }
            if completed {
                crate::time_limit::observe_construction_score(
                    &mut shared,
                    || director.score(),
                    || problem.is_complete(director.assignment()),
                );
            }

            result.constraint_evaluations += selection.constraint_evaluations;
            let installed = selection.selected.as_ref().is_some_and(|movement| {
                if matches!(movement.basic, Move::Change { .. }) {
                    result.assigned_basic_variables += 1;
                    true
                } else if matches!(movement.lists, ListMove::Assign { .. }) {
                    result.assigned_list_values += 1;
                    true
                } else {
                    false
                }
            });
            if config.assert_scores || config.assert_step_scores {
                director.assert_consistent()?;
            }
            if installed {
                if let Some(queue) = &mut value_queue {
                    queue.installed();
                }
                crate::time_limit::capture_constraint_metrics(&shared, |c| {
                    director.capture_constraint_metrics(c)
                });
                director.notify_best(|assignment| problem.is_complete(assignment));
            }
            if selection.evaluation_limit {
                result.termination = budget.construction_reason();
                break 'phases;
            }
            if let Some(reason) = scan.reason() {
                result.termination = reason;
                break 'phases;
            }
            if config.placement == crate::ConstructionPlacement::Pooled && !installed {
                break 'phases;
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

// Candidate order controls deterministic ties and early picks; every scan uses
// one immutable assignment while the director scores and undoes each trial.
fn pooled_moves<'a, B: Eq + Clone, L: Eq + Clone>(
    problem: &'a MixedProblem<B, L>,
    snapshot: &'a MixedAssignment<B, L>,
    order: MixedConstructionOrder,
    optional: crate::OptionalConstruction,
    list_order: crate::ListConstructionOrder,
) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a> {
    let basics = problem
        .basic()
        .variables()
        .iter()
        .enumerate()
        .filter(move |(i, v)| {
            !v.is_pinned()
                && snapshot.basic().values()[*i].is_none()
                && (!v.allows_unassigned() || optional != crate::OptionalConstruction::Preserve)
        })
        .flat_map(|(i, v)| {
            v.domain().iter().map(move |value| MixedMove {
                basic: Move::Change {
                    variable: i,
                    value: Some(value.clone()),
                },
                lists: ListMove::Composite(vec![]),
            })
        });
    let lists = problem.lists();
    let list_moves = lists
        .values()
        .iter()
        .filter(move |v| {
            snapshot.lists().location(v).is_none()
                && (!lists.allows_unassigned() || optional != crate::OptionalConstruction::Preserve)
        })
        .flat_map(move |value| {
            crate::list_neighborhood::construction_insertions(
                lists,
                snapshot.lists(),
                insertions(lists, snapshot.lists(), 0..lists.entities().len())
                    .filter(move |p| lists.accepts(p.entity, value)),
                value,
                list_order,
            )
            .map(move |destination| MixedMove {
                basic: Move::Composite(vec![]),
                lists: ListMove::Assign {
                    value: value.clone(),
                    destination,
                },
            })
        });
    match order {
        MixedConstructionOrder::BasicThenLists => Box::new(basics.chain(list_moves)),
        MixedConstructionOrder::ListsThenBasic => Box::new(list_moves.chain(basics)),
    }
}

pub(crate) fn construction_impl_monitored<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
) -> Result<MixedConstructionResult<B, L, S>, D::Error> {
    let clock =
        crate::solver_clock::monitored_clock(settings.construction.max_time.is_some(), &shared);
    construction_impl_with_clock_monitored(shared, problem, initial, settings, make_director, clock)
}
