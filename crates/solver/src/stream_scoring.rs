//! Bind a transactional stream graph to finite basic-variable moves.
//!
//! Planning rows have stable variable-index IDs and `(index, Option<value>)`
//! payloads. All variables, including unassigned ones, are present. Constraints
//! explicitly filter unassigned rows. Other graph sources are immutable during
//! the scoring session. Stream construction/local-search entry points share the
//! scoped solver algorithms, acceptance policies, budgets and termination rules.
//!
//! ```
//! use timetuck::{FiniteProblem, Variable, SimpleScore, StreamGraphBuilder,
//!     StreamScoreDirector, Move};
//! let problem = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2])])?;
//! let mut builder = StreamGraphBuilder::<SimpleScore>::new();
//! let source = builder.source::<usize, (usize, Option<i64>)>();
//! builder.score("value", source.port(), |(_, value)| SimpleScore::new(value.unwrap_or(0)))?;
//! let mut director = StreamScoreDirector::new(
//!     &problem, builder.build(), source, problem.initial_assignment())?;
//! assert_eq!(director.score(), SimpleScore::ZERO.with_structural(-1));
//! {
//!     let trial = director.apply(&Move::Change { variable: 0, value: Some(2) })?;
//!     assert_eq!(trial.score(), SimpleScore::new(2));
//! } // Both the assignment and graph roll back.
//! assert_eq!(director.assignment().values(), &[None]);
//! director.apply(&Move::Change { variable: 0, value: Some(1) })?.commit();
//! assert_eq!(director.score(), SimpleScore::new(1));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::StreamBatch;
use crate::scoring::with_structure;
use crate::{
    AdditiveScore, AppliedMove, Assignment, FiniteProblem, GraphError, GraphTransaction,
    ModelError, Move, StreamGraph, StreamSource, TupleChange,
};
use std::{error::Error, fmt};

pub type PlanningStreamSource<V> = StreamSource<usize, (usize, Option<V>)>;

#[derive(Debug)]
pub enum StreamScoringError {
    Model(ModelError),
    Graph(GraphError),
    PlanningSourceNotEmpty,
}
impl fmt::Display for StreamScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::PlanningSourceNotEmpty => {
                f.write_str("planning stream source must be empty before binding")
            }
        }
    }
}
impl Error for StreamScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            Self::Graph(e) => Some(e),
            Self::PlanningSourceNotEmpty => None,
        }
    }
}
impl From<ModelError> for StreamScoringError {
    fn from(e: ModelError) -> Self {
        Self::Model(e)
    }
}
impl From<GraphError> for StreamScoringError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

pub struct StreamScoreDirector<'p, 'a, V, S: AdditiveScore> {
    problem: &'p FiniteProblem<V>,
    assignment: Assignment<V>,
    graph: StreamGraph<'a, S>,
    source: PlanningStreamSource<V>,
    initial_constraint_evaluations: u128,
}
impl<'p, 'a, V: Eq + Clone + 'static, S: AdditiveScore + 'static>
    StreamScoreDirector<'p, 'a, V, S>
{
    pub fn new(
        problem: &'p FiniteProblem<V>,
        mut graph: StreamGraph<'a, S>,
        source: PlanningStreamSource<V>,
        assignment: Assignment<V>,
    ) -> Result<Self, StreamScoringError> {
        problem.validate(&assignment)?;
        if !graph.source_rows(&source)?.is_empty() {
            return Err(StreamScoringError::PlanningSourceNotEmpty);
        }
        let mut transaction = graph.transaction();
        let mut updates = StreamBatch::new();
        for (index, value) in assignment.values().iter().enumerate() {
            updates.push(&source, TupleChange::put(index, (index, value.clone())))?;
        }
        transaction.apply_updates(updates)?;
        let initial_constraint_evaluations = transaction.score_evaluations();
        transaction.commit()?;
        Ok(Self {
            problem,
            assignment,
            graph,
            source,
            initial_constraint_evaluations,
        })
    }
    pub fn assert_consistent(&mut self) -> Result<(), StreamScoringError> {
        self.graph.assert_consistent()?;
        Ok(())
    }
    /// Begin a live multi-move scope. Drop restores assignment and cached scores;
    /// commit retains edits. No scoring callbacks run to create or restore it.
    pub fn edit_session(&mut self) -> Result<crate::MoveSession<'_, Self>, StreamScoringError> {
        let assignment = self.assignment.clone();
        let graph = self.graph.capture_full();
        Ok(crate::MoveSession::new(self, move |director| {
            director.assignment = assignment;
            director.graph.install(graph);
        }))
    }
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
    pub fn graph(&self) -> &StreamGraph<'a, S> {
        &self.graph
    }
    pub fn score(&self) -> S {
        with_structure(self.graph.score(), self.problem, &self.assignment)
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.graph.constraint_scores()
    }

    /// Validate the final composite assignment, then deliver each touched variable
    /// once using its final value. Drop/ordinary unwind restore both assignments
    /// and graph state. Commit performs no callback replay.
    pub fn apply(
        &mut self,
        movement: &Move<V>,
    ) -> Result<StreamScoredMove<'_, 'a, V, S>, StreamScoringError> {
        let assignment = self.problem.apply(&mut self.assignment, movement)?;
        let mut graph = self.graph.transaction();
        let mut updates = StreamBatch::new();
        for variable in movement.touched_variables() {
            updates.push(
                &self.source,
                TupleChange::put(variable, (variable, assignment.values()[variable].clone())),
            )?;
        }
        graph.apply_updates(updates)?;
        let score = with_structure(graph.score()?, self.problem, &assignment);
        Ok(StreamScoredMove {
            assignment,
            graph,
            score,
        })
    }
}

