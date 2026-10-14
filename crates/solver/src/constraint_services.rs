//! Immutable constraint recipes shared by native solving and solution operations.
use crate::*;
use std::{fmt, io, ops::Deref, sync::Arc};

/// An actual graph and the planning source which its terminals consume. Each
/// compilation must allocate independent graph state; no graph is shared by jobs.
pub struct CompiledConstraintGraph<V, S: AdditiveScore> {
    pub graph: StreamGraph<'static, S>,
    pub source: PlanningStreamSource<V>,
}
/// Explicit concrete provider registration. The operation borrows the provider
/// immutably. Implementations must keep their scoring recipe immutable and must
/// not retain shared mutable graph state. Rust interior mutability is not prohibited
/// by this interface, and this is not a class scanner or a framework bean factory.
pub trait ReadOnlyConstraintProvider<V, S: AdditiveScore>: Send + Sync {
    fn compile(&self) -> Result<CompiledConstraintGraph<V, S>, GraphError>;
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintMetaDefinition<S> {
    pub name: String,
    pub weight: S,
}
/// Snapshot of the real weighted terminals compiled at factory construction.
/// The snapshot is immutable and shared; every later compilation checks it before
/// loading any planning rows. Legacy unweighted terminals have no declared weight
/// and are deliberately outside this resource's profile.
#[derive(Debug, Eq, PartialEq)]
pub struct ConstraintMetaModel<S> {
    constraints: Vec<ConstraintMetaDefinition<S>>,
}
impl<S> ConstraintMetaModel<S> {
    pub fn constraints(&self) -> &[ConstraintMetaDefinition<S>] {
        &self.constraints
    }
}
#[derive(Debug)]
pub enum ConstraintServiceError {
    Graph(GraphError),
    Scoring(StreamScoringError),
    UnweightedTerminal,
    MetadataChanged,
    ScoreMismatch { expected: String, actual: String },
}
impl fmt::Display for ConstraintServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "constraint service: {self:?}")
    }
}
impl std::error::Error for ConstraintServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Graph(e) => Some(e),
            Self::Scoring(e) => Some(e),
            _ => None,
        }
    }
}
impl From<GraphError> for ConstraintServiceError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}
impl From<StreamScoringError> for ConstraintServiceError {
    fn from(e: StreamScoringError) -> Self {
        Self::Scoring(e)
    }
}
fn metadata<S: AdditiveScore>(
    graph: &StreamGraph<'_, S>,
) -> Result<ConstraintMetaModel<S>, ConstraintServiceError> {
    let constraints: Vec<_> = graph
        .constraint_weights()
        .map(|(name, weight)| ConstraintMetaDefinition {
            name: name.into(),
            weight: weight.clone(),
        })
        .collect();
    if graph.constraint_scores().count() != constraints.len() {
        return Err(ConstraintServiceError::UnweightedTerminal);
    }
    Ok(ConstraintMetaModel { constraints })
}
fn validate_source<V: 'static, S: AdditiveScore>(
    prepared: &CompiledConstraintGraph<V, S>,
) -> Result<(), ConstraintServiceError> {
    if !prepared.graph.source_rows(&prepared.source)?.is_empty() {
        return Err(ConstraintServiceError::Scoring(
            StreamScoringError::PlanningSourceNotEmpty,
        ));
    }
    Ok(())
}
/// A reusable recipe, with the exact provider and metadata identities actually
/// consulted by solver callbacks, solution updates and verifier operations.
pub struct ScoreFactoryResource<V, S: AdditiveScore> {
    provider: Arc<dyn ReadOnlyConstraintProvider<V, S>>,
    meta_model: Arc<ConstraintMetaModel<S>>,
}
impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static> ScoreFactoryResource<V, S> {
    pub fn new(
        provider: Arc<dyn ReadOnlyConstraintProvider<V, S>>,
    ) -> Result<Self, ConstraintServiceError> {
        let prepared = provider.compile()?;
        validate_source(&prepared)?;
        Ok(Self {
            provider,
            meta_model: Arc::new(metadata(&prepared.graph)?),
        })
    }
    pub fn provider(&self) -> &Arc<dyn ReadOnlyConstraintProvider<V, S>> {
        &self.provider
    }
    pub fn meta_model(&self) -> &Arc<ConstraintMetaModel<S>> {
        &self.meta_model
    }
    pub fn compile(&self) -> Result<CompiledConstraintGraph<V, S>, ConstraintServiceError> {
        let prepared = self.provider.compile()?;
        validate_source(&prepared)?;
        if metadata(&prepared.graph)? != *self.meta_model {
            return Err(ConstraintServiceError::MetadataChanged);
        }
        Ok(prepared)
    }
    pub fn analyze(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
    ) -> Result<ConstraintServiceAnalysis<S>, ConstraintServiceError> {
        let prepared = self.compile()?;
        let director =
            StreamScoreDirector::new(problem, prepared.graph, prepared.source, assignment.clone())?;
        Ok(ConstraintServiceAnalysis {
            score: director.score(),
            constraint_scores: director
                .constraint_scores()
                .map(|(name, score)| (name.into(), score))
                .collect(),
            meta_model: self.meta_model.clone(),
        })
    }
    pub fn score(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
    ) -> Result<S, ConstraintServiceError> {
        Ok(self.analyze(problem, assignment)?.score)
    }
}
/// Actual named totals from an independently initialized scoring director. This
/// report includes the director's structural score; totals describe graph impacts.
/// Match justifications/indictments remain the existing typed explanation APIs.
pub struct ConstraintServiceAnalysis<S> {
    pub score: S,
    pub constraint_scores: Vec<(String, S)>,
    pub meta_model: Arc<ConstraintMetaModel<S>>,
}
pub struct ConstraintVerifier<V, S: AdditiveScore> {
    resource: Arc<ScoreFactoryResource<V, S>>,
}
impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static> ConstraintVerifier<V, S> {
    pub fn new(resource: Arc<ScoreFactoryResource<V, S>>) -> Self {
        Self { resource }
    }
    pub fn score_factory(&self) -> &Arc<ScoreFactoryResource<V, S>> {
        &self.resource
    }
    pub fn given_solution(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
    ) -> Result<S, ConstraintServiceError> {
        self.resource.score(problem, assignment)
    }
    pub fn verify_score(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
        expected: &S,
    ) -> Result<(), ConstraintServiceError> {
        let actual = self.given_solution(problem, assignment)?;
        if &actual == expected {
            Ok(())
        } else {
            Err(ConstraintServiceError::ScoreMismatch {
                expected: format!("{expected:?}"),
                actual: format!("{actual:?}"),
            })
        }
    }
}
/// Solution operations use the same immutable scoring recipe as jobs. Each
/// update delegates publication/refresh policy to the existing SolutionManager.
pub struct ConstraintSolutionManager<V, S: AdditiveScore> {
    resource: Arc<ScoreFactoryResource<V, S>>,
}
impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static> ConstraintSolutionManager<V, S> {
    pub fn new(resource: Arc<ScoreFactoryResource<V, S>>) -> Self {
        Self { resource }
    }
    pub fn score_factory(&self) -> &Arc<ScoreFactoryResource<V, S>> {
        &self.resource
    }
    pub fn update(
        &self,
        problem: &FiniteProblem<V>,
        solution: &mut ManagedSolution<Assignment<V>, (), S>,
        policy: SolutionUpdatePolicy,
    ) -> Result<Option<S>, ConstraintServiceError> {
        let mut manager = SolutionManager::new(
            |_| Ok(()),
            |assignment, _| {
                problem
                    .validate(assignment)
                    .map_err(|e| ConstraintServiceError::Scoring(e.into()))
            },
            |assignment, _| self.resource.score(problem, assignment),
        );
        Ok(manager.update_with_policy(solution, policy)?.cloned())
    }
    pub fn analyze(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
    ) -> Result<ConstraintServiceAnalysis<S>, ConstraintServiceError> {
        self.resource.analyze(problem, assignment)
    }
}
/// Canonical factory object retained by the bound manager. The native worker
/// callback receives this factory's exact scoring resource, not a reconstructed
/// recipe. It must consume the supplied configuration and independent input.
pub struct SharedConstraintSolverFactory<P, R, E, V, S: AdditiveScore + 'static> {
    resource: Arc<ScoreFactoryResource<V, S>>,
    runtime: SolverFactory<P, R, E, S>,
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    V: Eq + Clone + 'static,
    S: AdditiveScore + Send + Sync + 'static,
