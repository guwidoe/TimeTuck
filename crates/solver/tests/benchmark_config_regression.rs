//! Original prepared benchmark inheritance, bounded parallel execution and failures.
use std::sync::{
    Barrier, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
fn settings() -> BenchmarkSolverSettings {
    BenchmarkSolverSettings {
        environment: EnvironmentProfile::new(Some(EnvironmentMode::PhaseAssert), vec![None, None]),
        phases: vec![SolverPhaseKind::Construction, SolverPhaseKind::LocalSearch],
    }
}
#[test]
fn inherited_blueprints_expand_to_actual_phases_without_mutating_declarations() {
    let c = BenchmarkSuiteDeclaration {
        blueprints: vec![BenchmarkBlueprint::ConstructionWithAndWithoutLocalSearch],
        inherited: Some(SolverBenchmarkDeclaration {
            sub_runs: Some(2),
            solver: Some(settings()),
            statistics: ProblemBenchmarkStatistics {
                problem: Some(vec![
                    ProblemBenchmarkStatistic::StepScore,
                    ProblemBenchmarkStatistic::StepScore,
                ]),
                single: Some(vec![
                    SingleBenchmarkStatistic::ConstraintMatchTotalBestScore,
                ]),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    let p = c.prepare(8, 30_000).unwrap();
    assert_eq!(c.solvers.len(), 0);
    assert!(c.inherited.as_ref().unwrap().name.is_none());
    assert_eq!(p.solvers().len(), 2);
    assert_eq!(
        p.solvers()[0].solver.phases,
        vec![SolverPhaseKind::Construction]
    );
    assert_eq!(
        p.solvers()[1].solver.phases,
        vec![SolverPhaseKind::Construction, SolverPhaseKind::LocalSearch]
    );
    for (i, s) in p.solvers().iter().enumerate() {
        assert_eq!(s.name, format!("Config_{i}"));
        assert_eq!(s.sub_runs.get(), 2);
        assert_eq!(
            s.solver.environment.global_mode(),
            EnvironmentMode::PhaseAssert
        );
        assert_eq!(
            s.solver.environment.declared_phases().len(),
            s.solver.phases.len()
        );
        assert_eq!(
            s.metrics,
            vec![
                BenchmarkMetric::StepScore,
                BenchmarkMetric::ConstraintMatchTotalBestScore
            ]
        );
    }
    let c = BenchmarkSuiteDeclaration {
        solvers: vec![SolverBenchmarkDeclaration {
            sub_runs: Some(3),
            statistics: ProblemBenchmarkStatistics {
                enabled: Some(false),
                ..Default::default()
            },
            ..Default::default()
        }],
        inherited: c.inherited,
        ..Default::default()
    };
    let p = c.prepare(8, 0).unwrap();
    assert_eq!(p.solvers()[0].sub_runs.get(), 3);
    assert_eq!(
        p.solvers()[0].metrics,
        vec![BenchmarkMetric::ConstraintMatchTotalBestScore]
    );
}
#[test]
fn prepared_parallel_schedules_run_real_solves_with_matched_seeds_and_metrics() {
    let c = BenchmarkSuiteDeclaration {
        solvers: vec![
            SolverBenchmarkDeclaration {
                sub_runs: Some(2),
                solver: Some(settings()),
                ..Default::default()
            },
            SolverBenchmarkDeclaration {
                sub_runs: Some(3),
                solver: Some(settings()),
                ..Default::default()
            },
        ],
        parallel_count: Some("2".into()),
        warmup: BenchmarkWarmupLimits {
            milliseconds: Some(10_000),
            ..Default::default()
        },
        ..Default::default()
    };
    let plan = c.prepare(8, 0).unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1])]).unwrap();
    let active = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let gate = Barrier::new(2);
    let calls = Mutex::new(vec![]);
    let reports = plan
        .run(&[p], 1, 42, None, |c, p, t, warmup| {
            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(count, Ordering::SeqCst);
            if t.warmup {
                assert_eq!(warmup, Some(std::time::Duration::from_secs(10)));
                gate.wait();
            } else {
                assert!(warmup.is_none());
            }
            calls.lock().unwrap().push((c.name.clone(), t));
            assert_eq!(c.metrics, vec![BenchmarkMetric::BestScore]);
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| {
                SimpleScore::new(i64::from(*v[0].unwrap_or(&0)))
            })
            .unwrap();
            let metrics = SolverMetrics::<SimpleScore>::default();
            let config = SolverConfig {
                limits: SolverLimits {
                    max_time: warmup,
                    assert_phase_scores: c.solver.environment.global_mode()
                        == EnvironmentMode::PhaseAssert,
                    ..Default::default()
                },
                local_search: vec![LocalSearchConfig {
                    max_steps: 3,
                    tie_breaking: TieBreaking::Random { seed: t.seed },
                    ..Default::default()
                }],
                ..Default::default()
            };
            let r = solve_with_extensions(
                &p,
                &n,
                config,
                SolverExtensions {
                    metrics: Some(&metrics),
                    ..Default::default()
                },
            )
            .unwrap();
            let snapshot = metrics.snapshot();
            assert_eq!(snapshot.completed_runs, 1);
            assert_eq!(snapshot.latest.unwrap().best_score, r.score);
            active.fetch_sub(1, Ordering::SeqCst);
            Ok::<_, ()>(BenchmarkObservation {
                score: r.score.unwrap(),
                initialized: r.complete,
                work: r.work,
            })
        })
        .unwrap();
    assert_eq!(peak.load(Ordering::SeqCst), 2);
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(calls.lock().unwrap().len(), 7);
    for (i, r) in reports.iter().enumerate() {
        assert!(r.is_successful());
        assert_eq!(r.trials().len(), i + 3);
        assert_eq!(r.timing(0, 0).unwrap().successful_runs, i + 2);
        assert_eq!(r.timing(0, 0).unwrap().failed_runs, 0);
        for t in r.trials() {
            assert_eq!(t.result.as_ref().unwrap().score, SimpleScore::new(1));
        }
    }
    for (a, b) in reports[0].trials().iter().zip(reports[1].trials()) {
        assert_eq!(a.trial, b.trial);
        assert_eq!(
            a.result.as_ref().unwrap().work,
            b.result.as_ref().unwrap().work
        );
    }
}
#[test]
fn preparation_and_schedule_errors_happen_before_callbacks_and_trial_errors_are_retained() {
    let mut names = vec![
        SolverBenchmarkDeclaration::default(),
        SolverBenchmarkDeclaration {
            name: Some("x".into()),
            ..Default::default()
        },
        SolverBenchmarkDeclaration {
            name: Some("x".into()),
            ..Default::default()
        },
    ];
    let before = names.clone();
    assert!(generate_solver_benchmark_names(&mut names).is_err());
    assert_eq!(names, before);
    for n in ["0", "-2", "text"] {
        let c = BenchmarkSuiteDeclaration {
            solvers: vec![SolverBenchmarkDeclaration::default()],
            parallel_count: Some(n.into()),
            ..Default::default()
        };
        assert!(matches!(
            c.prepare(8, 0),
            Err(BenchmarkConfigError::ParallelCount(_))
        ));
    }
    let c = BenchmarkSuiteDeclaration {
        solvers: vec![SolverBenchmarkDeclaration {
            sub_runs: Some(2),
            ..Default::default()
        }],
        ..Default::default()
    };
    let plan = c.prepare(8, 0).unwrap();
    let calls = AtomicUsize::new(0);
    assert!(matches!(
        plan.run::<i32, SimpleScore, ()>(&[], 0, 0, None, |_, _, _, _| panic!("preflight")),
        Err(BenchmarkConfigError::Schedule(
            BenchmarkScheduleError::NoProblems
        ))
    ));
    assert!(matches!(
        plan.run(&[1_i32], usize::MAX, 0, None, |_, _, _, _| Ok::<_, ()>(
            BenchmarkObservation {
                score: SimpleScore::new(0),
                initialized: true,
                work: SolverWork::default()
            }
        )),
        Err(BenchmarkConfigError::Schedule(
            BenchmarkScheduleError::TooManyTrials
        ))
    ));
    let reports = plan
        .run(&[1_i32], 1, 0, None, |_, _, t, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            if t.warmup {
                Err("warmup failure")
            } else {
                Ok(BenchmarkObservation {
                    score: SimpleScore::new(1),
                    initialized: true,
                    work: SolverWork::default(),
                })
            }
        })
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(reports[0].is_complete());
    assert!(!reports[0].is_successful());
    assert_eq!(
        reports[0].trials()[0].result.as_ref().unwrap_err(),
        &"warmup failure"
    );
    assert_eq!(reports[0].timing(0, 0).unwrap().successful_runs, 2);
    let token = CancellationToken::new();
    token.cancel();
    let reports = plan
        .run::<i32, SimpleScore, ()>(&[1_i32], 1, 0, Some(&token), |_, _, _, _| {
            panic!("cancelled callback")
        })
        .unwrap();
    assert!(reports[0].trials().is_empty());
    assert!(!reports[0].is_complete());
}
#[test]
fn checked_duration_boundaries_and_auto_worker_bounds_are_explicit() {
    assert_eq!(
        BenchmarkWarmupLimits::default().millis(30_000).unwrap(),
        30_000
    );
    assert_eq!(
        BenchmarkWarmupLimits {
            milliseconds: Some(0),
            ..Default::default()
        }
        .millis(30_000)
        .unwrap(),
        0
    );
    for limits in [
        BenchmarkWarmupLimits {
            seconds: Some(-1),
            ..Default::default()
        },
        BenchmarkWarmupLimits {
            hours: Some(i64::MAX),
            ..Default::default()
        },
    ] {
        assert_eq!(limits.millis(0), Err(BenchmarkConfigError::Warmup));
    }
    let mut prior = 1;
    for cpu in 1..1000 {
        let workers = automatic_parallel_benchmarks(cpu);
        assert!(workers >= prior && workers <= cpu as usize);
        prior = workers;
    }
    let c = BenchmarkSuiteDeclaration {
        solvers: vec![SolverBenchmarkDeclaration::default()],
        parallel_count: Some("AUTO".into()),
        ..Default::default()
    };
    let p = c.prepare(17, 0).unwrap();
    assert_eq!(p.parallel_benchmarks(), 9);
    let reports = p
        .run(&[0], 0, 0, None, |_, _, _, _| {
            Ok::<_, ()>(BenchmarkObservation {
                score: SimpleScore::new(0),
                initialized: true,
                work: SolverWork::default(),
            })
        })
        .unwrap();
    assert_eq!(reports.len(), 1);
}