#[must_use = "dropping a scored stream move restores the assignment and graph"]
pub struct StreamScoredMove<'g, 'a, V, S: AdditiveScore> {
    assignment: AppliedMove<'g, V>,
    graph: GraphTransaction<'g, 'a, S>,
    score: S,
}
impl<V, S: AdditiveScore> StreamScoredMove<'_, '_, V, S> {
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score.clone()
    }
    pub fn graph(&self) -> &StreamGraph<'_, S> {
        self.graph.graph().expect("successfully scored transaction")
    }
    /// Number of terminal scoring callbacks invoked for this candidate.
    pub fn constraint_evaluations(&self) -> u128 {
        self.graph.score_evaluations()
    }
    pub fn checkpoint_count(&self) -> usize {
        self.graph.checkpoint_count()
    }
    pub fn commit(self) {
        self.graph
            .commit()
            .expect("successfully scored transaction");
        self.assignment.commit();
    }
}

use crate::scoring::Selection;
use crate::search_scoring::SearchDirector;
use crate::{
    ConstructionConfig, ConstructionResult, LocalSearchConfig, LocalSearchError, LocalSearchResult,
};

impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static> SearchDirector<V, S>
    for StreamScoreDirector<'_, '_, V, S>
{
    type Error = StreamScoringError;
    fn assignment(&self) -> &Assignment<V> {
        self.assignment()
    }
    fn score(&self) -> S {
        self.score()
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        self.initial_constraint_evaluations
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        mut accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<Selection<V>, Self::Error> {
        let mut result = Selection {
            selected_accepted: false,
            selected: None,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut forager = crate::MoveForager::new(forager.into());
        for movement in moves {
            if result.evaluations == max_evaluations {
                result.evaluation_limit = true;
                break;
            }
            let mut candidate = self.apply(&movement)?;
            result.evaluations += 1;
            result.constraint_evaluations += candidate.constraint_evaluations();
            if assert_scores {
                candidate.graph.assert_consistent()?;
            }
            let accepted = accepts(&movement, candidate.score());
            forager.try_add_with(candidate.score(), accepted, || -> Result<_, Self::Error> {
                Ok(SavedStreamCandidate {
                    accepted,
                    movement: movement.clone(),
                    assignment: candidate.assignment().clone(),
                    graph: candidate.graph.capture()?,
                })
            })?;
            let stop =
                forager.is_quit_early() || (accepted && quit_early.should_pick(candidate.score()));
            drop(candidate);
            if assert_scores {
                self.assert_consistent()?;
            }
            if stop {
                break;
            }
        }
        // Every trial has already rolled back. Discarding the saved winner
        // leaves assignment, contribution caches and graph state at the last step.
        if !quit_early.can_commit(result.evaluation_limit) {
            return Ok(result);
        }
        if let Some(saved) = forager.pick() {
            self.graph.install(saved.graph);
            self.assignment = saved.assignment;
            result.selected = Some(saved.movement);
            result.selected_accepted = saved.accepted;
        }
        Ok(result)
    }
}
struct SavedStreamCandidate<V, S> {
    accepted: bool,
    movement: Move<V>,
    assignment: Assignment<V>,
    graph: crate::stream_graph::GraphState<S>,
}
pub(crate) fn validate_source<V: 'static, S: AdditiveScore>(
    graph: &StreamGraph<'_, S>,
    source: &PlanningStreamSource<V>,
) -> Result<(), StreamScoringError> {
    if !graph.source_rows(source)?.is_empty() {
        return Err(StreamScoringError::PlanningSourceNotEmpty);
    }
    Ok(())
}
/// Greedy construction using a persistent stream graph. The planning source must
/// be empty. Shares the scoped solver's selection, budget and termination rules.
/// Counts terminal score callbacks, excluding graph preparation and diagnostic
/// rebuilds. Candidate snapshots install without callback replay.
pub fn greedy_stream_construction<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    config: ConstructionConfig,
) -> Result<ConstructionResult<V, S>, StreamScoringError> {
    greedy_stream_construction_from(problem, graph, source, problem.initial_assignment(), config)
}

