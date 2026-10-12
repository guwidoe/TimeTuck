// Complete adapted Community benchmark lifecycle/statistic/report TESTS.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{error::Error, fmt, path::PathBuf, sync::Arc};
use timetuck::*;
fn plan() -> PreparedBenchmarkSuite {
    BenchmarkSuiteDeclaration {
        solvers: vec![SolverBenchmarkDeclaration {
            solver: Some(BenchmarkSolverSettings {
                environment: EnvironmentProfile::default(),
                phases: vec![SolverPhaseKind::Construction, SolverPhaseKind::LocalSearch],
            }),
            ..Default::default()
        }],
        ..Default::default()
    }
    .prepare(1, 30_000)
    .unwrap()
}
#[test]
fn benchmarking_started_twice() {
    let mut b = BenchmarkSession::new(Some(plan()), Some(PathBuf::from("benchmark")));
    b.start().unwrap();
    let error = b.start().unwrap_err();
    assert_eq!(error, BenchmarkStartError::AlreadyStarted);
    assert!(error.source().is_none());
}
#[test]
fn solver_benchmark_result_list_is_empty() {
    let mut b = BenchmarkSession::new(None, Some(PathBuf::from("benchmark")));
    let e = b.start().unwrap_err();
    assert_eq!(e, BenchmarkStartError::NoConfigurations);
    assert!(e.to_string().contains("solverBenchmarkResultList"));
    assert!(e.to_string().contains("empty"));
}
#[test]
fn benchmark_directory_is_null() {
    let mut b = BenchmarkSession::new(Some(plan()), None);
    let e = b.start().unwrap_err();
    assert_eq!(e, BenchmarkStartError::NoDirectory);
    assert!(e.to_string().contains("benchmarkDirectory"));
    assert!(e.to_string().contains("null"));
}
#[derive(Debug)]
struct UnsupportedAccess;
impl fmt::Display for UnsupportedAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unsupported benchmark entity access")
    }
}
impl Error for UnsupportedAccess {}
#[derive(Clone, Debug)]
struct Failure(Arc<UnsupportedAccess>);
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.0)
    }
}
#[derive(Clone)]
struct MockSolution {
    failure: Failure,
}
impl MockSolution {
    fn entity_list(&self) -> Result<Vec<i32>, Failure> {
        Err(self.failure.clone())
    }
}
#[test]
fn exception_message_propagates_when_thrown_during_warm_up() {
    let exception = Arc::new(UnsupportedAccess);
    let solution = MockSolution {
        failure: Failure(exception.clone()),
    };
    let mut b = BenchmarkSession::new(Some(plan()), Some(PathBuf::from("benchmark")));
    let error = b
        .run(&[solution], 1, 0, None, |_, p, trial, _| {
            assert!(trial.warmup);
            let _entities = p.entity_list()?;
            Ok::<_, Failure>(BenchmarkObservation {
                score: SimpleScore::new(0),
                initialized: true,
                work: SolverWork::default(),
            })
        })
        .unwrap_err();
    let mut cause: &dyn Error = &error;
    while let Some(next) = cause.source() {
        cause = next;
    }
    let root = cause.downcast_ref::<UnsupportedAccess>().unwrap();
    assert!(std::ptr::eq(root, &*exception));
}
fn deviation(scores: &[i64], average: i64) -> Vec<f64> {
    benchmark_score_deviations(
        &scores
            .iter()
            .map(|&v| Some(SimpleScore::new(v)))
            .collect::<Vec<_>>(),
        &SimpleScore::new(average),
        scores.len(),
    )
    .unwrap()
}
#[test]
fn single_determine_standard_deviation_doubles() {
    let d = deviation(&[0], 0);
    assert_eq!(d.len(), 1);
    assert!(d[0].abs() <= 0.001);
}
#[test]
fn multiple_determine_standard_deviation_doubles() {
    let d = deviation(&[-2, -4], -3);
    assert_eq!(d.len(), 1);
    assert!((d[0] - 1.0).abs() <= 0.001);
}
#[test]
fn large_determine_standard_deviation_doubles() {
    let scores = [
        -19289560268_i64,
        -19345935795,
        -19715516752,
        -19589259253,
        -19390707618,
        -19641410518,
    ];
    let average = scores
        .iter()
        .fold(SimpleScore::new(0), |a, &v| a + SimpleScore::new(v))
        .divide(scores.len() as f64);
    let d = deviation(&scores, average.score());
    assert_eq!(d.len(), 1);
    assert!((d[0] - 160338212.294).abs() <= 0.001);
}
#[test]
fn get_standard_deviation_string() {
    assert_eq!(format_benchmark_deviations(None).unwrap(), None);
    for (value, expected) in [
        (2.0, "2.0"),
        (2_f64.sqrt(), "1.41"),
        (160338212.294, "1.6E8"),
        (2_000_000_000.0, "2.0E9"),
        (20_000_000_000.0, "2.0E10"),
    ] {
        assert_eq!(
            format_benchmark_deviations(Some(&[value]))
                .unwrap()
                .as_deref(),
            Some(expected)
        );
    }
}
struct TotalScoreComparator;
struct TotalRankWeightFactory;
#[test]
fn inherit_benchmark_report_config() {
    let inherited = BenchmarkReportOptions {
        locale: Some("en-CA".into()),
        ranking: Some(BenchmarkReportRanking::TotalRanking),
        comparator: Some(PlanningType::of::<TotalScoreComparator>()),
        weight_factory: Some(PlanningType::of::<TotalRankWeightFactory>()),
    };
    let report = inherited.clone();
    assert_eq!(report.locale, inherited.locale);
    assert_eq!(report.ranking, inherited.ranking);
    assert_eq!(report.comparator, inherited.comparator);
    assert_eq!(report.weight_factory, inherited.weight_factory);
}
#[test]
fn without_solver_benchmark_blue_print_type() {
    let e = BenchmarkBlueprintDeclaration::default()
        .validate()
        .unwrap_err();
    assert!(e.to_string().contains("solverBenchmarkBluePrintType"));
}
