//! Validated native benchmark configuration and execution over the serial harness.
use crate::*;
use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    num::NonZeroUsize,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum BenchmarkMetric {
    BestScore,
    StepScore,
    ScoreCalculationCount,
    MoveEvaluationCount,
    MemoryUse,
    ConstraintMatchTotalBestScore,
    ConstraintMatchTotalStepScore,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProblemBenchmarkStatistic {
    BestScore,
    StepScore,
    ScoreCalculationSpeed,
    MoveEvaluationSpeed,
    MemoryUse,
}
impl ProblemBenchmarkStatistic {
    pub fn metric(self) -> BenchmarkMetric {
        match self {
            Self::BestScore => BenchmarkMetric::BestScore,
            Self::StepScore => BenchmarkMetric::StepScore,
            Self::ScoreCalculationSpeed => BenchmarkMetric::ScoreCalculationCount,
            Self::MoveEvaluationSpeed => BenchmarkMetric::MoveEvaluationCount,
            Self::MemoryUse => BenchmarkMetric::MemoryUse,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleBenchmarkStatistic {
    ConstraintMatchTotalBestScore,
    ConstraintMatchTotalStepScore,
}
impl SingleBenchmarkStatistic {
    pub fn metric(self) -> BenchmarkMetric {
        match self {
            Self::ConstraintMatchTotalBestScore => BenchmarkMetric::ConstraintMatchTotalBestScore,
            Self::ConstraintMatchTotalStepScore => BenchmarkMetric::ConstraintMatchTotalStepScore,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProblemBenchmarkStatistics {
    pub enabled: Option<bool>,
    pub problem: Option<Vec<ProblemBenchmarkStatistic>>,
    pub single: Option<Vec<SingleBenchmarkStatistic>>,
}
fn merge_distinct<T: Eq + Clone>(current: &mut Option<Vec<T>>, inherited: &Option<Vec<T>>) {
    if let Some(base) = inherited {
        let out = current.get_or_insert_with(Vec::new);
        for v in base {
            if !out.contains(v) {
                out.push(v.clone());
            }
        }
    }
}
impl ProblemBenchmarkStatistics {
    pub fn problem_statistics(&self) -> Vec<ProblemBenchmarkStatistic> {
        if self.enabled == Some(false) {
            vec![]
        } else {
            self.problem
                .as_ref()
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(|| vec![ProblemBenchmarkStatistic::BestScore])
        }
    }
    pub fn single_statistics(&self) -> &[SingleBenchmarkStatistic] {
        self.single.as_deref().unwrap_or_default()
    }
    /// Merge explicitly supplied lists in encounter order, without duplicate keys.
    pub fn inherit(&mut self, base: &Self) {
        self.enabled = self.enabled.or(base.enabled);
        merge_distinct(&mut self.problem, &base.problem);
        merge_distinct(&mut self.single, &base.single);
    }
    /// Requests, not a claim that every backend collects every metric. Callback
    /// instrumentation must supply requested data; unavailable is never zero.
    pub fn solver_metrics(&self) -> Vec<BenchmarkMetric> {
        let mut out = Vec::new();
        for m in self
            .problem_statistics()
            .into_iter()
            .map(ProblemBenchmarkStatistic::metric)
            .chain(
                self.single_statistics()
                    .iter()
                    .copied()
                    .map(SingleBenchmarkStatistic::metric),
            )
        {
            if !out.contains(&m) {
                out.push(m);
            }
        }
        out
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BenchmarkSolverSettings {
    pub environment: EnvironmentProfile,
    pub phases: Vec<SolverPhaseKind>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SolverBenchmarkDeclaration {
    pub name: Option<String>,
    pub sub_runs: Option<i64>,
    pub solver: Option<BenchmarkSolverSettings>,
    pub monitoring: Option<Vec<BenchmarkMetric>>,
    pub statistics: ProblemBenchmarkStatistics,
}
impl SolverBenchmarkDeclaration {
    pub fn validate(&self) -> Result<(), BenchmarkConfigError> {
        validate_name(self.name.as_deref())?;
        if self.sub_runs.is_some_and(|n| n <= 0) {
            return Err(BenchmarkConfigError::SubRuns);
        }
        Ok(())
    }
    pub fn inherit(&mut self, base: &Self) {
        self.sub_runs = self.sub_runs.or(base.sub_runs);
        if self.solver.is_none() {
            self.solver = base.solver.clone();
        }
        if self.monitoring.is_none() {
            self.monitoring = base.monitoring.clone();
        }
        self.statistics.inherit(&base.statistics);
    }
    /// Build checks reject separately configured monitoring; benchmark statistics
    /// own metric requests. validate() checks local shape without building a run.
    pub fn prepare(&self) -> Result<PreparedSolverBenchmark, BenchmarkConfigError> {
        self.validate()?;
        if self.monitoring.is_some() {
            return Err(BenchmarkConfigError::Monitoring);
        }
        reject_phase_modes(self.solver.as_ref())?;
        let runs = usize::try_from(self.sub_runs.unwrap_or(1))
            .map_err(|_| BenchmarkConfigError::SubRuns)?;
        Ok(PreparedSolverBenchmark {
            name: self.name.clone().unwrap_or_default(),
            sub_runs: NonZeroUsize::new(runs).ok_or(BenchmarkConfigError::SubRuns)?,
            solver: self.solver.clone().unwrap_or_default(),
            metrics: self.statistics.solver_metrics(),
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkBlueprint {
    ConstructionWithAndWithoutLocalSearch,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BenchmarkWarmupLimits {
    pub hours: Option<i64>,
    pub minutes: Option<i64>,
    pub seconds: Option<i64>,
    pub milliseconds: Option<i64>,
}
impl BenchmarkWarmupLimits {
    /// Checked additive duration, retaining an explicit all-zero override.
    pub fn millis(&self, default: u64) -> Result<u64, BenchmarkConfigError> {
        if [self.hours, self.minutes, self.seconds, self.milliseconds]
            .iter()
            .all(Option::is_none)
        {
            return Ok(default);
        }
        let mut total = 0_u64;
        for (value, unit) in [
            (self.hours, 3_600_000),
            (self.minutes, 60_000),
            (self.seconds, 1000),
            (self.milliseconds, 1),
        ] {
            let v = u64::try_from(value.unwrap_or(0)).map_err(|_| BenchmarkConfigError::Warmup)?;
            total = total
                .checked_add(v.checked_mul(unit).ok_or(BenchmarkConfigError::Warmup)?)
                .ok_or(BenchmarkConfigError::Warmup)?;
        }
        Ok(total)
    }
}
#[derive(Clone, Debug, Default)]
pub struct BenchmarkSuiteDeclaration {
    pub name: Option<String>,
    pub solvers: Vec<SolverBenchmarkDeclaration>,
    pub blueprints: Vec<BenchmarkBlueprint>,
    pub inherited: Option<SolverBenchmarkDeclaration>,
    pub parallel_count: Option<String>,
    pub warmup: BenchmarkWarmupLimits,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BenchmarkConfigError {
    Name(String),
    NoSolvers,
    DuplicateName(String),
    PhaseEnvironment { phase: usize },
    SubRuns,
    Monitoring,
    ParallelCount(String),
    Warmup,
    Schedule(BenchmarkScheduleError),
}
impl fmt::Display for BenchmarkConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhaseEnvironment { phase } => write!(
                f,
                "phase {phase} cannot override the environment mode when in benchmark mode"
            ),
            _ => write!(f, "benchmark configuration: {self:?}"),
        }
    }
}
impl Error for BenchmarkConfigError {}
fn validate_name(name: Option<&str>) -> Result<(), BenchmarkConfigError> {
    if let Some(n) = name
        && (n.is_empty()
            || n.trim() != n
            || n.contains(['/', '\\'])
            || n.chars().any(char::is_control))
    {
        return Err(BenchmarkConfigError::Name(n.into()));
    }
    Ok(())
}
fn reject_phase_modes(
    solver: Option<&BenchmarkSolverSettings>,
) -> Result<(), BenchmarkConfigError> {
    if let Some(s) = solver
        && let Some(phase) = s
            .environment
            .declared_phases()
            .iter()
            .position(Option::is_some)
    {
        return Err(BenchmarkConfigError::PhaseEnvironment { phase });
    }
    Ok(())
}
/// Native automatic sizing policy inferred from supplied CPU-count criteria:
/// one through two CPUs, rounded-up half through four, then half plus one. Scheduling still
/// caps actual workers by configuration count. No Java executor/trace parity.
pub fn automatic_parallel_benchmarks(cpus: i64) -> usize {
    if cpus <= 2 {
        1
    } else {
        usize::try_from(if cpus <= 4 {
            (cpus + 1) / 2
        } else {
            cpus / 2 + 1
        })
        .unwrap_or(usize::MAX)
    }
}
/// Reserve all supplied names before filling Config_N gaps. Reject conflicts
/// before mutating any declaration, including a conflict late in the slice.
pub fn generate_solver_benchmark_names(
    solvers: &mut [SolverBenchmarkDeclaration],
) -> Result<(), BenchmarkConfigError> {
    let mut used = BTreeSet::new();
    for c in solvers.iter() {
        validate_name(c.name.as_deref())?;
        if let Some(n) = &c.name
            && !used.insert(n.clone())
        {
            return Err(BenchmarkConfigError::DuplicateName(n.clone()));
        }
    }
    let mut next = 0;
    let mut names = Vec::new();
    for c in solvers.iter() {
        if let Some(n) = &c.name {
            names.push(n.clone());
            continue;
        }
        loop {
            let n = format!("Config_{next}");
            next += 1;
            if used.insert(n.clone()) {
                names.push(n);
                break;
            }
        }
    }
    for (c, n) in solvers.iter_mut().zip(names) {
        c.name = Some(n);
    }
    Ok(())
}
impl BenchmarkSuiteDeclaration {
    pub fn validate(&self) -> Result<(), BenchmarkConfigError> {
        validate_name(self.name.as_deref())?;
        if self.solvers.is_empty() && self.blueprints.is_empty() {
            return Err(BenchmarkConfigError::NoSolvers);
        }
        reject_phase_modes(self.inherited.as_ref().and_then(|s| s.solver.as_ref()))?;
        for s in &self.solvers {
            reject_phase_modes(s.solver.as_ref())?;
        }
        Ok(())
    }
    pub fn parallel_benchmarks(&self, cpus: i64) -> Result<usize, BenchmarkConfigError> {
        let Some(s) = &self.parallel_count else {
            return Ok(1);
        };
        if s == "AUTO" {
            return Ok(automatic_parallel_benchmarks(cpus));
        }
        s.parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| BenchmarkConfigError::ParallelCount(s.clone()))
    }
    pub fn prepare(
        &self,
        cpus: i64,
        default_warmup_millis: u64,
    ) -> Result<PreparedBenchmarkSuite, BenchmarkConfigError> {
        self.validate()?;
        let mut declarations = self.solvers.clone();
        for blueprint in &self.blueprints {
            match blueprint {
                BenchmarkBlueprint::ConstructionWithAndWithoutLocalSearch => {
                    for phases in [
                        vec![SolverPhaseKind::Construction],
                        vec![SolverPhaseKind::Construction, SolverPhaseKind::LocalSearch],
                    ] {
                        let mut d = self.inherited.clone().unwrap_or_default();
                        d.name = None;
                        let settings = d.solver.get_or_insert_with(Default::default);
                        settings.environment = EnvironmentProfile::new(
                            settings.environment.declared_global(),
                            vec![None; phases.len()],
                        );
                        settings.phases = phases;
                        declarations.push(d);
                    }
                }
            }
        }
        for d in &mut declarations {
            if let Some(base) = &self.inherited {
                d.inherit(base);
            }
        }
        generate_solver_benchmark_names(&mut declarations)?;
        let solvers = declarations
            .iter()
            .map(SolverBenchmarkDeclaration::prepare)
            .collect::<Result<_, _>>()?;
        Ok(PreparedBenchmarkSuite {
            solvers,
            parallel: self.parallel_benchmarks(cpus)?,
            warmup: Duration::from_millis(self.warmup.millis(default_warmup_millis)?),
        })
    }
}
#[derive(Clone, Debug)]
pub struct PreparedSolverBenchmark {
    pub name: String,
    pub sub_runs: NonZeroUsize,
    pub solver: BenchmarkSolverSettings,
    pub metrics: Vec<BenchmarkMetric>,
}
#[derive(Clone, Debug)]
pub struct PreparedBenchmarkSuite {
    solvers: Vec<PreparedSolverBenchmark>,
    parallel: usize,
    warmup: Duration,
}
impl PreparedBenchmarkSuite {
    pub fn solvers(&self) -> &[PreparedSolverBenchmark] {
        &self.solvers
    }
    pub fn parallel_benchmarks(&self) -> usize {
        self.parallel
    }
    pub fn warmup_budget(&self) -> Duration {
        self.warmup
    }
    /// Execute per-configuration schedules with bounded scoped worker threads.
    /// Configuration reports retain declaration order; each report's local index
    /// is zero. Trial seeds match across configurations. Repetition counts may
    /// differ. All native schedule timing/cloning/error rules still apply.
    ///
    /// Warm-up budget is supplied only on warm-up trials; the callback must apply
    /// it cooperatively. warmup_runs controls repetitions separately. This is not
    /// preemption or Java warm-up/executor/order parity. Metric requests/settings
    /// reach the callback; unsupported instrumentation must remain unavailable.
    pub fn run<P: Clone + Send + Sync, S: Send, E: Send>(
        &self,
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
    ) -> Result<Vec<BenchmarkRunReport<S, E>>, BenchmarkConfigError> {
        // Validate every schedule before starting callbacks on any worker.
        if problems.is_empty() {
            return Err(BenchmarkConfigError::Schedule(
                BenchmarkScheduleError::NoProblems,
            ));
        }
        for c in &self.solvers {
            warmup_runs
                .checked_add(c.sub_runs.get())
                .and_then(|n| n.checked_mul(problems.len()))
                .ok_or(BenchmarkConfigError::Schedule(
                    BenchmarkScheduleError::TooManyTrials,
                ))?;
        }
        let next = AtomicUsize::new(0);
        let results = Mutex::new((0..self.solvers.len()).map(|_| None).collect::<Vec<_>>());
        std::thread::scope(|scope| {
            for _ in 0..self.parallel.min(self.solvers.len()) {
                let (next, results, solve) = (&next, &results, &solve);
                scope.spawn(move || {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(c) = self.solvers.get(index) else {
                            break;
                        };
                        let schedule = BenchmarkSchedule {
                            warmup_runs,
                            measured_runs: c.sub_runs,
                            seed,
                        };
                        let report = schedule
                            .run(
                                std::slice::from_ref(c),
                                problems,
                                cancellation,
                                |c, p, t| solve(c, p, t, t.warmup.then_some(self.warmup)),
                            )
                            .expect("schedule preflight");
                        results.lock().unwrap()[index] = Some(report);
                    }
                });
            }
        });
        Ok(results
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|r| r.expect("each configuration scheduled"))
            .collect())
    }
}
