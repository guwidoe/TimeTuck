//! Bounded local improvement of complete list assignments.
use crate::list_search_scoring::ListSearchDirector;
use crate::{
    AcceptanceError, AdditiveScore, ListAssignment, ListConstraintNetwork, ListProblem,
    ListScoreDirector, ListScoringError, LocalSearchConfig, LocalSearchTermination, MoveOrder,
    MoveSelection, RejectedMovePolicy, ScoredListAssignment,
};
use crate::{list_neighborhood::ordered, random::SplitMix64};
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum ListLocalSearchError {
    Scoring(ListScoringError),
    StreamScoring(crate::ListStreamScoringError),
    Acceptance(AcceptanceError),
    IncompleteInitialAssignment,
}
impl fmt::Display for ListLocalSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoring(e) => e.fmt(f),
            Self::StreamScoring(e) => e.fmt(f),
            Self::Acceptance(e) => e.fmt(f),
            Self::IncompleteInitialAssignment => {
                f.write_str("list local search requires a complete initial assignment")
            }
        }
    }
}
impl Error for ListLocalSearchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoring(e) => Some(e),
            Self::StreamScoring(e) => Some(e),
            Self::Acceptance(e) => Some(e),
            Self::IncompleteInitialAssignment => None,
        }
    }
}
impl From<ListScoringError> for ListLocalSearchError {
    fn from(e: ListScoringError) -> Self {
        Self::Scoring(e)
    }
}
impl From<crate::ListStreamScoringError> for ListLocalSearchError {
    fn from(e: crate::ListStreamScoringError) -> Self {
        Self::StreamScoring(e)
    }
}
impl From<AcceptanceError> for ListLocalSearchError {
    fn from(e: AcceptanceError) -> Self {
        Self::Acceptance(e)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListLocalSearchResult<V, S> {
    pub best: Option<ScoredListAssignment<V, S>>,
    pub evaluations: u64,
    pub accepted_moves: u64,
    pub completed_steps: u64,
    /// Completed steps since the last strict improvement of the retained best.
    pub unimproved_steps: u64,
    pub constraint_evaluations: u128,
    pub termination: LocalSearchTermination,
}
/// Improve a complete assignment, for example the output of greedy construction.
/// Uses the shared search configuration: Change includes relocations plus optional
/// assign/unassign moves; ChangeAndSwap adds ordered swaps and optional replacements.
/// Gather enumerates bounded identity groups at original insertion boundaries,
/// followed by optional subset unassignments. ShuffledVariables changes gather
/// destination order; member permutations remain in registry order.
/// ShuffledMoves reorders generated candidates within a bounded rolling window.
/// ShuffledVariables shuffles entity lists; insertion positions and value order
/// remain fixed. VariableTabu uses touched entity-list indices as stable keys.
///
/// The initial rescore counts once, each trial once; accepted and completed-step
/// limits are separate. First/best accepted candidates install saved caches without
/// replay. Optional rejected fallback may worsen the current state, but the first
/// global best is retained. No termination here proves global optimality.
/// Invalid shape, pins, completeness and acceptance settings fail even at zero budget.
pub fn list_local_search_from<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list_local_search_from_monitored(None, problem, network, initial, config)
}

pub(crate) fn list_local_search_from_monitored<V: Eq + Clone, S: AdditiveScore>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.validate_acceptance()?;
    network.validate(problem, &initial)?;
    search_impl_monitored(shared, problem, initial, config, |initial| {
        ListScoreDirector::new(problem, network, initial)
    })
}

pub(crate) fn search_impl_monitored<V: Eq + Clone, S: AdditiveScore, D: ListSearchDirector<V, S>>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError>
where
    ListLocalSearchError: From<D::Error>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.max_time.is_some() || config.max_unimproved_time.is_some(),
    );
    search_impl_with_clock_monitored(shared, problem, initial, config, make_director, clock)
}

#[cfg(test)]
pub(crate) fn search_impl_with_clock<V: Eq + Clone, S: AdditiveScore, D: ListSearchDirector<V, S>>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError>
where
    ListLocalSearchError: From<D::Error>,
{
    search_impl_with_clock_monitored(None, problem, initial, config, make_director, clock)
}

pub(crate) fn search_impl_with_clock_monitored<
    V: Eq + Clone,
    S: AdditiveScore,
    D: ListSearchDirector<V, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError>
