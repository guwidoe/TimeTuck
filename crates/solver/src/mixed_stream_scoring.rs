//! Transactional stream scoring across basic variables and ordered lists.
//!
//! All three planning sources must start empty; other sources are immutable facts.
//! Rows include unassigned basics/values and empty lists. A combined move delivers
//! final basic rows, final entity lists, then affected value/shadow rows. Callbacks
//! must handle intermediate joins; only the fully delivered candidate is final.
//!
//! ```
//! use timetuck::*;
//! let p = MixedProblem::new(
//!     FiniteProblem::new(vec![Variable::new(vec![1_i64,2]).with_initial(1)])?,
//!     ListProblem::new(vec![7], vec![ListEntity::new(vec![7])])?,
//! );
//! let mut b = StreamGraphBuilder::<SimpleScore>::new();
//! let sources = MixedStreamSources::new(&mut b);
//! let joined = b.join_equal(sources.basic.port(), sources.lists.entities.port(),
//!     |(i,_)| *i, |(i,_)| *i)?;
//! b.score("load", &joined, |((_,v),(_,list))| SimpleScore::new(v.unwrap_or(0)*list.len() as i64))?;
//! let mut d = MixedStreamScoreDirector::new(&p,b.build(),sources,p.initial_assignment())?;
//! d.apply(&MixedMove {basic: Move::Change {variable:0,value:Some(2)},
//!     lists: ListMove::Composite(vec![])})?.commit();
//! assert_eq!(d.score(), SimpleScore::new(2));
//! d.assert_consistent()?;
//! # Ok::<(),Box<dyn std::error::Error>>(())
//! ```
use crate::mixed_scoring::{MixedSelection, with_structure};
use crate::mixed_search_scoring::MixedSearchDirector;
use crate::{
    AdditiveScore, AppliedMixedMove, GraphError, GraphTransaction, ListAssignment,
    ListStreamSources, ListValueRow, MixedAssignment, MixedConstructionConfig,
    MixedConstructionResult, MixedError, MixedLocalSearchConfig, MixedLocalSearchError,
    MixedLocalSearchResult, MixedMove, MixedProblem, MixedScoringError, PlanningStreamSource,
    StreamGraph, StreamGraphBuilder, TupleChange,
};
use std::{error::Error, fmt};

#[derive(Clone)]
pub struct MixedStreamSources<B, L> {
    pub basic: PlanningStreamSource<B>,
    pub lists: ListStreamSources<L>,
}
impl<B: Clone + 'static, L: Clone + 'static> MixedStreamSources<B, L> {
    pub fn new<S: AdditiveScore + 'static>(builder: &mut StreamGraphBuilder<'_, S>) -> Self {
        Self {
            basic: builder.source(),
            lists: ListStreamSources::new(builder),
        }
    }
    fn validate<S: AdditiveScore>(
        &self,
        graph: &StreamGraph<'_, S>,
    ) -> Result<(), MixedStreamScoringError> {
        if !graph.source_rows(&self.basic)?.is_empty()
            || !graph.source_rows(&self.lists.entities)?.is_empty()
            || !graph.source_rows(&self.lists.values)?.is_empty()
        {
            return Err(MixedStreamScoringError::PlanningSourceNotEmpty);
        }
        Ok(())
    }
}
#[derive(Debug)]
pub enum MixedStreamScoringError {
    Scoring(MixedScoringError),
    Graph(GraphError),
    PlanningSourceNotEmpty,
}
impl fmt::Display for MixedStreamScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoring(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::PlanningSourceNotEmpty => {
                f.write_str("mixed planning sources must be empty before binding")
            }
        }
    }
}
impl Error for MixedStreamScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoring(e) => Some(e),
            Self::Graph(e) => Some(e),
            Self::PlanningSourceNotEmpty => None,
        }
    }
}
impl From<MixedError> for MixedStreamScoringError {
    fn from(e: MixedError) -> Self {
        Self::Scoring(e.into())
    }
}
impl From<MixedScoringError> for MixedStreamScoringError {
    fn from(e: MixedScoringError) -> Self {
        Self::Scoring(e)
    }
}
impl From<GraphError> for MixedStreamScoringError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

