// Complete adapted Community benchmark factory/configuration TESTS.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn suite() -> BenchmarkSuiteDeclaration {
    BenchmarkSuiteDeclaration {
        solvers: vec![SolverBenchmarkDeclaration::default()],
        ..Default::default()
    }
}
fn phases(
    global: Option<EnvironmentMode>,
    construction: Option<EnvironmentMode>,
    local: Option<EnvironmentMode>,
) -> SolverBenchmarkDeclaration {
    SolverBenchmarkDeclaration {
        solver: Some(BenchmarkSolverSettings {
            environment: EnvironmentProfile::new(global, vec![construction, local]),
            phases: vec![SolverPhaseKind::Construction, SolverPhaseKind::LocalSearch],
        }),
        ..Default::default()
    }
}
macro_rules! planner_name {
    ($test:ident,$name:literal,$valid:literal) => {
        #[test]
        fn $test() {
            let c = BenchmarkSuiteDeclaration {
                name: Some($name.into()),
                ..suite()
            };
            let result = c.validate();
            assert_eq!(result.is_ok(), $valid);
            if !$valid {
                assert!(matches!(result, Err(BenchmarkConfigError::Name(_))));
            }
        }
    };
}
planner_name!(
    planner_valid_name_with_underscore_and_space,
    "Valid_name with space_and_underscore",
    true
);
planner_name!(
    planner_valid_name_with_japanese,
    "Valid name (有効名 in Japanese)",
    true
);
planner_name!(planner_invalid_name_with_slash, "slash/name", false);
planner_name!(
    planner_invalid_name_with_suffix_whitespace,
    "Suffixed with space ",
    false
);
planner_name!(
    planner_invalid_name_with_prefix_whitespace,
    " prefixed with space",
    false
);
#[test]
fn planner_no_solver_configs() {
    assert_eq!(
        BenchmarkSuiteDeclaration::default().validate(),
        Err(BenchmarkConfigError::NoSolvers)
    );
}
#[test]
fn phase_without_environment_mode_is_valid() {
    BenchmarkSuiteDeclaration {
        solvers: vec![phases(Some(EnvironmentMode::FullAssert), None, None)],
        ..Default::default()
    }
    .validate()
    .unwrap();
}
fn assert_phase_rejection(c: BenchmarkSuiteDeclaration) {
    let e = c.validate().unwrap_err();
    assert!(matches!(e, BenchmarkConfigError::PhaseEnvironment { .. }));
    assert!(
        e.to_string()
            .contains("cannot override the environment mode when in benchmark mode")
    );
}
#[test]
fn phase_overriding_environment_mode_is_rejected() {
    assert_phase_rejection(BenchmarkSuiteDeclaration {
        solvers: vec![phases(
            Some(EnvironmentMode::PhaseAssert),
            None,
            Some(EnvironmentMode::FullAssert),
        )],
        ..Default::default()
    });
}
#[test]
fn any_environment_mode_override_is_rejected() {
    assert_phase_rejection(BenchmarkSuiteDeclaration {
        solvers: vec![
            phases(Some(EnvironmentMode::PhaseAssert), None, None),
            phases(
                Some(EnvironmentMode::PhaseAssert),
                None,
                Some(EnvironmentMode::FullAssert),
            ),
        ],
        ..Default::default()
    });
}
#[test]
fn solver_benchmark_without_solver_config_is_valid() {
    suite().validate().unwrap();
}
#[test]
fn inherited_config_without_phase_override_is_valid() {
    BenchmarkSuiteDeclaration {
        inherited: Some(phases(Some(EnvironmentMode::FullAssert), None, None)),
        ..suite()
    }
    .validate()
    .unwrap();
}
#[test]
fn inherited_config_overriding_environment_mode_is_rejected() {
    assert_phase_rejection(BenchmarkSuiteDeclaration {
        inherited: Some(phases(None, Some(EnvironmentMode::FullAssert), None)),
        ..suite()
    });
}
#[test]
fn inherited_config_overriding_environment_mode_is_rejected_with_blueprints_only() {
    assert_phase_rejection(BenchmarkSuiteDeclaration {
        inherited: Some(phases(None, Some(EnvironmentMode::FullAssert), None)),
        blueprints: vec![BenchmarkBlueprint::ConstructionWithAndWithoutLocalSearch],
        ..Default::default()
    });
}
#[test]
fn non_unique_solver_config_name() {
    let mut configs = vec![
        SolverBenchmarkDeclaration {
            name: Some("x".into()),
            ..Default::default()
        };
        2
    ];
    assert_eq!(
        generate_solver_benchmark_names(&mut configs),
        Err(BenchmarkConfigError::DuplicateName("x".into()))
    );
}
#[test]
fn unique_names_generated() {
    let mut configs = vec![
        SolverBenchmarkDeclaration::default(),
        SolverBenchmarkDeclaration::default(),
        SolverBenchmarkDeclaration {
            name: Some("Config_1".into()),
            ..Default::default()
        },
    ];
    generate_solver_benchmark_names(&mut configs).unwrap();
    assert_eq!(configs[2].name.as_deref(), Some("Config_1"));
    let names: std::collections::BTreeSet<_> =
        configs.iter().map(|c| c.name.as_ref().unwrap()).collect();
    for i in 0..configs.len() {
        assert!(names.contains(&format!("Config_{i}")));
    }
}
#[test]
fn resolve_parallel_benchmark_count_automatically() {
    for (cpus, expected) in [
        (-1, 1),
        (0, 1),
        (1, 1),
        (2, 1),
        (3, 2),
        (4, 2),
        (5, 3),
        (6, 4),
        (17, 9),
    ] {
        assert_eq!(automatic_parallel_benchmarks(cpus), expected);
    }
}
#[test]
fn parallel_benchmark_disabled_by_default() {
    assert_eq!(
        BenchmarkSuiteDeclaration::default()
            .parallel_benchmarks(8)
            .unwrap(),
        1
    );
}
#[test]
fn resolved_parallel_benchmark_count_negative() {
    let c = BenchmarkSuiteDeclaration {
        parallel_count: Some("-1".into()),
        ..Default::default()
    };
    assert!(matches!(
        c.parallel_benchmarks(8),
        Err(BenchmarkConfigError::ParallelCount(_))
    ));
}
#[test]
fn calculate_warm_up_time_millis_spent_limit() {
    let c = BenchmarkSuiteDeclaration {
        warmup: BenchmarkWarmupLimits {
            hours: Some(1),
            minutes: Some(2),
            seconds: Some(5),
            milliseconds: Some(753),
        },
        ..Default::default()
    };
    assert_eq!(c.warmup.millis(30_000).unwrap(), 3_725_753);
}
macro_rules! solver_name {
    ($test:ident,$name:literal,$valid:literal) => {
        #[test]
        fn $test() {
            let c = SolverBenchmarkDeclaration {
                name: Some($name.into()),
                sub_runs: Some(1),
                ..Default::default()
            };
            let result = c.validate();
            assert_eq!(result.is_ok(), $valid);
            if !$valid {
                assert!(matches!(result, Err(BenchmarkConfigError::Name(_))));
            }
        }
    };
}
solver_name!(
    solver_valid_name_with_underscore_and_space,
    "Valid_name with space_and_underscore",
    true
);
solver_name!(
    solver_valid_name_with_japanese,
    "Valid name (有効名 in Japanese)",
    true
);
solver_name!(solver_invalid_name_with_slash, "slash/name", false);
solver_name!(
    solver_invalid_name_with_suffix_whitespace,
    "Suffixed with space ",
    false
);
solver_name!(
    solver_invalid_name_with_prefix_whitespace,
    " prefixed with space",
    false
);
#[test]
fn valid_non_zero_sub_single_count() {
    SolverBenchmarkDeclaration {
        name: Some("name".into()),
        sub_runs: Some(2),
        ..Default::default()
    }
    .validate()
    .unwrap();
}
#[test]
fn valid_null_sub_single_count() {
    SolverBenchmarkDeclaration {
        name: Some("name".into()),
        sub_runs: None,
        ..Default::default()
    }
    .validate()
    .unwrap();
}
#[test]
fn invalid_zero_sub_single_count() {
    let c = SolverBenchmarkDeclaration {
        name: Some("name".into()),
        sub_runs: Some(0),
        ..Default::default()
    };
    assert_eq!(c.validate(), Err(BenchmarkConfigError::SubRuns));
}
#[test]
fn invalid_monitor_config() {
    let c = SolverBenchmarkDeclaration {
        name: Some("name".into()),
        sub_runs: Some(1),
        monitoring: Some(vec![BenchmarkMetric::StepScore]),
        solver: Some(BenchmarkSolverSettings::default()),
        ..Default::default()
    };
    assert!(matches!(c.prepare(), Err(BenchmarkConfigError::Monitoring)));
}
fn metric_config() -> SolverBenchmarkDeclaration {
    SolverBenchmarkDeclaration {
        name: Some("name".into()),
        sub_runs: Some(0),
        ..Default::default()
    }
}
#[test]
fn default_statistics_are_used_if_not_present() {
    let c = metric_config();
    assert_eq!(
        c.statistics.solver_metrics(),
        vec![BenchmarkMetric::BestScore]
    );
}
#[test]
fn convert_to_solver_metric() {
    let mut c = metric_config();
    c.statistics.problem = Some(vec![
        ProblemBenchmarkStatistic::ScoreCalculationSpeed,
        ProblemBenchmarkStatistic::MoveEvaluationSpeed,
    ]);
    assert_eq!(
        c.statistics.solver_metrics(),
        vec![
            BenchmarkMetric::ScoreCalculationCount,
            BenchmarkMetric::MoveEvaluationCount
        ]
    );
}
#[test]
fn problem_statistics_are_used_if_present() {
    let mut c = metric_config();
    c.statistics.problem = Some(vec![ProblemBenchmarkStatistic::StepScore]);
    c.statistics.single = Some(vec![
        SingleBenchmarkStatistic::ConstraintMatchTotalBestScore,
    ]);
    assert_eq!(
        c.statistics.solver_metrics(),
        vec![
            BenchmarkMetric::StepScore,
            BenchmarkMetric::ConstraintMatchTotalBestScore
        ]
    );
}
#[test]
fn determine_problem_statistic_type_list() {
    let mut c = ProblemBenchmarkStatistics::default();
    let default = vec![ProblemBenchmarkStatistic::BestScore];
    assert_eq!(c.problem_statistics(), default);
    c.problem = Some(vec![]);
    assert_eq!(c.problem_statistics(), default);
    assert_ne!(default, vec![ProblemBenchmarkStatistic::MemoryUse]);
    c.problem = Some(vec![ProblemBenchmarkStatistic::MemoryUse]);
    assert_eq!(
        c.problem_statistics(),
        vec![ProblemBenchmarkStatistic::MemoryUse]
    );
    c.enabled = Some(true);
    assert_eq!(
        c.problem_statistics(),
        vec![ProblemBenchmarkStatistic::MemoryUse]
    );
    c.enabled = Some(false);
    assert!(c.problem_statistics().is_empty());
}
#[test]
fn determine_single_statistic_type_list() {
    let mut c = ProblemBenchmarkStatistics::default();
    assert!(c.single_statistics().is_empty());
    c.single = Some(vec![]);
    assert!(c.single_statistics().is_empty());
    c.single = Some(vec![
        SingleBenchmarkStatistic::ConstraintMatchTotalStepScore,
    ]);
    assert_eq!(
        c.single_statistics(),
        &[SingleBenchmarkStatistic::ConstraintMatchTotalStepScore]
    );
}
#[test]
fn duplicated_default_metrics() {
    let statistics = ProblemBenchmarkStatistics {
        problem: Some(vec![ProblemBenchmarkStatistic::BestScore]),
        ..Default::default()
    };
    let default = SolverBenchmarkDeclaration {
        statistics: statistics.clone(),
        ..Default::default()
    };
    let mut inherited = SolverBenchmarkDeclaration {
        statistics,
        ..Default::default()
    };
    inherited.inherit(&default);
    assert_eq!(inherited.statistics.problem.unwrap().len(), 1);
}
