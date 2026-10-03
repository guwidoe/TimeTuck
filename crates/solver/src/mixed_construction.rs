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
    /// Evaluated scans across both components, including rejected optional scans.
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
/// ConstructionConfig::optional. Constraints must handle partial combined states.
///
/// One budget includes the initial score and every candidate in both phases.
/// Winners reuse saved caches without callback replay. A truncated scan retains
/// its best evaluated candidate and reports EvaluationLimit, even if now complete.
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
    network.validate(problem, &initial)?;
    construction_impl(problem, initial, settings, |initial| {
        MixedScoreDirector::new(problem, network, initial)
    })
}
pub(crate) fn construction_impl<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
) -> Result<MixedConstructionResult<B, L, S>, D::Error> {
    let clock = crate::time_limit::monotonic_clock(settings.construction.max_time.is_some());
    construction_impl_with_clock(problem, initial, settings, make_director, clock)
}
pub(crate) fn construction_impl_with_clock<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<MixedConstructionResult<B, L, S>, D::Error> {
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
    if problem.basic().search_space_size() == Some(0)
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
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_constraint_evaluations();
    if config.assert_scores {
        director.assert_consistent()?;
    }
    director.notify_best(|assignment| problem.is_complete(assignment));
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
        for index in 0..count {
            if config.cancelled() {
                result.termination = ConstructionTermination::Cancelled;
                break 'phases;
            }
            if time.expired() {
                result.termination = ConstructionTermination::TimeLimit;
                break 'phases;
            }
            let previous = director.score();
            let budget = crate::count_limit::CandidateBudget::new(
                config.max_evaluations,
                config.max_move_evaluations,
                result.evaluations,
            );
            let selection = match phase {
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
                    let moves = variable.domain().iter().map(|value| MixedMove {
                        basic: Move::Change {
                            variable: index,
                            value: Some(value.clone()),
                        },
                        lists: ListMove::Composite(vec![]),
                    });
                    director.select_best(
                        moves.take_while(|_| !config.cancelled() && !time.expired()),
                        budget.remaining,
                        false,
                        |_, score| !variable.allows_unassigned() || score >= previous,
                        |score| config.pick_early.should_pick(previous.clone(), score),
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
                    let moves = insertions(lists, snapshot.lists(), 0..lists.entities().len())
                        .filter(|position| lists.accepts(position.entity, value))
                        .map(|destination| MixedMove {
                            basic: Move::Composite(vec![]),
                            lists: ListMove::Assign {
                                value: value.clone(),
                                destination,
                            },
                        });
                    director.select_best(
                        moves.take_while(|_| !config.cancelled() && !time.expired()),
                        budget.remaining,
                        false,
                        |_, score| !lists.allows_unassigned() || score >= previous,
                        |score| config.pick_early.should_pick(previous.clone(), score),
                        config.assert_scores,
                    )?
                }
            };
            result.evaluations += selection.evaluations;
            result.completed_steps += u64::from(selection.evaluations > 0);
            result.constraint_evaluations += selection.constraint_evaluations;
            if selection.selected.is_some() {
                match phase {
                    Phase::Basic => result.assigned_basic_variables += 1,
                    Phase::Lists => result.assigned_list_values += 1,
                }
            }
            if config.assert_scores {
                director.assert_consistent()?;
            }
            if selection.selected.is_some() {
                director.notify_best(|assignment| problem.is_complete(assignment));
            }
            if selection.evaluation_limit {
                result.termination = budget.construction_reason();
                break 'phases;
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
