//! XML benchmark declarations, explicit runtime/file adapters and native journals.
use crate::*;
use std::{
    any::Any,
    collections::BTreeMap,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
pub const BENCHMARK_XML_NAMESPACE: &str = "https://timefold.ai/xsd/benchmark";
#[derive(Debug)]
pub enum BenchmarkXmlError {
    Xml(SolverXmlError),
    Config(BenchmarkConfigError),
    Signature(XmlFactoryError),
    Invalid(String),
    Io(io::Error),
    NullProblem(usize),
    WrongProblem(usize),
    NoProblems,
    Unregistered(XmlFactorySignature),
    UnregisteredLoader(String),
}
impl fmt::Display for BenchmarkXmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xml(e) => e.fmt(f),
            Self::Config(e) => e.fmt(f),
            Self::Signature(e) => e.fmt(f),
            Self::Invalid(e) => f.write_str(e),
            Self::Io(e) => e.fmt(f),
            Self::NullProblem(i) => write!(f, "problem {i} is null"),
            Self::WrongProblem(i) => write!(f, "problem {i} is not the registered solution type"),
            Self::NoProblems => f.write_str("no benchmark problems"),
            Self::Unregistered(k) => write!(f, "unregistered benchmark solver {k:?}"),
            Self::UnregisteredLoader(k) => write!(f, "unregistered benchmark file loader {k}"),
        }
    }
}
impl Error for BenchmarkXmlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Xml(e) => Some(e),
            Self::Config(e) => Some(e),
            Self::Signature(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
fn invalid(origin: &str, node: &str, reason: &str) -> BenchmarkXmlError {
    BenchmarkXmlError::Xml(SolverXmlError {
        origin: origin.into(),
        cause: SolverXmlCause::Schema(SolverXmlSchemaError {
            node: node.into(),
            reason: reason.into(),
        }),
    })
}
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn solver_fragment(node: roxmltree::Node<'_, '_>) -> String {
    fn write(n: roxmltree::Node<'_, '_>, out: &mut String) {
        if n.is_element() {
            out.push('<');
            out.push_str(n.tag_name().name());
            for a in n.attributes() {
                out.push(' ');
                out.push_str(a.name());
                out.push_str("=\"");
                out.push_str(&escaped(a.value()));
                out.push('"');
            }
            out.push('>');
            for c in n.children() {
                write(c, out);
            }
            out.push_str("</");
            out.push_str(n.tag_name().name());
            out.push('>');
        } else if n.is_text() {
            out.push_str(&escaped(n.text().unwrap_or_default()));
        }
    }
    let mut out = String::new();
    write(node, &mut out);
    out
}
fn value<'a, 'b>(n: roxmltree::Node<'a, 'b>, name: &str) -> Option<&'a str> {
    n.children()
        .find(|c| c.has_tag_name(name))
        .and_then(|n| n.text())
        .map(str::trim)
}
#[derive(Clone, Debug)]
pub struct XmlBenchmarkSolver {
    pub name: Option<String>,
    pub sub_runs: Option<i64>,
    pub solver: SolverXmlConfiguration,
}
#[derive(Clone, Debug)]
pub struct BenchmarkXmlConfiguration {
    pub directory: PathBuf,
    pub solvers: Vec<XmlBenchmarkSolver>,
    pub inherited: Option<SolverXmlConfiguration>,
    pub inputs: Vec<(String, PathBuf)>,
    pub parallel: Option<String>,
    pub warmup_seconds: Option<i64>,
}
impl BenchmarkXmlConfiguration {
    pub fn from_solver(solver: SolverXmlConfiguration, directory: Option<PathBuf>) -> Self {
        Self {
            directory: directory.unwrap_or_else(|| PathBuf::from("target/benchmark")),
            solvers: vec![XmlBenchmarkSolver {
                name: None,
                sub_runs: None,
                solver,
            }],
            inherited: None,
            inputs: vec![],
            parallel: None,
            warmup_seconds: None,
        }
    }
    pub fn parse(text: &str, origin: &str) -> Result<Self, BenchmarkXmlError> {
        if text.len() > SolverXmlLimits::default().bytes {
            return Err(invalid(origin, "plannerBenchmark", "byte limit exceeded"));
        }
        let doc = roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions {
                allow_dtd: false,
                nodes_limit: 100_000,
                ..Default::default()
            },
        )
        .map_err(|e| {
            BenchmarkXmlError::Xml(SolverXmlError {
                origin: origin.into(),
                cause: SolverXmlCause::Syntax(e),
            })
        })?;
        let root = doc.root_element();
        for node in doc.descendants().filter(|n| n.is_element()) {
            if node.ancestors().take(130).count() > 128 {
                return Err(invalid(
                    origin,
                    node.tag_name().name(),
                    "depth limit exceeded",
                ));
            }
            if node
                .tag_name()
                .namespace()
                .is_some_and(|n| n != BENCHMARK_XML_NAMESPACE && n != SOLVER_XML_NAMESPACE)
            {
                return Err(invalid(
                    origin,
                    node.tag_name().name(),
                    "foreign nested namespace",
                ));
            }
        }

