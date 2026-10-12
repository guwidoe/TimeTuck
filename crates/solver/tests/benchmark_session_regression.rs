//! Original global warm-up ordering, failure/cancellation and numerical checks.
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
fn plan() -> PreparedBenchmarkSuite {
    BenchmarkSuiteDeclaration {
        solvers: vec![
            SolverBenchmarkDeclaration {
                sub_runs: Some(2),
                ..Default::default()
            },
            SolverBenchmarkDeclaration {
                sub_runs: Some(3),
                ..Default::default()
            },
        ],
        parallel_count: Some("2".into()),
        ..Default::default()
    }
    .prepare(8, 10_000)
    .unwrap()
}
fn native_observation(
    value: i32,
    trial: BenchmarkTrial,
    budget: Option<std::time::Duration>,
) -> BenchmarkObservation<SimpleScore> {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, value])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
    })
    .unwrap();
    let result = solve(
        &p,
        &n,
        SolverConfig {
            limits: SolverLimits {
                max_time: budget,
                assert_phase_scores: true,
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                max_steps: 3,
                tie_breaking: TieBreaking::Random { seed: trial.seed },
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .unwrap();
    BenchmarkObservation {
        score: result.score.unwrap(),
        initialized: result.complete,
        work: result.work,
    }
}
#[test]
fn staged_sessions_match_existing_seeds_and_all_warmups_precede_measurements() {
    let plan = plan();
    let warmed = AtomicUsize::new(0);
    let events = Mutex::new(vec![]);
    let mut session = BenchmarkSession::new(Some(plan.clone()), Some(PathBuf::from("output")));
    let result = session
        .run(&[1_i32, 2], 1, 71, None, |c, p, t, budget| {
            if t.warmup {
                assert!(budget.is_some());
            } else {
                assert!(budget.is_none());
                assert_eq!(warmed.load(Ordering::SeqCst), 4);
            }
            events.lock().unwrap().push((c.name.clone(), t));
            let observation = native_observation(p, t, budget);
            if t.warmup {
                warmed.fetch_add(1, Ordering::SeqCst);
            }
            Ok::<_, ()>(observation)
        })
        .unwrap();
    assert!(result.is_successful());
    assert_eq!(session.state(), BenchmarkSessionState::Completed);
    assert_eq!(result.trials.len(), 14);
    assert!(result.trials[..4].iter().all(|t| t.trial.warmup));
    assert!(result.trials[4..].iter().all(|t| !t.trial.warmup));
    for (configuration, c) in plan.solvers().iter().enumerate() {
        let reference = BenchmarkSchedule {
            warmup_runs: 1,
            measured_runs: c.sub_runs,
            seed: 71,
        }
        .run(&[()], &[1, 2], None, |_, p, _| {
            Ok::<_, ()>(BenchmarkObservation {
                score: SimpleScore::new(p),
                initialized: true,
                work: SolverWork::default(),
            })
        })
        .unwrap();
        let got: Vec<_> = result
            .trials
            .iter()
            .filter(|t| t.trial.configuration == configuration)
            .collect();
        for (a, b) in got.iter().zip(reference.trials()) {
            assert_eq!(
                (
                    a.trial.problem,
                    a.trial.repetition,
                    a.trial.warmup,
                    a.trial.seed
                ),
                (
                    b.trial.problem,
                    b.trial.repetition,
                    b.trial.warmup,
                    b.trial.seed
                )
            );
            assert_eq!(
                a.result.as_ref().unwrap().score,
                b.result.as_ref().unwrap().score
            );
        }
    }
    let mut cancelled = BenchmarkSession::new(Some(plan), Some(PathBuf::from("output")));
    let token = CancellationToken::new();
    token.cancel();
    let r = cancelled
        .run::<i32, SimpleScore, ()>(&[1], 1, 0, Some(&token), |_, _, _, _| panic!("cancelled"))
        .unwrap();
    assert!(!r.complete);
    assert!(r.trials.is_empty());
    assert_eq!(cancelled.state(), BenchmarkSessionState::Cancelled);
}
#[test]
fn warmup_failures_preserve_parent_tokens_and_cannot_start_again() {
    let parent = CancellationToken::new();
    let measured = AtomicUsize::new(0);
    let mut session = BenchmarkSession::new(Some(plan()), Some(PathBuf::from("output")));
    let e = session
        .run(&[1], 1, 0, Some(&parent), |_, _, t, _| {
            if !t.warmup {
                measured.fetch_add(1, Ordering::SeqCst);
            }
            Err::<BenchmarkObservation<SimpleScore>, _>("original failure")
        })
        .unwrap_err();
    assert!(matches!(
        e,
        BenchmarkExecutionError::Warmup("original failure")
    ));
    assert_eq!(measured.load(Ordering::SeqCst), 0);
    assert!(!parent.is_cancelled());
    assert_eq!(session.state(), BenchmarkSessionState::Failed);
    assert_eq!(session.start(), Err(BenchmarkStartError::AlreadyStarted));
    let mut missing = BenchmarkSession::new(None, None);
    assert_eq!(missing.start(), Err(BenchmarkStartError::NoConfigurations));
    assert_eq!(missing.state(), BenchmarkSessionState::Idle);
    let mut session = BenchmarkSession::new(Some(plan()), Some(PathBuf::from("output")));
    let r = session
        .run(&[1], 0, 0, None, |_, _, _, _| {
            Err::<BenchmarkObservation<SimpleScore>, _>("measured failure")
        })
        .unwrap();
    assert!(r.complete);
    assert!(!r.is_successful());
    assert_eq!(r.trials.len(), 5);
    assert!(r.trials.iter().all(|t| t.result.is_err()));
}
#[test]
fn deviations_preserve_large_offsets_exclude_failures_and_validate_dimensions() {
    let offset = 9_000_000_000_000_000_000_i64;
    let d = benchmark_score_deviations(
        &[
            Some(SimpleScore::new(offset - 1)),
            None,
            Some(SimpleScore::new(offset + 1)),
        ],
        &SimpleScore::new(offset),
        2,
    )
    .unwrap();
    assert!((d[0] - 1.0).abs() < 1e-12);
    assert_eq!(
        benchmark_score_deviations(&[None::<SimpleScore>], &SimpleScore::new(0), 0),
        Err(BenchmarkDeviationError::Count)
    );
    assert_eq!(
        benchmark_score_deviations(&[Some(SimpleScore::new(0))], &SimpleScore::new(0), 2),
        Err(BenchmarkDeviationError::Count)
    );
    let values = [
        Some(HardSoftScore::new(-2, 4)),
        Some(HardSoftScore::new(-4, 8)),
    ];
    let d = benchmark_score_deviations(&values, &HardSoftScore::new(-3, 6), 2).unwrap();
    assert!((d[0] - 1.0).abs() < 1e-12);
    assert!((d[1] - 2.0).abs() < 1e-12);
    let scores = [Some(BendableScore::new([1], [2]))];
    let average = BendableScore::new([1, 2], [3]);
    assert!(matches!(
        benchmark_score_deviations(&scores, &average, 1),
        Err(BenchmarkDeviationError::Score(
            BenchmarkResultError::DifferentDimensions { .. }
        ))
    ));
}
#[test]
fn deviation_formatting_and_option_inheritance_remain_explicit() {
    assert_eq!(
        format_benchmark_deviations(Some(&[0.0, 1.234, 2e9]))
            .unwrap()
            .as_deref(),
        Some("0.0/1.23/2.0E9")
    );
    for x in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(
            format_benchmark_deviations(Some(&[x])),
            Err(BenchmarkDeviationError::NonFinite)
        );
    }
    let base = BenchmarkReportOptions {
        locale: Some("en-CA".into()),
        ranking: Some(BenchmarkReportRanking::TotalRanking),
        ..Default::default()
    };
    let mut own = BenchmarkReportOptions {
        locale: Some("de-AT".into()),
        ..Default::default()
    };
    own.inherit(&base);
    assert_eq!(own.locale.as_deref(), Some("de-AT"));
    assert_eq!(own.ranking, base.ranking);
    assert_eq!(base.locale.as_deref(), Some("en-CA"));
    assert_eq!(
        BenchmarkBlueprintDeclaration {
            kind: Some(BenchmarkBlueprint::ConstructionWithAndWithoutLocalSearch)
        }
        .validate()
        .unwrap(),
        BenchmarkBlueprint::ConstructionWithAndWithoutLocalSearch
    );
}
