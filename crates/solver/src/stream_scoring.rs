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
    Provider(crate::MoveProviderError),
    Model(ModelError),
    Graph(GraphError),
    PlanningSourceNotEmpty,
}
impl fmt::Display for StreamScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(e) => e.fmt(f),
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
            Self::Provider(e) => Some(e),
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
    structural: i64,
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
        let structural = with_structure(S::ZERO, problem, &assignment).structural();
        let mut transaction = graph.transaction();
        let mut updates = StreamBatch::new();
        for (index, value) in assignment.values().iter().enumerate() {
            updates.push(&source, TupleChange::put(index, (index, value.clone())))?;
        }
        transaction.bind_planning_rows(updates)?;
        let initial_constraint_evaluations = transaction.score_evaluations();
        transaction.commit()?;
        graph.finish_initial_facts();
        Ok(Self {
            problem,
            assignment,
            graph,
            source,
            initial_constraint_evaluations,
            structural,
        })
    }
    /// Change external fact rows without rebuilding the scoring graph. Planning
    /// sources are protected, preserving assignment/shadow consistency. Drop or
    /// unwind rolls back; commit retains all updates. Finish this transaction
    /// before continuing search, whose acceptance history starts afresh.
    /// This does not change model domains, pins, entities, or graph topology.
    pub fn edit_facts(&mut self) -> crate::FactTransaction<'_, 'a, S> {
        let structural = self.score().structural();
        crate::FactTransaction::new(&mut self.graph, vec![self.source.node_id()], structural)
    }
    pub fn assert_consistent(&mut self) -> Result<(), StreamScoringError> {
        self.graph.assert_consistent()?;
        crate::stream_assignment_change::check_structure(
            self.structural,
            with_structure(S::ZERO, self.problem, &self.assignment).structural(),
        )?;
        Ok(())
    }
    /// Begin a live multi-move scope. Drop restores assignment and cached scores;
    /// commit retains edits. No scoring callbacks run to create or restore it.
    pub fn edit_session(&mut self) -> Result<crate::MoveSession<'_, Self>, StreamScoringError> {
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
        config: ConstructionConfig,
    ) -> Result<ConstructionResult<V, S>, StreamScoringError> {
        self.run_construction_monitored(None, None, None, config)
    }

    pub(crate) fn run_construction_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<Assignment<V>>,
        mut observer: crate::retained_stream_phase::Observer<'_, Assignment<V>, S>,
        config: ConstructionConfig,
    ) -> Result<ConstructionResult<V, S>, StreamScoringError> {
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
        crate::construction::construction_impl_monitored(
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
    ) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
        self.run_local_search_monitored(None, None, None, config, None)
    }

    /// Run local search with a custom move provider on this retained graph.
    /// Successful calls restore the best cached state; errors retain the last commit.
    pub fn run_local_search_with_provider(
        &mut self,
        config: LocalSearchConfig<S>,
        provider: &mut dyn crate::MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
    ) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
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
        provider: &mut dyn crate::MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
    ) -> Result<crate::ProviderConstructionResult<Assignment<V>, S>, StreamScoringError> {
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
        config: LocalSearchConfig<S>,
        provider: &mut dyn crate::FallibleMoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
    ) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
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
        provider: &mut dyn crate::FallibleMoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
    ) -> Result<crate::ProviderConstructionResult<Assignment<V>, S>, StreamScoringError> {
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
        initial: Assignment<V>,
        config: crate::ProviderConstructionConfig<'_>,
        context: crate::provider_search::ProviderContext<
            '_,
            FiniteProblem<V>,
            Assignment<V>,
            Move<V>,
            S,
        >,
    ) -> Result<crate::ProviderConstructionResult<Assignment<V>, S>, StreamScoringError> {
        crate::provider_construction::basic_director(
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
        initial: Option<Assignment<V>>,
        mut observer: crate::retained_stream_phase::Observer<'_, Assignment<V>, S>,
        config: LocalSearchConfig<S>,
        provider: Option<crate::MoveProviderBinding<'_, FiniteProblem<V>, Assignment<V>, Move<V>>>,
    ) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
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
        let clock = crate::provider_search::clock(&config, &shared);
        let result = crate::local_search::search_impl_with_provider(
            shared,
            problem,
            Some(initial),
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
        initial: Assignment<V>,
    ) -> Result<u128, StreamScoringError> {
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
    ) -> Result<crate::SolverResult<Assignment<V>, S>, crate::SolverError<LocalSearchError>> {
        self.run_solver_with_extensions(config, crate::SolverExtensions::default())
    }
    /// Run a retained sequence with synchronous solver lifecycle events.
    pub fn run_solver_with_events<'c>(
        &mut self,
        config: impl Into<crate::SolverSequence<'c, S>>,
        mut observer: impl FnMut(crate::SolverEvent<'_, Assignment<V>, S>),
    ) -> Result<crate::SolverResult<Assignment<V>, S>, crate::SolverError<LocalSearchError>> {
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
            &mut crate::CustomPhaseContext<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
        ) -> Result<(), crate::CustomPhaseError<StreamScoringError>>,
    ) -> Result<crate::SolverResult<Assignment<V>, S>, crate::SolverError<LocalSearchError>> {
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
            &mut crate::CustomPhaseContext<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
        ) -> Result<(), crate::CustomPhaseError<StreamScoringError>>,
        mut observer: impl FnMut(crate::SolverEvent<'_, Assignment<V>, S>),
    ) -> Result<crate::SolverResult<Assignment<V>, S>, crate::SolverError<LocalSearchError>> {
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
            FiniteProblem<V>,
            Assignment<V>,
            Move<V>,
            S,
            StreamScoringError,
        >,
    ) -> Result<crate::SolverResult<Assignment<V>, S>, crate::SolverError<LocalSearchError>> {
        crate::solver::retained::basic(self.problem, self, config.into(), extensions)
    }
    pub(crate) fn install_retained_state(
        &mut self,
        assignment: Assignment<V>,
        graph: crate::stream_graph::GraphState<S>,
    ) {
        let structural = with_structure(S::ZERO, self.problem, &assignment).structural();
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
        next: Assignment<V>,
    ) -> Result<crate::StreamAssignmentChange<'_, 'a, Assignment<V>, S>, StreamScoringError> {
        self.replace_generated_assignment(next, true)
    }

    /// Only for one basic exact traversal: after a full first refresh, every
    /// variable uses a fixed representative for each planning identity. Domain
    /// values are Eq-unique, pins stay fixed, and completion-fixed payloads never
    /// enter the mutable domain. Clones must preserve scoring payloads. Arbitrary
    /// handoffs and public replacement must always request a full refresh.
    pub(crate) fn replace_generated_assignment(
        &mut self,
        next: Assignment<V>,
        refresh_all: bool,
    ) -> Result<crate::StreamAssignmentChange<'_, 'a, Assignment<V>, S>, StreamScoringError> {
        self.problem.validate(&next)?;
        let mut graph = self.graph.transaction();
        let mut updates = StreamBatch::new();
        for (i, value) in next.values().iter().enumerate() {
            if refresh_all || self.assignment.values()[i] != *value {
                updates.push(&self.source, TupleChange::put(i, (i, value.clone())))?;
            }
        }
        graph.apply_updates(updates)?;
        let score = with_structure(graph.score()?, self.problem, &next);
        Ok(crate::StreamAssignmentChange::new(
            &mut self.assignment,
            &mut self.structural,
            next,
            graph,
            score,
        ))
    }

    pub fn assignment(&self) -> &Assignment<V> {
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
    ) -> Result<u128, StreamScoringError> {
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

    /// Validate the final composite assignment, then deliver each touched variable
    /// once using its final value. Drop/ordinary unwind restore both assignments
    /// and graph state. Commit performs no callback replay.
    pub fn apply(
        &mut self,
        movement: &Move<V>,
    ) -> Result<StreamScoredMove<'_, 'a, V, S>, StreamScoringError> {
        let assignment = self
            .problem
            .apply_validated(&mut self.assignment, movement)?;
        let structural = match crate::scoring::basic_structural_delta(
            self.problem,
            &assignment,
            assignment.prior_values(),
            self.problem.variables().len(),
        ) {
            Some(delta) => self.structural + delta,
            None => with_structure(S::ZERO, self.problem, &assignment).structural(),
        };
        let mut graph = self.graph.transaction();
        let mut updates = StreamBatch::new();
        for variable in movement.touched_variables() {
            updates.push(
                &self.source,
                TupleChange::put(variable, (variable, assignment.values()[variable].clone())),
            )?;
        }
        graph.apply_updates(updates)?;
        let score = graph.score()?.with_structural(structural);
        Ok(StreamScoredMove {
            assignment,
            graph,
            score,
            structural,
            cached_structure: &mut self.structural,
        })
    }
}

