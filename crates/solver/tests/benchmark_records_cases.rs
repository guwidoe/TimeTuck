// Adapted from pinned Timefold Community benchmark TEST contracts (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
#[path = "fixtures/benchmark_records_fixture.rs"]
mod fixture;
use fixture::selected_records;
use timetuck::*;
fn compact(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}
fn semantic_xml(s: &str) -> Vec<(String, String)> {
    let d = roxmltree::Document::parse(s).unwrap();
    d.descendants()
        .filter(|n| n.is_element())
        .map(|n| {
            let attrs = n
                .attributes()
                .map(|a| format!("{}={}", a.name(), a.value()))
                .collect::<Vec<_>>()
                .join(",");
            (
                format!(
                    "{}:{}:{}",
                    n.ancestors().filter(|n| n.is_element()).count(),
                    n.tag_name().name(),
                    attrs
                ),
                n.children()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<String>()
                    .trim()
                    .to_owned(),
            )
        })
        .collect()
}
#[test]
fn xml_config_file_remains_same_after_read_write() {
    for source in [
        include_str!("fixtures/benchmark_records/testBenchmarkConfigWithoutNamespace.xml"),
        include_str!("fixtures/benchmark_records/testBenchmarkConfigWithNamespace.xml"),
    ] {
        let config = BenchmarkRecordConfig::parse(source).unwrap();
        assert_eq!(
            semantic_xml(&config.to_xml()),
            semantic_xml(&source.replace(" xmlns=\"https://timefold.ai/xsd/benchmark\"", ""))
        );
        assert_eq!(config.solvers.len(), 2);
        assert_eq!(config.locale.as_deref(), Some("cs-CZ"));
        assert_eq!(config.ranking, Some(RecordRanking::TotalScore));
        assert_eq!(
            config
                .inherited
                .as_ref()
                .unwrap()
                .problem
                .as_ref()
                .unwrap()
                .statistics
                .len(),
            6
        );
        assert_eq!(
            config
                .runtime_configuration()
                .prepare(4)
                .unwrap()
                .solvers()
                .len(),
            2
        );
    }
}
#[test]
fn read_and_validate_invalid_benchmark_config_fails_indicating_the_issue() {
    let source = "<plannerBenchmark xmlns=\"https://timefold.ai/xsd/benchmark\"><benchmarkDirectory>data</benchmarkDirectory><parallelBenchmarkCount>AUTO</parallelBenchmarkCount><solverBenchmark><name>Entity Tabu Search</name><solver><solutionKlazz>ai.timefold.solver.core.testdomain.TestdataSolution</solutionKlazz><entityClass>ai.timefold.solver.core.testdomain.TestdataEntity</entityClass></solver><problemBenchmarks><solutionFileIOClass>TestdataSolutionFileIO</solutionFileIOClass><inputSolutionFile>nonExistingDataset1.xml</inputSolutionFile></problemBenchmarks></solverBenchmark></plannerBenchmark>";
    let error = BenchmarkRecordConfig::parse(source).unwrap_err();
    assert!(error.to_string().contains("solutionKlazz"));
    assert!(
        matches!(error,BenchmarkRecordError::SolverXml(SolverXmlError {cause:SolverXmlCause::Schema(SolverXmlSchemaError {node,..}),..}) if node=="solutionKlazz")
    );
}
#[test]
fn assign_custom_solution_io() {
    // Source RigidTestdataSolutionFileIO input is a real fixed3entity/2value model.
    let configured = BenchmarkRecordFileIo::new(
        "txt",
        |_| Ok(FiniteProblem::new(vec![Variable::new(vec!["v1", "v2"]); 3]).unwrap()),
        |_, _| Ok(()),
    );
    assert_eq!(configured.extension, "txt");
    assert_eq!(
        configured
            .read(std::path::Path::new("unused.txt"))
            .unwrap()
            .variables()
            .len(),
        3
    );
}
type SummaryComparator =
    dyn Fn(&BenchmarkSummary, &BenchmarkSummary) -> std::cmp::Ordering + Send + Sync;