> SharedConstraintSolverFactory<P, R, E, V, S>
{
    pub fn new(
        resource: Arc<ScoreFactoryResource<V, S>>,
        configuration: impl Into<SolverSequence<'static, S>>,
        run: impl for<'a> Fn(
            &Arc<ScoreFactoryResource<V, S>>,
            P,
            SolverSequence<'a, S>,
            SolverProgress<R>,
        ) -> Result<R, E>
        + Send
        + Sync
        + 'static,
    ) -> Result<Arc<Self>, SolverFactoryError> {
        let scoring = resource.clone();
        let runtime =
            SolverFactory::new(configuration, move |problem, configuration, progress| {
                run(&scoring, problem, configuration, progress)
            })?;
        Ok(Arc::new(Self { resource, runtime }))
    }
    pub fn score_factory(&self) -> &Arc<ScoreFactoryResource<V, S>> {
        &self.resource
    }
    pub fn runtime(&self) -> &SolverFactory<P, R, E, S> {
        &self.runtime
    }
    pub fn solution_manager(&self) -> ConstraintSolutionManager<V, S> {
        ConstraintSolutionManager::new(self.resource.clone())
    }
    pub fn constraint_verifier(&self) -> ConstraintVerifier<V, S> {
        ConstraintVerifier::new(self.resource.clone())
    }
    pub fn meta_model(&self) -> &Arc<ConstraintMetaModel<S>> {
        self.resource.meta_model()
    }
    pub fn build_manager(
        self: &Arc<Self>,
        workers: usize,
    ) -> io::Result<SharedConstraintSolverManager<P, R, E, V, S>> {
        Ok(SharedConstraintSolverManager {
            factory: self.clone(),
            runtime: self.runtime.build_manager(workers)?,
        })
    }
}
pub struct SharedConstraintSolverManager<P, R, E, V, S: AdditiveScore + 'static> {
    factory: Arc<SharedConstraintSolverFactory<P, R, E, V, S>>,
    runtime: ConfiguredSolverManager<P, R, E, S>,
}
impl<P, R, E, V, S: AdditiveScore + 'static> SharedConstraintSolverManager<P, R, E, V, S> {
    pub fn factory(&self) -> &Arc<SharedConstraintSolverFactory<P, R, E, V, S>> {
        &self.factory
    }
}
impl<P, R, E, V, S: AdditiveScore + 'static> Deref
    for SharedConstraintSolverManager<P, R, E, V, S>
{
    type Target = ConfiguredSolverManager<P, R, E, S>;
    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    V,
    S: AdditiveScore + Send + Sync + 'static,
> SharedConstraintSolverManager<P, R, E, V, S>
{
    pub fn shutdown(&mut self) -> io::Result<()> {
        self.runtime.shutdown()
    }
}
