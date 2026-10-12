// Complete adapted pinned Community PlannerBenchmarkFactoryTest methods.
// Apache-2.0; resources and TEST helpers recorded in compat/provenance.json.
#[path = "fixtures/benchmark_xml_fixture.rs"]
mod fixture;
use fixture::*;
use std::{any::Any, error::Error};
use timetuck::*;
const BASE: &str = "ai/timefold/solver/benchmark/api/";
fn resource_config(name: &str) -> BenchmarkXmlConfiguration {
    BenchmarkXmlConfiguration::from_resource(&format!("{BASE}{name}"), resource).unwrap()
}
fn file_config(name: &str, template: bool) {
    let t = Temp::new();
    let path = t.0.join(name);
    std::fs::write(&path, resource(&format!("{BASE}{name}")).unwrap()).unwrap();
    execute(if template {
        BenchmarkXmlConfiguration::from_template_file(path).unwrap()
    } else {
        BenchmarkXmlConfiguration::from_file(path).unwrap()
    });
}
fn reason(e: &dyn Error) -> String {
    let mut out = e.to_string();
    let mut next = e.source();
    while let Some(e) = next {
        out.push_str(&e.to_string());
        next = e.source();
    }
    out
}
#[test]
fn create_from_solver_config_xml_resource() {
    let solver = SolverXmlConfiguration::from_resource(
        "ai/timefold/solver/core/config/solver/testdataSolverConfig.xml",
        resource,
    )
    .unwrap();
    let t = Temp::new();
    let r = runtime();
    for directory in [None, Some(t.0.join("output"))] {
        let c = BenchmarkXmlConfiguration::from_solver(solver.clone(), directory);
        assert_eq!(r.build(&c, &[problem()]).unwrap().problem_count(), 1);
    }
}
#[test]
fn create_from_solver_config_xml_resource_class_loader() {
    let solver=SolverXmlConfiguration::from_resource("divertThroughClassLoader/ai/timefold/solver/core/api/solver/classloaderTestdataSolverConfig.xml",resource).unwrap();
    let t = Temp::new();
    let r = runtime();
    for directory in [None, Some(t.0.join("output"))] {
        assert_eq!(
            r.build(
                &BenchmarkXmlConfiguration::from_solver(solver.clone(), directory),
                &[problem()]
            )
            .unwrap()
            .problem_count(),
            1
        );
    }
}
#[test]
fn problem_is_not_a_solution_instance() {
    let c = BenchmarkXmlConfiguration::from_solver(inherited(), None);
    let r = runtime();
    let bad = "This is not a solution instance.";
    assert!(matches!(
        r.build_untyped(&c, &[Some(&bad as &dyn Any)]),
        Err(BenchmarkXmlError::WrongProblem(0))
    ));
}
#[test]
fn problem_is_null() {
    let c = BenchmarkXmlConfiguration::from_solver(inherited(), None);
    let r = runtime();
    let p = problem();
    assert!(matches!(
        r.build_untyped(&c, &[Some(&p as &dyn Any), None]),
        Err(BenchmarkXmlError::NullProblem(1))
    ));
}
#[test]
fn create_from_xml_resource() {
    execute(resource_config("testdataBenchmarkConfig.xml"));
}
#[test]
fn create_from_xml_resource_class_loader() {
    execute(BenchmarkXmlConfiguration::from_resource("divertThroughClassLoader/ai/timefold/solver/benchmark/api/classloaderTestdataBenchmarkConfig.xml",resource).unwrap());
}
#[test]
fn create_from_xml_resource_non_existing() {
    let name = format!("{BASE}nonExistingBenchmarkConfig.xml");
    assert!(
        BenchmarkXmlConfiguration::from_resource(&name, resource)
            .unwrap_err()
            .to_string()
            .contains(&name)
    );
}
#[test]
fn create_from_invalid_xml_resource_fails_showing_both_resource_and_reason() {
    let name = format!("{BASE}invalidBenchmarkConfig.xml");
    let e = BenchmarkXmlConfiguration::from_resource(&name, resource).unwrap_err();
    assert!(e.to_string().contains(&name));
    assert!(reason(&e).contains("invalidElementThatShouldNotBeHere"));
}
#[test]
fn create_from_invalid_xml_file_fails_showing_both_path_and_reason() {
    let t = Temp::new();
    let p = t.0.join("invalidBenchmarkConfig.xml");
    std::fs::write(&p, INVALID).unwrap();
    let e = BenchmarkXmlConfiguration::from_file(&p).unwrap_err();
    assert!(e.to_string().contains(&p.display().to_string()));
    assert!(reason(&e).contains("invalidElementThatShouldNotBeHere"));
}
#[test]
fn create_from_xml_resource_uninitialized_best_solution() {
    let mut c = resource_config("testdataBenchmarkConfig.xml");
    let solver=SOLVER.replace("</solver>","<customPhase><customPhaseCommandClass>ai.timefold.solver.core.testutil.NoChangeCustomPhaseCommand</customPhaseCommandClass></customPhase></solver>");
    c.solvers[0].solver = SolverXmlConfiguration::parse(&solver).unwrap();
    execute(c);
}
#[test]
fn create_from_xml_resource_sub_single_count() {
    let mut c = resource_config("testdataBenchmarkConfig.xml");
    c.solvers[0].sub_runs = Some(3);
    execute(c);
}
#[test]
fn create_from_xml_file() {
    file_config("testdataBenchmarkConfig.xml", false);
}
#[test]
fn create_from_xml_file_class_loader() {
    file_config("classloaderTestdataBenchmarkConfig.xml", false);
}
#[test]
fn create_from_freemarker_xml_resource() {
    execute(
        BenchmarkXmlConfiguration::from_template_resource(
            &format!("{BASE}testdataBenchmarkConfigTemplate.xml.ftl"),
            resource,
        )
        .unwrap(),
    );
}
#[test]
fn create_from_freemarker_xml_resource_class_loader() {
    execute(BenchmarkXmlConfiguration::from_template_resource("divertThroughClassLoader/ai/timefold/solver/benchmark/api/classloaderTestdataBenchmarkConfigTemplate.xml.ftl",resource).unwrap());
}
#[test]
fn create_from_freemarker_xml_resource_non_existing() {
    assert!(
        BenchmarkXmlConfiguration::from_template_resource(
            &format!("{BASE}nonExistingBenchmarkConfigTemplate.xml.ftl"),
            resource
        )
        .is_err()
    );
}
#[test]
fn create_from_freemarker_xml_file() {
    file_config("testdataBenchmarkConfigTemplate.xml.ftl", true);
}
#[test]
fn create_from_freemarker_xml_file_class_loader() {
    file_config("classloaderTestdataBenchmarkConfigTemplate.xml.ftl", true);
}
#[test]
fn create_from_solver_config() {
    let c = inherited();
    let t = Temp::new();
    let r = runtime();
    for directory in [None, Some(t.0.join("output"))] {
        let run = r
            .build(
                &BenchmarkXmlConfiguration::from_solver(c.clone(), directory),
                &[problem()],
            )
            .unwrap();
        assert_eq!(run.problem_count(), 1);
    }
}
#[test]
fn build_planner_benchmark() {
    let c = BenchmarkXmlConfiguration {
        directory: "target/benchmark".into(),
        solvers: (0..3)
            .map(|_| XmlBenchmarkSolver {
                name: None,
                sub_runs: None,
                solver: SolverXmlConfiguration::default(),
            })
            .collect(),
        inherited: Some(inherited()),
        inputs: vec![],
        parallel: None,
        warmup_seconds: None,
    };
    let r = runtime();
    let run = r.build(&c, &[problem(), problem()]).unwrap();
    assert_eq!(run.solver_count(), 3);
    assert_eq!(run.problem_count(), 2);
}
