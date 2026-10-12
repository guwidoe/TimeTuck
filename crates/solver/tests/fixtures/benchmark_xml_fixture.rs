// Adapted Community TEST resources, rigid solution loader, score and no-change command.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{io, path::PathBuf, time::Duration};
use timetuck::*;
#[allow(dead_code)]
#[path = "xml_factory_fixture.rs"]
mod xml;
pub use xml::Temp;
pub const SIMPLE: &str = include_str!("benchmark_xml/testdataBenchmarkConfig.xml");
pub const DIVERTED: &str = include_str!("benchmark_xml/classloaderTestdataBenchmarkConfig.xml");
pub const INVALID: &str = include_str!("benchmark_xml/invalidBenchmarkConfig.xml");
pub const TEMPLATE: &str = include_str!("benchmark_xml/testdataBenchmarkConfigTemplate.xml.ftl");
pub const TEMPLATE_DIVERTED: &str =
    include_str!("benchmark_xml/classloaderTestdataBenchmarkConfigTemplate.xml.ftl");
pub const SOLVER: &str = include_str!("benchmark_xml/solverConfig.xml");
pub type Runtime = XmlBenchmarkRuntime<FiniteProblem<i32>, SimpleScore, String>;
pub fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new((0..3).map(|_| Variable::new(vec![1, 2])).collect()).unwrap()
}
pub fn inherited() -> SolverXmlConfiguration {
    SolverXmlConfiguration::parse(&SOLVER.replace("easyScoreCalculatorClass","constraintProviderClass").replace("ai.timefold.solver.core.testdomain.TestdataEasyScoreCalculator","ai.timefold.solver.benchmark.api.PlannerBenchmarkFactoryTest$TestdataConstraintProvider")).unwrap()
}
pub fn runtime() -> Runtime {
    let mut r = Runtime::default();
    for config in [
        SolverXmlConfiguration::parse(SOLVER).unwrap(),
        inherited(),
        BenchmarkXmlConfiguration::parse(DIVERTED, "fixture")
            .unwrap()
            .solvers
            .remove(0)
            .solver,
    ] {
        r.register_solver(
            XmlFactorySignature::from_configuration(&config).unwrap(),
            |config, p, t| {
                let count = p.variables().len();
                let mut n = ConstraintNetwork::new(count);
                if config
                    .root()
                    .child("scoreDirectorFactory")
                    .unwrap()
                    .value("easyScoreCalculatorClass")
                    .is_some()
                {
                    n.add_constraint("ordered equal value pairs", (0..count).collect(), |v| {
                        let mut score = 0;
                        for (i, l) in v.iter().enumerate() {
                            if let Some(l) = l {
                                for (j, r) in v.iter().enumerate() {
                                    if i != j && r == &Some(*l) {
                                        score -= 1;
                                    }
                                }
                            }
                        }
                        SimpleScore::new(score)
                    })
                    .unwrap();
                }
                if config.phase_names().contains(&"customPhase") {
                    let result = custom_phase(
                        &p,
                        &n,
                        p.initial_assignment(),
                        CustomPhaseConfig::default(),
                        |_| Ok(()),
                    )
                    .map_err(|e| e.to_string())?;
                    return Ok(BenchmarkObservation {
                        score: result.score.unwrap_or(SimpleScore::new(0)),
                        initialized: result.assignment.values().iter().all(Option::is_some),
                        work: SolverWork {
                            evaluations: result.work.evaluations,
                            ..Default::default()
                        },
                    });
                }
                let mut settings = SolverConfig::default();
                settings.limits.max_time = if t.warmup {
                    t.warmup_budget
                } else {
                    config
                        .root()
                        .child("termination")
                        .and_then(|e| e.value("secondsSpentLimit"))
                        .map(|s| Duration::from_secs(s.parse().unwrap()))
                };
                if let Some(ch) = config
                    .root()
                    .child("constructionHeuristic")
                    .and_then(|e| e.value("constructionHeuristicType"))
                {
                    settings.construction.pick_early = match ch {
                        "FIRST_FIT" => ConstructionPickEarly::FirstNonDeterioratingScore,
                        "CHEAPEST_INSERTION" => ConstructionPickEarly::Never,
                        _ => return Err(format!("unregistered native construction policy {ch}")),
                    };
                }
                let result = solve(&p, &n, settings).map_err(|e| e.to_string())?;
                Ok(BenchmarkObservation {
                    score: result.score.unwrap_or(SimpleScore::new(0)),
                    initialized: result.assignment.values().iter().all(Option::is_some),
                    work: result.work,
                })
            },
        )
        .unwrap();
    }
    r.register_loader(
        "ai.timefold.solver.benchmark.util.RigidTestdataSolutionFileIO".into(),
        |path| {
            std::fs::File::open(path).map_err(|e| e.to_string())?;
            Ok(problem())
        },
    )
    .unwrap();
    r
}
pub fn resource(name: &str) -> io::Result<String> {
    let name = name
        .strip_prefix("divertThroughClassLoader/")
        .unwrap_or(name);
    match name {
        "ai/timefold/solver/core/config/solver/testdataSolverConfig.xml" => Ok(SOLVER.into()),
        "ai/timefold/solver/core/api/solver/classloaderTestdataSolverConfig.xml" => {
            Ok(xml::DIVERTED.into())
        }
        "ai/timefold/solver/benchmark/api/testdataBenchmarkConfig.xml" => Ok(SIMPLE.into()),
        "ai/timefold/solver/benchmark/api/classloaderTestdataBenchmarkConfig.xml" => {
            Ok(DIVERTED.into())
        }
        "ai/timefold/solver/benchmark/api/invalidBenchmarkConfig.xml" => Ok(INVALID.into()),
        "ai/timefold/solver/benchmark/api/testdataBenchmarkConfigTemplate.xml.ftl" => {
            Ok(TEMPLATE.into())
        }
        "ai/timefold/solver/benchmark/api/classloaderTestdataBenchmarkConfigTemplate.xml.ftl" => {
            Ok(TEMPLATE_DIVERTED.into())
        }
        _ => Err(io::Error::new(io::ErrorKind::NotFound, "resource missing")),
    }
}
pub fn prepare_inputs(c: &BenchmarkXmlConfiguration) {
    for (_, p) in &c.inputs {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(p)
            .unwrap();
    }
}
pub fn execute(mut config: BenchmarkXmlConfiguration) -> PathBuf {
    let t = Temp::new();
    config.directory = t.0.join("output");
    prepare_inputs(&config);
    let runtime = runtime();
    let path = runtime
        .load_and_build(&config)
        .unwrap()
        .benchmark()
        .unwrap();
    assert!(path.is_dir());
    assert!(path.join("summary.json").is_file());
    path
}
