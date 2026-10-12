//! Native factory construction and shared configured-submission conveniences.
use crate::*;
use std::{error::Error, fmt, io, time::Duration};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolverTimeOverride {
    pub max_time: Option<Duration>,
    pub max_unimproved_time: Option<Duration>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverManagerConfig {
    pub parallel_solver_count: usize,
    pub consumer_count: usize,
}
impl Default for SolverManagerConfig {
    fn default() -> Self {
        Self {
            parallel_solver_count: 1,
            consumer_count: 1,
        }
    }
}
#[derive(Debug)]
pub enum ConfiguredManagerCreateError {
    Factory(SolverFactoryError),
    Pool(io::Error),
}
impl fmt::Display for ConfiguredManagerCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Factory(e) => e.fmt(f),
            Self::Pool(e) => e.fmt(f),
        }
    }
}
impl Error for ConfiguredManagerCreateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Factory(e) => e,
            Self::Pool(e) => e,
        })
    }
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
> SolverFactory<P, R, E, S>
{
    /// Native defaults deliberately use one solver/consumer; no Java AUTO policy.
    pub fn build_default_manager(&self) -> io::Result<ConfiguredSolverManager<P, R, E, S>> {
        self.build_manager_with_config(SolverManagerConfig::default())
    }
    pub fn build_manager_with_config(
        &self,
        config: SolverManagerConfig,
    ) -> io::Result<ConfiguredSolverManager<P, R, E, S>> {
        Ok(self.with_manager(SolverManager::with_consumer_count(
            config.parallel_solver_count,
            config.consumer_count,
        )?))
    }
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
> ConfiguredSolverManager<P, R, E, S>
{
    pub fn from_config(
        config: impl Into<SolverSequence<'static, S>>,
        run: impl for<'a> Fn(P, SolverSequence<'a, S>, SolverProgress<R>) -> Result<R, E>
        + Send
        + Sync
        + 'static,
    ) -> Result<Self, ConfiguredManagerCreateError> {
        Self::from_config_with_manager(config, run, SolverManagerConfig::default())
    }
    pub fn from_config_with_manager(
        config: impl Into<SolverSequence<'static, S>>,
        run: impl for<'a> Fn(P, SolverSequence<'a, S>, SolverProgress<R>) -> Result<R, E>
        + Send
        + Sync
        + 'static,
        manager: SolverManagerConfig,
    ) -> Result<Self, ConfiguredManagerCreateError> {
        let factory =
            SolverFactory::new(config, run).map_err(ConfiguredManagerCreateError::Factory)?;
        factory
            .build_manager_with_config(manager)
            .map_err(ConfiguredManagerCreateError::Pool)
    }
}
/// A submission builder contract. Default manager helpers share these actual
/// setter/run calls with alternate implementations; they do not bypass validation.
pub trait SolverSubmissionBuilder<P, R>: Sized {
    type Job;
    type Error;
    fn with_problem(self, problem: P) -> Self;
    fn with_final_consumer(self, consumer: impl FnOnce(u64, &R) + Send + 'static) -> Self;
    fn with_best_consumer(self, consumer: impl FnMut(u64, &R) + Send + 'static) -> Self;
    fn run(self) -> Result<Self::Job, Self::Error>;
}
pub trait SolverSubmissionManager<P, R> {
    type Job;
    type Error;
    type Builder<'a>: SolverSubmissionBuilder<P, R, Job = Self::Job, Error = Self::Error>
    where
        Self: 'a;
    fn prepare_job(&self, id: u64) -> Self::Builder<'_>;
    fn solve_problem(&self, id: u64, problem: P) -> Result<Self::Job, Self::Error> {
        self.prepare_job(id).with_problem(problem).run()
    }
    fn solve_with_final(
        &self,
        id: u64,
        problem: P,
        consumer: impl FnOnce(u64, &R) + Send + 'static,
    ) -> Result<Self::Job, Self::Error> {
        self.prepare_job(id)
            .with_problem(problem)
            .with_final_consumer(consumer)
            .run()
    }
    fn solve_and_listen(
        &self,
        id: u64,
        problem: P,
        consumer: impl FnMut(u64, &R) + Send + 'static,
    ) -> Result<Self::Job, Self::Error> {
        self.prepare_job(id)
            .with_problem(problem)
            .with_best_consumer(consumer)
            .run()
    }
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
> SolverSubmissionBuilder<P, R> for ConfiguredSolverJobBuilder<'_, P, R, E, S>
{
    type Job = ConfiguredSolverJob<R, E, S>;
    type Error = ConfiguredSolverJobBuildError;
    fn with_problem(self, problem: P) -> Self {
        Self::with_problem(self, problem)
    }
    fn with_final_consumer(self, consumer: impl FnOnce(u64, &R) + Send + 'static) -> Self {
        Self::with_final_consumer(self, consumer)
    }
    fn with_best_consumer(self, consumer: impl FnMut(u64, &R) + Send + 'static) -> Self {
        Self::with_best_consumer(self, consumer)
    }
    fn run(self) -> Result<Self::Job, Self::Error> {
        Self::run(self)
    }
}
impl<
    P: Send + 'static,
    R: Send + Sync + 'static,
    E: Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
> SolverSubmissionManager<P, R> for ConfiguredSolverManager<P, R, E, S>
{
    type Job = ConfiguredSolverJob<R, E, S>;
    type Error = ConfiguredSolverJobBuildError;
    type Builder<'a>
        = ConfiguredSolverJobBuilder<'a, P, R, E, S>
    where
        Self: 'a;
    fn prepare_job(&self, id: u64) -> Self::Builder<'_> {
        self.job_builder(id)
    }
}
