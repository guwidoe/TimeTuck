// Adapted BenchmarkAggregatorTest TEST-only setup, run helpers and constraint provider.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use timetuck::*;
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct ArchiveFixture {
    pub temp: PathBuf,
    pub root: PathBuf,
    pub names: Vec<String>,
    pub xml: PathBuf,
}
impl Drop for ArchiveFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp);
    }
}
pub const SOLVER: &str = r#"<solver><solutionClass>ai.timefold.solver.core.testdomain.TestdataSolution</solutionClass><entityClass>ai.timefold.solver.core.testdomain.TestdataEntity</entityClass><scoreDirectorFactory><constraintProviderClass>ai.timefold.solver.core.testdomain.TestdataConstraintProvider</constraintProviderClass></scoreDirectorFactory><termination><millisecondsSpentLimit>10</millisecondsSpentLimit></termination></solver>"#;
impl ArchiveFixture {
    pub fn setup() -> Self {
        let temp = loop {
            let p = std::env::temp_dir().join(format!(
                "timetuck-archive-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => break p,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("{e}"),
            }
        };
        let root = temp.join("benchmarks");
        std::fs::create_dir(&root).unwrap();
        let input = temp.join("input");
        std::fs::create_dir(&input).unwrap();
        let input = input.join("testSolution.xml");
        std::fs::write(&input, "<TestdataSolution/>").unwrap();
        let xml = temp.join("testdataBenchmarkConfig.xml");
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><plannerBenchmark><benchmarkDirectory>{}</benchmarkDirectory><warmUpSecondsSpentLimit>0</warmUpSecondsSpentLimit><benchmarkReport><locale>en_US</locale></benchmarkReport><solverBenchmark><problemBenchmarks><solutionFileIOClass>ai.timefold.solver.benchmark.util.RigidTestdataSolutionFileIO</solutionFileIOClass><inputSolutionFile>{}</inputSolutionFile></problemBenchmarks>{SOLVER}</solverBenchmark></plannerBenchmark>"#,
            root.display(),
            input.display()
        );
        std::fs::write(&xml, content).unwrap();
        let archive = BenchmarkArchive::at_directory(&root);
        let mut names = vec![];
        let runtime = runtime();
        for i in 0..3 {
            let mut c = BenchmarkXmlConfiguration::from_solver(
                SolverXmlConfiguration::parse(SOLVER).unwrap(),
                Some(temp.join("raw")),
            );
            c.warmup_seconds = Some(0);
            c.solvers[0].name = Some(format!("TestSolver-{i}"));
            let problem =
                FiniteProblem::new((0..3).map(|_| Variable::new(vec![1, 2])).collect()).unwrap();
            let raw = runtime.build(&c, &[problem]).unwrap().benchmark().unwrap();
            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(raw.join("summary.json")).unwrap()).unwrap();
            assert_eq!(report["complete"], true);
            assert_eq!(report["trials"].as_array().unwrap().len(), 1);
            assert_eq!(
                report["trials"][0]["result"]["score"],
                format!("{:?}", SimpleScore::new(-3))
            );
            assert_eq!(report["trials"][0]["result"]["initialized"], true);
            let archived = archive
                .archive_completed_report(&raw, &format!("2026-10-12_12000{i}"))
                .unwrap();
            names.push(archived.file_name().unwrap().to_str().unwrap().to_owned());
        }
        assert_eq!(archive.available_archived_runs().unwrap(), names);
        assert_eq!(names.len(), 3);
        Self {
            temp,
            root,
            names,
            xml,
        }
    }
    pub fn archive(&self) -> BenchmarkArchive {
        BenchmarkArchive::at_directory(&self.root)
    }
    pub fn path_archive(&self, name: &str) -> BenchmarkArchive {
        BenchmarkArchive::at_directory(self.temp.join(name))
    }
    pub fn empty(&self, name: &str) -> BenchmarkArchive {
        std::fs::create_dir(self.temp.join(name)).unwrap();
        self.path_archive(name)
    }
    pub fn file(&self, name: &str) -> BenchmarkArchive {
        std::fs::write(self.temp.join(name), "").unwrap();
        self.path_archive(name)
    }
}
pub fn runtime() -> XmlBenchmarkRuntime<FiniteProblem<i32>, SimpleScore, String> {
    let mut runtime = XmlBenchmarkRuntime::default();
    let config = SolverXmlConfiguration::parse(SOLVER).unwrap();
    runtime
        .register_solver(
            XmlFactorySignature::from_configuration(&config).unwrap(),
            |config, p: FiniteProblem<i32>, trial| {
                assert!(!trial.warmup);
                assert_eq!(trial.seed, 0);
                let millis = config
                    .root()
                    .child("termination")
                    .unwrap()
                    .value("millisecondsSpentLimit")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap();
                assert_eq!(millis, 10);
                let mut network = ConstraintNetwork::new(p.variables().len());
                network
                    .add_constraint(
                        "Always penalize",
                        (0..p.variables().len()).collect(),
                        |values| {
                            SimpleScore::new(
                                -(values.iter().filter(|v| v.is_some()).count() as i64),
                            )
                        },
                    )
                    .unwrap();
                let mut settings = SolverConfig::default();
                settings.limits.max_time = Some(Duration::from_millis(millis));
                let result = solve(&p, &network, settings).map_err(|e| e.to_string())?;
                Ok(BenchmarkObservation {
                    score: result.score.unwrap(),
                    initialized: result.assignment.values().iter().all(Option::is_some),
                    work: result.work,
                })
            },
        )
        .unwrap();
    runtime
        .register_loader(
            "ai.timefold.solver.benchmark.util.RigidTestdataSolutionFileIO".into(),
            |p: &Path| {
                std::fs::File::open(p).map_err(|e| e.to_string())?;
                Ok(
                    FiniteProblem::new((0..3).map(|_| Variable::new(vec![1, 2])).collect())
                        .unwrap(),
                )
            },
        )
        .unwrap();
    runtime
}
pub fn html(path: &Path) {
    assert!(path.exists());
    assert!(
        path.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".html")
    );
}