pub struct MixedStreamScoreDirector<'p, 'a, B, L, S: AdditiveScore> {
    problem: &'p MixedProblem<B, L>,
    assignment: MixedAssignment<B, L>,
    graph: StreamGraph<'a, S>,
    sources: MixedStreamSources<B, L>,
    initial_constraint_evaluations: u128,
}
impl<'p, 'a, B: Eq + Clone + 'static, L: Eq + Clone + 'static, S: AdditiveScore + 'static>
    MixedStreamScoreDirector<'p, 'a, B, L, S>
{
    pub fn new(
        problem: &'p MixedProblem<B, L>,
        mut graph: StreamGraph<'a, S>,
        sources: MixedStreamSources<B, L>,
        assignment: MixedAssignment<B, L>,
    ) -> Result<Self, MixedStreamScoringError> {
        problem.validate(&assignment)?;
        sources.validate(&graph)?;
        with_structure(S::ZERO, problem, &assignment)?;
        let mut tx = graph.transaction();
        for (i, v) in assignment.basic().values().iter().enumerate() {
            tx.apply(&sources.basic, TupleChange::put(i, (i, v.clone())))?;
        }
        for (i, list) in assignment.lists().lists().iter().enumerate() {
            tx.apply(
                &sources.lists.entities,
                TupleChange::put(i, (i, list.clone())),
            )?;
        }
        for (i, v) in problem.lists().values().iter().enumerate() {
            tx.apply(
                &sources.lists.values,
                TupleChange::put(i, ListValueRow::from_assignment(i, v, assignment.lists())),
            )?;
        }
        let initial_constraint_evaluations = tx.score_evaluations();
        tx.commit()?;
        Ok(Self {
            problem,
            assignment,
            graph,
            sources,
            initial_constraint_evaluations,
        })
    }
    pub fn assignment(&self) -> &MixedAssignment<B, L> {
        &self.assignment
    }
    pub fn graph(&self) -> &StreamGraph<'a, S> {
        &self.graph
    }
    pub fn score(&self) -> S {
        with_structure(self.graph.score(), self.problem, &self.assignment)
            .expect("structural count checked before binding or installing a move")
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.graph.constraint_scores()
    }
    pub fn assert_consistent(&mut self) -> Result<(), MixedStreamScoringError> {
        self.graph.assert_consistent()?;
        Ok(())
    }
    /// Validate both final components before graph callbacks. Deliver each touched
    /// basic/entity once and each value in a touched list before/after once. Drop,
    /// errors and ordinary unwind restore both components and all graph caches.
    pub fn apply(
        &mut self,
        movement: &MixedMove<B, L>,
    ) -> Result<MixedStreamScoredMove<'_, 'a, B, L, S>, MixedStreamScoringError> {
        let assignment = self.problem.apply(&mut self.assignment, movement)?;
        with_structure(S::ZERO, self.problem, &assignment)?;
        let mut graph = self.graph.transaction();
        for i in movement.basic.touched_variables() {
            graph.apply(
                &self.sources.basic,
                TupleChange::put(i, (i, assignment.basic().values()[i].clone())),
            )?;
        }
        let touched = movement.lists.touched_entities();
        for &i in &touched {
            graph.apply(
                &self.sources.lists.entities,
                TupleChange::put(i, (i, assignment.lists().lists()[i].clone())),
            )?;
        }
        for (i, v) in self.problem.lists().values().iter().enumerate() {
            let belongs = |a: &ListAssignment<L>| {
                a.location(v)
                    .is_some_and(|p| touched.binary_search(&p.entity).is_ok())
            };
            if belongs(assignment.original().lists()) || belongs(assignment.lists()) {
                graph.apply(
                    &self.sources.lists.values,
                    TupleChange::put(i, ListValueRow::from_assignment(i, v, assignment.lists())),
                )?;
            }
        }
        let score = with_structure(graph.score()?, self.problem, &assignment)?;
        Ok(MixedStreamScoredMove {
            assignment,
            graph,
            score,
        })
    }
}
#[must_use = "dropping a mixed scored stream move restores both components and graph"]
pub struct MixedStreamScoredMove<'g, 'a, B, L, S: AdditiveScore> {
    assignment: AppliedMixedMove<'g, B, L>,
    graph: GraphTransaction<'g, 'a, S>,
    score: S,
}
impl<B, L, S: AdditiveScore> MixedStreamScoredMove<'_, '_, B, L, S> {
    pub fn assignment(&self) -> &MixedAssignment<B, L> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score
    }
    pub fn graph(&self) -> &StreamGraph<'_, S> {
        self.graph.graph().expect("successfully scored transaction")
    }
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
struct Saved<B, L, S> {
    accepted: bool,
    movement: MixedMove<B, L>,
    score: S,
    assignment: MixedAssignment<B, L>,
    graph: crate::stream_graph::GraphState<S>,
}
impl<B: Eq + Clone + 'static, L: Eq + Clone + 'static, S: AdditiveScore + 'static>
    MixedSearchDirector<B, L, S> for MixedStreamScoreDirector<'_, '_, B, L, S>
{
    type Error = MixedStreamScoringError;
    fn assignment(&self) -> &MixedAssignment<B, L> {
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
        moves: impl IntoIterator<Item = MixedMove<B, L>>,
        max_evaluations: u64,
        allow_rejected: bool,
        mut accepts: impl FnMut(&MixedMove<B, L>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<MixedSelection<B, L>, Self::Error> {
        let mut result = MixedSelection {
            selected: None,
            selected_accepted: false,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut best: Option<Saved<B, L, S>> = None;
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
            if !accepted && (!allow_rejected || candidate.score().structural() < 0) {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|saved| (accepted, candidate.score()) > (saved.accepted, saved.score))
            {
                best = Some(Saved {
                    accepted,
                    movement: movement.clone(),
                    score: candidate.score(),
                    assignment: candidate.assignment().clone(),
                    graph: candidate.graph.capture()?,
                });
            }
            if accepted && quit_early(candidate.score()) {
                break;
            }
        }
        if let Some(saved) = best {
            self.graph.install(saved.graph);
            self.assignment = saved.assignment;
            result.selected = Some(saved.movement);
            result.selected_accepted = saved.accepted;
        }
        Ok(result)
    }
}

/// Construct a partial mixed model using transactional streams and the shared
/// mixed construction algorithm. Counts terminal weighting calls, including
/// intermediate joins, but excludes graph preprocessing and diagnostics.
pub fn greedy_mixed_stream_construction<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    settings: MixedConstructionConfig,
) -> Result<MixedConstructionResult<B, L, S>, MixedStreamScoringError> {
    sources.validate(&graph)?;
    crate::mixed_construction::construction_impl(
        problem,
        problem.initial_assignment(),
        settings,
        |initial| MixedStreamScoreDirector::new(problem, graph, sources, initial),
    )
}
/// Improve a complete mixed model using the shared neighborhoods, policies and
/// budgets. Each phase initializes its own director; saved winners install without
/// callback replay. Empty-source ownership validates even at zero budget.
pub fn mixed_stream_local_search_from<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    settings.search.acceptance.validate()?;
    sources.validate(&graph)?;
    crate::mixed_local_search::search_impl(problem, initial, settings, |initial| {
        MixedStreamScoreDirector::new(problem, graph, sources, initial)
    })
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::{
        FiniteProblem, ListEntity, ListMove, ListPosition, ListProblem, Move, SimpleScore, Variable,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};
    #[test]
    fn failed_scan_after_saved_joint_candidate_restores_base_graph_and_assignment() {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap(),
            ListProblem::new(
                vec![7],
                vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
            )
            .unwrap(),
        );
        for failure in 0..3 {
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::new(&mut b);
            let inspect = s.clone();
            let j = b
                .join_equal(s.basic.port(), s.lists.values.port(), |_| 0, |_| 0)
                .unwrap();
            b.score("coupled", &j, |((_, v), row)| {
                if *v == Some(2) && row.position.unwrap().entity == 1 {
                    if failure == 0 {
                        return SimpleScore::ZERO.with_structural(1);
                    }
                    assert_ne!(failure, 1, "scoring panic");
                }
                SimpleScore::new(i64::from(v.unwrap_or(0)))
            })
            .unwrap();
            let mut d =
                MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
            let moves = [1, 2].map(|v| MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(v),
                },
                lists: ListMove::Change {
                    source: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                    destination: ListPosition {
                        entity: 1,
                        index: 0,
                    },
                },
            });
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    moves,
                    2,
                    false,
                    |_, score| {
                        assert!(
                            !(failure == 2 && score == SimpleScore::new(2)),
                            "acceptor panic"
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
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.score(), SimpleScore::ZERO);
            assert_eq!(
                d.graph().source_rows(&inspect.basic).unwrap()[&0],
                (0, Some(0))
            );
            assert_eq!(
                d.graph().source_rows(&inspect.lists.values).unwrap()[&0].position,
                Some(ListPosition {
                    entity: 0,
                    index: 0
                })
            );
            d.assert_consistent().unwrap();
        }
    }
}
