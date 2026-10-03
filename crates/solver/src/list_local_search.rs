//! Bounded local improvement of complete list assignments.
use crate::list_search_scoring::ListSearchDirector;
use crate::{
    Acceptance, AcceptanceError, AdditiveScore, ListAssignment, ListConstraintNetwork, ListProblem,
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
    config.acceptance.validate()?;
    network.validate(problem, &initial)?;
    search_impl(problem, initial, config, |initial| {
        ListScoreDirector::new(problem, network, initial)
    })
}
pub(crate) fn search_impl<V: Eq + Clone, S: AdditiveScore, D: ListSearchDirector<V, S>>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    make_director: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError>
where
    ListLocalSearchError: From<D::Error>,
{
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    search_impl_with_clock(problem, initial, config, make_director, clock)
}

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
    let mut time_budget = crate::time_limit::TimeBudget::new(config.max_time, clock);
    config.acceptance.validate()?;
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
    let mut director = make_director(initial)?;
    let mut acceptor = config.acceptance.start(director.score())?;
    let mut random = match config.move_order {
        MoveOrder::Original => None,
        MoveOrder::ShuffledVariables { seed } => Some(SplitMix64::new(seed)),
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
    let mut progress = crate::StepProgress::default();
    loop {
        if let Some(reason) = config.step_termination(
            result.best.as_ref().expect("initial best").score,
            result.accepted_moves,
            progress,
        ) {
            result.termination = reason;
            return Ok(result);
        }
        if time_budget.expired() {
            result.termination = LocalSearchTermination::TimeLimit;
            return Ok(result);
        }
        let snapshot = director.assignment().clone();
        let mut order: Vec<_> = (0..problem.entities().len()).collect();
        if let Some(random) = &mut random {
            random.shuffle(&mut order);
        }
        let moves = ordered(
            problem,
            &snapshot,
            config.neighborhood,
            order.iter().copied(),
        );
        let current = director.score();
        let tracks_entities = matches!(config.acceptance, Acceptance::VariableTabu { .. });
        let selection = director.select_best(
            moves.take_while(|_| !time_budget.expired()),
            config.max_evaluations - result.evaluations,
            config.rejected_moves == RejectedMovePolicy::BestScore,
            |movement, score| {
                let keys = if tracks_entities {
                    movement.touched_entities()
                } else {
                    vec![]
                };
                acceptor.accepts(current, score, &keys)
            },
            |_| config.selection == MoveSelection::FirstAccepted,
            config.assert_scores,
        )?;
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        if let Some(movement) = &selection.selected {
            let improved = director.score() > result.best.as_ref().expect("initial best").score;
            if improved {
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
            acceptor.step_ended(director.score(), &keys);
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
        if time_budget.expired() {
            result.termination = LocalSearchTermination::TimeLimit;
            return Ok(result);
        }
        if selection.selected.is_none() {
            result.termination = LocalSearchTermination::NoAcceptedMove;
            return Ok(result);
        }
    }
}
