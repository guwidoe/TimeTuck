//! Named meter export over the existing live solver monitor.
use crate::{ProblemStatistics, Score, SolverMetricDimensions, SolverMetrics};
use std::{collections::BTreeMap, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SolverMeterType {
    LongTaskTimer,
    Counter,
    Gauge,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SolverMeterId {
    pub name: &'static str,
    pub tags: BTreeMap<String, String>,
    pub meter_type: SolverMeterType,
    pub base_unit: Option<String>,
    pub description: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SolverMeterReading {
    pub id: SolverMeterId,
    pub value: f64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverMeterViewError {
    ConcurrentProblemRuns,
    ActiveRebind,
    UsedRebind,
    DimensionMismatch,
}
impl fmt::Display for SolverMeterViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "solver meter view: {self:?}")
    }
}
impl Error for SolverMeterViewError {}

/// Export one problem's monitor with stable names, types and `problem.id` tags.
/// No meters appear before the first run. Duration/error meters are global and
/// remain registered after a run; per-problem gauges exist only while solving.
/// Explicit publication retains the last sampled value after gauge removal.
/// It never averages gauges or retains an assignment. Sampling clones the existing
/// monitor snapshot, so it is intended for exporter/observer polling, not each move.
///
/// Each monitor may have at most one active run in this view. A changed model uses
/// a new monitor with fresh dimensions, rebound after the old run ends. Rebinding
/// carries cumulative failures and retains last published samples; the caller must
/// attach the new monitor to the actual restarted solve. This does not manage jobs,
/// apply problem changes or install a process-global registry. Existing monitor and
/// solver defaults stay unchanged. Gauge numbers use floating estimates; exact work
/// counts and assignment scale bounds remain available on the native snapshots.
pub struct SolverMeterView<S> {
    monitor: SolverMetrics<S>,
    problem_id: String,
    dimensions: SolverMetricDimensions,
    scale: f64,
    retired_failures: u64,
    retired_runs: bool,
    published: BTreeMap<SolverMeterId, f64>,
}
impl<S: Score> SolverMeterView<S> {
    pub fn for_problem(
        monitor: SolverMetrics<S>,
        problem_id: impl Into<String>,
        problem: &impl ProblemStatistics,
    ) -> Self {
        Self {
            monitor,
            problem_id: problem_id.into(),
            dimensions: (&problem.problem_size_statistics()).into(),
            scale: problem.assignment_scale().scale.log10(),
            retired_failures: 0,
            retired_runs: false,
            published: BTreeMap::new(),
        }
    }
    /// Fresh model metadata is derived from the actual problem. Rebinding the
    /// same old monitor is unsupported: use a new monitor per changed model.
    pub fn rebind_problem_monitor(
        &mut self,
        monitor: SolverMetrics<S>,
        problem: &impl ProblemStatistics,
    ) -> Result<(), SolverMeterViewError> {
        let old = self.monitor.snapshot();
        let next = monitor.snapshot();
        if !old.active.is_empty() || !next.active.is_empty() {
            return Err(SolverMeterViewError::ActiveRebind);
        }
        if next.completed_runs != 0 || next.failed_runs != 0 {
            return Err(SolverMeterViewError::UsedRebind);
        }
        let dimensions = (&problem.problem_size_statistics()).into();
        let scale = problem.assignment_scale().scale.log10();
        self.retired_failures = self.retired_failures.saturating_add(old.failed_runs);
        self.retired_runs |= old.completed_runs != 0 || old.failed_runs != 0;
        self.monitor = monitor;
        self.dimensions = dimensions;
        self.scale = scale;
        Ok(())
    }
    pub fn current_solver_meters(&self) -> Result<Vec<SolverMeterReading>, SolverMeterViewError> {
        let snapshot = self.monitor.snapshot();
        if snapshot.active.len() > 1 {
            return Err(SolverMeterViewError::ConcurrentProblemRuns);
        }
        let mut readings = vec![];
        if !self.retired_runs
            && snapshot.active.is_empty()
            && snapshot.completed_runs == 0
            && snapshot.failed_runs == 0
        {
            return Ok(readings);
        }
        let mut add = |name, meter_type, per_problem, value| {
            let tags = if per_problem {
                BTreeMap::from([("problem.id".into(), self.problem_id.clone())])
            } else {
                BTreeMap::new()
            };
            readings.push(SolverMeterReading {
                id: SolverMeterId {
                    name,
                    tags,
                    meter_type,
                    base_unit: None,
                    description: None,
                },
                value,
            });
        };
        add(
            "timefold.solver.solve.duration",
            SolverMeterType::LongTaskTimer,
            false,
            snapshot.active_duration().as_secs_f64(),
        );
        add(
            "timefold.solver.errors",
            SolverMeterType::Counter,
            false,
            self.retired_failures.saturating_add(snapshot.failed_runs) as f64,
        );
        if let Some(run) = snapshot.active.first() {
            if run.dimensions != Some(self.dimensions) {
                return Err(SolverMeterViewError::DimensionMismatch);
            }
            for (name, value) in [
                (
                    "timefold.solver.score.calculation.count",
                    run.work.evaluations as f64,
                ),
                (
                    "timefold.solver.move.evaluation.count",
                    run.move_evaluations.total() as f64,
                ),
                (
                    "timefold.solver.problem.entities",
                    self.dimensions.entities as f64,
                ),
                (
                    "timefold.solver.problem.variables",
                    self.dimensions.variables as f64,
                ),
                (
                    "timefold.solver.problem.values",
                    self.dimensions.values as f64,
                ),
                ("timefold.solver.problem.size.log", self.scale),
            ] {
                add(name, SolverMeterType::Gauge, true, value);
            }
        }
        Ok(readings)
    }
    /// Replace sampled values only after all current meters validate. Stale samples
    /// retain their original value; querying registration still reflects live state.
    pub fn publish_solver_meters(
        &mut self,
    ) -> Result<Vec<SolverMeterReading>, SolverMeterViewError> {
        let readings = self.current_solver_meters()?;
        for reading in &readings {
            self.published.insert(reading.id.clone(), reading.value);
        }
        Ok(readings)
    }
    pub fn last_published_meter(&self, id: &SolverMeterId) -> Option<f64> {
        self.published.get(id).copied()
    }
}