        if !root.has_tag_name("plannerBenchmark")
            || root
                .tag_name()
                .namespace()
                .is_some_and(|n| n != BENCHMARK_XML_NAMESPACE)
        {
            return Err(invalid(
                origin,
                root.tag_name().name(),
                "expected plannerBenchmark namespace/root",
            ));
        }
        for c in root.children().filter(|c| c.is_element()) {
            if !matches!(
                c.tag_name().name(),
                "benchmarkDirectory"
                    | "warmUpSecondsSpentLimit"
                    | "parallelBenchmarkCount"
                    | "inheritedSolverBenchmark"
                    | "solverBenchmark"
            ) {
                return Err(invalid(
                    origin,
                    c.tag_name().name(),
                    "unknown benchmark element",
                ));
            }
        }
        let parse_solver = |n: roxmltree::Node<'_, '_>| {
            SolverXmlConfiguration::parse_with_limits(
                &solver_fragment(n),
                origin,
                SolverXmlLimits::default(),
            )
            .map_err(BenchmarkXmlError::Xml)
        };
        let inherited = root
            .children()
            .find(|n| n.has_tag_name("inheritedSolverBenchmark"))
            .and_then(|n| n.children().find(|c| c.has_tag_name("solver")))
            .map(parse_solver)
            .transpose()?;
        let mut solvers = vec![];
        let mut inputs = vec![];
        for entry in root
            .children()
            .filter(|n| n.has_tag_name("solverBenchmark"))
        {
            for child in entry.children().filter(|n| n.is_element()) {
                if !matches!(
                    child.tag_name().name(),
                    "name" | "subSingleCount" | "solver" | "problemBenchmarks"
                ) {
                    return Err(invalid(
                        origin,
                        child.tag_name().name(),
                        "unknown solver benchmark element",
                    ));
                }
            }
            let solver = entry
                .children()
                .find(|n| n.has_tag_name("solver"))
                .map(parse_solver)
                .transpose()?
                .unwrap_or_default();
            let sub_runs = value(entry, "subSingleCount")
                .map(|s| {
                    s.parse::<i64>()
                        .map_err(|_| invalid(origin, "subSingleCount", "expected integer"))
                })
                .transpose()?;
            solvers.push(XmlBenchmarkSolver {
                name: value(entry, "name").map(str::to_owned),
                sub_runs,
                solver,
            });
            if let Some(problem) = entry
                .children()
                .find(|n| n.has_tag_name("problemBenchmarks"))
            {
                let loader = value(problem, "solutionFileIOClass").unwrap_or_default();
                for child in problem.children().filter(|n| n.is_element()) {
                    if !matches!(
                        child.tag_name().name(),
                        "solutionFileIOClass" | "inputSolutionFile"
                    ) {
                        return Err(invalid(
                            origin,
                            child.tag_name().name(),
                            "unknown problem benchmark element",
                        ));
                    }
                    if child.has_tag_name("inputSolutionFile") {
                        let item = (
                            loader.to_owned(),
                            PathBuf::from(child.text().unwrap_or_default().trim()),
                        );
                        if !inputs.contains(&item) {
                            inputs.push(item);
                        }
                    }
                }
            }
        }
        let warmup_seconds = value(root, "warmUpSecondsSpentLimit")
            .map(|s| {
                s.parse::<i64>()
                    .map_err(|_| invalid(origin, "warmUpSecondsSpentLimit", "expected integer"))
            })
            .transpose()?;
        Ok(Self {
            directory: value(root, "benchmarkDirectory")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("target/benchmark")),
            solvers,
            inherited,
            inputs,
            parallel: value(root, "parallelBenchmarkCount").map(str::to_owned),
            warmup_seconds,
        })
    }
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, BenchmarkXmlError> {
        let path = path.as_ref();
        let origin = path.display().to_string();
        let text = read_text(path).map_err(|e| {
            BenchmarkXmlError::Xml(SolverXmlError {
                origin: origin.clone(),
                cause: SolverXmlCause::Io(e),
            })
        })?;
        Self::parse(&text, &origin)
    }
    pub fn from_resource(
        name: &str,
        mut resolve: impl FnMut(&str) -> io::Result<String>,
    ) -> Result<Self, BenchmarkXmlError> {
        let text = resolve(name).map_err(|e| {
            BenchmarkXmlError::Xml(SolverXmlError {
                origin: name.into(),
                cause: SolverXmlCause::Io(e),
            })
        })?;
        Self::parse(&text, name)
    }
    pub fn from_template_resource(
        name: &str,
        mut resolve: impl FnMut(&str) -> io::Result<String>,
    ) -> Result<Self, BenchmarkXmlError> {
        let text = resolve(name).map_err(|e| {
            BenchmarkXmlError::Xml(SolverXmlError {
                origin: name.into(),
                cause: SolverXmlCause::Io(e),
            })
        })?;
        Self::parse(&render_benchmark_template(&text)?, name)
    }
    pub fn from_template_file(path: impl AsRef<Path>) -> Result<Self, BenchmarkXmlError> {
        let path = path.as_ref();
        let text = read_text(path).map_err(BenchmarkXmlError::Io)?;
        Self::parse(
            &render_benchmark_template(&text)?,
            &path.display().to_string(),
        )
    }
    fn effective_solvers(&self) -> Vec<SolverXmlConfiguration> {
        self.solvers
            .iter()
            .map(|s| {
                self.inherited
                    .as_ref()
                    .map_or_else(|| s.solver.clone(), |base| s.solver.clone().inherit(base))
            })
            .collect()
    }
    pub fn prepare(&self, cpus: i64) -> Result<PreparedBenchmarkSuite, BenchmarkXmlError> {
        let effective = self.effective_solvers();
        let mut declarations = vec![];
        for (entry, solver) in self.solvers.iter().zip(effective.iter()) {
            let phases = solver
                .phase_names()
                .iter()
                .map(|p| match *p {
                    "constructionHeuristic" => Ok(SolverPhaseKind::Construction),
                    "localSearch" => Ok(SolverPhaseKind::LocalSearch),
                    "customPhase" => Ok(SolverPhaseKind::Custom),
                    "exhaustiveSearch" => Ok(SolverPhaseKind::Exact),
                    _ => Err(BenchmarkXmlError::Invalid(
                        "partitioned runtime requires an explicit native binding".into(),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?;
            declarations.push(SolverBenchmarkDeclaration {
                name: entry.name.clone(),
                sub_runs: entry.sub_runs,
                solver: Some(BenchmarkSolverSettings {
                    environment: EnvironmentProfile::new(
                        solver
                            .environment_mode()
                            .map_err(BenchmarkXmlError::Invalid)?,
                        vec![],
                    ),
                    phases,
                }),
                ..Default::default()
            });
        }
        BenchmarkSuiteDeclaration {
            solvers: declarations,
            parallel_count: self.parallel.clone(),
            warmup: BenchmarkWarmupLimits {
                seconds: self.warmup_seconds,
                ..Default::default()
            },
            ..Default::default()
        }
        .prepare(cpus, 30_000)
        .map_err(BenchmarkXmlError::Config)
    }
}
fn read_text(path: &Path) -> io::Result<String> {
    use std::io::Read;
    let mut text = String::new();
    fs::File::open(path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)?;
    Ok(text)
}
/// Bounded native literal-list templates: <#list ['a','b'] as key> ... ${key}
/// ... </#list>. No arbitrary expressions, includes, code execution or complete
/// FreeMarker language parity. Unsupported syntax fails rather than disappearing.
pub fn render_benchmark_template(template: &str) -> Result<String, BenchmarkXmlError> {
    let fail = |s: &str| BenchmarkXmlError::Invalid(format!("benchmark template: {s}"));
    if template.len() > 4 * 1024 * 1024 {
        return Err(fail("byte limit exceeded"));
    }
    let mut rest = template;
    let mut out = String::new();
    let mut expansions = 0;
    while let Some(start) = rest.find("<#list") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 6..];
        let end = tail
            .find('>')
            .ok_or_else(|| fail("missing directive end"))?;
        let (list, key) = tail[..end]
            .trim()
            .split_once(" as ")
            .ok_or_else(|| fail("expected literal list and key"))?;
        let list = list
            .trim()
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(|| fail("expected bracketed literal list"))?;
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(fail("invalid key"));
        }
        let body_tail = &tail[end + 1..];
        let close = body_tail
            .find("</#list>")
            .ok_or_else(|| fail("missing list close"))?;
        let body = &body_tail[..close];
        if body.contains("<#") {
            return Err(fail("nested/directive expressions unsupported"));
        }
        for literal in list.split(',').filter(|v| !v.trim().is_empty()) {
            let literal = literal.trim();
            let quote = literal
                .chars()
                .next()
                .ok_or_else(|| fail("empty literal"))?;
            if !matches!(quote, '\'' | '"') || !literal.ends_with(quote) || literal.len() < 2 {
                return Err(fail("expected quoted literal"));
            }
            let value = &literal[1..literal.len() - 1];
            expansions += 1;
            if expansions > 1024 {
                return Err(fail("expansion limit exceeded"));
            }
            let rendered = body.replace(&format!("${{{key}}}"), &escaped(value));
            if rendered.contains("${") {
                return Err(fail("unknown substitution"));
            }
            if out.len().saturating_add(rendered.len()) > 4 * 1024 * 1024 {
                return Err(fail("expanded byte limit exceeded"));
            }
            out.push_str(&rendered);
        }
        rest = &body_tail[close + 8..];
    }
    out.push_str(rest);
    if out.contains("<#")
        || out.contains("</#")
        || out.contains("${")
        || out.len() > 4 * 1024 * 1024
    {
        return Err(fail("unsupported remaining syntax or byte limit"));
    }
    Ok(out)
}
pub struct NativeBenchmarkTrial {
    pub seed: u64,
    pub warmup: bool,
    pub warmup_budget: Option<Duration>,
}
type Runner<P, S, E> = dyn Fn(&SolverXmlConfiguration, P, NativeBenchmarkTrial) -> Result<BenchmarkObservation<S>, E>
    + Send
    + Sync;
