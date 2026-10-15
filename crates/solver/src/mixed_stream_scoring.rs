//! Transactional stream scoring across basic variables and ordered lists.
//!
//! All three planning sources must start empty. Other sources are facts, updated
//! explicitly through `edit_facts`.
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
use crate::StreamBatch;
use crate::mixed_scoring::{MixedSelection, with_structure};
use crate::mixed_search_scoring::MixedSearchDirector;
use crate::{
    AdditiveScore, AppliedMixedMove, GraphError, GraphTransaction, ListStreamSources, ListValueRow,
    MixedAssignment, MixedConstructionConfig, MixedConstructionResult, MixedError,
    MixedLocalSearchConfig, MixedLocalSearchError, MixedLocalSearchResult, MixedMove, MixedProblem,
    MixedScoringError, PlanningStreamSource, StreamGraph, StreamGraphBuilder, TupleChange,
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
    pub(crate) fn validate<S: AdditiveScore>(
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
    Provider(crate::MoveProviderError),
    Scoring(MixedScoringError),
    Graph(GraphError),
    PlanningSourceNotEmpty,
}
impl fmt::Display for MixedStreamScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(e) => e.fmt(f),
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
            Self::Provider(e) => Some(e),
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
    structural: i64,
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
        let structural = with_structure(S::ZERO, problem, &assignment)?.structural();
        let mut tx = graph.transaction();
        let mut updates = StreamBatch::new();
        for (i, v) in assignment.basic().values().iter().enumerate() {
            updates.push(&sources.basic, TupleChange::put(i, (i, v.clone())))?;
        }
        for (i, list) in assignment.lists().lists().iter().enumerate() {
            updates.push(
                &sources.lists.entities,
                TupleChange::put(i, (i, list.clone())),
            )?;
        }
        let locations = problem.lists().value_locations(assignment.lists());
        for (i, v) in problem.lists().values().iter().enumerate() {
            updates.push(
                &sources.lists.values,
                TupleChange::put(
                    i,
                    ListValueRow::from_known_position(i, v, assignment.lists(), locations[i]),
                ),
            )?;
        }
        tx.bind_planning_rows(updates)?;
        let initial_constraint_evaluations = tx.score_evaluations();
        tx.commit()?;
        graph.bind_fixed_planning_source(&sources.basic, assignment.basic().values().len())?;
        graph.bind_fixed_planning_source(
            &sources.lists.entities,
            assignment.lists().lists().len(),
        )?;
        graph.bind_fixed_planning_source(&sources.lists.values, problem.lists().values().len())?;
        graph.finish_initial_facts();
        Ok(Self {
            problem,
            assignment,
            graph,
            sources,
            initial_constraint_evaluations,
            structural,
        })
    }
    /// Inspect a layout-valid candidate without changing this director or scoring it.
    pub fn inspect_move(
        &self,
        movement: &MixedMove<B, L>,
    ) -> Result<crate::MoveInspection<MixedAssignment<B, L>>, MixedStreamScoringError> {
        Ok(crate::inspect_mixed_move(
            self.problem,
            &self.assignment,
            movement,
        )?)
    }
    /// Begin a live multi-move scope. Drop restores assignment and cached scores;
    /// commit retains edits. No scoring callbacks run to create or restore it.
    pub fn edit_session(
        &mut self,
    ) -> Result<crate::MoveSession<'_, Self>, MixedStreamScoringError> {
        let assignment = self.assignment.clone();
        let graph = self.graph.capture_full();
        let structural = self.structural;
        Ok(crate::MoveSession::new(self, move |director| {
            director.assignment = assignment;
            director.structural = structural;
            director.graph.install(graph);
        }))
    }
    /// Run construction from the current assignment using the retained graph.
    /// Cached initialization does not invoke callbacks or count as new constraint
    /// work; the phase still consumes its usual initial score evaluation.
    /// Errors or callback panics retain the last committed, consistent step.
    pub fn run_construction(
        &mut self,
        config: MixedConstructionConfig,
    ) -> Result<MixedConstructionResult<B, L, S>, MixedStreamScoringError> {
        self.run_construction_monitored(None, None, None, config)
    }

    pub(crate) fn run_construction_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<MixedAssignment<B, L>>,
        mut observer: crate::retained_stream_phase::Observer<'_, MixedAssignment<B, L>, S>,
        config: MixedConstructionConfig,
    ) -> Result<MixedConstructionResult<B, L, S>, MixedStreamScoringError> {
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
        crate::mixed_construction::construction_impl_monitored(
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
        config: MixedLocalSearchConfig<S>,
    ) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
        self.run_local_search_monitored(None, None, None, config, None)
    }

    /// Run local search with a custom move provider on this retained graph.
    /// Successful calls restore the best cached state; errors retain the last commit.
    pub fn run_local_search_with_provider(
        &mut self,
        config: MixedLocalSearchConfig<S>,
        provider: &mut dyn crate::MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
    ) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
        self.run_local_search_monitored(
            None,
            None,
            None,
            config,
            Some(crate::MoveProviderBinding::Infallible(provider)),
        )
    }

    /// Construct from provider proposals without rebuilding this graph or its facts.
    pub fn run_construction_with_provider(
        &mut self,
        config: crate::ProviderConstructionConfig<'_>,
        provider: &mut dyn crate::MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
    ) -> Result<crate::ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedStreamScoringError>
    {
        self.run_provider_construction_monitored(
            self.assignment.clone(),
            config,
            crate::provider_search::ProviderContext {
                provider: crate::MoveProviderBinding::Infallible(provider),
                control: None,
                observer: None,
            },
        )
    }

    /// Fallible provider execution. Errors retain the last completed step.
    pub fn run_local_search_with_fallible_provider(
        &mut self,
        config: MixedLocalSearchConfig<S>,
        provider: &mut dyn crate::FallibleMoveProvider<
            MixedProblem<B, L>,
            MixedAssignment<B, L>,
            MixedMove<B, L>,
        >,
    ) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
        self.run_local_search_monitored(
            None,
            None,
            None,
            config,
            Some(crate::MoveProviderBinding::Fallible(provider)),
        )
    }
    /// Fallible provider execution. Errors retain the last completed step.
    pub fn run_construction_with_fallible_provider(
        &mut self,
        config: crate::ProviderConstructionConfig<'_>,
        provider: &mut dyn crate::FallibleMoveProvider<
            MixedProblem<B, L>,
            MixedAssignment<B, L>,
            MixedMove<B, L>,
        >,
    ) -> Result<crate::ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedStreamScoringError>
    {
        self.run_provider_construction_monitored(
            self.assignment.clone(),
            config,
            crate::provider_search::ProviderContext {
                provider: crate::MoveProviderBinding::Fallible(provider),
                control: None,
                observer: None,
            },
        )
    }

    pub(crate) fn run_provider_construction_monitored(
        &mut self,
        initial: MixedAssignment<B, L>,
        config: crate::ProviderConstructionConfig<'_>,
        context: MixedProviderContext<'_, B, L, S>,
    ) -> Result<crate::ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedStreamScoringError>
    {
        crate::provider_construction::mixed_director(
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
        initial: Option<MixedAssignment<B, L>>,
        mut observer: crate::retained_stream_phase::Observer<'_, MixedAssignment<B, L>, S>,
        config: MixedLocalSearchConfig<S>,
        provider: Option<MixedProvider<'_, B, L>>,
    ) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
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
        let clock = crate::provider_search::clock(&config.search, &shared);
        let result = crate::mixed_local_search::search_impl_with_provider(
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
            provider,
        )?;
        if let Some(best) = &result.best {
            let structural = best.score.structural();
            self.assignment = best.assignment.clone();
            self.structural = structural;
            self.graph
                .install(best_graph.expect("scored phase records its initial best"));
        }
        Ok(result)
    }

    pub(crate) fn prepare_retained_phase(
        &mut self,
        initial: MixedAssignment<B, L>,
    ) -> Result<u128, MixedStreamScoringError> {
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
        config: impl Into<crate::MixedSolverSequence<'c, S>>,
    ) -> Result<
        crate::SolverResult<MixedAssignment<B, L>, S>,
        crate::SolverError<MixedLocalSearchError>,
    > {
        self.run_solver_with_extensions(config, crate::SolverExtensions::default())
    }
    /// Run a retained sequence with synchronous solver lifecycle events.
    pub fn run_solver_with_events<'c>(
        &mut self,
        config: impl Into<crate::MixedSolverSequence<'c, S>>,
        mut observer: impl FnMut(crate::SolverEvent<'_, MixedAssignment<B, L>, S>),
    ) -> Result<
        crate::SolverResult<MixedAssignment<B, L>, S>,
        crate::SolverError<MixedLocalSearchError>,
    > {
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
        config: impl Into<crate::MixedSolverSequence<'c, S>>,
        mut command: impl FnMut(
            usize,
            &mut crate::CustomPhaseContext<
                '_,
                MixedAssignment<B, L>,
                MixedMove<B, L>,
                S,
                MixedStreamScoringError,
            >,
        ) -> Result<(), crate::CustomPhaseError<MixedStreamScoringError>>,
    ) -> Result<
        crate::SolverResult<MixedAssignment<B, L>, S>,
        crate::SolverError<MixedLocalSearchError>,
    > {
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
        config: impl Into<crate::MixedSolverSequence<'c, S>>,
        mut command: impl FnMut(
            usize,
            &mut crate::CustomPhaseContext<
                '_,
                MixedAssignment<B, L>,
                MixedMove<B, L>,
                S,
                MixedStreamScoringError,
            >,
        ) -> Result<(), crate::CustomPhaseError<MixedStreamScoringError>>,
        mut observer: impl FnMut(crate::SolverEvent<'_, MixedAssignment<B, L>, S>),
    ) -> Result<
        crate::SolverResult<MixedAssignment<B, L>, S>,
        crate::SolverError<MixedLocalSearchError>,
    > {
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
        config: impl Into<crate::MixedSolverSequence<'c, S>>,
        extensions: crate::MixedSolverExtensions<'_, B, L, S, MixedStreamScoringError>,
    ) -> Result<
        crate::SolverResult<MixedAssignment<B, L>, S>,
        crate::SolverError<MixedLocalSearchError>,
    > {
        crate::solver::retained::mixed(self.problem, self, config.into(), extensions)
    }
    pub(crate) fn install_retained_state(
        &mut self,
        assignment: MixedAssignment<B, L>,
        graph: crate::stream_graph::GraphState<S>,
    ) {
        let structural = with_structure(S::ZERO, self.problem, &assignment)
            .expect("validated retained assignment")
            .structural();
        self.assignment = assignment;
        self.structural = structural;
        self.graph.install(graph);
    }

    /// Validate and score a new assignment using this director's compiled graph.
    /// All planning rows are refreshed, even if `Eq` compares only value identity;
    /// immutable fact sources and graph configuration are retained. Drop rolls back.
    /// Initialization work counters still describe the original construction.
    pub fn replace_assignment(
        &mut self,
        next: MixedAssignment<B, L>,
    ) -> Result<
        crate::StreamAssignmentChange<'_, 'a, MixedAssignment<B, L>, S>,
        MixedStreamScoringError,
    > {
        self.problem.validate(&next)?;
        with_structure(S::ZERO, self.problem, &next)?;
        let mut graph = self.graph.transaction();
        let mut updates = StreamBatch::new();
        for (i, value) in next.basic().values().iter().enumerate() {
            updates.push(&self.sources.basic, TupleChange::put(i, (i, value.clone())))?;
        }
        for (i, list) in next.lists().lists().iter().enumerate() {
            updates.push(
                &self.sources.lists.entities,
                TupleChange::put(i, (i, list.clone())),
            )?;
        }
        let locations = self.problem.lists().value_locations(next.lists());
        let mut value_updates =
            self.problem
                .lists()
                .values()
                .iter()
                .enumerate()
                .map(|(i, value)| {
                    (
                        i,
                        ListValueRow::from_known_position(i, value, next.lists(), locations[i]),
                    )
                });
        if !graph.try_apply_owned_unobserved(&self.sources.lists.values, &mut value_updates)? {
            for (i, row) in value_updates.by_ref() {
                updates.push(&self.sources.lists.values, TupleChange::put(i, row))?;
            }
        }
        drop(value_updates);
        graph.apply_updates(updates)?;
        let score = with_structure(graph.score()?, self.problem, &next)?;
        Ok(crate::StreamAssignmentChange::new(
            &mut self.assignment,
            &mut self.structural,
            next,
            graph,
            score,
        ))
    }

    pub fn assignment(&self) -> &MixedAssignment<B, L> {
        &self.assignment
    }
    pub fn graph(&self) -> &StreamGraph<'a, S> {
        &self.graph
    }
    pub fn score(&self) -> S {
        self.graph.score().with_structural(self.structural)
    }
    /// Change live graph weights atomically, retaining this assignment and all
    /// source/constraint handles. Rebuilds live branches from retained source facts;
    /// returned work counts every replayed score callback, outside search budgets.
    /// A later retained phase starts with the new score and fresh acceptance state.
    /// See [`StreamGraph::apply_weight_overrides`] for rollback and replay costs.
    pub fn apply_weight_overrides(
        &mut self,
        overrides: &crate::ConstraintWeightOverrides<S>,
    ) -> Result<u128, MixedStreamScoringError> {
        Ok(self.graph.apply_weight_overrides(overrides)?)
    }
    pub(crate) fn capture_constraint_metrics(
        &self,
        capture: &crate::solver_metrics::ConstraintMetricCapture<S>,
    ) {
        self.graph.capture_constraint_metrics(capture);
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
                self.sources.basic.node_id(),
                self.sources.lists.entities.node_id(),
                self.sources.lists.values.node_id(),
            ],
            structural,
        )
    }
    pub fn assert_consistent(&mut self) -> Result<(), MixedStreamScoringError> {
        self.graph.assert_consistent()?;
        crate::stream_assignment_change::check_structure(
            self.structural,
            with_structure(S::ZERO, self.problem, &self.assignment)?.structural(),
        )?;
        Ok(())
    }
    /// Validate both final components before graph callbacks. Deliver each touched
    /// basic/entity once and each value in a touched list before/after once. Drop,
    /// errors and ordinary unwind restore both components and all graph caches.
    pub fn apply(
        &mut self,
        movement: &MixedMove<B, L>,
    ) -> Result<MixedStreamScoredMove<'_, 'a, B, L, S>, MixedStreamScoringError> {
        let assignment = self
            .problem
            .apply_validated(&mut self.assignment, movement)?;

        let world_size = self.problem.basic().variables().len().saturating_add(
            if self.problem.lists().allows_unassigned() {
                0
            } else {
                assignment.lists().lists().len()
            },
        );
        let structural = match crate::scoring::basic_structural_delta(
            self.problem.basic(),
            assignment.basic(),
            assignment.basic_prior_values(),
            world_size,
        ) {
            Some(basic_delta) => {
                let list_delta = if self.problem.lists().allows_unassigned() {
                    0
                } else {
                    assignment.assigned_list_count_delta()
                };
                let missing = i64::try_from(
                    -i128::from(self.structural) - i128::from(basic_delta) - i128::from(list_delta),
                )
                .map_err(|_| crate::MixedScoringError::StructuralCountOverflow)?;
                -missing
            }
            None => with_structure(S::ZERO, self.problem, &assignment)?.structural(),
        };
        let mut graph = self.graph.transaction();
        let basic_touched = movement.basic.touched_variables();
        let touched = movement.lists.touched_entities();
        let current_values = touched.iter().fold(0usize, |len, &entity| {
            len.saturating_add(assignment.lists().lists()[entity].len())
        });
        let value_capacity = self
            .problem
            .lists()
            .values()
            .len()
            .min(current_values.saturating_mul(2));
        let mut updates = StreamBatch::with_capacity(
            basic_touched
                .len()
                .saturating_add(touched.len())
                .saturating_add(value_capacity),
        );
        for i in basic_touched {
            updates.push(
                &self.sources.basic,
                TupleChange::put(i, (i, assignment.basic().values()[i].clone())),
            )?;
        }
        for &i in &touched {
            updates.push(
                &self.sources.lists.entities,
                TupleChange::put(i, (i, assignment.lists().lists()[i].clone())),
            )?;
        }
        let mut value_updates = self
            .problem
            .lists()
            .changed_value_positions(assignment.lists(), &touched, |entity| {
                assignment.original_list(entity).unwrap()
            })
            .into_iter()
            .map(|(i, position)| {
                (
                    i,
                    ListValueRow::from_known_position(
                        i,
                        &self.problem.lists().values()[i],
                        assignment.lists(),
                        position,
                    ),
                )
            });
        if !graph.try_apply_owned_unobserved(&self.sources.lists.values, &mut value_updates)? {
            for (i, row) in value_updates.by_ref() {
                updates.push(&self.sources.lists.values, TupleChange::put(i, row))?;
            }
        }
        drop(value_updates);
        graph.apply_updates(updates)?;
        let score = graph.score()?.with_structural(structural);
        Ok(MixedStreamScoredMove {
            assignment,
            graph,
            score,
            structural,
            cached_structure: &mut self.structural,
        })
    }
}
#[must_use = "dropping a mixed scored stream move restores both components and graph"]
pub struct MixedStreamScoredMove<'g, 'a, B, L, S: AdditiveScore> {
    assignment: AppliedMixedMove<'g, B, L>,
    graph: GraphTransaction<'g, 'a, S>,
    score: S,
    structural: i64,
    cached_structure: &'g mut i64,
}
impl<B, L, S: AdditiveScore> MixedStreamScoredMove<'_, '_, B, L, S> {
    pub fn assignment(&self) -> &MixedAssignment<B, L> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score.clone()
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
        *self.cached_structure = self.structural;
    }
}
struct Saved<B, L, S> {
    accepted: bool,
    structural: i64,
    movement: MixedMove<B, L>,
    assignment: crate::candidate_snapshot::MixedSnapshot<B, L>,
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
    fn capture_constraint_metrics(
        &self,
        capture: &crate::solver_metrics::ConstraintMetricCapture<S>,
    ) {
        self.capture_constraint_metrics(capture);
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
        forager: impl Into<crate::ForagerConfig<S>>,
        mut accepts: impl FnMut(&MixedMove<B, L>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<MixedSelection<B, L>, Self::Error> {
        let problem = self.problem;
        let mut result = MixedSelection {
            selected: None,
            selected_accepted: false,
            evaluations: 0,
            replay_evaluations: 0,
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
                Err(MixedStreamScoringError::Scoring(crate::MixedScoringError::Model(
                    crate::MixedError::Lists(crate::ListError::DependencyCycle { .. }),
                ))) => continue,
                Err(error) => return Err(error),
            };
            result.evaluations += 1;
            result.constraint_evaluations += candidate.constraint_evaluations();
            if assert_scores {
                candidate.graph.assert_consistent()?;
                crate::stream_assignment_change::check_structure(
                    candidate.structural,
                    with_structure(S::ZERO, problem, &candidate.assignment)?.structural(),
                )?;
            }
            let accepted = accepts(&movement, candidate.score());
            forager.try_add_with(candidate.score(), accepted, || -> Result<_, Self::Error> {
                Ok(Saved {
                    accepted,
                    structural: candidate.structural,
                    movement: movement.clone(),
                    assignment: candidate.assignment.snapshot(),
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
            self.assignment.install_snapshot(saved.assignment);
            self.structural = saved.structural;
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
    greedy_mixed_stream_construction_from(
        problem,
        graph,
        sources,
        problem.initial_assignment(),
        settings,
    )
}

/// Resume construction from a validated assignment, preserving supplied values and pins.
/// Missing required and configured optional values are considered; each call
/// initializes its own scoring cache.
/// Validation runs even with zero budget or a cancelled token.
pub fn greedy_mixed_stream_construction_from<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
) -> Result<MixedConstructionResult<B, L, S>, MixedStreamScoringError> {
    greedy_mixed_stream_construction_from_monitored(
        None, problem, graph, sources, initial, settings,
    )
}

pub(crate) fn greedy_mixed_stream_construction_from_monitored<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedConstructionConfig,
) -> Result<MixedConstructionResult<B, L, S>, MixedStreamScoringError> {
    sources.validate(&graph)?;
    crate::mixed_construction::construction_impl_monitored(
        shared,
        problem,
        initial,
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
    mixed_stream_local_search_from_monitored(None, problem, graph, sources, initial, settings)
}

pub(crate) fn mixed_stream_local_search_from_monitored<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    settings.search.validate_acceptance()?;
    sources.validate(&graph)?;
    crate::mixed_local_search::search_impl_monitored(
        shared,
        problem,
        initial,
        settings,
        |initial| MixedStreamScoreDirector::new(problem, graph, sources, initial),
    )
}

type MixedProvider<'p, B, L> =
    crate::MoveProviderBinding<'p, MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>;
type MixedProviderContext<'p, B, L, S> = crate::provider_search::ProviderContext<
    'p,
    MixedProblem<B, L>,
    MixedAssignment<B, L>,
    MixedMove<B, L>,
    S,
>;

impl From<crate::MoveProviderError> for MixedStreamScoringError {
    fn from(error: crate::MoveProviderError) -> Self {
        Self::Provider(error)
    }
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
            let moves = [1, 1, 2].map(|v| MixedMove {
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
                    3,
                    crate::ForagerConfig {
                        tie_breaking: crate::TieBreaking::Random { seed: 7 },
                        ..Default::default()
                    },
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
    #[test]
    fn structural_cache_uses_sparse_delta_and_independent_oracle() {
        let p = MixedProblem::new(
            crate::FiniteProblem::new(vec![crate::Variable::new(vec![0, 1]); 1024]).unwrap(),
            crate::ListProblem::new_indexed(vec![0], vec![crate::ListEntity::new(vec![]); 1024])
                .unwrap(),
        );
        let calls = std::cell::Cell::new(0);
        let mut b = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
        let s = MixedStreamSources::<i32, i32>::new(&mut b);
        b.score("basic", s.basic.port(), |_| {
            calls.set(calls.get() + 1);
            crate::SimpleScore::ZERO
        })
        .unwrap();
        let mut d =
            MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let original = d.structural;
        d.structural += 7;
        // Score reads use the retained component; the independent oracle must
        // detect rather than silently repair corruption.
        assert_eq!(d.score().structural(), original + 7);
        assert!(d.assert_consistent().is_err());
        assert_eq!(d.structural, original + 7);
        let candidate = d
            .apply(&MixedMove {
                basic: crate::Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                lists: crate::ListMove::Composite(vec![]),
            })
            .unwrap();
        assert_eq!(candidate.score().structural(), original + 8);
        drop(candidate);
        assert_eq!(d.structural, original + 7);
        d.structural = original;
        d.assert_consistent().unwrap();

        // Synthetic boundary cannot be allocated physically. A null-preserving
        // sparse move must reject the count before any callback is dispatched.
        d.structural = i64::MIN;
        let work = calls.get();
        assert!(matches!(
            d.apply(&crate::MixedMove {
                basic: crate::Move::Change {
                    variable: 1,
                    value: None
                },
                lists: crate::ListMove::Composite(vec![])
            }),
            Err(MixedStreamScoringError::Scoring(
                crate::MixedScoringError::StructuralCountOverflow
            ))
        ));
        assert_eq!(calls.get(), work);
        assert_eq!(d.assignment(), &p.initial_assignment());
    }
}
