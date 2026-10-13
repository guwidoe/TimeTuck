use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordScore {
    Simple(SimpleScore),
    HardSoft(HardSoftScore),
}
impl RecordScore {
    fn parse(class: &str, s: &str) -> Result<Self> {
        match class {
            "ai.timefold.solver.core.api.score.SimpleScore" => s
                .parse()
                .map(Self::Simple)
                .map_err(|e| fail(format!("score: {e}"))),
            "ai.timefold.solver.core.api.score.HardSoftScore" => s
                .parse()
                .map(Self::HardSoft)
                .map_err(|e| fail(format!("score: {e}"))),
            _ => Err(fail(format!("unsupported recorded score {class}"))),
        }
    }
    fn class(&self) -> &'static str {
        match self {
            Self::Simple(_) => "ai.timefold.solver.core.api.score.SimpleScore",
            Self::HardSoft(_) => "ai.timefold.solver.core.api.score.HardSoftScore",
        }
    }
    pub fn outcome(&self, initialized: bool) -> BenchmarkOutcome {
        match self {
            Self::Simple(s) => BenchmarkOutcome::success(s, initialized),
            Self::HardSoft(s) => BenchmarkOutcome::success(s, initialized),
        }
        .expect("built-in score levels")
    }
}
impl fmt::Display for RecordScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple(s) => s.fmt(f),
            Self::HardSoft(s) => s.fmt(f),
        }
    }
}
/// Historical counter: -1 means unavailable; other negative values are rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordCounter {
    Available(u64),
    Unavailable,
}
impl std::str::FromStr for RecordCounter {
    type Err = BenchmarkRecordError;
    fn from_str(s: &str) -> Result<Self> {
        if s == "-1" {
            Ok(Self::Unavailable)
        } else {
            s.parse()
                .map(Self::Available)
                .map_err(|_| fail("invalid nonnegative counter"))
        }
    }
}
impl fmt::Display for RecordCounter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available(n) => n.fmt(f),
            Self::Unavailable => f.write_str("-1"),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct RecordCounters {
    pub millis: Option<RecordCounter>,
    pub score_calculations: Option<RecordCounter>,
    pub move_evaluations: Option<RecordCounter>,
}
impl RecordCounters {
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        Ok(Self {
            millis: number(n, "timeMillisSpent")?,
            score_calculations: number(n, "scoreCalculationCount")?,
            move_evaluations: number(n, "moveEvaluationCount")?,
        })
    }
    fn write(&self, out: &mut String) {
        optional(out, "timeMillisSpent", &self.millis);
        optional(out, "scoreCalculationCount", &self.score_calculations);
        optional(out, "moveEvaluationCount", &self.move_evaluations);
    }
}
#[derive(Clone, Debug, Default)]
pub struct RecordSubRun {
    pub index: usize,
    pub succeeded: Option<bool>,
    pub score: Option<RecordScore>,
    pub initialized: Option<bool>,
    pub counters: RecordCounters,
}
impl RecordSubRun {
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        check(
            n,
            &[
                "subSingleBenchmarkIndex",
                "succeeded",
                "score",
                "initialized",
                "timeMillisSpent",
                "scoreCalculationCount",
                "moveEvaluationCount",
            ],
        )?;
        let score = children(n)
            .find(|n| n.has_tag_name("score"))
            .map(|n| {
                RecordScore::parse(
                    n.attribute("class")
                        .ok_or_else(|| fail("missing score class"))?,
                    n.text().unwrap_or_default().trim(),
                )
            })
            .transpose()?;
        Ok(Self {
            index: number(n, "subSingleBenchmarkIndex")?
                .ok_or_else(|| fail("missing sub-run index"))?,
            succeeded: number(n, "succeeded")?,
            score,
            initialized: number(n, "initialized")?,
            counters: RecordCounters::parse(n)?,
        })
    }
    fn write(&self, out: &mut String) {
        out.push_str("<subSingleBenchmarkResult>");
        field(out, "subSingleBenchmarkIndex", self.index);
        optional(out, "succeeded", &self.succeeded);
        if let Some(s) = &self.score {
            out.push_str(&format!("<score class=\"{}\">{s}</score>", s.class()));
        }
        optional(out, "initialized", &self.initialized);
        self.counters.write(out);
        out.push_str("</subSingleBenchmarkResult>");
    }
}
#[derive(Clone, Debug, Default)]
pub struct RecordSingleRun {
    pub id: u64,
    pub sub_runs: Vec<RecordSubRun>,
    pub all_initialized: Option<bool>,
    pub counters: RecordCounters,
    pub summary_score: Option<RecordScore>,
}
impl RecordSingleRun {
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        check(
            n,
            &[
                "subSingleBenchmarkResult",
                "allScoresInitialized",
                "summaryScore",
                "timeMillisSpent",
                "scoreCalculationCount",
                "moveEvaluationCount",
            ],
        )?;
        let id = n
            .attribute("id")
            .ok_or_else(|| fail("missing single run id"))?
            .parse()
            .map_err(|_| fail("invalid single run id"))?;
        let sub_runs = children(n)
            .filter(|n| n.has_tag_name("subSingleBenchmarkResult"))
            .map(RecordSubRun::parse)
            .collect::<Result<Vec<_>>>()?;
        let mut indices = BTreeSet::new();
        if sub_runs.iter().any(|r| !indices.insert(r.index)) {
            return Err(fail("duplicate sub-run index"));
        }
        Ok(Self {
            id,
            sub_runs,
            all_initialized: number(n, "allScoresInitialized")?,
            counters: RecordCounters::parse(n)?,
            summary_score: children(n)
                .find(|n| n.has_tag_name("summaryScore"))
                .map(|n| {
                    RecordScore::parse(
                        n.attribute("class")
                            .ok_or_else(|| fail("missing summary score class"))?,
                        n.text().unwrap_or_default().trim(),
                    )
                })
                .transpose()?,
        })
    }
    fn write(&self, out: &mut String) {
        out.push_str(&format!("<singleBenchmarkResult id=\"{}\">", self.id));
        for r in &self.sub_runs {
            r.write(out);
        }
        optional(out, "allScoresInitialized", &self.all_initialized);
        if let Some(s) = &self.summary_score {
            out.push_str(&format!(
                "<summaryScore class=\"{}\">{s}</summaryScore>",
                s.class()
            ));
        }
        self.counters.write(out);
        out.push_str("</singleBenchmarkResult>");
    }
    fn outcomes(&self) -> Result<Vec<BenchmarkOutcome>> {
        if let Some(s) = &self.summary_score {
            return Ok(vec![s.outcome(self.all_initialized.unwrap_or(false))]);
        }
        self.sub_runs
            .iter()
            .map(|r| {
                if r.succeeded == Some(false) {
                    Ok(BenchmarkOutcome::Failure)
                } else {
                    r.score
                        .as_ref()
                        .map(|s| s.outcome(r.initialized.unwrap_or(false)))
                        .ok_or_else(|| fail("unscored recorded success cannot be ranked"))
                }
            })
            .collect()
    }
}
#[derive(Clone, Debug)]
pub struct RecordSolver {
    pub name: String,
    pub sub_runs: usize,
    pub config: SolverXmlConfiguration,
    pub runs: Vec<RecordSingleRun>,
}
impl RecordSolver {
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        check(
            n,
            &[
                "name",
                "subSingleCount",
                "solverConfig",
                "singleBenchmarkResult",
            ],
        )?;
        let sub_runs =
            number(n, "subSingleCount")?.ok_or_else(|| fail("missing subSingleCount"))?;
        if sub_runs == 0 {
            return Err(fail("subSingleCount must be positive"));
        }
        Ok(Self {
            name: text(n, "name")
                .ok_or_else(|| fail("missing solver name"))?
                .into(),
            sub_runs,
            config: solver(
                children(n)
                    .find(|n| n.has_tag_name("solverConfig"))
                    .ok_or_else(|| fail("missing solverConfig"))?,
            )?,
            runs: children(n)
                .filter(|n| n.has_tag_name("singleBenchmarkResult"))
                .map(RecordSingleRun::parse)
                .collect::<Result<_>>()?,
        })
    }
    fn write(&self, out: &mut String) {
        out.push_str("<solverBenchmarkResult>");
        field(out, "name", &self.name);
        field(out, "subSingleCount", self.sub_runs);
        out.push_str(&solver_xml(&self.config, "solverConfig"));
        for r in &self.runs {
            r.write(out);
        }
        out.push_str("</solverBenchmarkResult>");
    }
}
#[derive(Clone, Debug)]
pub struct RecordProblemStatistic {
    pub kind: RecordStatistic,
    pub report_file: Option<String>,
    pub title: Option<String>,
    pub y_label: Option<String>,
    pub statistic_type: Option<RecordStatistic>,
}
impl RecordProblemStatistic {
    fn element(kind: RecordStatistic) -> Result<&'static str> {
        Ok(match kind {
            RecordStatistic::BestScore => "bestScoreProblemStatistic",
            RecordStatistic::StepScore => "stepScoreProblemStatistic",
            RecordStatistic::MemoryUse => "memoryUseProblemStatistic",
            RecordStatistic::BestSolutionMutation => "bestSolutionMutationProblemStatistic",
            RecordStatistic::MoveCountPerStep => "moveCountPerStepProblemStatistic",
            RecordStatistic::MoveCountPerType => "moveCountPerTypeProblemStatistic",
            RecordStatistic::ScoreCalculationSpeed => "scoreCalculationSpeedProblemStatistic",
            RecordStatistic::MoveEvaluationSpeed => "moveEvaluationSpeedProblemStatistic",
            _ => return Err(fail("not a problem statistic")),
        })
    }
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        check(
            n,
            &[
                "problemStatisticType",
                "reportFileName",
                "reportTitle",
                "yLabel",
                "statisticType",
            ],
        )?;
        let kind = RecordStatistic::parse(
            text(n, "problemStatisticType").ok_or_else(|| fail("missing problemStatisticType"))?,
        )?;
        if Self::element(kind)? != n.tag_name().name() {
            return Err(fail("statistic element/type mismatch"));
        }
        let statistic_type = text(n, "statisticType")
            .map(RecordStatistic::parse)
            .transpose()?;
        if statistic_type.is_some_and(|s| s != kind) {
            return Err(fail("statisticType mismatch"));
        }
        Ok(Self {
            kind,
            report_file: text(n, "reportFileName").map(str::to_owned),
            title: text(n, "reportTitle").map(str::to_owned),
            y_label: text(n, "yLabel").map(str::to_owned),
            statistic_type,
        })
    }
    fn write(&self, out: &mut String) {
        let name = Self::element(self.kind).expect("validated problem statistic");
        out.push_str(&format!("<{name}>"));
        field(out, "problemStatisticType", self.kind.label());
        optional(out, "reportFileName", &self.report_file);
        optional(out, "reportTitle", &self.title);
        optional(out, "yLabel", &self.y_label);
        if let Some(s) = self.statistic_type {
            field(out, "statisticType", s.label());
        }
        out.push_str(&format!("</{name}>"));
    }
}
#[derive(Clone, Debug, Default)]
pub struct RecordProblem {
    pub name: String,
    pub file: Option<PathBuf>,
    pub write_output: Option<bool>,
    pub statistics: Vec<RecordProblemStatistic>,
    pub run_ids: Vec<u64>,
    pub entity_count: Option<u64>,
    pub variable_count: Option<u64>,
    pub maximum_values: Option<u64>,
    pub scale: Option<u64>,
    pub loading_millis: Option<RecordCounter>,
}
impl RecordProblem {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| fail("invalid problem file name"))?
            .to_owned();
        Ok(Self {
            name,
            file: Some(path),
            ..Default::default()
        })
    }
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        check(
            n,
            &[
                "name",
                "fileProblemProvider",
                "writeOutputSolutionEnabled",
                "bestScoreProblemStatistic",
                "stepScoreProblemStatistic",
                "memoryUseProblemStatistic",
                "bestSolutionMutationProblemStatistic",
                "moveCountPerStepProblemStatistic",
                "moveCountPerTypeProblemStatistic",
                "scoreCalculationSpeedProblemStatistic",
                "moveEvaluationSpeedProblemStatistic",
                "singleBenchmarkResult",
                "entityCount",
                "variableCount",
                "maximumValueCount",
                "problemScale",
                "inputSolutionLoadingTimeMillisSpent",
            ],
        )?;
        let file = children(n)
            .find(|n| n.has_tag_name("fileProblemProvider"))
            .map(|n| {
                check(n, &["inputSolutionFile"])?;
                Ok(text(n, "inputSolutionFile")
                    .map(PathBuf::from)
                    .unwrap_or_default())
            })
            .transpose()?;
        Ok(Self {
            name: text(n, "name")
                .ok_or_else(|| fail("missing problem name"))?
                .into(),
            file,
            write_output: number(n, "writeOutputSolutionEnabled")?,
            statistics: children(n)
                .filter(|n| n.tag_name().name().ends_with("ProblemStatistic"))
                .map(RecordProblemStatistic::parse)
                .collect::<Result<_>>()?,
            run_ids: children(n)
                .filter(|n| n.has_tag_name("singleBenchmarkResult"))
                .map(|n| {
                    n.text()
                        .unwrap_or_default()
                        .trim()
                        .parse()
                        .map_err(|_| fail("invalid single run reference"))
                })
                .collect::<Result<_>>()?,
            entity_count: number(n, "entityCount")?,
            variable_count: number(n, "variableCount")?,
            maximum_values: number(n, "maximumValueCount")?,
            scale: number(n, "problemScale")?,
            loading_millis: number(n, "inputSolutionLoadingTimeMillisSpent")?,
        })
    }
    fn write(&self, out: &mut String) {
        out.push_str("<unifiedProblemBenchmarkResult>");
        field(out, "name", &self.name);
        if let Some(p) = &self.file {
            if p.as_os_str().is_empty() {
                out.push_str("<fileProblemProvider/>");
            } else {
                out.push_str("<fileProblemProvider>");
                field(out, "inputSolutionFile", p.display());
                out.push_str("</fileProblemProvider>");
            }
        }
        optional(out, "writeOutputSolutionEnabled", &self.write_output);
        for s in &self.statistics {
            s.write(out);
        }
        for id in &self.run_ids {
            field(out, "singleBenchmarkResult", id);
        }
        optional(out, "entityCount", &self.entity_count);
        optional(out, "variableCount", &self.variable_count);
        optional(out, "maximumValueCount", &self.maximum_values);
        optional(out, "problemScale", &self.scale);
        optional(
            out,
            "inputSolutionLoadingTimeMillisSpent",
            &self.loading_millis,
        );
        out.push_str("</unifiedProblemBenchmarkResult>");
    }
}
#[derive(Clone, Debug, Default)]
pub struct RecordSystemMetadata {
    pub processors: Option<usize>,
    pub logging_level: Option<String>,
    pub max_memory: Option<u64>,
    pub solver_version: Option<String>,
    pub java_version: Option<String>,
    pub java_vm: Option<String>,
    pub os: Option<String>,
    pub parallel: Option<usize>,
    pub warmup_millis: Option<u64>,
}
impl RecordSystemMetadata {
    pub fn capture() -> Self {
        Self {
            processors: Some(std::thread::available_parallelism().map_or(1, usize::from)),
            os: Some(format!(
                "{} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )),
            ..Default::default()
        }
    }
    fn parse(n: roxmltree::Node<'_, '_>) -> Result<Self> {
        Ok(Self {
            processors: number(n, "availableProcessors")?,
            logging_level: text(n, "loggingLevelTimefoldSolverCore").map(str::to_owned),
            max_memory: number(n, "maxMemory")?,
            solver_version: text(n, "timefoldSolverVersion").map(str::to_owned),
            java_version: text(n, "javaVersion").map(str::to_owned),
            java_vm: text(n, "javaVM").map(str::to_owned),
            os: text(n, "operatingSystem").map(str::to_owned),
            parallel: number(n, "parallelBenchmarkCount")?,
            warmup_millis: number(n, "warmUpTimeMillisSpentLimit")?,
        })
    }
    fn write(&self, out: &mut String) {
        optional(out, "availableProcessors", &self.processors);
        optional(out, "loggingLevelTimefoldSolverCore", &self.logging_level);
        optional(out, "maxMemory", &self.max_memory);
        optional(out, "timefoldSolverVersion", &self.solver_version);
        optional(out, "javaVersion", &self.java_version);
        optional(out, "javaVM", &self.java_vm);
        optional(out, "operatingSystem", &self.os);
        optional(out, "parallelBenchmarkCount", &self.parallel);
        optional(out, "warmUpTimeMillisSpentLimit", &self.warmup_millis);
    }
}
#[derive(Clone, Debug, Default)]
pub struct BenchmarkRecordSet {
    pub name: Option<String>,
    pub aggregation: bool,
    pub system: RecordSystemMetadata,
    pub solvers: Vec<RecordSolver>,
    pub problems: Vec<RecordProblem>,
    pub started: Option<String>,
    pub elapsed_millis: Option<u64>,
}
impl BenchmarkRecordSet {
    pub fn parse(input: &str) -> Result<Self> {
        let d = document(input)?;
        let n = d.root_element();
        if !n.has_tag_name("plannerBenchmarkResult") {
            return Err(fail("expected plannerBenchmarkResult"));
        }
        check(
            n,
            &[
                "name",
                "aggregation",
                "availableProcessors",
                "loggingLevelTimefoldSolverCore",
                "maxMemory",
                "timefoldSolverVersion",
                "javaVersion",
                "javaVM",
                "operatingSystem",
                "parallelBenchmarkCount",
                "warmUpTimeMillisSpentLimit",
                "solverBenchmarkResult",
                "unifiedProblemBenchmarkResult",
                "startingTimestamp",
                "benchmarkTimeMillisSpent",
            ],
        )?;
        let out = Self {
            name: text(n, "name").map(str::to_owned),
            aggregation: number(n, "aggregation")?.unwrap_or(false),
            system: RecordSystemMetadata::parse(n)?,
            solvers: children(n)
                .filter(|n| n.has_tag_name("solverBenchmarkResult"))
                .map(RecordSolver::parse)
                .collect::<Result<_>>()?,
            problems: children(n)
                .filter(|n| n.has_tag_name("unifiedProblemBenchmarkResult"))
                .map(RecordProblem::parse)
                .collect::<Result<_>>()?,
            started: text(n, "startingTimestamp").map(str::to_owned),
            elapsed_millis: number(n, "benchmarkTimeMillisSpent")?,
        };
        out.validate()?;
        Ok(out)
    }
    pub fn validate(&self) -> Result<()> {
        let mut runs = BTreeSet::new();
        for s in &self.solvers {
            for r in &s.runs {
                if !runs.insert(r.id) {
                    return Err(fail("duplicate single run id"));
                }
            }
        }
        let mut refs = BTreeSet::new();
        for p in &self.problems {
            for id in &p.run_ids {
                if !runs.contains(id) {
                    return Err(fail("dangling single run reference"));
                }
                if !refs.insert(*id) {
                    return Err(fail("single run has multiple problem owners"));
                }
            }
        }
        if runs != refs {
            return Err(fail("single run missing problem owner"));
        }
        Ok(())
    }
    pub fn to_xml(&self) -> Result<String> {
        self.validate()?;
        let mut out =
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plannerBenchmarkResult>".to_owned();
        optional(&mut out, "name", &self.name);
        field(&mut out, "aggregation", self.aggregation);
        self.system.write(&mut out);
        for s in &self.solvers {
            s.write(&mut out);
        }
        for p in &self.problems {
            p.write(&mut out);
        }
        optional(&mut out, "startingTimestamp", &self.started);
        optional(&mut out, "benchmarkTimeMillisSpent", &self.elapsed_millis);
        out.push_str("</plannerBenchmarkResult>");
        Ok(out)
    }
    pub fn observations(&self) -> Result<Vec<BenchmarkComparison<usize>>> {
        self.validate()?;
        let owners: BTreeMap<_, _> = self
            .problems
            .iter()
            .enumerate()
            .flat_map(|(i, p)| p.run_ids.iter().map(move |&id| (id, i)))
            .collect();
        let mut out = vec![];
        for (configuration, s) in self.solvers.iter().enumerate() {
            for r in &s.runs {
                for outcome in r.outcomes()? {
                    out.push(BenchmarkComparison {
                        configuration,
                        problem: owners[&r.id],
                        outcome,
                    });
                }
            }
        }
        Ok(out)
    }
    /// Merge selected real single runs in encounter order. Solver allocation identity
    /// is retained as the grouping key; file problem keys unify across input reports.
    /// Run IDs are freshly document-scoped, relationships point to the cloned runs.
    /// The source reports/handles remain untouched, including independent sub-runs.
    pub fn merge(selected: &[RecordSelection]) -> Result<Self> {
        let mut out = Self {
            aggregation: true,
            system: RecordSystemMetadata::capture(),
            ..Default::default()
        };
        let mut solver_keys = BTreeMap::<usize, usize>::new();
        let mut problem_keys = BTreeMap::<(Option<PathBuf>, String), usize>::new();
        let mut next_id = 1_u64;
        for selection in selected {
            let key = Arc::as_ptr(&selection.solver) as usize;
            let si = *solver_keys.entry(key).or_insert_with(|| {
                let i = out.solvers.len();
                let mut s = (*selection.solver).clone();
                s.runs.clear();
                out.solvers.push(s);
                i
            });
            let file = selection
                .problem
                .file
                .as_ref()
                .filter(|p| !p.as_os_str().is_empty())
                .cloned();
            let key = (
                file.clone(),
                if file.is_some() {
                    String::new()
                } else {
                    selection.problem.name.clone()
                },
            );
            let pi = *problem_keys.entry(key).or_insert_with(|| {
                let i = out.problems.len();
                let mut p = (*selection.problem).clone();
                p.run_ids.clear();
                out.problems.push(p);
                i
            });
            let mut run = selection.run.clone();
            run.id = next_id;
            next_id = next_id
                .checked_add(1)
                .ok_or_else(|| fail("run id overflow"))?;
            out.problems[pi].run_ids.push(run.id);
            out.solvers[si].runs.push(run);
        }
        out.validate()?;
        Ok(out)
    }
}
#[derive(Clone)]
pub struct RecordSelection {
    pub solver: Arc<RecordSolver>,
    pub problem: Arc<RecordProblem>,
    pub run: RecordSingleRun,
}
