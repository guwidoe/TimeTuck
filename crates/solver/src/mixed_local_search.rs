//! Bounded local search under one objective over basic variables and ordered lists.
use crate::mixed_search_scoring::MixedSearchDirector;
use crate::{
    AcceptanceError, AdditiveScore, LocalSearchConfig, LocalSearchTermination, MixedAssignment,
    MixedConstraintNetwork, MixedProblem, MixedScoreDirector, MixedScoringError, MoveOrder,
    ScoredMixedAssignment,
};
use crate::{
    mixed_neighborhood::{ordered, tabu_keys},
    random::SplitMix64,
};
use std::{error::Error, fmt};

/// Shared search controls, plus optional simultaneous basic/list candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedLocalSearchConfig<'a, S = crate::SimpleScore> {
    pub search: LocalSearchConfig<'a, S>,
    /// Include the Cartesian product after single-component candidates. This may
    /// escape coupled local traps but can make neighborhoods much larger.
    pub joint_moves: bool,
}
impl<S> Default for MixedLocalSearchConfig<'_, S> {
    fn default() -> Self {
        Self {
            search: LocalSearchConfig::default(),
            joint_moves: false,
        }
    }
}
#[derive(Debug)]
pub enum MixedLocalSearchError {
    DiminishedReturns(crate::DiminishedReturnsError),
    Scoring(MixedScoringError),
    StreamScoring(crate::MixedStreamScoringError),
    Acceptance(AcceptanceError),
    IncompleteInitialAssignment,
}
impl fmt::Display for MixedLocalSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoring(e) => e.fmt(f),
            Self::StreamScoring(e) => e.fmt(f),
            Self::Acceptance(e) => e.fmt(f),
            Self::DiminishedReturns(e) => e.fmt(f),
            Self::IncompleteInitialAssignment => {
                f.write_str("mixed local search requires a complete initial assignment")
            }
        }
    }
}
impl Error for MixedLocalSearchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoring(e) => Some(e),
            Self::StreamScoring(e) => Some(e),
            Self::Acceptance(e) => Some(e),
            Self::DiminishedReturns(e) => Some(e),
            Self::IncompleteInitialAssignment => None,
        }
    }
}
impl From<MixedScoringError> for MixedLocalSearchError {
    fn from(e: MixedScoringError) -> Self {
        Self::Scoring(e)
    }
}
impl From<crate::MixedStreamScoringError> for MixedLocalSearchError {
    fn from(e: crate::MixedStreamScoringError) -> Self {
        Self::StreamScoring(e)
    }
}
impl From<AcceptanceError> for MixedLocalSearchError {
    fn from(e: AcceptanceError) -> Self {
        Self::Acceptance(e)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedLocalSearchResult<B, L, S> {
    pub best: Option<ScoredMixedAssignment<B, L, S>>,
    pub evaluations: u64,
    pub accepted_moves: u64,
    pub completed_steps: u64,
    /// Completed steps since the last strict improvement of the retained best.
    pub unimproved_steps: u64,
    pub constraint_evaluations: u128,
    pub termination: LocalSearchTermination,
}

/// Improve a complete combined assignment with one shared evaluation/step budget.
/// The initial rescore counts once, each trial once. First/best accepted selection,
/// all native acceptance policies and optional best-rejected fallback are supported.
/// Saved winners install without rescoring; the earliest global best survives
/// worsening steps. No termination proves global optimality or hard feasibility.
///
/// Basic changes/swaps precede list moves; optional joint moves follow both.
/// ShuffledMoves reorders the combined basic/list/joint candidate stream within a
/// bounded rolling window. ShuffledVariables independently shuffles basic variables and list owners using
/// one seeded stream, preserving family/value/index order. VariableTabu gives basic
/// variables and list owners disjoint keys; joint moves touch both sets.
/// Invalid model shape, pins, completeness or acceptance reject even at zero budget.
pub fn mixed_local_search_from<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    mixed_local_search_from_monitored(None, problem, network, initial, settings)
}

pub(crate) fn mixed_local_search_from_monitored<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    settings.search.validate_acceptance()?;
    network.validate(problem, &initial)?;
    search_impl_monitored(shared, problem, initial, settings, |initial| {
        MixedScoreDirector::new(problem, network, initial)
    })
}

