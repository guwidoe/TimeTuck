//! Transactional stream scoring for ordered-list variables and their shadow views.
//!
//! Both bound sources start empty. Every entity and registered value has a row,
//! including empty lists and unassigned values. Other sources are facts, updated explicitly through `edit_facts`.
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
use crate::StreamBatch;
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
    #[cfg(test)]
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
        let mut updates = StreamBatch::new();
        for (i, list) in assignment.lists().iter().enumerate() {
            updates.push(&sources.entities, TupleChange::put(i, (i, list.clone())))?;
        }
        let locations = problem.value_locations(&assignment);
        for (i, value) in problem.values().iter().enumerate() {
            updates.push(
                &sources.values,
                TupleChange::put(
                    i,
                    ListValueRow::from_known_position(i, value, &assignment, locations[i]),
                ),
            )?;
        }
        tx.apply_updates(updates)?;
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
    /// Inspect a layout-valid candidate without changing this director or scoring it.
    pub fn inspect_move(
        &self,
        movement: &ListMove<V>,
    ) -> Result<crate::MoveInspection<ListAssignment<V>>, ListStreamScoringError> {
        Ok(crate::inspect_list_move(
            self.problem,
            &self.assignment,
            movement,
        )?)
    }
    /// Begin a live multi-move scope. Drop restores assignment and cached scores;
    /// commit retains edits. No scoring callbacks run to create or restore it.
    pub fn edit_session(&mut self) -> Result<crate::MoveSession<'_, Self>, ListStreamScoringError> {
        let assignment = self.assignment.clone();
        let graph = self.graph.capture_full();
        Ok(crate::MoveSession::new(self, move |director| {
            director.assignment = assignment;
            director.graph.install(graph);
        }))
    }
    /// Run construction from the current assignment using the retained graph.
    /// Cached initialization does not invoke callbacks or count as new constraint
    /// work; the phase still consumes its usual initial score evaluation.
    /// Errors or callback panics retain the last committed, consistent step.
    pub fn run_construction(
        &mut self,
        config: ConstructionConfig,
    ) -> Result<ListConstructionResult<V, S>, ListStreamScoringError> {
        self.run_construction_monitored(None, None, None, config)
    }

    pub(crate) fn run_construction_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<ListAssignment<V>>,
        mut observer: crate::retained_stream_phase::Observer<'_, ListAssignment<V>, S>,
        config: ConstructionConfig,
    ) -> Result<ListConstructionResult<V, S>, ListStreamScoringError> {
        let problem = self.problem;
        let initial = initial.unwrap_or_else(|| self.assignment.clone());
        let observed = observer.is_some();
        let mut notify = |director: &Self, complete| {
            if let Some(observer) = &mut observer {
                observer(crate::SolutionUpdate {
                    assignment: director.assignment(),
                    score: director.score(),
                    complete,
                });
            }
        };
        crate::list_construction::construction_impl_monitored(
            shared,
            problem,
            initial,
            config,
            |initial| {
                let initialization_work = self.prepare_retained_phase(initial)?;
                Ok(crate::retained_stream_phase::RetainedPhase {
                    director: self,
                    initialization_work,
                    on_best: observed.then_some(&mut notify),
                })
            },
        )
    }

    /// Run local search with fresh phase acceptance/random state and the retained
    /// scoring graph. On success, install the best assignment and cached graph
    /// without rescoring. Zero-budget/cancelled phases leave this director unchanged.
    /// Constraint work excludes the director's earlier initialization and diagnostics.
    /// Errors or callback panics retain the last committed, consistent step.
    pub fn run_local_search(
        &mut self,
        config: LocalSearchConfig<S>,
    ) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
        self.run_local_search_monitored(None, None, None, config, None)
    }

    /// Run local search with a custom move provider on this retained graph.
    /// Successful calls restore the best cached state; errors retain the last commit.
    pub fn run_local_search_with_provider(
        &mut self,
        config: LocalSearchConfig<S>,
        provider: &mut dyn crate::MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
    ) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
        self.run_local_search_monitored(None, None, None, config, Some(provider))
    }

    /// Construct from provider proposals without rebuilding this graph or its facts.
    pub fn run_construction_with_provider(
        &mut self,
        config: crate::ProviderConstructionConfig<'_>,
        provider: &mut dyn crate::MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
    ) -> Result<crate::ProviderConstructionResult<ListAssignment<V>, S>, ListStreamScoringError>
    {
        self.run_provider_construction_monitored(
            self.assignment.clone(),
            config,
            crate::provider_search::ProviderContext {
                provider,
                control: None,
                observer: None,
            },
        )
    }

    pub(crate) fn run_provider_construction_monitored(
        &mut self,
        initial: ListAssignment<V>,
        config: crate::ProviderConstructionConfig<'_>,
        context: crate::provider_search::ProviderContext<
            '_,
            ListProblem<V>,
            ListAssignment<V>,
            ListMove<V>,
            S,
        >,
    ) -> Result<crate::ProviderConstructionResult<ListAssignment<V>, S>, ListStreamScoringError>
    {
        crate::provider_construction::list_director(
            self.problem,
            initial,
            config,
            context,
            |initial| {
                let initialization_work = self.prepare_retained_phase(initial)?;
                Ok(crate::retained_stream_phase::RetainedPhase {
                    director: self,
                    initialization_work,
                    on_best: None,
                })
            },
        )
    }

    pub(crate) fn run_local_search_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<ListAssignment<V>>,
        mut observer: crate::retained_stream_phase::Observer<'_, ListAssignment<V>, S>,
        config: LocalSearchConfig<S>,
        provider: Option<
            &mut (dyn crate::MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>> + '_),
        >,
    ) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
        let problem = self.problem;
        let initial = initial.unwrap_or_else(|| self.assignment.clone());
        let mut best_graph = None;
        let mut save_best = |director: &Self, complete| {
            best_graph = Some(director.graph.capture_full());
            if let Some(observer) = &mut observer {
                observer(crate::SolutionUpdate {
                    assignment: director.assignment(),
                    score: director.score(),
                    complete,
                });
            }
        };
        let clock = crate::provider_search::clock(&config);
        let result = crate::list_local_search::search_impl_with_provider(
            shared,
            problem,
            initial,
            config,
            |initial| {
                let initialization_work = self.prepare_retained_phase(initial)?;
                Ok(crate::retained_stream_phase::RetainedPhase {
                    director: &mut *self,
                    initialization_work,
                    on_best: Some(&mut save_best),
                })
            },
            clock,
            provider.map(|p| {
                p as &mut dyn crate::MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
            }),
        )?;
        if let Some(best) = &result.best {
            self.assignment = best.assignment.clone();
            self.graph
                .install(best_graph.expect("scored phase records its initial best"));
        }
        Ok(result)
    }

    pub(crate) fn prepare_retained_phase(
        &mut self,
        initial: ListAssignment<V>,
    ) -> Result<u128, ListStreamScoringError> {
        // These inputs come from this solve's own assignment snapshots/preparation.
        if self.assignment == initial {
            return Ok(0);
        }
        let change = self.replace_assignment(initial)?;
        let work = change.constraint_evaluations();
        change.commit();
        Ok(work)
    }

    /// Run an ordered solver sequence on this retained graph, starting from its
    /// current assignment. Earlier initialization is outside this call's counters.
    pub fn run_solver<'c>(
        &mut self,
        config: impl Into<crate::SolverSequence<'c, S>>,
    ) -> Result<crate::SolverResult<ListAssignment<V>, S>, crate::SolverError<ListLocalSearchError>>
    {
        self.run_solver_with_extensions(config, crate::SolverExtensions::default())
    }
    /// Run a retained sequence with synchronous solver lifecycle events.
    pub fn run_solver_with_events<'c>(
        &mut self,
        config: impl Into<crate::SolverSequence<'c, S>>,
        mut observer: impl FnMut(crate::SolverEvent<'_, ListAssignment<V>, S>),
    ) -> Result<crate::SolverResult<ListAssignment<V>, S>, crate::SolverError<ListLocalSearchError>>
    {
        self.run_solver_with_extensions(
            config,
            crate::SolverExtensions {
                events: Some(&mut observer),
                ..Default::default()
            },
        )
    }
    /// Bind a typed command callback to custom phase indices in a retained sequence.
    pub fn run_solver_with_commands<'c>(
        &mut self,
        config: impl Into<crate::SolverSequence<'c, S>>,
        mut command: impl FnMut(
            usize,
            &mut crate::CustomPhaseContext<
                '_,
                ListAssignment<V>,
                ListMove<V>,
                S,
                ListStreamScoringError,
            >,
        ) -> Result<(), crate::CustomPhaseError<ListStreamScoringError>>,
    ) -> Result<crate::SolverResult<ListAssignment<V>, S>, crate::SolverError<ListLocalSearchError>>
    {
        self.run_solver_with_extensions(
            config,
            crate::SolverExtensions {
                command: Some(&mut command),
                ..Default::default()
            },
        )
    }
    /// Run retained custom phases with the usual solver lifecycle events.
    pub fn run_solver_with_commands_and_events<'c>(
        &mut self,
        config: impl Into<crate::SolverSequence<'c, S>>,
        mut command: impl FnMut(
            usize,
            &mut crate::CustomPhaseContext<
                '_,
                ListAssignment<V>,
                ListMove<V>,
                S,
                ListStreamScoringError,
            >,
        ) -> Result<(), crate::CustomPhaseError<ListStreamScoringError>>,
        mut observer: impl FnMut(crate::SolverEvent<'_, ListAssignment<V>, S>),
    ) -> Result<crate::SolverResult<ListAssignment<V>, S>, crate::SolverError<ListLocalSearchError>>
    {
        self.run_solver_with_extensions(
            config,
            crate::SolverExtensions {
                command: Some(&mut command),
                events: Some(&mut observer),
                ..Default::default()
            },
        )
    }
    /// Run retained phases with move providers, commands, events and independent
    /// phase score checks. A shared provider receives one solve lifecycle.
    pub fn run_solver_with_extensions<'c>(
        &mut self,
        config: impl Into<crate::SolverSequence<'c, S>>,
        extensions: crate::SolverExtensions<
            '_,
            ListProblem<V>,
            ListAssignment<V>,
            ListMove<V>,
            S,
            ListStreamScoringError,
        >,
    ) -> Result<crate::SolverResult<ListAssignment<V>, S>, crate::SolverError<ListLocalSearchError>>
    {
        crate::solver::retained::list(self.problem, self, config.into(), extensions)
    }
    pub(crate) fn install_retained_state(
        &mut self,
        assignment: ListAssignment<V>,
        graph: crate::stream_graph::GraphState<S>,
    ) {
        self.assignment = assignment;
        self.graph.install(graph);
    }

    /// Validate and score a new assignment using this director's compiled graph.
    /// All planning rows are refreshed, even if `Eq` compares only value identity;
    /// immutable fact sources and graph configuration are retained. Drop rolls back.
    /// Initialization work counters still describe the original construction.
    pub fn replace_assignment(
        &mut self,
        next: ListAssignment<V>,
    ) -> Result<crate::StreamAssignmentChange<'_, 'a, ListAssignment<V>, S>, ListStreamScoringError>
    {
        self.problem.validate(&next)?;
        let mut graph = self.graph.transaction();
        let mut updates = StreamBatch::new();
        for (i, list) in next.lists().iter().enumerate() {
            updates.push(
                &self.sources.entities,
                TupleChange::put(i, (i, list.clone())),
            )?;
        }
        let locations = self.problem.value_locations(&next);
        for (i, value) in self.problem.values().iter().enumerate() {
            updates.push(
                &self.sources.values,
                TupleChange::put(
                    i,
                    ListValueRow::from_known_position(i, value, &next, locations[i]),
                ),
            )?;
        }
        graph.apply_updates(updates)?;
        let score = with_structure(graph.score()?, self.problem, &next);
        Ok(crate::StreamAssignmentChange::new(
            &mut self.assignment,
            next,
            graph,
            score,
        ))
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
    /// Change external fact rows without rebuilding the scoring graph. Planning
    /// sources are protected, preserving assignment/shadow consistency. Drop or
    /// unwind rolls back; commit retains all updates. Finish this transaction
    /// before continuing search, whose acceptance history starts afresh.
    /// This does not change model domains, pins, entities, or graph topology.
    pub fn edit_facts(&mut self) -> crate::FactTransaction<'_, 'a, S> {
        let structural = self.score().structural();
        crate::FactTransaction::new(
            &mut self.graph,
            vec![
                self.sources.entities.node_id(),
                self.sources.values.node_id(),
            ],
            structural,
        )
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
        let assignment = self
            .problem
            .apply_validated(&mut self.assignment, movement)?;
        let touched = movement.touched_entities();
        let mut graph = self.graph.transaction();
        let mut updates = StreamBatch::new();
        for &entity in &touched {
            updates.push(
                &self.sources.entities,
                TupleChange::put(entity, (entity, assignment.lists()[entity].clone())),
            )?;
        }
        for (i, position) in self
            .problem
            .changed_value_positions(&assignment, &touched, |entity| {
                assignment.original_list(entity).unwrap()
            })
        {
            updates.push(
                &self.sources.values,
                TupleChange::put(
                    i,
                    ListValueRow::from_known_position(
                        i,
                        &self.problem.values()[i],
                        &assignment,
                        position,
                    ),
                ),
            )?;
        }
        graph.apply_updates(updates)?;
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
        forager: impl Into<crate::ForagerConfig<S>>,
        mut accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, Self::Error> {
        let mut result = ListSelection {
            selected: None,
            selected_accepted: false,
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
            let mut candidate = match self.apply(&movement) {
                Ok(candidate) => candidate,
                Err(ListStreamScoringError::Model(crate::ListError::DependencyCycle {
                    ..
                })) => continue,
                Err(error) => return Err(error),
            };
            result.evaluations += 1;
            result.constraint_evaluations += candidate.constraint_evaluations();
            if assert_scores {
                candidate.graph.assert_consistent()?;
            }
            let accepted = accepts(&movement, candidate.score());
            forager.try_add_with(candidate.score(), accepted, || -> Result<_, Self::Error> {
                Ok(Saved {
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
    greedy_list_stream_construction_from_monitored(None, problem, graph, sources, initial, config)
}

pub(crate) fn greedy_list_stream_construction_from_monitored<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: ConstructionConfig,
) -> Result<ListConstructionResult<V, S>, ListStreamScoringError> {
    sources.validate(&graph)?;
    crate::list_construction::construction_impl_monitored(
        shared,
        problem,
        initial,
        config,
        |initial| ListStreamScoreDirector::new(problem, graph, sources, initial),
    )
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
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.validate_acceptance()?;
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
                    [1, 1, 2].map(|value| ListMove::Assign {
                        value,
                        destination: ListPosition {
                            entity: 0,
                            index: 1,
                        },
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