/// Resume construction from a validated assignment, preserving supplied values and pins.
/// Missing required and configured optional values are considered; each call
/// initializes its own scoring cache.
/// Validation runs even with zero budget or a cancelled token.
pub fn greedy_stream_construction_from<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: ConstructionConfig,
) -> Result<ConstructionResult<V, S>, StreamScoringError> {
    greedy_stream_construction_from_monitored(None, problem, graph, source, initial, config)
}

pub(crate) fn greedy_stream_construction_from_monitored<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: ConstructionConfig,
) -> Result<ConstructionResult<V, S>, StreamScoringError> {
    validate_source(&graph, &source)?;
    problem.validate(&initial)?;
    crate::construction::construction_impl_monitored(shared, problem, initial, config, |initial| {
        StreamScoreDirector::new(problem, graph, source, initial)
    })
}
/// First-fit initialization followed by bounded local search with stream scoring.
/// The same acceptance policies and move ordering as `local_search` apply.
pub fn stream_local_search<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    stream_search(None, problem, graph, source, None, config)
}
/// Improve an explicit complete assignment using a fresh stream scoring session.
/// Initialization counts once; graph preparation before this call is excluded.
pub fn stream_local_search_from<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    stream_local_search_from_monitored(None, problem, graph, source, initial, config)
}

pub(crate) fn stream_local_search_from_monitored<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    stream_search(shared, problem, graph, source, Some(initial), config)
}
fn stream_search<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Option<Assignment<V>>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.validate_acceptance()?;
    validate_source(&graph, &source)?;
    crate::local_search::search_impl_monitored(shared, problem, initial, config, |initial| {
        StreamScoreDirector::new(problem, graph, source, initial)
    })
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::{SimpleScore, Variable};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn failed_scan_keeps_base_after_saving_an_earlier_winner() {
        let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0)]).unwrap();
        for failure in 0..3 {
            let mut b = crate::StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<i64>)>();
            let inspect = s.clone();
            b.score("value", s.port(), move |(_, v)| {
                if *v == Some(2) {
                    if failure == 0 {
                        return SimpleScore::ZERO.with_structural(1);
                    }
                    if failure == 1 {
                        panic!("scoring failure");
                    }
                }
                SimpleScore::new(v.unwrap_or(0))
            })
            .unwrap();
            let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    [1, 1, 2].map(|v| Move::Change {
                        variable: 0,
                        value: Some(v),
                    }),
                    3,
                    crate::ForagerConfig {
                        tie_breaking: crate::TieBreaking::Random { seed: 7 },
                        ..Default::default()
                    },
                    |_, score| {
                        assert!(
                            !(failure == 2 && score == SimpleScore::new(2)),
                            "acceptor failure"
                        );
                        true
                    },
                    |_| false,
                    true,
                )
            }));
            if failure == 0 {
                assert!(outcome.unwrap().is_err());
            } else {
                assert!(outcome.is_err());
            }
            assert_eq!(d.assignment().values(), &[Some(0)]);
            assert_eq!(d.score(), SimpleScore::ZERO);
            assert_eq!(d.graph().source_rows(&inspect).unwrap()[&0], (0, Some(0)));
            d.assert_consistent().unwrap();
            let selected = d
                .select_best(
                    [Move::Change {
                        variable: 0,
                        value: Some(1),
                    }],
                    1,
                    false,
                    |_, _| true,
                    |_| false,
                    true,
                )
                .unwrap();
            assert!(selected.selected_accepted);
            assert_eq!(selected.evaluations, 1);
            assert_eq!(d.score(), SimpleScore::new(1));
            d.assert_consistent().unwrap();
        }
    }
}