pub(crate) fn search_impl_monitored<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError>
where
    MixedLocalSearchError: From<D::Error>,
{
    let clock = crate::time_limit::monotonic_clock(
        settings.search.max_time.is_some()
            || settings.search.max_unimproved_time.is_some()
            || settings.search.diminished_returns.is_some()
            || settings.search.significant_improvement.is_some(),
    );
    search_impl_with_clock_monitored(shared, problem, initial, settings, make_director, clock)
}

#[cfg(test)]
pub(crate) fn search_impl_with_clock<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError>
where
    MixedLocalSearchError: From<D::Error>,
{
    search_impl_with_clock_monitored(None, problem, initial, settings, make_director, clock)
}

pub(crate) fn search_impl_with_clock_monitored<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
    clock: impl FnMut() -> std::time::Duration,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError>
where
    MixedLocalSearchError: From<D::Error>,
{
    let config = settings.search;
    let joint_moves = settings.joint_moves;
    let mut time_budget = crate::diminished_returns::LocalTimeBudget::new(&config, clock, shared)?;
    config.validate_acceptance()?;
    problem.validate(&initial).map_err(D::Error::from)?;
    if !problem.is_complete(&initial) {
        return Err(MixedLocalSearchError::IncompleteInitialAssignment);
    }
    let mut result = MixedLocalSearchResult {
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
    let mut acceptor = config.start_acceptance(
        director.score(),
        problem.basic().variables().len() + problem.lists().entities().len(),
    )?;
    let mut random = match config.move_order {
        MoveOrder::Original => None,
        MoveOrder::ShuffledVariables { seed } | MoveOrder::ShuffledMoves { seed, .. } => {
            Some(SplitMix64::new(seed))
        }
    };
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_constraint_evaluations();
    result.best = Some(ScoredMixedAssignment {
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
        if let Some(reason) = time_budget.stop(&result.best.as_ref().expect("initial best").score) {
            result.termination = reason;
            return Ok(result);
        }
        let snapshot = director.assignment().clone();
        let mut basic_order: Vec<_> = (0..problem.basic().variables().len()).collect();
        let mut list_order: Vec<_> = (0..problem.lists().entities().len()).collect();
        if matches!(config.move_order, MoveOrder::ShuffledVariables { .. })
            && let Some(random) = &mut random
        {
            random.shuffle(&mut basic_order);
            random.shuffle(&mut list_order);
        }
        let moves = ordered(
            problem,
            &snapshot,
            config.neighborhood,
            joint_moves,
            basic_order.iter().copied(),
            list_order.iter().copied(),
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
                .take_while(|_| {
                    !config.cancelled()
                        && !time_budget.expired(&result.best.as_ref().expect("initial best").score)
                }),
            budget.remaining,
            config.forager(
                current.clone(),
                result.best.as_ref().expect("initial best").score.clone(),
                progress.completed_steps(),
            ),
            |movement, score| {
                let keys = if tracks_entities {
                    tabu_keys(movement, problem.basic().variables().len())
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
            |_| false,
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
                result.best = Some(ScoredMixedAssignment {
                    assignment: director.assignment().clone(),
                    score: director.score(),
                });
            }
            let keys = if tracks_entities {
                tabu_keys(movement, problem.basic().variables().len())
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
            time_budget.complete_step(improved, &result.best.as_ref().expect("initial best").score);
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
        if let Some(reason) = time_budget.stop(&result.best.as_ref().expect("initial best").score) {
            result.termination = reason;
            return Ok(result);
        }
        if selection.selected.is_none() {
            result.termination = LocalSearchTermination::NoAcceptedMove;
            return Ok(result);
        }
    }
}

impl<B, L, S> MixedLocalSearchResult<B, L, S> {
    /// Candidate evaluations, including rejected trials; initialization excluded.
    pub fn move_evaluations(&self) -> u64 {
        self.evaluations.saturating_sub(1)
    }
}

impl From<crate::DiminishedReturnsError> for MixedLocalSearchError {
    fn from(e: crate::DiminishedReturnsError) -> Self {
        Self::DiminishedReturns(e)
    }
}
