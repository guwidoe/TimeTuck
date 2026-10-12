//! Once-only benchmark sessions with a global warm-up barrier and typed failures.
use crate::*;
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkSessionState {
    Idle,
    Started,
    Completed,
    Cancelled,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkStartError {
    AlreadyStarted,
    NoConfigurations,
    NoDirectory,
}
impl fmt::Display for BenchmarkStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AlreadyStarted => "benchmarking already started",
            Self::NoConfigurations => "solverBenchmarkResultList is empty",
            Self::NoDirectory => "benchmarkDirectory is null",
        })
    }
}
impl Error for BenchmarkStartError {}
#[derive(Debug)]
pub enum BenchmarkExecutionError<E> {
    Start(BenchmarkStartError),
    Configuration(BenchmarkConfigError),
    Warmup(E),
}
impl<E: fmt::Display> fmt::Display for BenchmarkExecutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(e) => e.fmt(f),
            Self::Configuration(e) => e.fmt(f),
            Self::Warmup(e) => write!(f, "benchmark warm-up failed: {e}"),
        }
    }
}
impl<E: Error + 'static> Error for BenchmarkExecutionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Start(e) => Some(e),
            Self::Configuration(e) => Some(e),
            Self::Warmup(e) => Some(e),
        }
    }
}
#[derive(Debug)]
pub struct BenchmarkSessionResult<S, E> {
    pub trials: Vec<BenchmarkTrialResult<S, E>>,
    pub complete: bool,
}
impl<S, E> BenchmarkSessionResult<S, E> {
    pub fn is_successful(&self) -> bool {
        self.complete && self.trials.iter().all(|r| r.result.is_ok())
    }
}
/// Output directory is retained metadata; this runner does not create directories
/// or write Java reports. Missing configuration/directory and repeated starts fail
/// before callbacks. Successful start consumes this session, including after error,
/// cancellation or panic. Panics propagate and leave Started, never a false success.
///
/// All warm-ups finish before any measured callback. A warm-up error cancels only
/// the linked local token, joins active workers, and retains the original typed
/// error as its cause. Already running callbacks are cooperative, not preempted.
/// Measured failures remain in the returned report and do not abort other trials.
/// Native trial seeds/timing match BenchmarkSchedule; output indices are global.
/// No Java executor, file-output, scheduler trace or warm-up timing parity claim.
#[derive(Debug)]
pub struct BenchmarkSession {
    plan: Option<PreparedBenchmarkSuite>,
    directory: Option<PathBuf>,
    state: BenchmarkSessionState,
}
#[derive(Clone, Copy)]
struct Stage {
    warmup: bool,
    repetitions: usize,
    seed: u64,
}
impl BenchmarkSession {
    pub fn new(plan: Option<PreparedBenchmarkSuite>, directory: Option<PathBuf>) -> Self {
        Self {
            plan,
            directory,
            state: BenchmarkSessionState::Idle,
        }
    }
    pub fn state(&self) -> BenchmarkSessionState {
        self.state
    }
    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }
    pub fn start(&mut self) -> Result<(), BenchmarkStartError> {
        if self.state != BenchmarkSessionState::Idle {
            return Err(BenchmarkStartError::AlreadyStarted);
        }
        if self.plan.as_ref().is_none_or(|p| p.solvers().is_empty()) {
            return Err(BenchmarkStartError::NoConfigurations);
        }
        if self.directory.is_none() {
            return Err(BenchmarkStartError::NoDirectory);
        }
        self.state = BenchmarkSessionState::Started;
        Ok(())
    }
    pub fn run<P: Clone + Send + Sync, S: Send, E: Send>(
        &mut self,
        problems: &[P],
        warmup_runs: usize,
        seed: u64,
        cancellation: Option<&CancellationToken>,
        solve: impl Fn(
            &PreparedSolverBenchmark,
            P,
            BenchmarkTrial,
            Option<Duration>,
        ) -> Result<BenchmarkObservation<S>, E>
        + Sync,
    ) -> Result<BenchmarkSessionResult<S, E>, BenchmarkExecutionError<E>> {
        self.start().map_err(BenchmarkExecutionError::Start)?;
        let plan = self.plan.as_ref().expect("started plan");
        if problems.is_empty() {
            self.state = BenchmarkSessionState::Failed;
            return Err(BenchmarkExecutionError::Configuration(
                BenchmarkConfigError::Schedule(BenchmarkScheduleError::NoProblems),
            ));
        }
        let planned = plan.solvers().iter().try_fold(0usize, |sum, c| {
            warmup_runs
                .checked_add(c.sub_runs.get())
                .and_then(|n| n.checked_mul(problems.len()))
                .and_then(|n| sum.checked_add(n))
        });
        let Some(planned) = planned else {
            self.state = BenchmarkSessionState::Failed;
            return Err(BenchmarkExecutionError::Configuration(
                BenchmarkConfigError::Schedule(BenchmarkScheduleError::TooManyTrials),
            ));
        };
        let token = CancellationToken::linked(cancellation);
        let mut warmups = self.stage(
            problems,
            Stage {
                warmup: true,
                repetitions: warmup_runs,
                seed,
            },
            &token,
            &solve,
        );
        if let Some(index) = warmups.iter().position(|t| t.result.is_err()) {
            self.state = BenchmarkSessionState::Failed;
            let error = warmups
                .swap_remove(index)
                .result
                .err()
                .expect("failed trial");
            return Err(BenchmarkExecutionError::Warmup(error));
        }
        if !token.is_cancelled() {
            warmups.extend(self.stage(
                problems,
                Stage {
                    warmup: false,
                    repetitions: 0,
                    seed,
                },
                &token,
                &solve,
            ));
        }
        let complete = warmups.len() == planned;
        self.state = if complete {
            BenchmarkSessionState::Completed
        } else {
            BenchmarkSessionState::Cancelled
        };
        Ok(BenchmarkSessionResult {
            trials: warmups,
            complete,
        })
    }
    fn stage<P: Clone + Send + Sync, S: Send, E: Send>(
        &self,
        problems: &[P],
        stage: Stage,
        token: &CancellationToken,
        solve: &(
             impl Fn(
            &PreparedSolverBenchmark,
            P,
            BenchmarkTrial,
            Option<Duration>,
        ) -> Result<BenchmarkObservation<S>, E>
             + Sync
         ),
    ) -> Vec<BenchmarkTrialResult<S, E>> {
        let plan = self.plan.as_ref().expect("started plan");
        let next = AtomicUsize::new(0);
        let results = Mutex::new(
            (0..plan.solvers().len())
                .map(|_| Vec::new())
                .collect::<Vec<_>>(),
        );
        std::thread::scope(|scope| {
            for _ in 0..plan.parallel_benchmarks().min(plan.solvers().len()) {
                let (next, results) = (&next, &results);
                scope.spawn(move || {
                    loop {
                        let configuration = next.fetch_add(1, Ordering::Relaxed);
                        let Some(c) = plan.solvers().get(configuration) else {
                            break;
                        };
                        let count = if stage.warmup {
                            stage.repetitions
                        } else {
                            c.sub_runs.get()
                        };
                        let mut trials = Vec::new();
                        'trials: for repetition in 0..count {
                            for (problem, p) in problems.iter().enumerate() {
                                if token.is_cancelled() {
                                    break 'trials;
                                }
                                let input = p.clone(); // Input cloning remains outside callback timing.
                                let seed = stage.seed.wrapping_add(repetition as u64).wrapping_add(
                                    (problem as u64).wrapping_mul(0x9e3779b97f4a7c15),
                                ) ^ if stage.warmup { 0xd1b54a32d192ed03 } else { 0 };
                                let trial = BenchmarkTrial {
                                    configuration,
                                    problem,
                                    repetition,
                                    warmup: stage.warmup,
                                    seed,
                                };
                                let start = Instant::now();
                                let result = solve(
                                    c,
                                    input,
                                    trial,
                                    stage.warmup.then_some(plan.warmup_budget()),
                                );
                                let elapsed = start.elapsed();
                                if stage.warmup && result.is_err() {
                                    token.cancel();
                                }
                                trials.push(BenchmarkTrialResult {
                                    trial,
                                    elapsed,
                                    result,
                                });
                            }
                        }
                        results.lock().unwrap()[configuration] = trials;
                    }
                });
            }
        });
        results
            .into_inner()
            .unwrap()
            .into_iter()
            .flatten()
            .collect()
    }
}
