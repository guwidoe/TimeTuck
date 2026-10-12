// Adapted TEST-only TestdataEasyScoreCalculator and resource fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use timetuck::*;
pub const SIMPLE: &str = include_str!("solver_xml/testdataSolverConfig.xml");
pub const DIVERTED: &str = include_str!("solver_xml/classloaderTestdataSolverConfig.xml");
pub const INVALID: &str = include_str!("solver_xml/invalidSolverConfig.xml");
pub const NAMESPACE: &str = include_str!("solver_xml/testSolverConfigWithNamespace.xml");
pub const COMPLETE: &str = include_str!("solver_xml/testSolverConfigWithoutNamespace.xml");
pub const ENUMS: &str = include_str!("solver_xml/testSolverConfigWithEnumSet.xml");
pub const RESOURCE: &str = "ai/timefold/solver/core/api/solver/testdataSolverConfig.xml";
pub type Factory = SolverFactory<FiniteProblem<i32>, SimpleScore, String>;
pub type Registry = XmlSolverFactoryRegistry<FiniteProblem<i32>, SimpleScore, String, SimpleScore>;
pub fn registry() -> Registry {
    let mut r = Registry::default();
    let key =
        XmlFactorySignature::from_configuration(&SolverXmlConfiguration::parse(SIMPLE).unwrap())
            .unwrap();
    r.register(key, |_| {
        SolverFactory::new(
            SolverConfig::<SimpleScore>::default(),
            |p: FiniteProblem<i32>, config, progress| {
                let count = p.variables().len();
                let mut n = ConstraintNetwork::new(count);
                n.add_constraint(
                    "ordered equal value pairs",
                    (0..count).collect(),
                    |values| {
                        let mut score = 0;
                        for (i, left) in values.iter().enumerate() {
                            if let Some(left) = left {
                                for (j, right) in values.iter().enumerate() {
                                    if i != j && right == &Some(*left) {
                                        score -= 1;
                                    }
                                }
                            }
                        }
                        SimpleScore::new(score)
                    },
                )
                .unwrap();
                let result = solve(&p, &n, config).map_err(|e| e.to_string())?;
                progress.record_work(result.work);
                Ok(result.score.unwrap())
            },
        )
    })
    .unwrap();
    r
}
pub fn resource(name: &str) -> io::Result<String> {
    match name {
        RESOURCE => Ok(SIMPLE.into()),
        "divertThroughClassLoader/ai/timefold/solver/core/api/solver/classloaderTestdataSolverConfig.xml" => {
            Ok(DIVERTED.into())
        }
        "ai/timefold/solver/core/api/solver/invalidSolverConfig.xml" => Ok(INVALID.into()),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "resource is absent",
        )),
    }
}
pub fn actual_solver(factory: Factory) {
    let mut solver = factory.build_manager(1).unwrap();
    assert!(!solver.factory().configuration().phases.is_empty());
    let p = FiniteProblem::new((0..3).map(|_| Variable::new(vec![1, 2, 3])).collect()).unwrap();
    let result = solver.job_builder(1).with_problem(p).run().unwrap().wait();
    assert_eq!(result.as_ref().as_ref().unwrap(), &SimpleScore::new(0));
    solver.shutdown().unwrap();
}
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new() -> Self {
        loop {
            let p = std::env::temp_dir().join(format!(
                "timetuck-xml-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("{e}"),
            }
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Default for Temp {
    fn default() -> Self {
        Self::new()
    }
}
