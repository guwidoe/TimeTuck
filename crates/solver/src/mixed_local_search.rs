//! Bounded local search under one objective over basic variables and ordered lists.
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
    Acceptance(AcceptanceError),
    IncompleteInitialAssignment,
}
impl fmt::Display for MixedLocalSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoring(e) => e.fmt(f),
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
    let config = settings.search;
    let joint_moves = settings.joint_moves;
    config.acceptance.validate()?;
    network.validate(problem, &initial)?;
    if !problem.is_complete(&initial) {
        return Err(MixedLocalSearchError::IncompleteInitialAssignment);
    }
    let mut result = MixedLocalSearchResult {
        best: None,
        evaluations: 0,
        accepted_moves: 0,
        completed_steps: 0,
        constraint_evaluations: 0,
        termination: LocalSearchTermination::EvaluationLimit,
    };
    if config.max_evaluations == 0 {
        return Ok(result);
    }
    let mut director = MixedScoreDirector::new(problem, network, initial)?;
    let mut acceptor = config.acceptance.start(director.score())?;
    let mut random = match config.move_order {
        MoveOrder::Original => None,
        MoveOrder::ShuffledVariables { seed } => Some(SplitMix64::new(seed)),
    };
    result.evaluations = 1;
    result.constraint_evaluations = network.constraint_count() as u128;
    result.best = Some(ScoredMixedAssignment {
        assignment: director.assignment().clone(),
        score: director.score(),
    });
    if config.assert_scores {
        director.assert_consistent()?;
    }
    loop {
        if result.accepted_moves == config.max_accepted_moves {
            result.termination = LocalSearchTermination::AcceptedMoveLimit;
            return Ok(result);
        }
        if result.completed_steps == config.max_steps {
            result.termination = LocalSearchTermination::StepLimit;
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
            if director.score() > result.best.as_ref().expect("initial best").score {
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
            result.completed_steps += 1;
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
