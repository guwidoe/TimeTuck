//! Opt-in monitoring of active solves, without retaining assignments or justifications.
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
/// Last successfully completed step, independent of the best score. Attempts
/// are numbered from zero within each phase; unsuccessful attempts leave this
/// snapshot unchanged. Construction may publish partial assignments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverStepMetric<S> {
    pub phase: usize,
    pub kind: crate::SolverPhaseKind,
    pub step: u64,
    pub score: S,
    pub complete: bool,
}
/// One constraint's ordinary score and active match count. Zero-score and empty
/// registered constraints are retained; no justifications or assignments are copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintTotalMetric<S> {
    pub score: S,
    pub match_count: usize,
}
/// Why requested totals are unavailable. Unavailable is distinct from an enabled
/// backend reporting verified empty totals; calculator totals never validate that the
/// calculator's independent score agrees with its registered contributions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstraintTotalUnavailable {
    UnsupportedBackend,
    MatchingDisabled,
    NoBestYet,
    RegistryUnavailable,
    FinalScoreNotObserved,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BestConstraintTotals<S> {
    Disabled,
    Unavailable(ConstraintTotalUnavailable),
    Available(Arc<BTreeMap<String, ConstraintTotalMetric<S>>>),
}
/// One invocation. Work is published after each scoring phase returns and on
/// success at solve end; an unfinished/failed phase's partial aggregate work is unavailable.
/// Move-family counts update independently and retain observed partial work.
/// Best scores update during phases, before user best-event callbacks. This is
/// separate from `last_step`, which also tracks worsening completed steps.
/// Duration includes callbacks and score diagnostics;
/// diagnostic evaluations do not contribute to the solver's work counters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolveMetricSnapshot<S> {
    pub run: u64,
    pub dimensions: Option<SolverMetricDimensions>,
    pub duration: Duration,
    pub work: SolverWork,
    /// Live candidate-family counts, independent of phase-boundary `work` publication.
    pub move_evaluations: crate::MoveEvaluationCounts,
    pub last_phase: Option<usize>,
    pub best_score: Option<S>,
    pub best_constraint_totals: BestConstraintTotals<S>,
    /// Last completed construction/local-search/repair step or committed custom
    /// move. Published before StepEnded callbacks; exact enumeration has no steps.
    pub last_step: Option<SolverStepMetric<S>>,
    pub completed_step_count: u64,
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
    moves: crate::move_metrics::MoveCounter,
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
/// changes, completed steps and phase completion. Step scores publish before
/// StepEnded callbacks and survive best-state restoration at phase/solve end.
/// Exact enumeration, diagnostic scores, previews and failed attempts do not
/// publish steps. Move-family counts use per-run atomics without
/// acquiring the monitor lock for each candidate evaluation.
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
    constraint_totals: bool,
}
impl<S> Clone for SolverMetrics<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            constraint_totals: self.constraint_totals,
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
            constraint_totals: false,
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
    /// Request best constraint totals for subsequent solves using this handle.
    /// Fresh calculator plans enable optional match collection. Retained calculator
    /// plans use their existing setting without reset; construct their director with
    /// `request_matches = true`. Scoped and stream plans use cached contributions.
    /// Scoped constraints expose one aggregate match each (zero for disabled
    /// weights); stream constraints expose their retained terminal tuple counts.
    /// An active zero-score match counts, even though it contributes no score.
    /// Totals publish with the best score before best-event callbacks, including
    /// partial construction states. Scoring callbacks and work counts are unchanged.
    /// Copying totals costs O(registered constraints). Fresh exact search captures
    /// each evaluated node before its scoring state is dropped or rolled back.
    /// No totals traversal, allocation, lock, or extra scoring runs when disabled.
    /// Clones made before this builder retain their original request setting.
    pub fn with_best_constraint_totals(mut self) -> Self {
        self.constraint_totals = true;
        self
    }
    pub(crate) fn requests_constraint_totals(&self) -> bool {
        self.constraint_totals
    }
    pub fn snapshot(&self) -> SolverMetricsSnapshot<S> {
        let now = (self.inner.clock)();
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        let active = state
            .active
            .values_mut()
            .map(|r| {
                r.snapshot.duration = r.snapshot.duration.max(now.saturating_sub(r.started));
                r.snapshot.move_evaluations = r.moves.snapshot();
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
        let moves = crate::move_metrics::MoveCounter::default();
        state.active.insert(
            id,
            Run {
                started,
                moves: moves.clone(),
                snapshot: SolveMetricSnapshot {
                    run: id,
                    dimensions: self.inner.dimensions,
                    duration: Duration::ZERO,
                    work: SolverWork::default(),
                    move_evaluations: Default::default(),
                    last_phase: None,
                    best_score: None,
                    best_constraint_totals: if self.constraint_totals {
                        BestConstraintTotals::Unavailable(
                            ConstraintTotalUnavailable::UnsupportedBackend,
                        )
                    } else {
                        BestConstraintTotals::Disabled
                    },
                    last_step: None,
                    completed_step_count: 0,
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
            moves,
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
    pub(crate) fn constraint_source(&self) {
        self.update(|r| {
            r.best_constraint_totals =
                BestConstraintTotals::Unavailable(ConstraintTotalUnavailable::NoBestYet)
        });
    }
    pub(crate) fn step(&self, step: SolverStepMetric<S>) {
        self.update(|r| {
            r.last_step = Some(step);
            r.completed_step_count = r.completed_step_count.saturating_add(1);
        });
    }
    pub(crate) fn best(&self, score: &S, complete: bool, totals: Option<BestConstraintTotals<S>>) {
        // Clone user score values before locking so a user Clone panic cannot
        // poison the monitor or cause its unwind guard to lock recursively.
        let score = score.clone();
        self.update(|r| {
            r.best_score = Some(score);
            if let Some(totals) = totals {
                r.best_constraint_totals = totals;
            }
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
    pub(crate) moves: crate::move_metrics::MoveCounter,
    closed: bool,
}
impl<S: Score> MetricRun<S> {
    pub(crate) fn finish<A, E>(&mut self, result: &Result<SolverResult<A, S>, E>) {
        if let Ok(r) = result {
            let score = r.score.clone();
            self.writer.update(|s| {
                if s.best_score != score
                    && matches!(s.best_constraint_totals, BestConstraintTotals::Available(_))
                {
                    s.best_constraint_totals = BestConstraintTotals::Unavailable(
                        ConstraintTotalUnavailable::FinalScoreNotObserved,
                    );
                }
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
            r.snapshot.move_evaluations = r.moves.snapshot();
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

/// Per-run staging between the actual director best hook and global-best filter.
/// Rejected phase-local bests do not overwrite published totals. The last capture
/// remains available for construction final-state handoffs.
#[derive(Clone)]
pub(crate) struct ConstraintMetricCapture<S>(Arc<Mutex<Option<BestConstraintTotals<S>>>>);
impl<S> Default for ConstraintMetricCapture<S> {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl<S> ConstraintMetricCapture<S> {
    pub(crate) fn capture_totals<'a>(&self, totals: impl IntoIterator<Item = (&'a str, S, usize)>) {
        let value = BestConstraintTotals::Available(
            totals
                .into_iter()
                .map(|(name, score, match_count)| {
                    (
                        name.to_owned(),
                        ConstraintTotalMetric { score, match_count },
                    )
                })
                .collect::<BTreeMap<_, _>>()
                .into(),
        );
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }
}
impl<S: crate::AdditiveScore> ConstraintMetricCapture<S> {
    pub(crate) fn capture<J, E>(&self, registry: Result<&crate::MatchRegistry<S, J>, E>) {
        let value = match registry {
            Ok(registry) if registry.is_enabled() => BestConstraintTotals::Available(
                registry
                    .constraints()
                    .expect("enabled registry")
                    .iter()
                    .map(|(name, c)| {
                        (
                            name.clone(),
                            ConstraintTotalMetric {
                                score: c.score().clone(),
                                match_count: c.matches().len(),
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
                    .into(),
            ),
            Ok(_) => {
                BestConstraintTotals::Unavailable(ConstraintTotalUnavailable::MatchingDisabled)
            }
            Err(_) => {
                BestConstraintTotals::Unavailable(ConstraintTotalUnavailable::RegistryUnavailable)
            }
        };
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }
}
impl<S: Clone> ConstraintMetricCapture<S> {
    pub(crate) fn snapshot(&self) -> BestConstraintTotals<S> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or(BestConstraintTotals::Unavailable(
                ConstraintTotalUnavailable::NoBestYet,
            ))
    }
}
