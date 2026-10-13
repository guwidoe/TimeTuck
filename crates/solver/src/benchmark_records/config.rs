use super::*;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
/// Known report statistic requests. Requests are metadata; a collector must
/// provide measurements, and unavailable metrics are never fabricated as zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordStatistic {
    BestScore,
    StepScore,
    MemoryUse,
    BestSolutionMutation,
    MoveCountPerStep,
    MoveCountPerType,
    ScoreCalculationSpeed,
    MoveEvaluationSpeed,
    ConstraintMatchTotalBestScore,
    ConstraintMatchTotalStepScore,
    PickedMoveTypeBestScoreDiff,
    PickedMoveTypeStepScoreDiff,
}
impl RecordStatistic {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "BEST_SCORE" => Self::BestScore,
            "STEP_SCORE" => Self::StepScore,
            "MEMORY_USE" => Self::MemoryUse,
            "BEST_SOLUTION_MUTATION" => Self::BestSolutionMutation,
            "MOVE_COUNT_PER_STEP" => Self::MoveCountPerStep,
            "MOVE_COUNT_PER_TYPE" => Self::MoveCountPerType,
            "SCORE_CALCULATION_SPEED" => Self::ScoreCalculationSpeed,
            "MOVE_EVALUATION_SPEED" => Self::MoveEvaluationSpeed,
            "CONSTRAINT_MATCH_TOTAL_BEST_SCORE" => Self::ConstraintMatchTotalBestScore,
            "CONSTRAINT_MATCH_TOTAL_STEP_SCORE" => Self::ConstraintMatchTotalStepScore,
            "PICKED_MOVE_TYPE_BEST_SCORE_DIFF" => Self::PickedMoveTypeBestScoreDiff,
            "PICKED_MOVE_TYPE_STEP_SCORE_DIFF" => Self::PickedMoveTypeStepScoreDiff,
            _ => return Err(fail(format!("unknown statistic {s}"))),
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::BestScore => "BEST_SCORE",
            Self::StepScore => "STEP_SCORE",
            Self::MemoryUse => "MEMORY_USE",
            Self::BestSolutionMutation => "BEST_SOLUTION_MUTATION",
            Self::MoveCountPerStep => "MOVE_COUNT_PER_STEP",
            Self::MoveCountPerType => "MOVE_COUNT_PER_TYPE",
            Self::ScoreCalculationSpeed => "SCORE_CALCULATION_SPEED",
            Self::MoveEvaluationSpeed => "MOVE_EVALUATION_SPEED",
            Self::ConstraintMatchTotalBestScore => "CONSTRAINT_MATCH_TOTAL_BEST_SCORE",
            Self::ConstraintMatchTotalStepScore => "CONSTRAINT_MATCH_TOTAL_STEP_SCORE",
            Self::PickedMoveTypeBestScoreDiff => "PICKED_MOVE_TYPE_BEST_SCORE_DIFF",
            Self::PickedMoveTypeStepScoreDiff => "PICKED_MOVE_TYPE_STEP_SCORE_DIFF",
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct RecordProblemConfig {
    pub statistics: Vec<RecordStatistic>,
    pub single_statistics: Vec<RecordStatistic>,
    pub loader: Option<String>,
    pub write_output: Option<bool>,
    pub inputs: Vec<PathBuf>,
}
impl RecordProblemConfig {
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        check(
            n,
            &[
                "problemStatisticType",
                "singleStatisticType",
                "solutionFileIOClass",
                "writeOutputSolutionEnabled",
                "inputSolutionFile",
            ],
        )?;
        Ok(Self {
            statistics: children(n)
                .filter(|n| n.has_tag_name("problemStatisticType"))
                .map(|n| RecordStatistic::parse(n.text().unwrap_or_default().trim()))
                .collect::<Result<_>>()?,
            single_statistics: children(n)
                .filter(|n| n.has_tag_name("singleStatisticType"))
                .map(|n| RecordStatistic::parse(n.text().unwrap_or_default().trim()))
                .collect::<Result<_>>()?,
            loader: text(n, "solutionFileIOClass").map(str::to_owned),
            write_output: number(n, "writeOutputSolutionEnabled")?,
            inputs: children(n)
                .filter(|n| n.has_tag_name("inputSolutionFile"))
                .map(|n| PathBuf::from(n.text().unwrap_or_default().trim()))
                .collect(),
        })
    }
    fn write(&self, out: &mut String) {
        out.push_str("<problemBenchmarks>");
        for s in &self.statistics {
            field(out, "problemStatisticType", s.label());
        }
        for s in &self.single_statistics {
            field(out, "singleStatisticType", s.label());
        }
        optional(out, "solutionFileIOClass", &self.loader);
        optional(out, "writeOutputSolutionEnabled", &self.write_output);
        for p in &self.inputs {
            field(out, "inputSolutionFile", p.display());
        }
        out.push_str("</problemBenchmarks>");
    }
}
#[derive(Clone, Debug)]
pub struct RecordSolverConfig {
    pub name: Option<String>,
    pub solver: Option<SolverXmlConfiguration>,
    pub problem: Option<RecordProblemConfig>,
    pub sub_runs: Option<i64>,
}
impl RecordSolverConfig {
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        check(
            n,
            &["name", "solver", "problemBenchmarks", "subSingleCount"],
        )?;
        Ok(Self {
            name: text(n, "name").map(str::to_owned),
            solver: children(n)
                .find(|n| n.has_tag_name("solver"))
                .map(solver)
                .transpose()?,
            problem: children(n)
                .find(|n| n.has_tag_name("problemBenchmarks"))
                .map(RecordProblemConfig::parse)
                .transpose()?,
            sub_runs: number(n, "subSingleCount")?,
        })
    }
    fn write(&self, out: &mut String, name: &str) {
        out.push_str(&format!("<{name}>"));
        optional(out, "name", &self.name);
        if let Some(c) = &self.solver {
            out.push_str(&solver_xml(c, "solver"));
        }
        if let Some(p) = &self.problem {
            p.write(out);
        }
        optional(out, "subSingleCount", &self.sub_runs);
        out.push_str(&format!("</{name}>"));
    }
}
#[derive(Clone, Debug, Default)]
pub struct BenchmarkRecordConfig {
    pub directory: Option<PathBuf>,
    pub parallel: Option<String>,
    pub warmup_seconds: Option<i64>,
    pub locale: Option<String>,
    pub ranking: Option<RecordRanking>,
    pub inherited: Option<RecordSolverConfig>,
    pub solvers: Vec<RecordSolverConfig>,
}
impl BenchmarkRecordConfig {
    pub fn parse(input: &str) -> Result<Self> {
        let d = document(input)?;
        let n = d.root_element();
        if !n.has_tag_name("plannerBenchmark") {
            return Err(fail("expected plannerBenchmark"));
        }
        check(
            n,
            &[
                "benchmarkDirectory",
                "parallelBenchmarkCount",
                "warmUpSecondsSpentLimit",
                "benchmarkReport",
                "inheritedSolverBenchmark",
                "solverBenchmark",
            ],
        )?;
        let report = children(n).find(|n| n.has_tag_name("benchmarkReport"));
        if let Some(r) = report {
            check(r, &["locale", "solverRankingType"])?;
        }
        Ok(Self {
            directory: text(n, "benchmarkDirectory").map(PathBuf::from),
            parallel: text(n, "parallelBenchmarkCount").map(str::to_owned),
            warmup_seconds: number(n, "warmUpSecondsSpentLimit")?,
            locale: report.and_then(|n| text(n, "locale")).map(str::to_owned),
            ranking: report
                .and_then(|n| text(n, "solverRankingType"))
                .map(RecordRanking::parse)
                .transpose()?,
            inherited: children(n)
                .find(|n| n.has_tag_name("inheritedSolverBenchmark"))
                .map(RecordSolverConfig::parse)
                .transpose()?,
            solvers: children(n)
                .filter(|n| n.has_tag_name("solverBenchmark"))
                .map(RecordSolverConfig::parse)
                .collect::<Result<_>>()?,
        })
    }
    pub fn to_xml(&self) -> String {
        let mut out = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plannerBenchmark>".to_owned();
        if let Some(p) = &self.directory {
            field(&mut out, "benchmarkDirectory", p.display());
        }
        optional(&mut out, "parallelBenchmarkCount", &self.parallel);
        optional(&mut out, "warmUpSecondsSpentLimit", &self.warmup_seconds);
        if self.locale.is_some() || self.ranking.is_some() {
            out.push_str("<benchmarkReport>");
            optional(&mut out, "locale", &self.locale);
            if let Some(r) = self.ranking {
                field(&mut out, "solverRankingType", r.label());
            }
            out.push_str("</benchmarkReport>");
        }
        if let Some(s) = &self.inherited {
            s.write(&mut out, "inheritedSolverBenchmark");
        }
        for s in &self.solvers {
            s.write(&mut out, "solverBenchmark");
        }
        out.push_str("</plannerBenchmark>");
        out
    }
    /// Translate typed declarations into the existing executable benchmark runtime.
    /// Statistic requests remain available in these records; this adapter does not
    /// invent collectors for unavailable Java-specific instrumentation.
    pub fn runtime_configuration(&self) -> BenchmarkXmlConfiguration {
        BenchmarkXmlConfiguration {
            directory: self
                .directory
                .clone()
                .unwrap_or_else(|| PathBuf::from("target/benchmark")),
            solvers: self
                .solvers
                .iter()
                .map(|s| XmlBenchmarkSolver {
                    name: s.name.clone(),
                    sub_runs: s.sub_runs,
                    solver: s.solver.clone().unwrap_or_default(),
                })
                .collect(),
            inherited: self.inherited.as_ref().and_then(|s| s.solver.clone()),
            inputs: self
                .solvers
                .iter()
                .flat_map(|s| {
                    s.problem.iter().flat_map(|p| {
                        p.inputs
                            .iter()
                            .map(|f| (p.loader.clone().unwrap_or_default(), f.clone()))
                    })
                })
                .collect(),
            parallel: self.parallel.clone(),
            warmup_seconds: self.warmup_seconds,
        }
    }
}
type Read<P> = dyn Fn(&Path) -> std::result::Result<P, String> + Send + Sync;
type Write<P> = dyn Fn(&P, &Path) -> std::result::Result<(), String> + Send + Sync;
/// Actual typed file callbacks replace reflective SolutionFileIO class loading.
/// Cloning retains the binding; read/write invoke the registered callbacks.
#[derive(Clone)]
pub struct BenchmarkRecordFileIo<P> {
    pub extension: String,
    read: Arc<Read<P>>,
    write: Arc<Write<P>>,
}
impl<P> BenchmarkRecordFileIo<P> {
    pub fn new(
        extension: impl Into<String>,
        read: impl Fn(&Path) -> std::result::Result<P, String> + Send + Sync + 'static,
        write: impl Fn(&P, &Path) -> std::result::Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            extension: extension.into(),
            read: Arc::new(read),
            write: Arc::new(write),
        }
    }
    pub fn read(&self, path: &Path) -> std::result::Result<P, String> {
        (self.read)(path)
    }
    pub fn write(&self, p: &P, path: &Path) -> std::result::Result<(), String> {
        (self.write)(p, path)
    }
    pub fn register<S, E>(
        &self,
        runtime: &mut XmlBenchmarkRuntime<P, S, E>,
        key: String,
        map_error: impl Fn(String) -> E + Send + Sync + 'static,
    ) -> std::result::Result<(), BenchmarkXmlError>
    where
        P: 'static,
    {
        let read = self.read.clone();
        runtime.register_loader(key, move |p| read(p).map_err(&map_error))
    }
}