where
    ListLocalSearchError: From<D::Error>,
{
    let mut time_budget =
        crate::time_limit::TimeBudget::new(config.max_time, config.max_unimproved_time, clock)
            .with_shared(shared);
    config.validate_acceptance()?;
    problem.validate(&initial).map_err(D::Error::from)?;
    if !problem.is_complete(&initial) {
        return Err(ListLocalSearchError::IncompleteInitialAssignment);
    }
    let mut result = ListLocalSearchResult {
        best: None,
        evaluations: 0,
        accepted_moves: 0,
        completed_steps: 0,
        unimproved_steps: 0,
        constraint_evaluations: 0,
        termination: LocalSearchTermination::EvaluationLimit,
    };
    if config.max_evaluations == 0 {
        return Ok(result);
    }
    if config.cancelled() {
        result.termination = LocalSearchTermination::Cancelled;
        return Ok(result);
    }
    let mut director = make_director(initial)?;
    let mut acceptor = config.start_acceptance(director.score(), problem.entities().len())?;
    let mut random = match config.move_order {
        MoveOrder::Original => None,
        MoveOrder::ShuffledVariables { seed } | MoveOrder::ShuffledMoves { seed, .. } => {
            Some(SplitMix64::new(seed))
        }
    };
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_constraint_evaluations();
    result.best = Some(ScoredListAssignment {
        assignment: director.assignment().clone(),
        score: director.score(),
    });
    if config.assert_scores {
        director.assert_consistent()?;
    }
    time_budget.start_search();
    director.notify_best(|_| true);
    let mut progress = crate::StepProgress::default();
    loop {
        if config.cancelled() {
            result.termination = LocalSearchTermination::Cancelled;
            return Ok(result);
        }
        if let Some(reason) = config.step_termination(
            result.best.as_ref().expect("initial best").score.clone(),
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
        let snapshot = director.assignment().clone();
        let mut order: Vec<_> = (0..problem.entities().len()).collect();
        if matches!(config.move_order, MoveOrder::ShuffledVariables { .. })
            && let Some(random) = &mut random
        {
            random.shuffle(&mut order);
        }
        let moves = ordered(
            problem,
            &snapshot,
            config.neighborhood,
            order.iter().copied(),
        );
        let current = director.score();
        let tracks_entities = config.tracks_variables();
        let tracks_values = config.tracks_values();
        let budget = crate::count_limit::CandidateBudget::new(
            config.max_evaluations,
            config.max_move_evaluations,
            result.evaluations,
        );
        let selection = director.select_best(
            crate::random::order_moves(moves, config.move_order, random.as_mut(), budget.remaining)
                .take_while(|_| !config.cancelled() && !time_budget.expired()),
            budget.remaining,
            config.rejected_moves == RejectedMovePolicy::BestScore,
            |movement, score| {
                let keys = if tracks_entities {
                    movement.touched_entities()
                } else {
                    vec![]
                };
                let values = if tracks_values {
                    problem
                        .planning_values(&snapshot, movement)
                        .expect("scored move has valid value metadata")
                } else {
                    Vec::new()
                };
                acceptor.accepts(current.clone(), score, &keys, movement, &values)
            },
            |_| config.selection == MoveSelection::FirstAccepted,
            config.assert_scores,
        )?;
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        let mut notify_improvement = false;
        if let Some(movement) = &selection.selected {
            let improved = director.score() > result.best.as_ref().expect("initial best").score;
            if improved {
                notify_improvement = true;
                time_budget.record_improvement();
                result.best = Some(ScoredListAssignment {
                    assignment: director.assignment().clone(),
                    score: director.score(),
                });
            }
            let keys = if tracks_entities {
                movement.touched_entities()
            } else {
                vec![]
            };
            let values = if tracks_values {
                problem
                    .planning_values(&snapshot, movement)
                    .expect("scored move has valid value metadata")
            } else {
                Vec::new()
            };
            acceptor.step_ended(director.score(), &keys, movement, &values);
            result.accepted_moves += u64::from(selection.selected_accepted);
            progress.complete_step(improved);
            time_budget.complete_step(improved);
            result.completed_steps = progress.completed_steps();
            result.unimproved_steps = progress.unimproved_steps();
        }
        if config.assert_scores {
            director.assert_consistent()?;
        }
        if notify_improvement {
            director.notify_best(|_| true);
        }
        if selection.evaluation_limit {
            result.termination = budget.local_reason();
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

impl<V, S> ListLocalSearchResult<V, S> {
    /// Candidate evaluations, including rejected trials; initialization excluded.
    pub fn move_evaluations(&self) -> u64 {
        self.evaluations.saturating_sub(1)
    }
}
