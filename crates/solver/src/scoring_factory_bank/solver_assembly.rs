//! Validated model/scoring assembly and actual runtime phase-mode coordination.
pub(crate) mod phase_views;
mod schema_repairs;
use super::{
    FactoryScoreDirector, FactoryScoreShape, PreparedScoreFactory, ScoreFactoryBuildError,
    ScoringFactoryDeclaration,
};
use crate::{
    ContextFailure, ContextMatchPolicy, ContextMetrics, ContextScoreResource, ContextScoreWork,
    ContextSettings, EnvironmentMode, EnvironmentProfile, FactoryRandomStream, PlanningSchema,
    ScoringContextError, ScoringContextManager, SolverPhase,
};
use std::{error::Error, fmt, rc::Rc};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FactoryWorkerRequest {
    #[default]
    None,
    Auto,
    Count(i64),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoryWorkerError {
    pub request: String,
    pub resolved: Option<i64>,
}
impl fmt::Display for FactoryWorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "moveThreadCount ({}) resulted in a resolvedMoveThreadCount ({:?}) that is lower than 1 or not a supported integer",
            self.request, self.resolved
        )
    }
}
impl Error for FactoryWorkerError {}
impl FactoryWorkerRequest {
    pub fn parse_factory_workers(text: &str) -> Result<Self, FactoryWorkerError> {
        match text {
            "NONE" => Ok(Self::None),
            "AUTO" => Ok(Self::Auto),
            _ => {
                let value = text.parse::<i64>().map_err(|_| FactoryWorkerError {
                    request: text.into(),
                    resolved: None,
                })?;
                resolve_factory_workers(Self::Count(value), 0)?;
                Ok(Self::Count(value))
            }
        }
    }
}
/// A native advisory budget, not a thread-launch operation. Automatic selection
/// reserves two CPUs and chooses at most four workers, with at least two needed
/// to justify workers. Unknown/nonpositive CPU counts yield no worker request.
pub fn resolve_factory_workers(
    request: FactoryWorkerRequest,
    cpus: isize,
) -> Result<Option<usize>, FactoryWorkerError> {
    match request {
        FactoryWorkerRequest::None => Ok(None),
        FactoryWorkerRequest::Auto => Ok((cpus > 3).then(|| (cpus - 2).min(4) as usize)),
        FactoryWorkerRequest::Count(count) => usize::try_from(count)
            .ok()
            .filter(|&n| n > 0)
            .map(Some)
            .ok_or_else(|| FactoryWorkerError {
                request: count.to_string(),
                resolved: Some(count),
            }),
    }
}

