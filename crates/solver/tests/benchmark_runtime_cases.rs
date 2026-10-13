// Adapted pinned Community TEST contracts; Apache-2.0, see provenance.
#[path = "fixtures/benchmark_runtime_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn hibernation() {
    let dir = temp("hibernate");
    for (kind, points) in input_points() {
        let path = dir.join(format!("{}.csv", kind.label()));
        let mut statistic = BenchmarkStatistic::new(kind, points).unwrap();
        statistic.hibernate(&path).unwrap();
        assert!(statistic.points().is_none());
        let fresh = BenchmarkStatistic::read_csv(kind, path).unwrap();
        assert_source_points(kind, fresh.points().unwrap());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn serialization() {
    let dir = temp("serialization");
    for (kind, points) in input_points() {
        let statistic = BenchmarkStatistic::new(kind, points).unwrap();
        let path = dir.join(format!("{}.xml", kind.label()));
        std::fs::write(&path, statistic.to_xml().unwrap()).unwrap();
        let deserialized =
            BenchmarkStatistic::from_xml(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(deserialized.kind, statistic.kind);
        assert_source_points(kind, deserialized.points().unwrap());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
fn speed_chart(kind: RecordStatistic, title: &str, y: &str) {
    let runs = vec![BenchmarkStatisticRun {
        label: "label".into(),
        successful: true,
        statistic: BenchmarkStatistic::new(
            kind,
            vec![
                BenchmarkStatisticPoint::Long {
                    time: i64::MIN,
                    value: i64::MIN,
                },
                BenchmarkStatisticPoint::Long {
                    time: i64::MAX,
                    value: i64::MAX,
                },
            ],
        )
        .unwrap(),
    }];
    let charts = benchmark_statistic_charts("Problem_0", kind, &runs).unwrap();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].title, title);
    assert_eq!(charts[0].x_label, "Time spent");
    assert_eq!(charts[0].y_label, y);
    if let BenchmarkChartGeometry::Lines(lines) = &charts[0].geometry {
        assert_eq!(lines[0].0, "label");
        assert_eq!(lines[0].1.points().len(), 2);
    } else {
        panic!("line geometry");
    }
}
#[test]
fn score_calculation_speed_generate_charts() {
    speed_chart(
        RecordStatistic::ScoreCalculationSpeed,
        "Problem_0 score calculation speed statistic",
        "Score calculation speed per second",
    );
}
#[test]
fn move_evaluation_speed_generate_charts() {
    speed_chart(
        RecordStatistic::MoveEvaluationSpeed,
        "Problem_0 move evaluation speed statistic",
        "Move evaluation speed per second",
    );
}
#[test]
fn move_count_per_type_generate_charts() {
    let kind = RecordStatistic::MoveCountPerType;
    let points = input_points()
        .into_iter()
        .find(|(k, _)| *k == kind)
        .unwrap()
        .1;
    let charts = benchmark_statistic_charts(
        "Problem_0",
        kind,
        &[BenchmarkStatisticRun {
            label: "label".into(),
            successful: true,
            statistic: BenchmarkStatistic::new(kind, points).unwrap(),
        }],
    )
    .unwrap();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].title, "Problem_0 move count per type statistic");
    assert_eq!(charts[0].x_label, "Type");
    assert_eq!(charts[0].y_label, "Count");
}
#[test]
fn xml_read_benchmark_result_aggregated() {
    let dir = temp("aggregated");
    let records =
        BenchmarkRecordSet::parse(include_str!("fixtures/benchmark_runtime/aggregator.xml"))
            .unwrap();
    let report = BenchmarkRecordAggregation::from_records(&records).unwrap();
    report.write_summary(dir.join("summary.json")).unwrap();
    assert_eq!(report.solver_summaries.len(), 6);
    assert_eq!(report.problem_count, 2);
    assert_eq!(report.failure_count, 0);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn run_planner_benchmark() {
    let dir = temp("run");
    let problem = FiniteProblem::new(vec![Variable::new(vec!["v1", "v2"]); 3]).unwrap();
    let mut network = ConstraintNetwork::new(3);
    for entity in 0..3 {
        network
            .add_constraint(format!("Always penalize {entity}"), vec![entity], |v| {
                SimpleScore::new(if v[0].is_some() { -1 } else { 0 })
            })
            .unwrap();
    }
    let schedule = BenchmarkSchedule {
        warmup_runs: 1,
        measured_runs: std::num::NonZeroUsize::new(1).unwrap(),
        seed: 37,
    };
    let config = SolverConfig {
        limits: SolverLimits {
            max_unimproved_time: Some(std::time::Duration::from_millis(100)),
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        }],
        ..Default::default()
    };
    let report = schedule
        .run(&[config], &[problem], None, |config, p, trial| {
            let path = if trial.warmup {
                dir.join("warmup/BEST_SCORE.csv")
            } else {
                dir.join("Problem_0/Config_0/sub0/BEST_SCORE.csv")
            };
            let result = benchmark_solve_with_best_csv(&p, &network, config.clone(), path)?;
            Ok::<_, std::io::Error>(BenchmarkObservation {
                score: result.score.unwrap(),
                initialized: result.assignment.values().iter().all(Option::is_some),
                work: result.work,
            })
        })
        .unwrap();
    assert!(report.is_successful());
    let path = dir.join("Problem_0/Config_0/sub0/BEST_SCORE.csv");
    assert!(path.exists());
    let input = std::fs::read_to_string(path).unwrap();
    let lines: Vec<_> = input.lines().collect();
    assert!(lines.len() > 1);
    assert_eq!(lines[0], "\"timeMillisSpent\",\"score\",\"initialized\"");
    assert!(lines.last().unwrap().ends_with("\"-3\",\"true\""));
    std::fs::remove_dir_all(dir).unwrap();
}
