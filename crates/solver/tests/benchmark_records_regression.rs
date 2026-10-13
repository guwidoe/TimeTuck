// Original native controls; no additional upstream method credit.
#[path = "fixtures/benchmark_records_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
#[test]
fn merged_records_preserve_actual_scores_seeds_links_and_isolate_mutation() {
    let selected = selected_records();
    let mut merged = BenchmarkRecordSet::merge(&selected).unwrap();
    assert_eq!(merged.solvers[0].runs[0].sub_runs.len(), 2);
    assert_eq!(selected[0].run.sub_runs.len(), 2);
    for (i, s) in merged.solvers.iter().enumerate() {
        assert_eq!(
            s.config.root().value("randomSeed"),
            Some(i.to_string().as_str())
        );
    }
    let observations = merged.observations().unwrap();
    assert_eq!(observations.len(), 5);
    // Historical unknown initialization is conservatively incomplete. Fewer
    // incomplete observations precede total scores in the native policy.
    for (index, (incomplete, total)) in [(2, -21), (2, -4300), (1, -50000)].into_iter().enumerate()
    {
        let summary = BenchmarkSummary::new(
            observations
                .iter()
                .filter(|o| o.configuration == index)
                .map(|o| o.outcome.clone()),
        );
        assert_eq!(summary.incomplete_count(), incomplete);
        assert_eq!(summary.failure_count(), 0);
        assert_eq!(
            summary.total_score().unwrap().levels(),
            &[Decimal::from(total)]
        );
    }
    assert_eq!(
        BenchmarkRecordReportPolicy::default()
            .rank(&merged)
            .unwrap(),
        [2, 0, 1]
    );
    let serialized = merged.to_xml().unwrap();
    let roundtrip = BenchmarkRecordSet::parse(&serialized).unwrap();
    assert_eq!(
        BenchmarkRecordReportPolicy::default()
            .rank(&roundtrip)
            .unwrap(),
        [2, 0, 1]
    );
    merged.solvers[0].runs[0].summary_score = Some(RecordScore::Simple(SimpleScore::new(-999999)));
    merged.solvers[0].runs[0].sub_runs.clear();
    assert_eq!(selected[0].run.sub_runs.len(), 2);
    assert_eq!(
        selected[0].run.summary_score,
        Some(RecordScore::Simple(SimpleScore::new(-1)))
    );
    assert_ne!(
        BenchmarkRecordReportPolicy::default()
            .rank(&merged)
            .unwrap(),
        [2, 0, 1]
    );
}
#[test]
fn typed_xml_mutation_changes_reports_and_rejects_broken_references_unknowns() {
    let source = include_str!("fixtures/benchmark_records/dummyPlannerBenchmarkResult.xml");
    let mut records = BenchmarkRecordSet::parse(source).unwrap();
    assert!(
        records
            .observations()
            .unwrap_err()
            .to_string()
            .contains("unscored")
    );
    records.solvers[0].runs[0].sub_runs[0].score = Some(RecordScore::Simple(SimpleScore::new(7)));
    records.solvers[0].runs[0].sub_runs[0].initialized = Some(true);
    records.problems[0].statistics[0].title = Some("New & useful title".into());
    records.solvers[0].name = "Renamed".into();
    let xml = records.to_xml().unwrap();
    assert!(xml.contains("New &amp; useful title"));
    let changed = BenchmarkRecordSet::parse(&xml).unwrap();
    assert_eq!(changed.solvers[0].name, "Renamed");
    assert_eq!(changed.observations().unwrap().len(), 1);
    records.problems[0].run_ids.push(999);
    assert!(records.validate().is_err());
    assert!(
        BenchmarkRecordSet::parse(&source.replace(
            "<singleBenchmarkResult>1</singleBenchmarkResult>",
            "<singleBenchmarkResult>999</singleBenchmarkResult>"
        ))
        .is_err()
    );
    assert!(
        BenchmarkRecordSet::parse(&source.replace(
            "<aggregation>false</aggregation>",
            "<aggregation>false</aggregation><aggregation>true</aggregation>"
        ))
        .is_err()
    );
    assert!(
        BenchmarkRecordSet::parse(&source.replace(
            "<initialized>false</initialized>",
            "<unknown>true</unknown>"
        ))
        .is_err()
    );
    assert!(
        BenchmarkRecordSet::parse(&source.replace(
            "<timeMillisSpent>-1</timeMillisSpent>",
            "<timeMillisSpent>-2</timeMillisSpent>"
        ))
        .is_err()
    );
}
#[test]
fn real_comparator_and_weight_callbacks_rank_data_and_conflicts_invoke_neither() {
    let records = BenchmarkRecordSet::merge(&selected_records()).unwrap();
    let comparisons = Arc::new(AtomicUsize::new(0));
    let c = comparisons.clone();
    let comparator = Arc::new(move |a: &BenchmarkSummary, b: &BenchmarkSummary| {
        c.fetch_add(1, Ordering::SeqCst);
        a.compare(b, BenchmarkRanking::TotalScore)
    });
    let policy = BenchmarkRecordReportPolicy {
        comparator: Some(comparator.clone()),
        ..Default::default()
    };
    assert_eq!(policy.rank(&records).unwrap(), [2, 0, 1]);
    assert!(comparisons.load(Ordering::SeqCst) > 0);
    let weights = Arc::new(AtomicUsize::new(0));
    let c = weights.clone();
    let weight = Arc::new(move |s: &BenchmarkStanding| {
        c.fetch_add(1, Ordering::SeqCst);
        -(s.rank as i128)
    });
    assert_eq!(
        BenchmarkRecordReportPolicy {
            weight: Some(weight.clone()),
            ..Default::default()
        }
        .rank(&records)
        .unwrap(),
        [0, 1, 2]
    );
    assert_eq!(weights.load(Ordering::SeqCst), 3);
    let old = comparisons.load(Ordering::SeqCst);
    let conflict = BenchmarkRecordReportPolicy {
        comparator: Some(comparator),
        weight: Some(weight),
        ..Default::default()
    };
    assert!(conflict.rank(&records).is_err());
    assert_eq!(comparisons.load(Ordering::SeqCst), old);
    assert_eq!(weights.load(Ordering::SeqCst), 3);
}
#[test]
fn parsed_configs_real_file_binding_and_native_solver_execute_written_benchmarks() {
    let mut config = BenchmarkRecordConfig::parse(include_str!(
        "fixtures/benchmark_records/testBenchmarkConfigWithoutNamespace.xml"
    ))
    .unwrap();
    config.directory =
        Some(std::env::temp_dir().join(format!("timetuck-records-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(config.directory.as_ref().unwrap());
    let settings = config.runtime_configuration();
    let mut runtime = XmlBenchmarkRuntime::<FiniteProblem<i32>, SimpleScore, String>::default();
    let loads = Arc::new(AtomicUsize::new(0));
    let load_count = loads.clone();
    let writes = Arc::new(AtomicUsize::new(0));
    let write_count = writes.clone();
    let io = BenchmarkRecordFileIo::new(
        "txt",
        move |_| {
            load_count.fetch_add(1, Ordering::SeqCst);
            FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).map_err(|e| e.to_string())
        },
        move |p: &FiniteProblem<i32>, _| {
            assert_eq!(p.variables().len(), 3);
            write_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    );
    let key = config.solvers[0]
        .problem
        .as_ref()
        .unwrap()
        .loader
        .clone()
        .unwrap();
    io.register(&mut runtime, key, |e| e).unwrap();
    io.write(
        &io.read(Path::new("rigid.txt")).unwrap(),
        Path::new("ignored.txt"),
    )
    .unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    let solves = Arc::new(AtomicUsize::new(0));
    let calls = solves.clone();
    let signature =
        XmlFactorySignature::from_configuration(config.solvers[0].solver.as_ref().unwrap())
            .unwrap();
    runtime
        .register_solver(signature, move |configuration, p, trial| {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(
                configuration.phase_names(),
                ["constructionHeuristic", "localSearch"]
            );
            let mut network = ConstraintNetwork::new(3);
            network
                .add_constraint("sum", vec![0, 1, 2], |v| {
                    SimpleScore::new(v.iter().flatten().map(|&&v| i64::from(v)).sum())
                })
                .unwrap();
            let result = solve(
                &p,
                &network,
                SolverConfig {
                    local_search: vec![
                        LocalSearchPhasePlan::new(LocalSearchConfig {
                            max_steps: 2,
                            tie_breaking: TieBreaking::Random { seed: trial.seed },
                            ..Default::default()
                        })
                        .with_recorded_late_acceptance(configuration, 0)
                        .map_err(|e| e.to_string())?
                        .into_config(),
                    ],
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
            Ok(BenchmarkObservation {
                score: result.score.unwrap(),
                initialized: result.assignment.values().iter().all(Option::is_some),
                work: result.work,
            })
        })
        .unwrap();
    let directory = runtime
        .load_and_build(&settings)
        .unwrap()
        .benchmark()
        .unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("summary.json")).unwrap()).unwrap();
    assert!(solves.load(Ordering::SeqCst) > 0);
    assert_eq!(loads.load(Ordering::SeqCst), 5);
    assert_eq!(report["complete"], true);
    assert!(
        report["trials"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["result"]["initialized"] == true)
    );
    std::fs::remove_dir_all(config.directory.unwrap()).unwrap();
}

#[test]
fn validated_late_acceptance_size_reaches_actual_local_search() {
    let xml = "<solver><localSearch><acceptor><lateAcceptanceSize>3</lateAcceptanceSize></acceptor></localSearch></solver>";
    for invalid in ["0", "-1", "invalid"] {
        assert!(
            SolverXmlConfiguration::parse(&xml.replace(">3<", &format!(">{invalid}<"))).is_err()
        );
    }
    let parsed = SolverXmlConfiguration::parse(xml).unwrap();
    let plan = LocalSearchPhasePlan::new(LocalSearchConfig {
        max_steps: 2,
        ..Default::default()
    })
    .with_recorded_late_acceptance(&parsed, 0)
    .unwrap()
    .into_config();
    assert!(matches!(
        plan.acceptance,
        Acceptance::LateAcceptance {
            history_size: 3,
            ..
        }
    ));
    let problem = FiniteProblem::new(vec![Variable::new(vec![-1, 0, 1]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |values| {
            SimpleScore::new(i64::from(*values[0].unwrap()))
        })
        .unwrap();
    let run = |local| {
        solve(
            &problem,
            &network,
            SolverConfig {
                local_search: vec![local],
                ..Default::default()
            },
        )
        .unwrap()
    };
    let late = run(plan);
    let hill = run(LocalSearchConfig {
        max_steps: 2,
        ..Default::default()
    });
    assert_eq!(late.score, Some(SimpleScore::new(1)));
    assert_eq!(hill.score, late.score);
    assert_eq!(hill.work.accepted_moves, 1);
    assert_eq!(late.work.accepted_moves, 2);
}
