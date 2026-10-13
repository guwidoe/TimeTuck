// Original native controls, no additional source method credit.

use timetuck::*;
fn temp(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "timetuck-runtime-original-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn disk_and_xml_escape_validate_and_keep_failure_state() {
    let dir = temp("escaping");
    let kind = RecordStatistic::PickedMoveTypeBestScoreDiff;
    let points = vec![BenchmarkStatisticPoint::MoveScore {
        time: -5,
        name: "a, \"quoted\" & <name>\nline".into(),
        score: SimpleScore::new(-19),
    }];
    let mut stat = BenchmarkStatistic::new(kind, points.clone()).unwrap();
    let xml = stat.to_xml().unwrap();
    assert_eq!(
        BenchmarkStatistic::from_xml(&xml).unwrap().points(),
        Some(points.as_slice())
    );
    assert!(BenchmarkStatistic::from_xml(&xml.replace("<field>", "<unknown>")).is_err());
    assert!(stat.hibernate(dir.join("missing/file")).is_err());
    assert_eq!(stat.points(), Some(points.as_slice()));
    let path = dir.join("points.csv");
    stat.hibernate(&path).unwrap();
    assert_eq!(
        BenchmarkStatistic::read_csv(kind, &path).unwrap().points(),
        Some(points.as_slice())
    );
    std::fs::write(&path, "wrong header\n").unwrap();
    assert!(BenchmarkStatistic::read_csv(kind, path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn chart_files_contain_reduced_actual_series_and_exact_bars() {
    let dir = temp("chart");
    let kind = RecordStatistic::ScoreCalculationSpeed;
    let points = (0..100)
        .map(|time| BenchmarkStatisticPoint::Long {
            time,
            value: if time == 50 { 1000 } else { time },
        })
        .collect();
    let good = BenchmarkStatisticRun {
        label: "actual".into(),
        successful: true,
        statistic: BenchmarkStatistic::new(kind, points).unwrap(),
    };
    let mut failed = good.clone();
    failed.label = "failed".into();
    failed.successful = false;
    let charts = benchmark_statistic_charts("control", kind, &[good, failed]).unwrap();
    charts[0].write_data(dir.join("line.json"), 10).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("line.json")).unwrap()).unwrap();
    let series = data["data"]["series"].as_array().unwrap();
    assert_eq!(series.len(), 1);
    let points = series[0]["points"].as_array().unwrap();
    assert_eq!(points.len(), 10);
    assert_eq!(points[0][0], 0.0);
    assert_eq!(points[9][0], 99.0);
    assert!(points.iter().any(|p| p[1] == 1000.0));
    assert!(
        benchmark_statistic_charts("empty", kind, &[])
            .unwrap()
            .is_empty()
    );
    let bars = benchmark_statistic_charts(
        "control",
        RecordStatistic::MoveCountPerType,
        &[BenchmarkStatisticRun {
            label: "actual".into(),
            successful: true,
            statistic: BenchmarkStatistic::new(
                RecordStatistic::MoveCountPerType,
                vec![BenchmarkStatisticPoint::MoveType {
                    name: "change".into(),
                    count: i64::MAX,
                }],
            )
            .unwrap(),
        }],
    )
    .unwrap();
    bars[0].write_data(dir.join("bar.json"), 0).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("bar.json")).unwrap()).unwrap();
    assert_eq!(data["data"]["bars"][0][2].as_i64(), Some(i64::MAX));
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn aggregation_counts_actual_failures_and_refuses_broken_links() {
    let mut records =
        BenchmarkRecordSet::parse(include_str!("fixtures/benchmark_runtime/aggregator.xml"))
            .unwrap();
    let before = BenchmarkRecordAggregation::from_records(&records).unwrap();
    records.solvers[0].runs[0].sub_runs[0].succeeded = Some(false);
    let after = BenchmarkRecordAggregation::from_records(&records).unwrap();
    assert_eq!(after.failure_count, before.failure_count + 1);
    records.problems[0].run_ids.push(u64::MAX);
    assert!(BenchmarkRecordAggregation::from_records(&records).is_err());
}

#[test]
fn live_solver_records_only_local_search_scores_and_real_improvements() {
    let dir = temp("live-scores");
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    let without_ls = SolverConfig {
        local_search: vec![],
        ..Default::default()
    };
    let initial =
        benchmark_solve_with_best_csv(&problem, &network, without_ls, dir.join("construction.csv"))
            .unwrap();
    assert_eq!(initial.score, Some(SimpleScore::new(0)));
    assert!(
        BenchmarkStatistic::read_csv(RecordStatistic::BestScore, dir.join("construction.csv"))
            .unwrap()
            .points()
            .unwrap()
            .is_empty()
    );
    let config = SolverConfig {
        local_search: vec![LocalSearchConfig {
            max_steps: 2,
            ..Default::default()
        }],
        ..Default::default()
    };
    let result =
        benchmark_solve_with_best_csv(&problem, &network, config, dir.join("local.csv")).unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(1)));
    assert_eq!(result.work.accepted_moves, 1);
    let stat =
        BenchmarkStatistic::read_csv(RecordStatistic::BestScore, dir.join("local.csv")).unwrap();
    let scores: Vec<_> = stat
        .points()
        .unwrap()
        .iter()
        .map(|p| {
            if let BenchmarkStatisticPoint::Score {
                score, initialized, ..
            } = p
            {
                assert!(*initialized);
                *score
            } else {
                panic!("score point")
            }
        })
        .collect();
    assert_eq!(scores, [SimpleScore::new(0), SimpleScore::new(1)]);
    std::fs::remove_dir_all(dir).unwrap();
}
