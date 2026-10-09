//! Original profile binding and actual solve regressions.
#![allow(dead_code)]
include!("solver_support/fixture.rs");
use std::time::Duration;
fn profile(steps: Option<i64>, moves: Option<i64>) -> TerminationProfile {
    TerminationProfile::new(None, None, steps, moves, None, None).unwrap()
}
#[test]
fn profile_steps_span_phases_across_all_six_solver_paths() {
    let p = profile(Some(3), None);
    for mode in 0..6 {
        let result = p
            .with_sequence(
                SolverSequence {
                    phases: vec![
                        SolverPhase::LocalSearch(LocalSearchConfig {
                            max_steps: 2,
                            acceptance: Acceptance::HillClimbing,
                            selection: MoveSelection::FirstAccepted,
                            ..Default::default()
                        });
                        4
                    ],
                    ..Default::default()
                },
                |plan| {
                    run_scored(
                        (mode, true, MixedConstructionOrder::BasicThenLists),
                        plan,
                        &Cell::new(0),
                        None,
                        || {},
                        |_| SimpleScore::ZERO,
                    )
                },
            )
            .unwrap();
        assert_eq!(result.reason, SolverTermination::Condition, "mode {mode}");
        assert_eq!(result.work.completed_steps, 3);
        assert_eq!(result.phases.len(), 2);
        assert!(result.complete);
    }
}
#[test]
fn move_budget_stops_candidate_work_in_all_six_paths() {
    for mode in 0..6 {
        let result = profile(None, Some(1))
            .with_sequence(SolverConfig::default(), |plan| {
                run_scored(
                    (mode, true, MixedConstructionOrder::BasicThenLists),
                    plan,
                    &Cell::new(0),
                    None,
                    || {},
                    |_| SimpleScore::ZERO,
                )
            })
            .unwrap();
        assert_eq!(
            result.reason,
            SolverTermination::MoveEvaluationLimit,
            "mode {mode}"
        );
        assert_eq!(result.work.move_evaluations, 1);
    }
}
#[test]
fn existing_expression_is_preserved_and_zero_profile_stops_before_scoring() {
    let old = SearchTermination::Condition(SearchLimit::Steps(1));
    for mode in 0..6 {
        let p = profile(Some(3), None);
        let result = p
            .with_sequence(
                SolverConfig {
                    limits: SolverLimits {
                        termination: Some(&old),
                        ..Default::default()
                    },
                    local_search: vec![LocalSearchConfig {
                        acceptance: Acceptance::HillClimbing,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                |plan| {
                    run_scored(
                        (mode, true, MixedConstructionOrder::BasicThenLists),
                        plan,
                        &Cell::new(0),
                        None,
                        || {},
                        |_| SimpleScore::ZERO,
                    )
                },
            )
            .unwrap();
        assert_eq!(result.reason, SolverTermination::Condition);
        assert_eq!(result.work.completed_steps, 1);
        let factories = Cell::new(0);
        let result = profile(Some(0), None)
            .with_sequence(SolverConfig::default(), |plan| {
                run_scored(
                    (mode, true, MixedConstructionOrder::BasicThenLists),
                    plan,
                    &factories,
                    None,
                    || panic!("zero steps must not score"),
                    SimpleScore::new,
                )
            })
            .unwrap();
        assert_eq!(result.work.evaluations, 0);
        assert_eq!(factories.get(), 0);
    }
}
#[test]
fn binding_overlays_only_specified_fields_and_mixed_tuning() {
    struct NonClone;
    let old = DiminishedReturnsConfig::new(Duration::from_secs(8), 1, 2).unwrap();
    let p = TerminationProfile::new(
        Some(Duration::from_secs(7)),
        None,
        None,
        Some(9),
        None,
        Some((1, 100)),
    )
    .unwrap();
    let plan = MixedSolverSequence::<NonClone> {
        limits: SolverLimits {
            max_evaluations: 777,
            max_unimproved_steps: Some(5),
            ..Default::default()
        },
        phases: vec![
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 12,
                    diminished_returns: Some(old),
                    ..Default::default()
                },
                joint_moves: false,
            }),
            MixedSolverPhase::Exact(Default::default()),
        ],
    };
    p.with_mixed_sequence(plan, |plan| {
        assert_eq!(plan.limits.max_time, Some(Duration::from_secs(7)));
        assert_eq!(plan.limits.max_move_evaluations, Some(9));
        assert_eq!(plan.limits.max_evaluations, 777);
        assert_eq!(plan.limits.max_unimproved_steps, Some(5));
        assert!(plan.limits.termination.is_none());
        let MixedSolverPhase::LocalSearch(local) = &plan.phases[0] else {
            panic!()
        };
        assert!(!local.joint_moves);
        assert_eq!(local.search.max_steps, 12);
        let policy = local.search.diminished_returns.unwrap();
        assert_eq!(policy.window(), Duration::from_secs(30));
        assert_eq!(policy.ratio(), (1, 100));
        assert!(matches!(plan.phases[1], MixedSolverPhase::Exact(_)));
    });
    let default = TerminationProfile::default();
    default.with_sequence(
        SolverSequence::<NonClone> {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                diminished_returns: Some(old),
                ..Default::default()
            })],
            ..Default::default()
        },
        |plan| {
            let SolverPhase::LocalSearch(local) = &plan.phases[0] else {
                panic!()
            };
            assert_eq!(local.diminished_returns, Some(old));
        },
    );
}
#[test]
fn fallback_is_atomic_and_exact_tuning_validates_before_binding() {
    let primary = profile(Some(0), None);
    let fallback =
        TerminationProfile::new(None, Some(Duration::ZERO), None, None, None, None).unwrap();
    let before = (primary.clone(), fallback.clone());
    assert!(primary.with_fallback(Some(&fallback)).is_err());
    assert_eq!((primary, fallback), before);
    assert!(TerminationProfile::new(None, None, None, None, None, Some((1, 0))).is_err());
    assert!(TerminationProfile::new(None, None, None, None, Some(Duration::ZERO), None).is_err());
    let p = TerminationProfile::new(None, None, None, None, Some(Duration::from_nanos(1)), None)
        .unwrap();
    assert_eq!(p.diminished_returns().unwrap().ratio(), (1, 10000));
    assert_eq!(
        p.diminished_returns().unwrap().window(),
        Duration::from_nanos(1)
    );
}