#[must_use = "dropping a scored stream move restores the assignment and graph"]
pub struct StreamScoredMove<'g, 'a, V, S: AdditiveScore> {
    assignment: AppliedMove<'g, V>,
    graph: GraphTransaction<'g, 'a, S>,
    score: S,
    structural: i64,
    cached_structure: &'g mut i64,
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
        *self.cached_structure = self.structural;
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
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        mut accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<Selection<V>, Self::Error> {
        let problem = self.problem;
        let mut result = Selection {
            selected_accepted: false,
            selected: None,
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
            let mut candidate = self.apply(&movement)?;
            result.evaluations += 1;
            result.constraint_evaluations += candidate.constraint_evaluations();
            if assert_scores {
                candidate.graph.assert_consistent()?;
                crate::stream_assignment_change::check_structure(
                    candidate.structural,
                    with_structure(S::ZERO, problem, &candidate.assignment).structural(),
                )?;
            }
            let accepted = accepts(&movement, candidate.score());
            forager.try_add_with(candidate.score(), accepted, || -> Result<_, Self::Error> {
                Ok(SavedStreamCandidate {
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
            saved.assignment.install(&mut self.assignment);
            self.structural = saved.structural;
            result.selected = Some(saved.movement);
            result.selected_accepted = saved.accepted;
        }
        Ok(result)
    }
}
struct SavedStreamCandidate<V, S> {
    accepted: bool,
    structural: i64,
    movement: Move<V>,
    assignment: crate::candidate_snapshot::BasicSnapshot<V>,
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
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
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
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    stream_search(shared, problem, graph, source, Some(initial), config)
}
fn stream_search<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
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

impl From<crate::MoveProviderError> for StreamScoringError {
    fn from(error: crate::MoveProviderError) -> Self {
        Self::Provider(error)
    }
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
    #[test]
    fn structural_cache_uses_sparse_delta_and_independent_oracle() {
        let p = FiniteProblem::new(vec![crate::Variable::new(vec![0, 1]); 1024]).unwrap();
        let mut b = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let original = d.structural;
        d.structural += 7;
        // Score reads use the retained component; the independent oracle must
        // detect rather than silently repair corruption.
        assert_eq!(d.score().structural(), original + 7);
        assert!(d.assert_consistent().is_err());
        assert_eq!(d.structural, original + 7);
        let candidate = d
            .apply(&Move::Change {
                variable: 0,
                value: Some(1),
            })
            .unwrap();
        assert_eq!(candidate.score().structural(), original + 8);
        drop(candidate);
        assert_eq!(d.structural, original + 7);
        d.structural = original;
        d.assert_consistent().unwrap();
    }
}

#[cfg(test)]
mod winner_tests;

mod context;