fn comparator() -> Arc<SummaryComparator> {
    Arc::new(|a, b| a.compare(b, BenchmarkRanking::TotalScore))
}
fn weight() -> Arc<dyn Fn(&BenchmarkStanding) -> i128 + Send + Sync> {
    Arc::new(|s| -(s.rank as i128))
}
#[test]
fn build_with_solver_ranking_type_and_solver_ranking_comparator_class() {
    let p = BenchmarkRecordReportPolicy {
        ranking: Some(RecordRanking::TotalRanking),
        comparator: Some(comparator()),
        ..Default::default()
    };
    let e = p
        .rank(&BenchmarkRecordSet::default())
        .unwrap_err()
        .to_string();
    assert!(e.contains("solverRankingType"));
    assert!(e.contains("solverRankingComparatorClass"));
}
#[test]
fn build_with_solver_ranking_type_and_solver_ranking_weight_factory_class() {
    let p = BenchmarkRecordReportPolicy {
        ranking: Some(RecordRanking::TotalRanking),
        weight: Some(weight()),
        ..Default::default()
    };
    let e = p
        .rank(&BenchmarkRecordSet::default())
        .unwrap_err()
        .to_string();
    assert!(e.contains("solverRankingType"));
    assert!(e.contains("solverRankingWeightFactoryClass"));
}
#[test]
fn build_with_solver_ranking_comparator_class_and_solver_ranking_weight_factory_class() {
    let p = BenchmarkRecordReportPolicy {
        comparator: Some(comparator()),
        weight: Some(weight()),
        ..Default::default()
    };
    let e = p
        .rank(&BenchmarkRecordSet::default())
        .unwrap_err()
        .to_string();
    assert!(e.contains("solverRankingComparatorClass"));
    assert!(e.contains("solverRankingWeightFactoryClass"));
}
#[test]
fn create_merged_result() {
    let selected = selected_records();
    let merged = BenchmarkRecordSet::merge(&selected).unwrap();
    assert!(merged.aggregation);
    assert_eq!(merged.solvers.len(), 3);
    assert_eq!(
        merged
            .solvers
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Solver X", "Solver Y", "Solver Z"]
    );
    assert_eq!(merged.problems.len(), 2);
    assert_eq!(
        merged
            .problems
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["problemA", "problemB"]
    );
}
#[test]
fn xml_report_remains_same_after_read_write() {
    let source = include_str!("fixtures/benchmark_records/testPlannerBenchmarkResult.xml");
    let records = BenchmarkRecordSet::parse(source).unwrap();
    assert_eq!(compact(&records.to_xml().unwrap()), compact(source));
}
#[test]
fn should_deserialize_xml() {
    let source = include_str!("fixtures/benchmark_records/dummyPlannerBenchmarkResult.xml");
    let records = BenchmarkRecordSet::parse(source).unwrap();
    assert_eq!(records.problems.len(), 1);
    let mut actual = records.problems[0]
        .statistics
        .iter()
        .map(|s| s.kind.label())
        .collect::<Vec<_>>();
    let mut expected = vec![
        "BEST_SCORE",
        "STEP_SCORE",
        "MEMORY_USE",
        "BEST_SOLUTION_MUTATION",
        "MOVE_COUNT_PER_STEP",
        "MOVE_COUNT_PER_TYPE",
        "SCORE_CALCULATION_SPEED",
        "MOVE_EVALUATION_SPEED",
    ];
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
#[test]
fn should_serialize_back_to_identical_xml() {
    let source = include_str!("fixtures/benchmark_records/dummyPlannerBenchmarkResult.xml");
    let records = BenchmarkRecordSet::parse(source).unwrap();
    assert_eq!(compact(&records.to_xml().unwrap()), compact(source));
}
