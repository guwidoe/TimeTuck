//! Transactional stream scoring for ordered-list variables and their shadow views.
//!
//! Both bound sources start empty. Every entity and registered value has a row,
//! including empty lists and unassigned values. Other sources are immutable facts.
//! Changes deliver final entity lists, then affected value views; callbacks may
//! see intermediate joins, but the completed candidate is consistent.
//!
//! ```
//! use timetuck::*;
//! let p = ListProblem::new(vec![10,20],vec![ListEntity::new(vec![10])])?;
//! let mut b = StreamGraphBuilder::<SimpleScore>::new();
//! let sources = ListStreamSources::new(&mut b);
//! let assigned = b.filter(sources.values.port(), |r| r.position.is_some())?;
//! b.score("assigned", &assigned, |_| SimpleScore::new(1))?;
//! let mut d = ListStreamScoreDirector::new(&p,b.build(),sources,p.initial_assignment())?;
//! assert_eq!(d.score(),SimpleScore::new(1).with_structural(-1));
//! d.apply(&ListMove::Assign {value:20,destination:ListPosition{entity:0,index:1}})?.commit();
//! assert_eq!(d.score(),SimpleScore::new(2));
//! d.assert_consistent()?;
//! # Ok::<(),Box<dyn std::error::Error>>(())
//! ```
use crate::list_scoring::{ListSelection, with_structure};
use crate::list_search_scoring::ListSearchDirector;
use crate::{
    AdditiveScore, AppliedListMove, ConstructionConfig, GraphError, GraphTransaction,
    ListAssignment, ListConstructionResult, ListError, ListLocalSearchError, ListLocalSearchResult,
    ListMove, ListPosition, ListProblem, LocalSearchConfig, StreamGraph, StreamGraphBuilder,
    StreamSource, TupleChange,
};
use std::{error::Error, fmt};

/// Stable range index is separate from value identity. Unassigned values have
/// no position or neighbors. Owned immutable snapshots follow model Eq semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListValueRow<V> {
    pub range_index: usize,
    pub value: V,
    pub position: Option<ListPosition>,
    pub previous: Option<V>,
    pub next: Option<V>,
}
impl<V: Eq + Clone> ListValueRow<V> {
    pub(crate) fn from_assignment(index: usize, value: &V, assignment: &ListAssignment<V>) -> Self {
        let element = assignment.element(value);
        Self {
            range_index: index,
            value: value.clone(),
            position: element.as_ref().map(|e| e.position),
            previous: element.as_ref().and_then(|e| e.previous.cloned()),
            next: element.as_ref().and_then(|e| e.next.cloned()),
        }
    }
}
#[derive(Clone)]
pub struct ListStreamSources<V> {
    /// Entity index -> (entity index, ordered list).
    pub entities: StreamSource<usize, (usize, Vec<V>)>,
    /// Value-range index -> identity and derived owner/index/neighbor snapshot.
    pub values: StreamSource<usize, ListValueRow<V>>,
}
impl<V: Clone + 'static> ListStreamSources<V> {
    pub fn new<S: AdditiveScore + 'static>(builder: &mut StreamGraphBuilder<'_, S>) -> Self {
        Self {
            entities: builder.source(),
            values: builder.source(),
        }
    }
    pub(crate) fn validate<S: AdditiveScore>(
        &self,
        graph: &StreamGraph<'_, S>,
    ) -> Result<(), ListStreamScoringError> {
        if !graph.source_rows(&self.entities)?.is_empty()
            || !graph.source_rows(&self.values)?.is_empty()
        {
            return Err(ListStreamScoringError::PlanningSourceNotEmpty);
        }
        Ok(())
    }
}
#[derive(Debug)]
pub enum ListStreamScoringError {
    Model(ListError),
    Graph(GraphError),
    PlanningSourceNotEmpty,
}
impl fmt::Display for ListStreamScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::PlanningSourceNotEmpty => {
                f.write_str("list planning sources must be empty before binding")
            }
        }
    }
}
impl Error for ListStreamScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            Self::Graph(e) => Some(e),
            Self::PlanningSourceNotEmpty => None,
        }
    }
}
impl From<ListError> for ListStreamScoringError {
    fn from(e: ListError) -> Self {
        Self::Model(e)
    }
}
impl From<GraphError> for ListStreamScoringError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

