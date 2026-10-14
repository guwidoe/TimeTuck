//! Owned working copies connected to executable planning bindings and native solves.
use crate::{
    Assignment, ConfiguredSolverManager, FiniteProblem, PlanningHierarchyError,
    PlanningSolutionBinding, SolverFactory, SolverFactoryError, SolverProgress, SolverResult,
    SolverSequence,
};
use std::{error::Error, fmt, sync::Arc};

/// Failure before publication of a fully solved, committed working copy.
#[derive(Debug)]
pub enum ModelSessionError<E> {
    Binding(PlanningHierarchyError),
    Solve(E),
    MissingScore,
}
impl<E: fmt::Display> fmt::Display for ModelSessionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Solve(e) => e.fmt(f),
            Self::MissingScore => f.write_str("native solve returned no score"),
        }
    }
}
impl<E: Error + 'static> Error for ModelSessionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Binding(e) => Some(e),
            Self::Solve(e) => Some(e),
            Self::MissingScore => None,
        }
    }
}

/// Reusable session boundary around a genuine solution layout. Every job clones
/// its retained input, opens live ranges, runs the caller's native solver, then
/// validates and commits its assignment before setting the returned score.
///
/// Model Clone must create independent mutable entity storage. The native callback
/// must consume the effective sequence (including per-job cancellation), construct
/// independent scoring state and return its actual assignment, score and work.
/// Partially initialized results can be published when a native limit stops early;
/// missing scores reject. Binding projection/setter contracts still apply.
pub struct ModelSolverSession<S, V, E, Score: 'static = crate::SimpleScore> {
    factory: SolverFactory<Arc<S>, S, ModelSessionError<E>, Score>,
    value: std::marker::PhantomData<fn() -> V>,
}
impl<S, V, E, Score> ModelSolverSession<S, V, E, Score>
where
    S: Clone + Send + Sync + 'static,
    V: Clone + Eq + 'static,
    E: Send + Sync + 'static,
    Score: Clone + Send + Sync + 'static,
{
    pub fn new(
        binding: PlanningSolutionBinding<S, V>,
        configuration: SolverSequence<'static, Score>,
        run: impl for<'a> Fn(
            FiniteProblem<V>,
            SolverSequence<'a, Score>,
            &SolverProgress<S>,
        ) -> Result<SolverResult<Assignment<V>, Score>, E>
        + Send
        + Sync
        + 'static,
        set_score: impl Fn(&mut S, Score) + Send + Sync + 'static,
    ) -> Result<Self, SolverFactoryError> {
        let factory = SolverFactory::new(configuration, move |input: Arc<S>, config, progress| {
            let mut working = input.as_ref().clone();
            let model = binding.open(&working).map_err(ModelSessionError::Binding)?;
            let result = run(model, config, &progress).map_err(ModelSessionError::Solve)?;
            let score = result.score.ok_or(ModelSessionError::MissingScore)?;
            binding
                .commit(&mut working, &result.assignment)
                .map_err(ModelSessionError::Binding)?;
            set_score(&mut working, score);
            progress.record_work(result.work);
            Ok(working)
        })?;
        Ok(Self {
            factory,
            value: std::marker::PhantomData,
        })
    }
    pub fn factory(&self) -> &SolverFactory<Arc<S>, S, ModelSessionError<E>, Score> {
        &self.factory
    }
    pub fn build_manager(
        &self,
        workers: usize,
    ) -> std::io::Result<ConfiguredSolverManager<Arc<S>, S, ModelSessionError<E>, Score>> {
        self.factory.build_manager(workers)
    }
}
