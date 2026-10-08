//! Repeatable serial benchmark scheduling with explicit measurement boundaries.
use crate::{
    BenchmarkOutcome, BenchmarkResultError, BenchmarkSummary, CancellationToken, Score, SolverWork,
};
use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BenchmarkSchedule {
    pub warmup_runs: usize,
    pub measured_runs: NonZeroUsize,
    pub seed: u64,
}
impl Default for BenchmarkSchedule {
    fn default() -> Self {
        Self {
            warmup_runs: 1,
            measured_runs: NonZeroUsize::new(3).unwrap(),
            seed: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkScheduleError {
    NoConfigurations,
    NoProblems,
    TooManyTrials,
}
impl fmt::Display for BenchmarkScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoConfigurations => "benchmark configuration list is empty",
            Self::NoProblems => "benchmark problem list is empty",
            Self::TooManyTrials => "benchmark trial count exceeds usize",
        })
    }
}
impl Error for BenchmarkScheduleError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BenchmarkTrial {
    pub configuration: usize,
    pub problem: usize,
    /// Zero-based within warm-ups or measured repetitions separately.
    pub repetition: usize,
    pub warmup: bool,
    /// Equal for every configuration on a given problem/repetition/stage.
    pub seed: u64,
}
#[derive(Clone, Debug)]
pub struct BenchmarkObservation<S> {
    pub score: S,
    pub initialized: bool,
    /// Supplied by the callback, using the solver's existing counter definitions.
    pub work: SolverWork,
}
#[derive(Debug)]
pub struct BenchmarkTrialResult<S, E> {
    pub trial: BenchmarkTrial,
    pub elapsed: Duration,
    pub result: Result<BenchmarkObservation<S>, E>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkTiming {
    pub successful_runs: usize,
    pub failed_runs: usize,
    /// Successful measured callbacks only; warm-ups and failures are excluded.
    pub successful_elapsed: Duration,
    pub fastest: Option<Duration>,
    pub slowest: Option<Duration>,
}
impl BenchmarkTiming {
    pub fn mean_seconds(&self) -> Option<f64> {
        (self.successful_runs != 0)
            .then(|| self.successful_elapsed.as_secs_f64() / self.successful_runs as f64)
    }
}
#[derive(Debug)]
pub struct BenchmarkRunReport<S, E> {
    configurations: usize,
    problems: usize,
    planned_trials: usize,
    trials: Vec<BenchmarkTrialResult<S, E>>,
}
impl<S, E> BenchmarkRunReport<S, E> {
    pub fn trials(&self) -> &[BenchmarkTrialResult<S, E>] {
        &self.trials
    }
    pub fn into_trials(self) -> Vec<BenchmarkTrialResult<S, E>> {
        self.trials
    }
    pub fn planned_trials(&self) -> usize {
        self.planned_trials
    }
    /// Every scheduled callback returned, including errors. This is not success.
    pub fn is_complete(&self) -> bool {
        self.trials.len() == self.planned_trials
    }
    /// False if cancellation prevented any scheduled callback from starting.
    /// Callback failures remain in the report and do not stop subsequent trials.
    pub fn is_successful(&self) -> bool {
        self.is_complete() && self.trials.iter().all(|t| t.result.is_ok())
    }
    pub fn timing(&self, configuration: usize, problem: usize) -> Option<BenchmarkTiming> {
        if configuration >= self.configurations || problem >= self.problems {
            return None;
        }
        let mut timing = BenchmarkTiming {
            successful_runs: 0,
            failed_runs: 0,
            successful_elapsed: Duration::ZERO,
            fastest: None,
            slowest: None,
        };
        for t in self.trials.iter().filter(|t| {
            !t.trial.warmup && t.trial.configuration == configuration && t.trial.problem == problem
        }) {
            if t.result.is_ok() {
                timing.successful_runs += 1;
                timing.successful_elapsed = timing.successful_elapsed.saturating_add(t.elapsed);
                timing.fastest = Some(timing.fastest.map_or(t.elapsed, |old| old.min(t.elapsed)));
                timing.slowest = Some(timing.slowest.map_or(t.elapsed, |old| old.max(t.elapsed)));
            } else {
                timing.failed_runs += 1;
            }
        }
        Some(timing)
    }
}
impl<S: Score, E> BenchmarkRunReport<S, E> {
    /// Aggregate measured outcomes for one configuration. A cancelled report may
    /// contain different run counts; compare summaries only after checking completeness.
    /// Warm-up failures are retained in trials/is_successful, never score rankings.
    pub fn summary(
        &self,
        configuration: usize,
    ) -> Result<Option<BenchmarkSummary>, BenchmarkResultError> {
        if configuration >= self.configurations {
            return Ok(None);
        }
        let outcomes = self
            .trials
            .iter()
            .filter(|t| !t.trial.warmup && t.trial.configuration == configuration)
            .map(|t| match &t.result {
                Ok(value) => BenchmarkOutcome::success(&value.score, value.initialized),
                Err(_) => Ok(BenchmarkOutcome::Failure),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(BenchmarkSummary::new(outcomes)))
    }
}
impl BenchmarkSchedule {
    /// Run all warm-ups before all measured trials. Within each stage the order
    /// is repetition, problem, configuration. Each callback receives a fresh
    /// P::clone; cloning is outside the timer and its depth follows P's Clone.
    /// Configuration and problem indices retain their supplied slice ordering.
    /// The callback must apply trial.seed to its solver to get matched randomness.
    /// Seed depends on problem, stage and repetition, not configuration or failures.
    ///
    /// Only callback execution is timed, including whatever solver setup it does.
    /// Score conversion, report aggregation, cloning, and scheduling are excluded.
    /// Cancellation is checked before each clone/callback; a running callback is
    /// cooperative and may use the same token itself. Panics propagate normally.
    /// Callback errors (including warm-up errors) are retained and execution continues.
    /// This is a serial harness, not statistical proof of a performance difference.
    ///
    /// ```
    /// use timetuck::*;
    /// let schedule=BenchmarkSchedule::default();
    /// let report=schedule.run(&[1_i64,2], &[vec![3_i64]], None, |factor, problem, _trial| {
    ///     Ok::<_, std::convert::Infallible>(BenchmarkObservation {
    ///         score:SimpleScore::new(factor*problem[0]),initialized:true,work:SolverWork::default(),
    ///     })
    /// })?;
    /// assert!(report.is_successful());
    /// assert_eq!(report.timing(0,0).unwrap().successful_runs,3);
    /// assert_eq!(report.summary(1)?.unwrap().run_count(),3);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn run<C, P: Clone, S, E>(
        &self,
        configurations: &[C],
        problems: &[P],
        cancellation: Option<&CancellationToken>,
        solve: impl FnMut(&C, P, BenchmarkTrial) -> Result<BenchmarkObservation<S>, E>,
    ) -> Result<BenchmarkRunReport<S, E>, BenchmarkScheduleError> {
        let origin = Instant::now();
        self.run_with_clock(configurations, problems, cancellation, solve, || {
            origin.elapsed()
        })
    }
    fn run_with_clock<C, P: Clone, S, E>(
        &self,
        configurations: &[C],
        problems: &[P],
        cancellation: Option<&CancellationToken>,
        mut solve: impl FnMut(&C, P, BenchmarkTrial) -> Result<BenchmarkObservation<S>, E>,
        mut clock: impl FnMut() -> Duration,
    ) -> Result<BenchmarkRunReport<S, E>, BenchmarkScheduleError> {
        if configurations.is_empty() {
            return Err(BenchmarkScheduleError::NoConfigurations);
        }
        if problems.is_empty() {
            return Err(BenchmarkScheduleError::NoProblems);
        }
        let planned_trials = self
            .warmup_runs
            .checked_add(self.measured_runs.get())
            .and_then(|n| n.checked_mul(problems.len()))
            .and_then(|n| n.checked_mul(configurations.len()))
            .ok_or(BenchmarkScheduleError::TooManyTrials)?;
        let mut report = BenchmarkRunReport {
            configurations: configurations.len(),
            problems: problems.len(),
            planned_trials,
            trials: vec![],
        };
        for (warmup, count) in [(true, self.warmup_runs), (false, self.measured_runs.get())] {
            for repetition in 0..count {
                for (problem, input) in problems.iter().enumerate() {
                    let seed = self
                        .seed
                        .wrapping_add(repetition as u64)
                        .wrapping_add((problem as u64).wrapping_mul(0x9e3779b97f4a7c15))
                        ^ if warmup { 0xd1b54a32d192ed03 } else { 0 };
                    for (configuration, config) in configurations.iter().enumerate() {
                        if cancellation.is_some_and(CancellationToken::is_cancelled) {
                            return Ok(report);
                        }
                        let input = input.clone();
                        let trial = BenchmarkTrial {
                            configuration,
                            problem,
                            repetition,
                            warmup,
                            seed,
                        };
                        let started = clock();
                        let result = solve(config, input, trial);
                        let elapsed = clock().saturating_sub(started);
                        report.trials.push(BenchmarkTrialResult {
                            trial,
                            elapsed,
                            result,
                        });
                    }
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    use std::{cell::Cell, rc::Rc};
    #[test]
    fn timing_excludes_clones_and_aggregates_only_successful_measurements() {
        struct Input(Rc<Cell<u64>>);
        impl Clone for Input {
            fn clone(&self) -> Self {
                self.0.set(self.0.get() + 100);
                Self(self.0.clone())
            }
        }
        let clock = Rc::new(Cell::new(0));
        let schedule = BenchmarkSchedule {
            warmup_runs: 1,
            measured_runs: NonZeroUsize::new(3).unwrap(),
            seed: 0,
        };
        let r = schedule
            .run_with_clock(
                &[()],
                &[Input(clock.clone())],
                None,
                |_, input, trial| {
                    input.0.set(input.0.get() + 5 + trial.repetition as u64);
                    if !trial.warmup && trial.repetition == 1 {
                        Err("failed")
                    } else {
                        Ok(BenchmarkObservation {
                            score: SimpleScore::new(1),
                            initialized: true,
                            work: SolverWork::default(),
                        })
                    }
                },
                || Duration::from_millis(clock.get()),
            )
            .unwrap();
        assert_eq!(
            r.trials
                .iter()
                .map(|t| t.elapsed.as_millis())
                .collect::<Vec<_>>(),
            [5, 5, 6, 7]
        );
        let t = r.timing(0, 0).unwrap();
        assert_eq!((t.successful_runs, t.failed_runs), (2, 1));
        assert_eq!(t.successful_elapsed, Duration::from_millis(12));
        assert_eq!(t.mean_seconds(), Some(0.006));
        assert_eq!(t.fastest, Some(Duration::from_millis(5)));
        assert_eq!(t.slowest, Some(Duration::from_millis(7)));
        assert_eq!(r.summary(0).unwrap().unwrap().failure_count(), 1);
        assert!(r.is_complete());
        assert!(!r.is_successful());
    }
}