pub struct ListStreamScoreDirector<'p, 'a, V, S: AdditiveScore> {
    problem: &'p ListProblem<V>,
    assignment: ListAssignment<V>,
    graph: StreamGraph<'a, S>,
    sources: ListStreamSources<V>,
    initial_constraint_evaluations: u128,
}
impl<'p, 'a, V: Eq + Clone + 'static, S: AdditiveScore + 'static>
    ListStreamScoreDirector<'p, 'a, V, S>
{
    pub fn new(
        problem: &'p ListProblem<V>,
        mut graph: StreamGraph<'a, S>,
        sources: ListStreamSources<V>,
        assignment: ListAssignment<V>,
    ) -> Result<Self, ListStreamScoringError> {
        problem.validate(&assignment)?;
        sources.validate(&graph)?;
        let mut tx = graph.transaction();
        for (i, list) in assignment.lists().iter().enumerate() {
            tx.apply(&sources.entities, TupleChange::put(i, (i, list.clone())))?;
        }
        for (i, value) in problem.values().iter().enumerate() {
            tx.apply(
                &sources.values,
                TupleChange::put(i, ListValueRow::from_assignment(i, value, &assignment)),
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
    pub fn assignment(&self) -> &ListAssignment<V> {
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
    pub fn assert_consistent(&mut self) -> Result<(), ListStreamScoringError> {
        self.graph.assert_consistent()?;
        Ok(())
    }
    /// Validate the final composite before graph callbacks. Update each touched
    /// entity once and each value belonging to a touched list before or after
    /// the move once. Drop/error/ordinary unwind restores both model and graph.
    pub fn apply(
        &mut self,
        movement: &ListMove<V>,
    ) -> Result<ListStreamScoredMove<'_, 'a, V, S>, ListStreamScoringError> {
        let assignment = self.problem.apply(&mut self.assignment, movement)?;
        let touched = movement.touched_entities();
        let mut graph = self.graph.transaction();
        for &entity in &touched {
            graph.apply(
                &self.sources.entities,
                TupleChange::put(entity, (entity, assignment.lists()[entity].clone())),
            )?;
        }
        for (i, value) in self.problem.values().iter().enumerate() {
            let belongs = |a: &ListAssignment<V>| {
                a.location(value)
                    .is_some_and(|p| touched.binary_search(&p.entity).is_ok())
            };
            if belongs(assignment.original()) || belongs(&assignment) {
                graph.apply(
                    &self.sources.values,
                    TupleChange::put(i, ListValueRow::from_assignment(i, value, &assignment)),
                )?;
            }
        }
        let score = with_structure(graph.score()?, self.problem, &assignment);
        Ok(ListStreamScoredMove {
            assignment,
            graph,
            score,
        })
    }
}
#[must_use = "dropping a scored list stream move restores the assignment and graph"]
pub struct ListStreamScoredMove<'g, 'a, V, S: AdditiveScore> {
    assignment: AppliedListMove<'g, V>,
    graph: GraphTransaction<'g, 'a, S>,
    score: S,
}
impl<V, S: AdditiveScore> ListStreamScoredMove<'_, '_, V, S> {
    pub fn assignment(&self) -> &ListAssignment<V> {
        &self.assignment
    }
    pub fn graph(&self) -> &StreamGraph<'_, S> {
        self.graph.graph().expect("successfully scored transaction")
    }
    pub fn score(&self) -> S {
        self.score.clone()
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
struct Saved<V, S> {
    accepted: bool,
    movement: ListMove<V>,
    score: S,
    assignment: ListAssignment<V>,
    graph: crate::stream_graph::GraphState<S>,
}
impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static> ListSearchDirector<V, S>
    for ListStreamScoreDirector<'_, '_, V, S>
{
    type Error = ListStreamScoringError;
    fn assignment(&self) -> &ListAssignment<V> {
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
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        allow_rejected: bool,
        mut accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, Self::Error> {
        let mut result = ListSelection {
            selected: None,
            selected_accepted: false,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut best: Option<Saved<V, S>> = None;
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
                .is_none_or(|saved| (accepted, &candidate.score()) > (saved.accepted, &saved.score))
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
/// Greedy list insertion using stream scores. Both planning sources must be empty;
/// preloaded immutable facts are allowed. Counts terminal weighting calls only,
/// excluding preprocessing and diagnostic rebuilds. Winners install without replay.
pub fn greedy_list_stream_construction<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    config: ConstructionConfig,
) -> Result<ListConstructionResult<V, S>, ListStreamScoringError> {
    greedy_list_stream_construction_from(
        problem,
        graph,
        sources,
        problem.initial_assignment(),
        config,
    )
}

/// Resume construction from a validated assignment, preserving supplied values and pins.
/// Missing required and configured optional values are considered; each call
/// initializes its own scoring cache.
/// Validation runs even with zero budget or a cancelled token.
pub fn greedy_list_stream_construction_from<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
) -> Result<ListConstructionResult<V, S>, ListStreamScoringError> {
    sources.validate(&graph)?;
    crate::list_construction::construction_impl(problem, initial, config, |initial| {
        ListStreamScoreDirector::new(problem, graph, sources, initial)
    })
}
/// Improve a complete list assignment with the existing list search policies,
/// ordering and budgets. Initializes a fresh stream scoring session for this phase.
pub fn list_stream_local_search_from<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list_stream_local_search_from_monitored(None, problem, graph, sources, initial, config)
}

pub(crate) fn list_stream_local_search_from_monitored<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.acceptance.validate()?;
    sources.validate(&graph)?;
    crate::list_local_search::search_impl_monitored(shared, problem, initial, config, |initial| {
        ListStreamScoreDirector::new(problem, graph, sources, initial)
    })
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::{ListEntity, SimpleScore};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    #[test]
    fn failed_scan_restores_base_after_saving_a_list_candidate() {
        let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0])])
            .unwrap()
            .allowing_unassigned();
        for failure in 0..3 {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = ListStreamSources::new(&mut b);
            let inspect = s.clone();
            b.score("length", s.entities.port(), move |(_, v)| {
                if v.contains(&2) {
                    if failure == 0 {
                        return SimpleScore::ZERO.with_structural(1);
                    }
                    assert_ne!(failure, 1, "scorer failure");
                }
                SimpleScore::new(v.iter().map(|v| i64::from(*v)).sum())
            })
            .unwrap();
            let mut d =
                ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    [1, 2].map(|value| ListMove::Assign {
                        value,
                        destination: ListPosition {
                            entity: 0,
                            index: 1,
                        },
                    }),
                    2,
                    false,
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
                assert!(result.unwrap().is_err());
            } else {
                assert!(result.is_err());
            }
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.score(), SimpleScore::ZERO);
            assert_eq!(
                d.graph().source_rows(&inspect.entities).unwrap()[&0],
                (0, vec![0])
            );
            assert!(
                d.graph().source_rows(&inspect.values).unwrap()[&1]
                    .position
                    .is_none()
            );
            d.assert_consistent().unwrap();
            d.select_best(
                [ListMove::Assign {
                    value: 1,
                    destination: ListPosition {
                        entity: 0,
                        index: 1,
                    },
                }],
                1,
                false,
                |_, _| true,
                |_| false,
                true,
            )
            .unwrap();
            assert_eq!(d.assignment().lists(), &[vec![0, 1]]);
            d.assert_consistent().unwrap();
        }
    }
}