#[derive(Debug)]
pub enum FactoryAssemblyError<E> {
    MissingSolutionRegistration,
    MissingEntityRegistration,
    ModelSchemaType,
    PhaseModes {
        phases: usize,
        modes: usize,
    },
    UnsupportedConstraintProfiling,
    Workers(FactoryWorkerError),
    Scoring(ScoreFactoryBuildError<E>),
    Resource(E),
    SchemaRuin {
        phase: usize,
        error: crate::SchemaRuinError,
    },
}
impl<E: fmt::Display> fmt::Display for FactoryAssemblyError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSolutionRegistration=>f.write_str("solver configuration must have a planning solution schema; if using a model-binding integration, register the solution schema before preparing the factory"),
            Self::MissingEntityRegistration=>f.write_str("solver configuration must have at least 1 registered entity type; if using a model-binding integration, register entity types before preparing the factory"),
            Self::ModelSchemaType=>f.write_str("solver model type differs from registered solution schema type"),
            Self::PhaseModes {phases,modes}=>write!(f,"solver has {phases} phases but {modes} declared phase modes"),
            Self::UnsupportedConstraintProfiling=>f.write_str("Constraint profiling from the reference configuration is unsupported; remove constraintStreamProfilingEnabled from the solver configuration"),
            Self::SchemaRuin { phase, error } => write!(f,"phase {phase}: {error}"),
            Self::Workers(e)=>e.fmt(f),Self::Scoring(e)=>e.fmt(f),Self::Resource(e)=>e.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for FactoryAssemblyError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Workers(e) => Some(e),
            Self::Scoring(e) => Some(e),
            Self::Resource(e) => Some(e),
            Self::SchemaRuin { error, .. } => Some(error),
            _ => None,
        }
    }
}
/// Explicit native registration and phase plan. An empty runtime entity collection
/// is valid when the schema still declares an entity type. No XML/bean discovery
/// is performed. Empty phase vectors calculate the supplied working state only.
pub struct SolverAssemblyConfiguration<'a, M, S> {
    pub model: Rc<M>,
    pub schema: Option<Rc<PlanningSchema>>,
    pub environment: EnvironmentProfile,
    pub phases: Vec<SolverPhase<'a, S>>,
    pub metrics: ContextMetrics,
    pub seed: u64,
    pub workers: FactoryWorkerRequest,
    pub reference_constraint_profiling: bool,
    pub schema_ruin_phases: Vec<crate::SchemaRuinPhaseConfiguration>,
}
impl<M, S> SolverAssemblyConfiguration<'_, M, S> {
    pub fn for_factory_model(model: Rc<M>) -> Self {
        Self {
            model,
            schema: None,
            environment: EnvironmentProfile::default(),
            phases: vec![],
            metrics: ContextMetrics::default(),
            seed: 0,
            workers: FactoryWorkerRequest::None,
            reference_constraint_profiling: false,
            schema_ruin_phases: vec![],
        }
    }
}
pub struct AssembledSolverFactory<'a, M, P, I, D: ContextScoreResource> {
    bank: PreparedScoreFactory<'a, M, P, I, D, D::Error>,
    schema: Rc<PlanningSchema>,
    global: EnvironmentMode,
    modes: Vec<EnvironmentMode>,
    phases: Vec<SolverPhase<'a, D::Score>>,
    metrics: ContextMetrics,
    seed: u64,
    workers: Option<usize>,
    schema_ruin: Vec<Option<crate::SchemaRuinSelector>>,
}
impl<'a, P, I, D> ScoringFactoryDeclaration<'a, P, I, D, D::Error>
where
    D: ContextScoreResource,
{
    /// Validate registration/profile/worker requests before preparing scoring data.
    /// Preparation is cached in the same bank across all sessions from this factory.
    pub fn assemble_solver_factory<M: 'static>(
        self,
        mut config: SolverAssemblyConfiguration<'a, M, D::Score>,
        shape: Rc<FactoryScoreShape>,
    ) -> Result<AssembledSolverFactory<'a, M, P, I, D>, FactoryAssemblyError<D::Error>> {
        let schema = config
            .schema
            .take()
            .ok_or(FactoryAssemblyError::MissingSolutionRegistration)?;
        if schema.entities().len() == 0 {
            return Err(FactoryAssemblyError::MissingEntityRegistration);
        }
        if !schema.solution_type().is::<M>() {
            return Err(FactoryAssemblyError::ModelSchemaType);
        }
        if config.reference_constraint_profiling {
            return Err(FactoryAssemblyError::UnsupportedConstraintProfiling);
        }
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get() as isize)
            .unwrap_or(0);
        let workers =
            resolve_factory_workers(config.workers, cpus).map_err(FactoryAssemblyError::Workers)?;
        let declarations = config.environment.declared_phases();
        if declarations.is_empty() {
            config.environment = EnvironmentProfile::new(
                config.environment.declared_global(),
                vec![None; config.phases.len()],
            );
        } else if declarations.len() != config.phases.len() {
            return Err(FactoryAssemblyError::PhaseModes {
                phases: config.phases.len(),
                modes: declarations.len(),
            });
        }
        let global = config.environment.global_mode();
        let modes: Vec<_> = config.environment.phase_modes().collect();
        // Seed only configured shuffled move orders and random ties; acceptance
        // and provider-specific seeds remain explicit in their own configurations.
        // Phase modes also enable diagnostics in the actual native phase runner.
        for (index, (phase, mode)) in config.phases.iter_mut().zip(&modes).enumerate() {
            let full = matches!(
                mode,
                EnvironmentMode::FullAssert
                    | EnvironmentMode::NonIntrusiveFullAssert
                    | EnvironmentMode::TrackedFullAssert
            );
            let step = *mode == EnvironmentMode::StepAssert;
            match phase {
                SolverPhase::Construction(construction) => {
                    construction.assert_scores |= full;
                    construction.assert_step_scores |= step;
                }
                SolverPhase::LocalSearch(search) => {
                    search.assert_scores |= full;
                    search.assert_step_scores |= step;
                    let phase_seed = config.seed.wrapping_add(index as u64);
                    match &mut search.move_order {
                        crate::MoveOrder::Original => {}
                        crate::MoveOrder::ShuffledVariables { seed }
                        | crate::MoveOrder::ShuffledMoves { seed, .. } => *seed = phase_seed,
                    }
                    if let crate::TieBreaking::Random { seed } = &mut search.tie_breaking {
                        *seed = phase_seed;
                    }
                }
                SolverPhase::RuinRecreate(search) => {
                    search.seed = config.seed.wrapping_add(index as u64);
                    search.assert_scores |= full;
                    search.assert_step_scores |= step;
                }
                SolverPhase::Exact(_) | SolverPhase::Custom(_) => {}
            }
        }
        let schema_ruin = schema_repairs::prepare_schema_repairs(
            &schema,
            &config.phases,
            &config.schema_ruin_phases,
        )?;
        let bank = self
            .prepare_score_factory(config.model, shape, config.environment)
            .map_err(FactoryAssemblyError::Scoring)?;
        Ok(AssembledSolverFactory {
            bank,
            schema,
            global,
            modes,
            phases: config.phases,
            metrics: config.metrics,
            seed: config.seed,
            workers,
            schema_ruin,
        })
    }
}
struct AssemblyResource<'a, M, P, I, D: ContextScoreResource>(
    FactoryScoreDirector<'a, M, P, I, D, D::Error>,
);
impl<M, P, I, D> ContextScoreResource for AssemblyResource<'_, M, P, I, D>
where
    D: ContextScoreResource,
    D::Error: 'static,
{
    type Score = D::Score;
    type Error = FactoryAssemblyError<D::Error>;
    fn context_score(&self) -> D::Score {
        self.0.resource.context_score()
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        self.0
            .resource
            .check_context()
            .map_err(FactoryAssemblyError::Resource)
    }
    fn context_totals(&self) -> Vec<(String, D::Score)> {
        self.0.resource.context_totals()
    }
    fn release_context(&mut self) -> Result<(), Self::Error> {
        self.0
            .resource
            .release_context()
            .map_err(FactoryAssemblyError::Resource)
    }
}
type AssemblyRebuilder<'a, M, P, I, D> = Box<
    dyn FnMut(
            &mut AssemblyResource<'a, M, P, I, D>,
            ContextSettings,
        ) -> Result<
            AssemblyResource<'a, M, P, I, D>,
            FactoryAssemblyError<<D as ContextScoreResource>::Error>,
        > + 'a,