type Loader<P, E> = dyn Fn(&Path) -> Result<P, E> + Send + Sync;
pub struct XmlBenchmarkRuntime<P, S, E> {
    runners: BTreeMap<XmlFactorySignature, Box<Runner<P, S, E>>>,
    loaders: BTreeMap<String, Box<Loader<P, E>>>,
}
impl<P, S, E> Default for XmlBenchmarkRuntime<P, S, E> {
    fn default() -> Self {
        Self {
            runners: BTreeMap::new(),
            loaders: BTreeMap::new(),
        }
    }
}
impl<P, S, E> XmlBenchmarkRuntime<P, S, E> {
    pub fn register_solver(
        &mut self,
        key: XmlFactorySignature,
        run: impl Fn(
            &SolverXmlConfiguration,
            P,
            NativeBenchmarkTrial,
        ) -> Result<BenchmarkObservation<S>, E>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), BenchmarkXmlError> {
        if self.runners.contains_key(&key) {
            return Err(BenchmarkXmlError::Invalid(
                "duplicate solver binding".into(),
            ));
        }
        self.runners.insert(key, Box::new(run));
        Ok(())
    }
    pub fn register_loader(
        &mut self,
        key: String,
        load: impl Fn(&Path) -> Result<P, E> + Send + Sync + 'static,
    ) -> Result<(), BenchmarkXmlError> {
        if self.loaders.contains_key(&key) {
            return Err(BenchmarkXmlError::Invalid(
                "duplicate file loader binding".into(),
            ));
        }
        self.loaders.insert(key, Box::new(load));
        Ok(())
    }
}
pub struct XmlBenchmarkRun<'a, P, S, E> {
    plan: PreparedBenchmarkSuite,
    configs: Vec<SolverXmlConfiguration>,
    problems: Vec<P>,
    directory: PathBuf,
    runtime: &'a XmlBenchmarkRuntime<P, S, E>,
}
impl<'a, P: Clone + Send + Sync + 'static, S: Send, E: Send> XmlBenchmarkRuntime<P, S, E> {
    pub fn build(
        &'a self,
        config: &BenchmarkXmlConfiguration,
        problems: &[P],
    ) -> Result<XmlBenchmarkRun<'a, P, S, E>, BenchmarkXmlError> {
        let plan = config.prepare(1)?;
        let configs = config.effective_solvers();
        for c in &configs {
            let key =
                XmlFactorySignature::from_configuration(c).map_err(BenchmarkXmlError::Signature)?;
            if !self.runners.contains_key(&key) {
                return Err(BenchmarkXmlError::Unregistered(key));
            }
        }
        if problems.is_empty() {
            return Err(BenchmarkXmlError::NoProblems);
        }
        Ok(XmlBenchmarkRun {
            plan,
            configs,
            problems: problems.to_vec(),
            directory: config.directory.clone(),
            runtime: self,
        })
    }
    pub fn build_untyped(
        &'a self,
        config: &BenchmarkXmlConfiguration,
        problems: &[Option<&dyn Any>],
    ) -> Result<XmlBenchmarkRun<'a, P, S, E>, BenchmarkXmlError> {
        let typed = problems
            .iter()
            .enumerate()
            .map(|(i, p)| {
                p.ok_or(BenchmarkXmlError::NullProblem(i))?
                    .downcast_ref::<P>()
                    .cloned()
                    .ok_or(BenchmarkXmlError::WrongProblem(i))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.build(config, &typed)
    }
    pub fn load_and_build(
        &'a self,
        config: &BenchmarkXmlConfiguration,
    ) -> Result<XmlBenchmarkRun<'a, P, S, E>, BenchmarkFileError<E>> {
        config.prepare(1).map_err(BenchmarkFileError::Build)?;
        for c in config.effective_solvers() {
            let key = XmlFactorySignature::from_configuration(&c)
                .map_err(|e| BenchmarkFileError::Build(BenchmarkXmlError::Signature(e)))?;
            if !self.runners.contains_key(&key) {
                return Err(BenchmarkFileError::Build(BenchmarkXmlError::Unregistered(
                    key,
                )));
            }
        }
        // Resolve all file adapters before invoking any, so missing registrations
        // cannot leave a partially loaded externally observable problem list.
        for (key, _) in &config.inputs {
            if !self.loaders.contains_key(key) {
                return Err(BenchmarkFileError::Build(
                    BenchmarkXmlError::UnregisteredLoader(key.clone()),
                ));
            }
        }
        let problems = config
            .inputs
            .iter()
            .map(|(key, path)| self.loaders[key](path).map_err(BenchmarkFileError::Read))
            .collect::<Result<Vec<_>, _>>()?;
        self.build(config, &problems)
            .map_err(BenchmarkFileError::Build)
    }
}
#[derive(Debug)]
pub enum BenchmarkFileError<E> {
    Build(BenchmarkXmlError),
    Read(E),
    Run(BenchmarkExecutionError<E>),
    Io(io::Error),
}
impl<E: fmt::Display> fmt::Display for BenchmarkFileError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Build(e) => e.fmt(f),
            Self::Read(e) => write!(f, "benchmark input: {e}"),
            Self::Run(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for BenchmarkFileError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Build(e) => e,
            Self::Read(e) => e,
            Self::Run(e) => e,
            Self::Io(e) => e,
        })
    }
}
static RUN_ID: AtomicU64 = AtomicU64::new(0);
impl<P: Clone + Send + Sync, S: Send + fmt::Debug, E: Send + fmt::Display>
    XmlBenchmarkRun<'_, P, S, E>
{
    pub fn solver_count(&self) -> usize {
        self.plan.solvers().len()
    }
    pub fn problem_count(&self) -> usize {
        self.problems.len()
    }
    pub fn benchmark(self) -> Result<PathBuf, BenchmarkFileError<E>> {
        fs::create_dir_all(&self.directory).map_err(BenchmarkFileError::Io)?;
        let output = loop {
            let p = self.directory.join(format!(
                "timetuck-{}-{}",
                std::process::id(),
                RUN_ID.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&p) {
                Ok(()) => break p,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(BenchmarkFileError::Io(e)),
            }
        };
        let warmups = usize::from(!self.plan.warmup_budget().is_zero());
        let mut session = BenchmarkSession::new(Some(self.plan), Some(output.clone()));
        let result = session
            .run(&self.problems, warmups, 0, None, |_, p, t, budget| {
                let config = &self.configs[t.configuration];
                let key = XmlFactorySignature::from_configuration(config)
                    .expect("validated native signature");
                self.runtime.runners[&key](
                    config,
                    p,
                    NativeBenchmarkTrial {
                        seed: t.seed,
                        warmup: t.warmup,
                        warmup_budget: budget,
                    },
                )
            })
            .map_err(BenchmarkFileError::Run)?;
        let rows=result.trials.iter().map(|r|serde_json::json!({"configuration":r.trial.configuration,"problem":r.trial.problem,"repetition":r.trial.repetition,"warmup":r.trial.warmup,"result":match &r.result{Ok(o)=>serde_json::json!({"score":format!("{:?}",o.score),"initialized":o.initialized,"evaluations":o.work.evaluations}),Err(e)=>serde_json::json!({"error":e.to_string()})}})).collect::<Vec<_>>();
        let report =
            serde_json::json!({"schema_version":1,"complete":result.complete,"trials":rows});
        fs::write(
            output.join("summary.json"),
            serde_json::to_vec_pretty(&report).expect("native report contains only JSON values"),
        )
        .map_err(BenchmarkFileError::Io)?;
        Ok(output)
    }
}
