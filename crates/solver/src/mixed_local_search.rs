//! Bounded local search under one objective over basic variables and ordered lists.
use crate::mixed_search_scoring::MixedSearchDirector;
use crate::{
    Acceptance, AcceptanceError, AdditiveScore, LocalSearchConfig, LocalSearchTermination,
    MixedAssignment, MixedConstraintNetwork, MixedProblem, MixedScoreDirector, MixedScoringError,
    MoveOrder, MoveSelection, RejectedMovePolicy, ScoredMixedAssignment,
};
use crate::{
    mixed_neighborhood::{ordered, tabu_keys},
    random::SplitMix64,
};
use std::{error::Error, fmt};

/// Shared search controls, plus optional simultaneous basic/list candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedLocalSearchConfig<S = crate::SimpleScore> {
    pub search: LocalSearchConfig<S>,
    /// Include the Cartesian product after single-component candidates. This may
    /// escape coupled local traps but can make neighborhoods much larger.
    pub joint_moves: bool,
}
impl<S> Default for MixedLocalSearchConfig<S> {
    fn default() -> Self {
        Self {
            search: LocalSearchConfig::default(),
            joint_moves: false,
        }
    }
}
#[derive(Debug)]
pub enum MixedLocalSearchError {
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
/// ShuffledVariables independently shuffles basic variables and list owners using
/// one seeded stream, preserving family/value/index order. VariableTabu gives basic
/// variables and list owners disjoint keys; joint moves touch both sets.
/// Invalid model shape, pins, completeness or acceptance reject even at zero budget.
pub fn mixed_local_search_from<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    settings.search.acceptance.validate()?;
    network.validate(problem, &initial)?;
    search_impl(problem, initial, settings, |initial| {
        MixedScoreDirector::new(problem, network, initial)
    })
}
pub(crate) fn search_impl<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    make_director: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError>
where
    MixedLocalSearchError: From<D::Error>,
{
    let config = settings.search;
    let joint_moves = settings.joint_moves;
    config.acceptance.validate()?;
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
    let mut director = make_director(initial)?;
    let mut acceptor = config.acceptance.start(director.score())?;
    let mut random = match config.move_order {
        MoveOrder::Original => None,
        MoveOrder::ShuffledVariables { seed } => Some(SplitMix64::new(seed)),
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
        let snapshot = director.assignment().clone();
        let mut basic_order: Vec<_> = (0..problem.basic().variables().len()).collect();
        let mut list_order: Vec<_> = (0..problem.lists().entities().len()).collect();
        if let Some(random) = &mut random {
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
        let tracks_entities = matches!(config.acceptance, Acceptance::VariableTabu { .. });
        let selection = director.select_best(
            moves,
            config.max_evaluations - result.evaluations,
            config.rejected_moves == RejectedMovePolicy::BestScore,
            |movement, score| {
                let keys = if tracks_entities {
                    tabu_keys(movement, problem.basic().variables().len())
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
        if selection.selected.is_none() {
            result.termination = LocalSearchTermination::NoAcceptedMove;
            return Ok(result);
        }
    }
}