>;
type AssemblyManager<'a, M, P, I, D> =
    ScoringContextManager<AssemblyResource<'a, M, P, I, D>, AssemblyRebuilder<'a, M, P, I, D>>;
#[derive(Debug)]
pub enum FactoryExecutionError<E> {
    Build(FactoryAssemblyError<E>),
    Context(ScoringContextError<FactoryAssemblyError<E>>),
}
pub type FactoryExecutionFailure<E> =
    ContextFailure<FactoryExecutionError<E>, FactoryAssemblyError<E>>;
pub struct FactoryAssemblySolution<I, S, W> {
    pub working: I,
    pub score: S,
    pub phases: Vec<W>,
    pub logical_calculations: u128,
}
/// An independently owned scoring run. Same-mode phases retain the resource;
/// mode transitions use cached preparation and copy current working input once.
/// The coordinator installs existing context-manager boundary diagnostics/counts;
/// per-step checks remain with the native phase runner. No thread is launched.
pub struct AssembledFactoryRun<'f, 'a, M, P, I, D: ContextScoreResource>
where
    D::Error: 'static,
{
    owner: &'f AssembledSolverFactory<'a, M, P, I, D>,
    manager: AssemblyManager<'a, M, P, I, D>,
    next_phase: usize,
}
impl<'a, M: 'a, P: 'a, I: 'a, D: ContextScoreResource + 'a> AssembledSolverFactory<'a, M, P, I, D>
where
    D::Error: 'static,
{
    pub fn assembled_model(&self) -> &Rc<M> {
        self.bank.factory_model()
    }
    pub fn assembled_schema(&self) -> &Rc<PlanningSchema> {
        &self.schema
    }
    pub fn assembled_scoring_factory(&self) -> &PreparedScoreFactory<'a, M, P, I, D, D::Error> {
        &self.bank
    }
    pub fn assembled_global_mode(&self) -> EnvironmentMode {
        self.global
    }
    pub fn assembled_phase_modes(&self) -> &[EnvironmentMode] {
        &self.modes
    }
    pub fn assembled_worker_budget(&self) -> Option<usize> {
        self.workers
    }
    pub fn open_assembled_random(&self) -> FactoryRandomStream {
        FactoryRandomStream::seeded_factory_stream(self.seed)
    }
    pub fn start_assembled_run(
        &self,
        input: I,
    ) -> Result<AssembledFactoryRun<'_, 'a, M, P, I, D>, FactoryExecutionFailure<D::Error>> {
        // Match the existing context-manager reporting contract explicitly.
        let matching =
            if self.metrics.best_constraint_totals || self.global >= EnvironmentMode::PhaseAssert {
                ContextMatchPolicy::Enabled
            } else {
                ContextMatchPolicy::Disabled
            };
        let director = self
            .bank
            .score_factory_builder_in(input, self.global)
            .with_factory_matching(matching)
            .instantiate_score_director()
            .map_err(|e| {
                ContextFailure::new(FactoryExecutionError::Build(FactoryAssemblyError::Scoring(
                    e,
                )))
            })?;
        let bank = self.bank.clone();
        let rebuild: AssemblyRebuilder<'a, M, P, I, D> = Box::new(move |old, settings| {
            let prepared = bank
                .select_scoring_factory(settings.mode)
                .map_err(FactoryAssemblyError::Scoring)?;
            let input = (bank.registry.recipe.child_input)(&old.0.resource).map_err(|e| {
                FactoryAssemblyError::Scoring(ScoreFactoryBuildError::ChildInput(e))
            })?;
            let director = prepared
                .score_factory_builder_in(input, settings.mode)
                .with_factory_matching(settings.matching)
                .instantiate_score_director()
                .map_err(FactoryAssemblyError::Scoring)?;
            Ok(AssemblyResource(director))
        });
        let mut manager = ScoringContextManager::new(
            AssemblyResource(director),
            self.global,
            self.modes.clone(),
            self.metrics,
            rebuild,
        );
        if let Err(e) = manager.start() {
            let mut failure = ContextFailure::new(FactoryExecutionError::Context(e));
            manager.solving_error(&mut failure);
            return Err(failure);
        }
        Ok(AssembledFactoryRun {
            owner: self,
            manager,
            next_phase: 0,
        })
    }
    /// Execute the declared native phases through an application binding. The
    /// binding receives the actual retained director and exact configured phase,
    /// and returns a real ContextScoreWork result. No scores/results are supplied
    /// by the factory. Errors retain the primary typed failure plus release errors.
    pub fn solve_assembled_with<W: ContextScoreWork>(
        &self,
        input: I,
        mut run: impl FnMut(&mut D, &SolverPhase<'a, D::Score>) -> Result<W, D::Error>,
    ) -> Result<FactoryAssemblySolution<I, D::Score, W>, FactoryExecutionFailure<D::Error>> {
        let mut session = self.start_assembled_run(input)?;
        let mut results = vec![];
        while let Some(work) = session.run_next_assembled_phase(&mut run)? {
            results.push(work);
        }
        let (working, score, count) = session.finish_assembled_state()?;
        Ok(FactoryAssemblySolution {
            working,
            score,
            phases: results,
            logical_calculations: count,
        })
    }
}
impl<'a, M: 'a, P: 'a, I: 'a, D: ContextScoreResource + 'a> AssembledFactoryRun<'_, 'a, M, P, I, D>
where
    D::Error: 'static,
{
    fn context_failure(
        &mut self,
        primary: ScoringContextError<FactoryAssemblyError<D::Error>>,
    ) -> FactoryExecutionFailure<D::Error> {
        let mut failure = ContextFailure::new(FactoryExecutionError::Context(primary));
        self.manager.solving_error(&mut failure);
        failure
    }
    pub fn inspect_assembled_resource<R>(&self, read: impl FnOnce(&D) -> R) -> Option<R> {
        let handle = self.manager.handle();
        let director = handle.director()?;
        Some(read(&director.0.resource))
    }
    pub fn assembled_runtime_settings(&self) -> Option<super::FactoryDirectorSettings> {
        let handle = self.manager.handle();
        Some(handle.director()?.0.factory_director_settings())
    }
    pub fn run_next_assembled_phase<W: ContextScoreWork>(
        &mut self,
        mut run: impl FnMut(&mut D, &SolverPhase<'a, D::Score>) -> Result<W, D::Error>,
    ) -> Result<Option<W>, FactoryExecutionFailure<D::Error>> {
        let Some(phase) = self.owner.phases.get(self.next_phase) else {
            return Ok(None);
        };
        if let Err(e) = self.manager.start_phase(self.next_phase) {
            return Err(self.context_failure(e));
        }
        match self.manager.run(|director| {
            run(&mut director.0.resource, phase).map_err(FactoryAssemblyError::Resource)
        }) {
            Ok(work) => {
                self.next_phase += 1;
                Ok(Some(work))
            }
            Err(e) => Err(self.context_failure(e)),
        }
    }
    pub fn finish_assembled_state(
        mut self,
    ) -> Result<(I, D::Score, u128), FactoryExecutionFailure<D::Error>> {
        let handle = self.manager.handle();
        let state = handle.director().ok_or_else(|| {
            ContextFailure::new(FactoryExecutionError::Context(ScoringContextError::Closed))
        })?;
        let input = (self.owner.bank.registry.recipe.child_input)(&state.0.resource);
        let score = state.context_score();
        drop(state);
        let input = match input {
            Ok(input) => input,
            Err(e) => {
                return Err(self.context_failure(ScoringContextError::Scoring(
                    FactoryAssemblyError::Scoring(ScoreFactoryBuildError::ChildInput(e)),
                )));
            }
        };
        let count = self.manager.calculation_count();
        self.manager
            .finish()
            .map_err(|e| ContextFailure::new(FactoryExecutionError::Context(e)))?;
        Ok((input, score, count))
    }
}
