//! Reusable native solve functions with immutable defaults and per-job settings.
use crate::*;
use std::{error::Error, fmt, io, ops::Deref, sync::Arc, time::Duration};

type FactoryRun<P, R, E, S> =
    dyn for<'a> Fn(P, SolverSequence<'a, S>, SolverProgress<R>) -> Result<R, E> + Send + Sync;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverFactoryError {
    EmptyPhases,
    SharedCancellationToken,
    PhaseCancellationToken { phase: usize },
}
impl fmt::Display for SolverFactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPhases => f.write_str("a solver factory requires at least one phase"),
            Self::SharedCancellationToken => {
                f.write_str("solver factory cancellation belongs to each job")
            }
            Self::PhaseCancellationToken { phase } => write!(
                f,
                "solver factory phase {phase} must not retain a cancellation token"
            ),
        }
    }
}
impl Error for SolverFactoryError {}
fn validate<S>(config: &SolverSequence<'_, S>) -> Result<(), SolverFactoryError> {
    if config.phases.is_empty() {
        return Err(SolverFactoryError::EmptyPhases);
    }
    if config.limits.cancellation.is_some() {
        return Err(SolverFactoryError::SharedCancellationToken);
    }
    for (phase, p) in config.phases.iter().enumerate() {
        let cancellation = match p {
            SolverPhase::Construction(c) => c.cancellation,
            SolverPhase::LocalSearch(c) => c.cancellation,
            SolverPhase::RuinRecreate(c) => c.cancellation,
            SolverPhase::Exact(c) => c.cancellation,
            SolverPhase::Custom(c) => c.cancellation,
        };
        if cancellation.is_some() {
            return Err(SolverFactoryError::PhaseCancellationToken { phase });
        }
    }
    Ok(())
}
struct Factory<P, R, E, S: 'static> {
    config: SolverSequence<'static, S>,
    run: Box<FactoryRun<P, R, E, S>>,
}
/// Immutable default configuration and a reusable native solve function. The
/// function receives the effective sequence with the job's cancellation token
/// already bound. It must pass that sequence to the native solver to enforce its
/// limits. It may be called concurrently and must create independent working and
/// scoring state for each owned input. No Clone bound is imposed on P, R or E.
///
/// Templates reject cancellation references (including phase-local ones) so jobs
/// cannot accidentally share cancellation. Empty phase vectors are rejected.
/// All other model/phase validation remains with the native solver at execution.
///
/// ```
/// use timetuck::*;
/// let factory = SolverFactory::new(SolverConfig::<SimpleScore> {
///     local_search: vec![], ..Default::default()
/// }, |problem: FiniteProblem<i64>, config, progress| {
///     let n = ConstraintNetwork::<_, SimpleScore>::new(problem.variables().len());
///     let result = solve(&problem, &n, config).map_err(|e| e.to_string())?;
///     progress.record_work(result.work);
///     Ok::<_, String>(result.assignment.values().to_vec())
/// })?;
/// let mut manager = factory.build_manager(1)?;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])])?;
/// let job = manager.job_builder(1).with_problem(p).run()?;
/// assert_eq!(job.wait().as_ref().as_ref().unwrap(), &vec![Some(1)]);
/// assert_eq!(job.configuration().phases.len(), 1);
/// manager.shutdown()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SolverFactory<P, R, E, S: 'static = SimpleScore> {
    inner: Arc<Factory<P, R, E, S>>,
}
impl<P, R, E, S: 'static> Clone for SolverFactory<P, R, E, S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
> SolverFactory<P, R, E, S>
{
    pub fn new(
        config: impl Into<SolverSequence<'static, S>>,
        run: impl for<'a> Fn(P, SolverSequence<'a, S>, SolverProgress<R>) -> Result<R, E>
        + Send
        + Sync
        + 'static,
    ) -> Result<Self, SolverFactoryError> {
        let config = config.into();
        validate(&config)?;
        Ok(Self {
            inner: Arc::new(Factory {
                config,
                run: Box::new(run),
            }),
        })
    }
    pub fn configuration(&self) -> &SolverSequence<'static, S> {
        &self.inner.config
    }
    pub fn build_manager(&self, workers: usize) -> io::Result<ConfiguredSolverManager<P, R, E, S>> {
        Ok(self.with_manager(SolverManager::new(workers)?))
    }
    /// Attach these defaults to an existing native pool, including a pool created
    /// with custom consumer counts or a custom thread factory.
    pub fn with_manager(&self, pool: SolverManager<R, E>) -> ConfiguredSolverManager<P, R, E, S> {
        ConfiguredSolverManager {
            pool,
            factory: self.clone(),
        }
    }
}
/// A manager bound to a reusable factory. Existing pool methods are available via
/// Deref; arbitrary direct submissions still have their original closure contracts.
pub struct ConfiguredSolverManager<P, R, E, S: 'static = SimpleScore> {
    pool: SolverManager<R, E>,
    factory: SolverFactory<P, R, E, S>,
}
impl<P, R, E, S> Deref for ConfiguredSolverManager<P, R, E, S> {
    type Target = SolverManager<R, E>;
    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
> ConfiguredSolverManager<P, R, E, S>
{
    pub fn factory(&self) -> &SolverFactory<P, R, E, S> {
        &self.factory
    }
    pub fn job_builder(&self, problem_id: u64) -> ConfiguredSolverJobBuilder<'_, P, R, E, S> {
        ConfiguredSolverJobBuilder {
            manager: self,
            problem_id,
            finder: None,
            callbacks: SolverJobCallbacks::default(),
            size: None,
            config: self.factory.inner.config.clone(),
        }
    }
    pub fn shutdown(&mut self) -> io::Result<()> {
        self.pool.shutdown()
    }
}
/// A normal job handle plus the immutable effective configuration captured at
/// submission. Settings remain inspectable while queued, after failure/cancellation
/// and after ID reuse. Deref provides ordinary result/statistics/cancellation APIs.
pub struct ConfiguredSolverJob<R, E, S: 'static = SimpleScore> {
    job: SolverJob<R, E>,
    config: Arc<SolverSequence<'static, S>>,
}
impl<R, E, S: 'static> Clone for ConfiguredSolverJob<R, E, S> {
    fn clone(&self) -> Self {
        Self {
            job: self.job.clone(),
            config: self.config.clone(),
        }
    }
}
impl<R, E, S> Deref for ConfiguredSolverJob<R, E, S> {
    type Target = SolverJob<R, E>;
    fn deref(&self) -> &Self::Target {
        &self.job
    }
}
impl<R, E, S> ConfiguredSolverJob<R, E, S> {
    pub fn configuration(&self) -> &SolverSequence<'static, S> {
        &self.config
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfiguredSolverJobBuildError {
    Configuration(SolverFactoryError),
    Submission(SolverJobBuildError),
}
impl fmt::Display for ConfiguredSolverJobBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(e) => e.fmt(f),
            Self::Submission(e) => e.fmt(f),
        }
    }
}
impl Error for ConfiguredSolverJobBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Configuration(e) => e,
            Self::Submission(e) => e,
        })
    }
}
type Finder<P, E> = Box<dyn FnOnce(u64) -> Result<P, E> + Send>;
/// Per-job configuration copy. Whole-limit and whole-phase replacements apply in
/// call order; time convenience setters alter only their respective limit. None
/// explicitly disables that time limit. Factory defaults are never mutated.
/// Submission and queued cancellation follow SolverJobBuilder's captured-input
/// semantics, including CancelledBeforeStart rather than returning the input.
pub struct ConfiguredSolverJobBuilder<'a, P, R, E, S: 'static = SimpleScore> {
    manager: &'a ConfiguredSolverManager<P, R, E, S>,
    problem_id: u64,
    finder: Option<Finder<P, E>>,
    callbacks: SolverJobCallbacks<R, E>,
    size: Option<ProblemSizeStatistics>,
    config: SolverSequence<'static, S>,
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
> ConfiguredSolverJobBuilder<'_, P, R, E, S>
{
    pub fn with_problem(mut self, problem: P) -> Self {
        self.finder = Some(Box::new(move |_| Ok(problem)));
        self
    }
    pub fn with_problem_finder(
        mut self,
        finder: impl FnOnce(u64) -> Result<P, E> + Send + 'static,
    ) -> Self {
        self.finder = Some(Box::new(finder));
        self
    }
    pub fn with_callbacks(mut self, callbacks: SolverJobCallbacks<R, E>) -> Self {
        self.callbacks = callbacks;
        self
    }
    pub fn with_problem_size(mut self, size: ProblemSizeStatistics) -> Self {
        self.size = Some(size);
        self
    }
    pub fn with_limits(mut self, limits: SolverLimits<'static, S>) -> Self {
        self.config.limits = limits;
        self
    }
    pub fn with_phases(mut self, phases: Vec<SolverPhase<'static, S>>) -> Self {
        self.config.phases = phases;
        self
    }
    pub fn with_time_limit(mut self, limit: Option<Duration>) -> Self {
        self.config.limits.max_time = limit;
        self
    }
    pub fn with_unimproved_time_limit(mut self, limit: Option<Duration>) -> Self {
        self.config.limits.max_unimproved_time = limit;
        self
    }
    pub fn configuration(&self) -> &SolverSequence<'static, S> {
        &self.config
    }
    pub fn run(self) -> Result<ConfiguredSolverJob<R, E, S>, ConfiguredSolverJobBuildError> {
        validate(&self.config).map_err(ConfiguredSolverJobBuildError::Configuration)?;
        let config = Arc::new(self.config);
        let effective = config.clone();
        let factory = self.manager.factory.clone();
        let mut builder = self
            .manager
            .pool
            .job_builder(self.problem_id, move |problem, token, progress| {
                let mut sequence = effective.as_ref().clone();
                sequence.limits.cancellation = Some(&token);
                (factory.inner.run)(problem, sequence, progress)
            })
            .with_callbacks(self.callbacks);
        if let Some(finder) = self.finder {
            builder = builder.with_problem_finder(finder);
        }
        if let Some(size) = self.size {
            builder = builder.with_problem_size(size);
        }
        let job = builder
            .run()
            .map_err(ConfiguredSolverJobBuildError::Submission)?;
        Ok(ConfiguredSolverJob { job, config })
    }
}

impl<S> SolverLimits<'_, S> {
    /// Progress toward the configured time criteria, capped at one. The two
    /// criteria combine as OR (maximum gradient). Zero limits are immediately one;
    /// no time criteria gives None. Counts/scores/cancellation and phase-local
    /// limits are not included. Callers supply elapsed solving and unimproved time
    /// from their own clock; this method does not infer stagnation from job duration.
    pub fn time_gradient(&self, elapsed: Duration, unimproved: Duration) -> Option<f64> {
        fn ratio(elapsed: Duration, limit: Duration) -> f64 {
            if elapsed >= limit {
                1.0
            } else {
                elapsed.as_secs_f64() / limit.as_secs_f64()
            }
        }
        match (self.max_time, self.max_unimproved_time) {
            (None, None) => None,
            (Some(limit), None) => Some(ratio(elapsed, limit)),
            (None, Some(limit)) => Some(ratio(unimproved, limit)),
            (Some(a), Some(b)) => Some(ratio(elapsed, a).max(ratio(unimproved, b))),
        }
    }
}
