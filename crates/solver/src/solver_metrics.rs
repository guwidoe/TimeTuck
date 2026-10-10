//! Opt-in monitoring of active solves, without retaining assignments or match data.
use crate::{ProblemSizeStatistics, Score, SolverResult, SolverTermination, SolverWork};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Native model dimensions supplied when creating a monitor. No assignment-space
/// count is retained and no problem statistics are calculated in the search loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverMetricDimensions {
    pub entities: usize,
    pub variables: usize,
    pub values: usize,
}
impl From<&ProblemSizeStatistics> for SolverMetricDimensions {
    fn from(size: &ProblemSizeStatistics) -> Self {
        Self {
            entities: size.entity_count,
            variables: size.variable_count,
            values: size.value_count,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveMetricOutcome {
    Completed(SolverTermination),
    Failed,
    Panicked,
}
/// One invocation. Work is published after each scoring phase returns and on
/// success at solve end; an unfinished/failed phase's partial work is unavailable.
/// Best scores update during phases, before user best-event callbacks. This is
/// not a current-step score. Duration includes callbacks and score diagnostics;
/// diagnostic evaluations do not contribute to the solver's work counters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveMetricSnapshot<S> {
    pub run: u64,
    pub dimensions: Option<SolverMetricDimensions>,
    pub duration: Duration,
    pub work: SolverWork,
    pub last_phase: Option<usize>,
    pub best_score: Option<S>,
    pub complete: bool,
    pub outcome: Option<SolveMetricOutcome>,
}
/// Concurrent active runs plus the most recently finished invocation. Finished
/// history is bounded to one snapshot; cumulative counters count all invocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverMetricsSnapshot<S> {
    pub active: Vec<SolveMetricSnapshot<S>>,
    pub latest: Option<SolveMetricSnapshot<S>>,
    pub completed_runs: u64,
    pub failed_runs: u64,
}
impl<S> SolverMetricsSnapshot<S> {
    /// Sum of current active durations, zero when idle. Finished durations remain
    /// available in `latest.duration` instead of contributing to this gauge.
    pub fn active_duration(&self) -> Duration {
        self.active
            .iter()
            .fold(Duration::ZERO, |n, r| n.saturating_add(r.duration))
    }
}
struct Run<S> {
    started: Duration,
    snapshot: SolveMetricSnapshot<S>,
}
struct State<S> {
    next: u64,
    active: BTreeMap<u64, Run<S>>,
    latest: Option<SolveMetricSnapshot<S>>,
    completed: u64,
    failed: u64,
}
struct Inner<S> {
    state: Mutex<State<S>>,
    clock: Box<dyn Fn() -> Duration + Send + Sync>,
    dimensions: Option<SolverMetricDimensions>,
}
/// Attach through `SolverExtensions::metrics`. Clones share the monitor and may
/// observe simultaneous solves from different threads when the score is Send.
/// Disabled solves acquire no metric locks. Enabled solves publish on best-score
/// changes and phase completion, never on each candidate evaluation.
///
/// Invalid plans rejected before execution do not start a metric run. Normal
/// errors and unwinding panics after execution starts close it automatically.
/// Use separate monitors for problems with different dimensions. Tags, exporters
/// and finished-run history belong to the application.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1,2])])?;
/// let n = ConstraintNetwork::<_, SimpleScore>::new(1);
/// let metrics = SolverMetrics::new(Some((&p.problem_size_statistics()).into()));
/// let r = solve_with_extensions(&p, &n, SolverConfig::default(), SolverExtensions {
///     metrics: Some(&metrics), ..Default::default()
/// })?;
/// let snapshot = metrics.snapshot();
/// assert!(snapshot.active.is_empty());
/// assert_eq!(snapshot.latest.unwrap().work, r.work);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SolverMetrics<S> {
    inner: Arc<Inner<S>>,
}
impl<S> Clone for SolverMetrics<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<S: Score> Default for SolverMetrics<S> {
    fn default() -> Self {
        Self::new(None)
    }
}
impl<S: Score> SolverMetrics<S> {
    pub fn new(dimensions: Option<SolverMetricDimensions>) -> Self {
        let epoch = Instant::now();
        Self::with_clock(dimensions, move || epoch.elapsed())
    }
    /// Supply an absolute monotonic clock for deterministic monitoring. Clock
    /// calls happen outside the metric lock. Backwards readings cannot decrease
    /// an invocation's published duration. This does not change solver deadlines.
    pub fn with_clock(
        dimensions: Option<SolverMetricDimensions>,
        clock: impl Fn() -> Duration + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    next: 0,
                    active: BTreeMap::new(),
                    latest: None,
                    completed: 0,
                    failed: 0,
                }),
                clock: Box::new(clock),
                dimensions,
            }),
        }
    }
    pub fn snapshot(&self) -> SolverMetricsSnapshot<S> {
        let now = (self.inner.clock)();
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        let active = state
            .active
            .values_mut()
            .map(|r| {
                r.snapshot.duration = r.snapshot.duration.max(now.saturating_sub(r.started));
                r.snapshot.clone()
            })
            .collect();
        SolverMetricsSnapshot {
            active,
            latest: state.latest.clone(),
            completed_runs: state.completed,
            failed_runs: state.failed,
        }
    }
    pub(crate) fn start(&self) -> MetricRun<S> {
        let started = (self.inner.clock)();
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        let id = state.next;
        state.next = state
            .next
            .checked_add(1)
            .expect("metric run identity exhausted");
        state.active.insert(
            id,
            Run {
                started,
                snapshot: SolveMetricSnapshot {
                    run: id,
                    dimensions: self.inner.dimensions,
                    duration: Duration::ZERO,
                    work: SolverWork::default(),
                    last_phase: None,
                    best_score: None,
                    complete: false,
                    outcome: None,
                },
            },
        );
        MetricRun {
            writer: MetricWriter {
                metrics: self.clone(),
                id,
            },
            closed: false,
        }
    }
}
pub(crate) struct MetricWriter<S> {
    metrics: SolverMetrics<S>,
    id: u64,
}
impl<S> Clone for MetricWriter<S> {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            id: self.id,
        }
    }
}
impl<S: Score> MetricWriter<S> {
    pub(crate) fn best(&self, score: &S, complete: bool) {
        // Clone user score values before locking so a user Clone panic cannot
        // poison the monitor or cause its unwind guard to lock recursively.
        let score = score.clone();
        self.update(|r| {
            r.best_score = Some(score);
            r.complete = complete;
        });
    }
    fn update(&self, f: impl FnOnce(&mut SolveMetricSnapshot<S>)) {
        let mut state = self
            .metrics
            .inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(r) = state.active.get_mut(&self.id) {
            f(&mut r.snapshot);
        }
    }
    pub(crate) fn phase(&self, index: usize, work: SolverWork) {
        self.update(|r| {
            r.work = work;
            r.last_phase = Some(index);
        });
    }
}
pub(crate) struct MetricRun<S: Score> {
    pub(crate) writer: MetricWriter<S>,
    closed: bool,
}
impl<S: Score> MetricRun<S> {
    pub(crate) fn finish<A, E>(&mut self, result: &Result<SolverResult<A, S>, E>) {
        if let Ok(r) = result {
            let score = r.score.clone();
            self.writer.update(|s| {
                s.work = r.work;
                s.best_score = score;
                s.complete = r.complete;
            });
        }
        self.close(
            match result {
                Ok(r) => SolveMetricOutcome::Completed(r.termination),
                Err(_) => SolveMetricOutcome::Failed,
            },
            true,
        );
    }
    fn close(&mut self, outcome: SolveMetricOutcome, sample_clock: bool) {
        if self.closed {
            return;
        }
        // Avoid invoking user clock code while already unwinding.
        let now = sample_clock.then(|| (self.writer.metrics.inner.clock)());
        let mut state = self
            .writer
            .metrics
            .inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(mut r) = state.active.remove(&self.writer.id) {
            if let Some(now) = now {
                r.snapshot.duration = r.snapshot.duration.max(now.saturating_sub(r.started));
            }
            r.snapshot.outcome = Some(outcome);
            state.completed = state.completed.saturating_add(1);
            if !matches!(outcome, SolveMetricOutcome::Completed(_)) {
                state.failed = state.failed.saturating_add(1);
            }
            state.latest = Some(r.snapshot);
        }
        self.closed = true;
    }
}
impl<S: Score> Drop for MetricRun<S> {
    fn drop(&mut self) {
        self.close(
            if std::thread::panicking() {
                SolveMetricOutcome::Panicked
            } else {
                SolveMetricOutcome::Failed
            },
            false,
        );
    }
}
