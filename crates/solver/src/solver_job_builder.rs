//! Configure an owned job before submitting it to the existing worker pool.
use crate::{
    CancellationToken, ProblemSizeStatistics, SolverJob, SolverJobCallbacks, SolverManager,
    SolverProgress, SubmitError,
};
use std::{error::Error, fmt};

type Finder<P, E> = Box<dyn FnOnce(u64) -> Result<P, E> + Send>;
type Run<P, R, E> = Box<dyn FnOnce(P, CancellationToken, SolverProgress<R>) -> Result<R, E> + Send>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverJobBuildError {
    MissingProblem,
    Submission(SubmitError),
}
impl fmt::Display for SolverJobBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProblem => f.write_str("a problem or problem finder is required"),
            Self::Submission(e) => e.fmt(f),
        }
    }
}
impl Error for SolverJobBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingProblem => None,
            Self::Submission(e) => Some(e),
        }
    }
}

/// Configure one submission with an owned problem or a deferred, fallible lookup.
/// Nothing is scheduled until `run`. Repeated source/callback/size setters replace
/// the previous setting. The finder executes once on the solver worker, after the
/// started callback and before solving; its time is included in job statistics.
/// Lookup errors and unwinding panics use normal solve-error delivery. Rejection or
/// cancellation while queued never invokes the finder, solver or callbacks.
///
/// Problem and result types may differ; neither must implement Clone. The problem
/// is captured by the solve task, so queued cancellation drops it and returns
/// CancelledBeforeStart. Use SolverManager::submit_owned when queued cancellation
/// must return the original input. An unused builder also drops its captures.
///
/// ```
/// use timetuck::{SolverManager, SolverJobCallbacks};
/// let mut manager = SolverManager::<usize, String>::new(1)?;
/// let job = manager.job_builder(7, |problem: String, _, _| Ok(problem.len()))
///     .with_problem_finder(|id| Ok(format!("job-{id}")))
///     .with_callbacks(SolverJobCallbacks::default().on_final(|id, size| {
///         assert_eq!((id, *size), (7, 5));
///     }))
///     .run()?;
/// assert_eq!(job.wait().as_ref().as_ref().unwrap(), &5);
/// manager.shutdown()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SolverJobBuilder<'a, P, R, E> {
    manager: &'a SolverManager<R, E>,
    problem_id: u64,
    finder: Option<Finder<P, E>>,
    solve: Run<P, R, E>,
    callbacks: SolverJobCallbacks<R, E>,
    size: Option<ProblemSizeStatistics>,
}
impl<R: Send + Sync + 'static, E: Send + Sync + 'static> SolverManager<R, E> {
    pub fn job_builder<P: Send + 'static>(
        &self,
        problem_id: u64,
        solve: impl FnOnce(P, CancellationToken, SolverProgress<R>) -> Result<R, E> + Send + 'static,
    ) -> SolverJobBuilder<'_, P, R, E> {
        SolverJobBuilder {
            manager: self,
            problem_id,
            finder: None,
            solve: Box::new(solve),
            callbacks: SolverJobCallbacks::default(),
            size: None,
        }
    }
}
impl<P: Send + 'static, R: Send + Sync + 'static, E: Send + Sync + 'static>
    SolverJobBuilder<'_, P, R, E>
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
    /// Supply caller-computed metadata visible even while this job is queued.
    /// The solver can subsequently report revisioned metadata through its publisher.
    pub fn with_problem_size(mut self, size: ProblemSizeStatistics) -> Self {
        self.size = Some(size);
        self
    }
    pub fn run(self) -> Result<SolverJob<R, E>, SolverJobBuildError> {
        let finder = self.finder.ok_or(SolverJobBuildError::MissingProblem)?;
        let id = self.problem_id;
        let solve = self.solve;
        let run = move |token, progress| solve(finder(id)?, token, progress);
        match self.size {
            Some(size) => self
                .manager
                .submit_with_statistics(id, size, run, self.callbacks),
            None => self.manager.submit_with_progress(id, run, self.callbacks),
        }
        .map_err(SolverJobBuildError::Submission)
    }
}
