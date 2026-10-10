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
/// A continuation must use the original configuration/problem ordering and values.
/// Counts are checked; arbitrary generic input contents cannot be verified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkResumeError {
    ConfigurationCount { expected: usize, actual: usize },
    ProblemCount { expected: usize, actual: usize },
}
impl fmt::Display for BenchmarkResumeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigurationCount { expected, actual } => write!(
                f,
                "benchmark continuation requires {expected} configurations, got {actual}"
            ),
            Self::ProblemCount { expected, actual } => write!(
                f,
                "benchmark continuation requires {expected} problems, got {actual}"
            ),
        }
    }
}
impl Error for BenchmarkResumeError {}
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
    schedule: BenchmarkSchedule,
    segments: Vec<std::ops::Range<usize>>,
    trials: Vec<BenchmarkTrialResult<S, E>>,
}
impl<S, E> BenchmarkRunReport<S, E> {
    pub fn schedule(&self) -> BenchmarkSchedule {
        self.schedule
    }
    /// Nonempty ranges in trials(), one per execution that returned any callbacks.
    /// Exposes interruptions: resumed timings need not share thermal/environment
    /// conditions, even when their scores and trial seeds match.
    pub fn execution_segments(&self) -> &[std::ops::Range<usize>] {
        &self.segments
    }
    /// Continue at the first unexecuted trial, retaining all measurements and errors.
    /// Uses this report's original schedule and seeds, with no repeated warm-ups.
    /// A fresh cancellation token may stop this continuation again. A cancelled
    /// token or complete report causes no cloning, callback execution or new segment.
    ///
    /// The caller must supply the SAME configuration/problem values in the SAME
    /// order and the same measurement callback semantics. Only lengths can be
    /// checked for arbitrary generic inputs; matching lengths do not prove identity.
    /// Clone/timing/failure semantics match BenchmarkSchedule::run. Returned errors
    /// are completed trials, never retries. On panic, already appended results remain;
    /// the panicking callback has no result and may execute again if the caller resumes.
    /// Inspect execution_segments when comparing resumed timings. For a cold restart
    /// requiring new warm-ups, start a fresh report instead.
    pub fn resume<C, P: Clone>(
        &mut self,
        configurations: &[C],
        problems: &[P],
        cancellation: Option<&CancellationToken>,
        solve: impl FnMut(&C, P, BenchmarkTrial) -> Result<BenchmarkObservation<S>, E>,
    ) -> Result<(), BenchmarkResumeError> {
        let origin = Instant::now();
        self.resume_with_clock(configurations, problems, cancellation, solve, || {
            origin.elapsed()
        })
    }
    fn resume_with_clock<C, P: Clone>(
        &mut self,
        configurations: &[C],
        problems: &[P],
        cancellation: Option<&CancellationToken>,
        mut solve: impl FnMut(&C, P, BenchmarkTrial) -> Result<BenchmarkObservation<S>, E>,
        mut clock: impl FnMut() -> Duration,
    ) -> Result<(), BenchmarkResumeError> {
        if configurations.len() != self.configurations {
            return Err(BenchmarkResumeError::ConfigurationCount {
                expected: self.configurations,
                actual: configurations.len(),
            });
        }
        if problems.len() != self.problems {
            return Err(BenchmarkResumeError::ProblemCount {
                expected: self.problems,
                actual: problems.len(),
            });
        }
        let first = self.trials.len();
        // Decode the next flat index directly, avoiding a scan of completed trials.
        let width = self.configurations * self.problems;
        let warmups = self.schedule.warmup_runs * width;
        for index in first..self.planned_trials {
            if cancellation.is_some_and(CancellationToken::is_cancelled) {
                break;
            }
            let warmup = index < warmups;
            let offset = if warmup { index } else { index - warmups };
            let repetition = offset / width;
            let problem = (offset % width) / self.configurations;
            let configuration = offset % self.configurations;
            let seed = self
                .schedule
                .seed
                .wrapping_add(repetition as u64)
                .wrapping_add((problem as u64).wrapping_mul(0x9e3779b97f4a7c15))
                ^ if warmup { 0xd1b54a32d192ed03 } else { 0 };
            let input = problems[problem].clone();
            let trial = BenchmarkTrial {
                configuration,
                problem,
                repetition,
                warmup,
                seed,
            };
            let started = clock();
            let result = solve(&configurations[configuration], input, trial);
            let elapsed = clock().saturating_sub(started);
            self.trials.push(BenchmarkTrialResult {
                trial,
                elapsed,
                result,
            });
            if index == first {
                self.segments.push(first..self.trials.len());
            } else {
                self.segments.last_mut().unwrap().end = self.trials.len();
            }
        }
        Ok(())
    }
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
    /// Pair measured timings for one problem by repetition and seed. Returns None
    /// for unknown configurations/problems. Comparing a configuration with itself
    /// is allowed. Raw failed/unequal-quality pairs and cancelled-run gaps remain
    /// visible; aggregate ratios use only initialized equal-quality successes.
    pub fn compare_timing(
        &self,
        baseline: usize,
        candidate: usize,
        problem: usize,
    ) -> Option<crate::BenchmarkTimingComparison<'_, S, E>> {
        if baseline >= self.configurations
            || candidate >= self.configurations
            || problem >= self.problems
        {
            return None;
        }
        Some(crate::BenchmarkTimingComparison::from_trials(
            baseline,
            candidate,
            problem,
            self.is_complete(),
            &self.trials,
        ))
    }
    /// Rank complete measured runs, matching both problem and repetition.
    /// Warm-ups never contribute; inspect is_successful for warm-up failures.
    pub fn standings(
        &self,
    ) -> Result<Vec<crate::BenchmarkStanding>, crate::BenchmarkStandingError> {
        if !self.is_complete() {
            return Err(crate::BenchmarkStandingError::IncompleteRun);
        }
        let entries = self
            .trials
            .iter()
            .filter(|t| !t.trial.warmup)
            .map(|t| {
                Ok(crate::BenchmarkComparison {
                    configuration: t.trial.configuration,
                    problem: (t.trial.problem, t.trial.repetition),
                    outcome: match &t.result {
                        Ok(value) => BenchmarkOutcome::success(&value.score, value.initialized)?,
                        Err(_) => BenchmarkOutcome::Failure,
                    },
                })
            })
            .collect::<Result<Vec<_>, BenchmarkResultError>>()?;
        crate::rank_benchmarks(self.configurations, &entries)
    }
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
        solve: impl FnMut(&C, P, BenchmarkTrial) -> Result<BenchmarkObservation<S>, E>,
        clock: impl FnMut() -> Duration,
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
            schedule: *self,
            segments: vec![],
            trials: vec![],
        };
        report
            .resume_with_clock(configurations, problems, cancellation, solve, clock)
            .expect("report dimensions were just validated");
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

    #[test]
    fn paired_timings_keep_quality_failures_zeroes_and_work_visible() {
        for count in [9, 10] {
            let clock = Cell::new(0);
            let report = BenchmarkSchedule {
                warmup_runs: 1,
                measured_runs: NonZeroUsize::new(count).unwrap(),
                seed: 731,
            }
            .run_with_clock(
                &[0, 1, 2],
                &[0, 1],
                None,
                |&configuration, _, trial| {
                    let r = trial.repetition;
                    let elapsed = if trial.warmup {
                        1000
                    } else {
                        match (configuration, r) {
                            (0, 1) => 100,
                            (0, 8) | (1, 5) => 0,
                            (1, 0) => 20,
                            (1, 9) => 5,
                            _ => 10,
                        }
                    };
                    clock.set(clock.get() + elapsed);
                    if trial.warmup
                        || (configuration == 1 && r == 3)
                        || (configuration == 0 && r == 6)
                    {
                        return Err("failed callback");
                    }
                    Ok(BenchmarkObservation {
                        score: SimpleScore::new(i64::from(
                            configuration == 1 && (r == 2 || r == 7),
                        )),
                        initialized: r != 4 && !(configuration == 1 && r == 7),
                        work: SolverWork {
                            evaluations: if configuration == 1 && r == 0 { 2 } else { 1 },
                            ..Default::default()
                        },
                    })
                },
                || Duration::from_millis(clock.get()),
            )
            .unwrap();
            assert!(!report.is_successful()); // Failed warm-ups remain visible in the run.
            for problem in 0..2 {
                let comparison = report.compare_timing(0, 1, problem).unwrap();
                assert!(comparison.schedule_complete);
                assert_eq!(comparison.pairs.len(), count);
                assert!(comparison.baseline_unpaired.is_empty());
                assert!(comparison.candidate_unpaired.is_empty());
                assert_eq!(comparison.comparable_pairs(), count - 7);
                assert_eq!(
                    comparison.median_time_ratio(),
                    Some(if count == 9 { 1.05 } else { 0.5 })
                );
                for (r, pair) in comparison.pairs.iter().enumerate() {
                    assert_eq!(pair.baseline.trial.problem, problem);
                    assert_eq!(pair.candidate.trial.problem, problem);
                    assert_eq!(pair.baseline.trial.repetition, r);
                    assert_eq!(pair.candidate.trial.repetition, r);
                    assert_eq!(pair.baseline.trial.seed, pair.candidate.trial.seed);
                    assert!(!pair.baseline.trial.warmup);
                }
                let pairs = &comparison.pairs;
                assert_eq!(pairs[0].candidate_over_baseline(), Some(2.0));
                assert_eq!(pairs[0].equal_work(), Some(false));
                assert_eq!(pairs[1].equal_work(), Some(true));
                assert_eq!(pairs[2].quality_order(), Some(std::cmp::Ordering::Greater));
                assert_eq!(pairs[3].quality_order(), None);
                assert_eq!(pairs[3].equal_work(), None);
                assert_eq!(pairs[4].quality_order(), Some(std::cmp::Ordering::Equal));
                assert_eq!(pairs[6].quality_order(), None);
                assert_eq!(pairs[7].quality_order(), Some(std::cmp::Ordering::Less));
                for pair in &pairs[2..9] {
                    assert_eq!(pair.candidate_over_baseline(), None);
                }
                let reverse = report.compare_timing(1, 0, problem).unwrap();
                assert_eq!(reverse.pairs[0].candidate_over_baseline(), Some(0.5));
                assert_eq!(
                    reverse.pairs[2].quality_order(),
                    Some(std::cmp::Ordering::Less)
                );
                let same = report.compare_timing(0, 0, problem).unwrap();
                assert_eq!(same.median_time_ratio(), Some(1.0));
            }
            assert!(report.compare_timing(3, 0, 0).is_none());
            assert!(report.compare_timing(0, 3, 0).is_none());
            assert!(report.compare_timing(0, 1, 2).is_none());
        }
    }

    #[test]
    fn cancelled_timing_comparison_keeps_unmatched_trials_without_inventing_pairs() {
        let token = CancellationToken::new();
        let clock = Cell::new(0);
        let report = BenchmarkSchedule {
            warmup_runs: 0,
            measured_runs: NonZeroUsize::new(3).unwrap(),
            seed: 7,
        }
        .run_with_clock(
            &[(), ()],
            &[()],
            Some(&token),
            |_, _, trial| {
                clock.set(clock.get() + 10);
                if trial.repetition == 1 {
                    token.cancel();
                }
                Ok::<_, ()>(BenchmarkObservation {
                    score: SimpleScore::ZERO,
                    initialized: true,
                    work: SolverWork::default(),
                })
            },
            || Duration::from_millis(clock.get()),
        )
        .unwrap();
        let comparison = report.compare_timing(0, 1, 0).unwrap();
        assert!(!comparison.schedule_complete);
        assert_eq!(comparison.pairs.len(), 1);
        assert_eq!(comparison.baseline_unpaired.len(), 1);
        assert_eq!(comparison.baseline_unpaired[0].trial.repetition, 1);
        assert!(comparison.candidate_unpaired.is_empty());
        assert_eq!(comparison.median_time_ratio(), Some(1.0));
        let reverse = report.compare_timing(1, 0, 0).unwrap();
        assert_eq!(reverse.candidate_unpaired.len(), 1);
        assert!(reverse.baseline_unpaired.is_empty());
        // A pre-cancelled report is distinct from a successful zero-time run.
        let empty = BenchmarkSchedule::default()
            .run(
                &[(), ()],
                &[()],
                Some(&token),
                |_, _, _| -> Result<BenchmarkObservation<SimpleScore>, ()> { panic!("cancelled") },
            )
            .unwrap();
        let comparison = empty.compare_timing(0, 1, 0).unwrap();
        assert!(!comparison.schedule_complete);
        assert!(comparison.pairs.is_empty());
        assert_eq!(comparison.median_time_ratio(), None);
    }

    #[test]
    fn timing_pairs_require_equal_seeds_as_well_as_repetitions() {
        let mut report = BenchmarkSchedule {
            warmup_runs: 0,
            measured_runs: NonZeroUsize::new(1).unwrap(),
            seed: 9,
        }
        .run(&[(), ()], &[()], None, |_, _, _| {
            Ok::<_, ()>(BenchmarkObservation {
                score: SimpleScore::ZERO,
                initialized: true,
                work: SolverWork::default(),
            })
        })
        .unwrap();
        // Exercise pairing independently of the runner's equal-seed guarantee.
        report.trials[1].trial.seed = 10;
        let comparison = report.compare_timing(0, 1, 0).unwrap();
        assert!(comparison.pairs.is_empty());
        assert_eq!(comparison.baseline_unpaired.len(), 1);
        assert_eq!(comparison.candidate_unpaired.len(), 1);
        assert_eq!(comparison.comparable_pairs(), 0);
        assert_eq!(comparison.median_time_ratio(), None);
        for trial in &mut report.trials {
            trial.elapsed = Duration::from_millis(1);
        }
        let original = report.trials[0].trial;
        for mismatch in 0..5 {
            report.trials[1].trial = original;
            report.trials[1].trial.configuration = 1;
            match mismatch {
                0 => report.trials[1].trial.seed += 1,
                1 => report.trials[1].trial.problem += 1,
                2 => report.trials[1].trial.repetition += 1,
                3 => report.trials[1].trial.warmup = true,
                _ => {}
            }
            let pair = crate::BenchmarkTimingPair {
                baseline: &report.trials[0],
                candidate: &report.trials[1],
            };
            assert_eq!(
                pair.candidate_over_baseline(),
                (mismatch == 4).then_some(1.0)
            );
        }
    }
}

