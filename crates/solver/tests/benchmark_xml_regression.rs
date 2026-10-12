//! Original benchmark XML execution, journals, preflight, templates and warm-ups.
#[allow(dead_code)]
#[path = "fixtures/benchmark_xml_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
#[test]
fn repeated_runs_write_distinct_complete_journals_with_all_trials() {
    let t = Temp::new();
    let mut c = BenchmarkXmlConfiguration::parse(
        TEMPLATE
            .replace(
                "<#list ['FIRST_FIT', 'CHEAPEST_INSERTION'] as constructionHeuristicType>",
                "",
            )
            .replace("</#list>", "")
            .replace("${constructionHeuristicType}", "FIRST_FIT")
            .as_str(),
        "original",
    )
    .unwrap();
    c.directory = t.0.join("output");
    c.solvers[0].sub_runs = Some(3);
    c.solvers.push(c.solvers[0].clone());
    let r = runtime();
    let a = r
        .build(&c, &[problem(), problem()])
        .unwrap()
        .benchmark()
        .unwrap();
    let b = r
        .build(&c, &[problem(), problem()])
        .unwrap()
        .benchmark()
        .unwrap();
    assert_ne!(a, b);
    for p in [&a, &b] {
        let j: serde_json::Value =
            serde_json::from_slice(&std::fs::read(p.join("summary.json")).unwrap()).unwrap();
        assert_eq!(j["complete"], true);
        assert_eq!(j["trials"].as_array().unwrap().len(), 12);
    }
}
#[test]
fn uninitialized_native_custom_results_remain_visible_in_the_written_report() {
    let t = Temp::new();
    let custom=SOLVER.replace("</solver>","<customPhase><customPhaseCommandClass>NoChange</customPhaseCommandClass></customPhase></solver>");
    let mut c = BenchmarkXmlConfiguration::from_solver(
        SolverXmlConfiguration::parse(&custom).unwrap(),
        Some(t.0.join("output")),
    );
    c.warmup_seconds = Some(0);
    let r = runtime();
    let p = r.build(&c, &[problem()]).unwrap().benchmark().unwrap();
    let j: serde_json::Value =
        serde_json::from_slice(&std::fs::read(p.join("summary.json")).unwrap()).unwrap();
    assert_eq!(j["trials"][0]["result"]["initialized"], false);
    assert!(j["trials"][0]["result"]["evaluations"].as_u64().unwrap() > 0);
}
#[test]
fn templates_expand_data_with_escaping_and_reject_unhandled_language() {
    let s = render_benchmark_template(
        "<solver><#list ['A&B','C'] as key><solutionClass>${key}</solutionClass></#list></solver>",
    )
    .unwrap();
    assert!(s.contains("A&amp;B"));
    assert_eq!(s.matches("solutionClass>").count(), 4);
    for bad in [
        "<#include 'x'>",
        "<#list variable as key>x</#list>",
        "<#list ['a'] as key>${unknown}</#list>",
        "<#list ['a'] as key><#list ['b'] as nested>x</#list></#list>",
    ] {
        assert!(render_benchmark_template(bad).is_err());
    }
    let c =
        BenchmarkXmlConfiguration::parse(&render_benchmark_template(TEMPLATE).unwrap(), "template")
            .unwrap();
    assert_eq!(c.solvers.len(), 2);
    assert_eq!(
        c.solvers
            .iter()
            .map(|s| s
                .solver
                .root()
                .child("constructionHeuristic")
                .unwrap()
                .value("constructionHeuristicType")
                .unwrap())
            .collect::<Vec<_>>(),
        vec!["FIRST_FIT", "CHEAPEST_INSERTION"]
    );
}
#[test]
fn invalid_solver_and_loader_bindings_fail_before_any_input_callback() {
    let mut c = BenchmarkXmlConfiguration::parse(SIMPLE, "fixture").unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let mut r = Runtime::default();
    r.register_loader(c.inputs[0].0.clone(), {
        let count = count.clone();
        move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(problem())
        }
    })
    .unwrap();
    assert!(matches!(
        r.load_and_build(&c),
        Err(BenchmarkFileError::Build(BenchmarkXmlError::Unregistered(
            _
        )))
    ));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    c.solvers[0].sub_runs = Some(0);
    assert!(r.load_and_build(&c).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(
        BenchmarkXmlConfiguration::parse(
            "<plannerBenchmark><solverBenchmark xmlns='foreign'/></plannerBenchmark>",
            "bad"
        )
        .is_err()
    );
}
#[test]
fn nonzero_warmup_finishes_before_measurements_and_is_recorded() {
    let t = Temp::new();
    let mut c = BenchmarkXmlConfiguration::from_solver(
        SolverXmlConfiguration::parse(SOLVER).unwrap(),
        Some(t.0.join("output")),
    );
    c.warmup_seconds = Some(1);
    c.solvers.push(c.solvers[0].clone());
    let warmed = Arc::new(AtomicUsize::new(0));
    let measured = Arc::new(AtomicUsize::new(0));
    let mut r = Runtime::default();
    r.register_solver(
        XmlFactorySignature::from_configuration(&c.solvers[0].solver).unwrap(),
        {
            let warmed = warmed.clone();
            let measured = measured.clone();
            move |_, _, trial| {
                if trial.warmup {
                    assert!(trial.warmup_budget.is_some());
                    warmed.fetch_add(1, Ordering::SeqCst);
                } else {
                    assert_eq!(warmed.load(Ordering::SeqCst), 2);
                    measured.fetch_add(1, Ordering::SeqCst);
                }
                Ok(BenchmarkObservation {
                    score: SimpleScore::new(0),
                    initialized: true,
                    work: SolverWork::default(),
                })
            }
        },
    )
    .unwrap();
    let out = r.build(&c, &[problem()]).unwrap().benchmark().unwrap();
    assert_eq!(measured.load(Ordering::SeqCst), 2);
    let j: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("summary.json")).unwrap()).unwrap();
    assert_eq!(j["trials"].as_array().unwrap().len(), 4);
}