/// Validated symbolic nearby declaration retained in historical records. Meter
/// keys require an explicit native distance callback for execution; this query
/// does not instantiate Java classes or promise a distribution trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedNearbySelection {
    pub meter: String,
    pub origin_selector: String,
    pub parabolic_maximum: Option<usize>,
}
impl SolverXmlConfiguration {
    pub fn recorded_nearby_selections(&self) -> Result<Vec<RecordedNearbySelection>> {
        fn visit(
            n: &SolverXmlElement,
            ids: &mut Vec<String>,
            out: &mut Vec<RecordedNearbySelection>,
        ) -> Result<()> {
            if let Some((_, id)) = n.attributes.iter().find(|(k, _)| k == "id") {
                ids.push(id.clone());
            }
            if n.name == "nearbySelection" {
                let meter = n
                    .value("nearbyDistanceMeterClass")
                    .ok_or_else(|| fail("missing nearby meter"))?
                    .to_owned();
                let origin = n
                    .child("originEntitySelector")
                    .and_then(|e| e.attributes.iter().find(|(k, _)| k == "mimicSelectorRef"))
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| fail("missing nearby origin selector"))?;
                let parabolic_maximum = n
                    .value("parabolicDistributionSizeMaximum")
                    .map(|v| {
                        v.parse::<usize>()
                            .map_err(|_| fail("invalid nearby positive bound"))
                    })
                    .transpose()?;
                if parabolic_maximum == Some(0) {
                    return Err(fail("nearby bound must be positive"));
                }
                out.push(RecordedNearbySelection {
                    meter,
                    origin_selector: origin,
                    parabolic_maximum,
                });
            }
            for c in &n.children {
                visit(c, ids, out)?;
            }
            Ok(())
        }
        let mut ids = vec![];
        let mut out = vec![];
        visit(self.root(), &mut ids, &mut out)?;
        for declaration in &out {
            if ids
                .iter()
                .filter(|id| **id == declaration.origin_selector)
                .count()
                != 1
            {
                return Err(fail("nearby origin requires one registered selector id"));
            }
        }
        Ok(out)
    }
}
impl<'a, S> LocalSearchPhasePlan<'a, S> {
    /// Apply the validated recorded late-acceptance size to this real native phase.
    /// Other acceptance declarations remain the explicit factory's responsibility.
    pub fn with_recorded_late_acceptance(
        self,
        solver: &SolverXmlConfiguration,
        phase: usize,
    ) -> Result<Self> {
        let local = solver
            .root()
            .children
            .iter()
            .filter(|e| e.name == "localSearch")
            .nth(phase)
            .ok_or_else(|| fail("missing local-search phase"))?;
        let mut config = self.into_config();
        if let Some(size) = local
            .child("acceptor")
            .and_then(|a| a.value("lateAcceptanceSize"))
        {
            let history_size = size
                .parse::<usize>()
                .map_err(|_| fail("lateAcceptanceSize must be positive"))?;
            if history_size == 0 {
                return Err(fail("lateAcceptanceSize must be positive"));
            }
            config.acceptance = Acceptance::LateAcceptance {
                history_size,
                hill_climbing: true,
            };
        }
        Ok(Self::new(config))
    }
}