#[cfg(test)]
mod resume_tests {
    use super::*;
    use crate::SimpleScore;
    use std::cell::Cell;

    fn observation(trial: BenchmarkTrial) -> Result<BenchmarkObservation<SimpleScore>, String> {
        if trial.problem == 1 && trial.configuration == 1 && trial.repetition == 0 {
            Err("retained failure".into())
        } else {
            Ok(BenchmarkObservation {
                score: SimpleScore::new(trial.problem as i64),
                initialized: true,
                work: SolverWork {
                    evaluations: trial.repetition as u64 + 1,
                    ..Default::default()
                },
            })
        }
    }
    fn tick(clock: &Cell<u64>, increment: u64) -> Duration {
        clock.set(clock.get() + increment);
        Duration::from_nanos(clock.get())
    }
    #[test]
    fn resume_at_every_boundary_preserves_prefix_seeds_failures_and_timings() {
        let schedule = BenchmarkSchedule {
            warmup_runs: 1,
            measured_runs: NonZeroUsize::new(2).unwrap(),
            seed: u64::MAX,
        };
        let configurations = [0, 1];
        let problems = [vec![10], vec![20]];
        let full = schedule
            .run(&configurations, &problems, None, |_, _, t| observation(t))
            .unwrap();
        for cut in 0..=full.planned_trials() {
            let token = CancellationToken::new();
            if cut == 0 {
                token.cancel();
            }
            let clock = Cell::new(0);
            let mut calls = 0;
            let mut report = schedule
                .run_with_clock(
                    &configurations,
                    &problems,
                    Some(&token),
                    |_, _, t| {
                        calls += 1;
                        if calls == cut {
                            token.cancel();
                        }
                        observation(t)
                    },
                    || tick(&clock, 3),
                )
                .unwrap();
            assert_eq!(report.trials().len(), cut);
            let mut resumed = vec![];
            report
                .resume_with_clock(
                    &configurations,
                    &problems,
                    None,
                    |config, mut input, t| {
                        assert_eq!(*config, t.configuration);
                        assert_eq!(input, problems[t.problem]);
                        input.push(999); // Every callback still receives a fresh clone.
                        resumed.push(t);
                        observation(t)
                    },
                    || tick(&clock, 5),
                )
                .unwrap();
            assert_eq!(
                resumed,
                full.trials()[cut..]
                    .iter()
                    .map(|t| t.trial)
                    .collect::<Vec<_>>()
            );
            assert_eq!(report.schedule(), schedule);
            assert!(report.is_complete());
            assert!(!report.is_successful());
            for (i, (actual, expected)) in report.trials().iter().zip(full.trials()).enumerate() {
                assert_eq!(actual.trial, expected.trial);
                assert_eq!(
                    actual.elapsed,
                    Duration::from_nanos(if i < cut { 3 } else { 5 })
                );
                assert_eq!(
                    actual
                        .result
                        .as_ref()
                        .map(|r| (&r.score, r.initialized, r.work)),
                    expected
                        .result
                        .as_ref()
                        .map(|r| (&r.score, r.initialized, r.work))
                );
            }
            let expected_segments: Vec<_> = [0..cut, cut..12]
                .into_iter()
                .filter(|r| !r.is_empty())
                .collect();
            assert_eq!(report.execution_segments(), expected_segments);
            assert_eq!(report.summary(1).unwrap().unwrap().failure_count(), 1);
            for (actual, expected) in report
                .standings()
                .unwrap()
                .iter()
                .zip(full.standings().unwrap())
            {
                assert_eq!(
                    (
                        actual.configuration,
                        actual.rank,
                        actual.wins,
                        actual.ties,
                        actual.losses
                    ),
                    (
                        expected.configuration,
                        expected.rank,
                        expected.wins,
                        expected.ties,
                        expected.losses
                    )
                );
                assert_eq!(actual.compare(&expected), std::cmp::Ordering::Equal);
            }
            let comparison = report.compare_timing(0, 1, 0).unwrap();
            assert_eq!(comparison.comparable_pairs(), 2);
        }
    }
    #[test]
    fn invalid_shapes_and_cancelled_or_complete_resumes_preserve_history_without_work() {
        #[derive(Debug)]
        struct Input;
        impl Clone for Input {
            fn clone(&self) -> Self {
                panic!("no cloning expected")
            }
        }
        let token = CancellationToken::new();
        token.cancel();
        let schedule = BenchmarkSchedule::default();
        let mut report = schedule
            .run(
                &[()],
                &[Input],
                Some(&token),
                |_, _, _| -> Result<BenchmarkObservation<SimpleScore>, ()> { panic!() },
            )
            .unwrap();
        let callback = |_: &(),
                        _: Input,
                        _: BenchmarkTrial|
         -> Result<BenchmarkObservation<SimpleScore>, ()> {
            panic!("no callback expected")
        };
        assert_eq!(
            report.resume(&[], &[Input], None, callback),
            Err(BenchmarkResumeError::ConfigurationCount {
                expected: 1,
                actual: 0
            })
        );
        assert_eq!(
            report.resume(&[()], &[], None, callback),
            Err(BenchmarkResumeError::ProblemCount {
                expected: 1,
                actual: 0
            })
        );
        report
            .resume(&[()], &[Input], Some(&token), callback)
            .unwrap();
        assert!(report.trials().is_empty());
        assert!(report.execution_segments().is_empty());
        let mut complete = schedule
            .run(&[()], &[()], None, |_, _, t| observation(t))
            .unwrap();
        let count = complete.trials().len();
        complete
            .resume(&[()], &[Input], None, |_, _, _| panic!())
            .unwrap();
        assert_eq!(complete.trials().len(), count);
        assert_eq!(
            complete.execution_segments(),
            std::slice::from_ref(&(0..count))
        );
    }
    #[test]
    fn repeated_continuations_and_panic_keep_finished_trials_without_retries() {
        let schedule = BenchmarkSchedule {
            warmup_runs: 0,
            measured_runs: NonZeroUsize::new(4).unwrap(),
            seed: 81,
        };
        let stop = CancellationToken::new();
        stop.cancel();
        let mut report = schedule
            .run(&[()], &[()], Some(&stop), |_, _, t| observation(t))
            .unwrap();
        for count in 1..=2 {
            let stop = CancellationToken::new();
            report
                .resume(&[()], &[()], Some(&stop), |_, _, t| {
                    stop.cancel();
                    observation(t)
                })
                .unwrap();
            assert_eq!(report.trials().len(), count);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            report
                .resume(&[()], &[()], None, |_, _, t| {
                    if t.repetition == 3 {
                        panic!("interrupted callback");
                    }
                    observation(t)
                })
                .unwrap();
        }));
        assert!(result.is_err());
        assert_eq!(report.trials().len(), 3);
        assert_eq!(report.execution_segments(), &[0..1, 1..2, 2..3]);
        let mut calls = 0;
        report
            .resume(&[()], &[()], None, |_, _, t| {
                calls += 1;
                assert_eq!(t.repetition, 3);
                observation(t)
            })
            .unwrap();
        assert_eq!(calls, 1);
        assert!(report.is_successful());
        assert_eq!(report.execution_segments(), &[0..1, 1..2, 2..3, 3..4]);
    }
}
