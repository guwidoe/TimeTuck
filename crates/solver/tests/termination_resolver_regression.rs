//! Original request/platform resolution and solver integration checks.
#![allow(dead_code)]
include!("solver_support/fixture.rs");
use std::time::Duration;
fn request(
    unimproved: Option<Duration>,
    steps: Option<i64>,
    moves: Option<i64>,
) -> TerminationProfile {
    TerminationProfile::new(
        None,
        unimproved,
        steps,
        moves,
        Some(Duration::from_secs(1)),
        Some((1, 2)),
    )
    .unwrap()
}
fn local() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        acceptance: Acceptance::HillClimbing,
        max_steps: 100,
        diminished_returns: Some(
            DiminishedReturnsConfig::new(Duration::from_nanos(1), 1, 1).unwrap(),
        ),
        ..Default::default()
    }
}
#[test]
fn resolved_counts_or_stop_and_clear_stagnation_in_all_six_paths() {
    let platform =
        TerminationResolver::new(Duration::from_secs(60), Some(Duration::ZERO), None, None)
            .unwrap();
    for mode in 0..6 {
        for (steps, moves, wanted) in [
            (1, 100, SolverTermination::Condition),
            (100, 1, SolverTermination::MoveEvaluationLimit),
        ] {
            let r = platform.resolve(Some(&request(None, Some(steps), Some(moves))));
            let out = r
                .with_sequence(
                    SolverSequence {
                        limits: SolverLimits {
                            max_unimproved_time: Some(Duration::ZERO),
                            ..Default::default()
                        },
                        phases: vec![SolverPhase::LocalSearch(local())],
                    },
                    |plan| {
                        assert!(plan.limits.max_unimproved_time.is_none());
                        let SolverPhase::LocalSearch(c) = &plan.phases[0] else {
                            panic!()
                        };
                        assert!(c.diminished_returns.is_none());
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
            assert_eq!(out.reason, wanted, "mode {mode}, {steps}, {moves}");
            if steps == 1 {
                assert_eq!(out.work.completed_steps, 1);
            } else {
                assert_eq!(out.work.move_evaluations, 1);
            }
        }
    }
}
#[test]
fn resolved_stagnation_clears_platform_and_base_counts_in_all_six_paths() {
    let platform =
        TerminationResolver::new(Duration::from_secs(60), None, Some(0), Some(0)).unwrap();
    let r = platform.resolve(Some(&request(Some(Duration::ZERO), None, None)));
    for mode in 0..6 {
        let out = r
            .with_sequence(
                SolverSequence {
                    limits: SolverLimits {
                        max_move_evaluations: Some(0),
                        ..Default::default()
                    },
                    phases: vec![SolverPhase::LocalSearch(local())],
                },
                |plan| {
                    assert_eq!(plan.limits.max_move_evaluations, None);
                    assert!(plan.limits.termination.is_none());
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
        assert_eq!(
            out.reason,
            SolverTermination::UnimprovedTimeLimit,
            "mode {mode}"
        );
        assert_eq!(out.work.move_evaluations, 0);
        assert!(out.complete);
    }
}
#[test]
fn default_partial_tuning_and_disabled_policy_bind_mixed_plans_without_clone() {
    struct NonClone;
    let platform = TerminationResolver::new(Duration::from_secs(10), None, None, None).unwrap();
    let partial = TerminationProfile::new(None, None, None, None, None, Some((1, 3))).unwrap();
    let disabled = request(None, Some(2), None);
    for (r, expected) in [
        (
            platform.resolve(None),
            Some(default_diminished_returns_config()),
        ),
        (
            platform.resolve(Some(&partial)),
            Some(DiminishedReturnsConfig::new(Duration::from_secs(30), 1, 3).unwrap()),
        ),
        (platform.resolve(Some(&disabled)), None),
    ] {
        r.with_mixed_sequence(
            MixedSolverSequence::<NonClone> {
                limits: SolverLimits {
                    max_evaluations: 99,
                    max_move_evaluations: Some(0),
                    max_unimproved_time: Some(Duration::ZERO),
                    ..Default::default()
                },
                phases: vec![
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search: LocalSearchConfig {
                            max_steps: 17,
                            diminished_returns: Some(
                                DiminishedReturnsConfig::new(Duration::from_secs(1), 1, 2).unwrap(),
                            ),
                            ..Default::default()
                        },
                        joint_moves: false,
                    }),
                    MixedSolverPhase::Exact(Default::default()),
                ],
            },
            |plan| {
                assert_eq!(plan.limits.max_evaluations, 99);
                assert_eq!(plan.limits.max_time, Some(Duration::from_secs(10)));
                assert_eq!(plan.limits.max_move_evaluations, None);
                assert_eq!(plan.limits.max_unimproved_time, None);
                let MixedSolverPhase::LocalSearch(c) = &plan.phases[0] else {
                    panic!()
                };
                assert_eq!(c.search.diminished_returns, expected);
                assert_eq!(c.search.max_steps, 17);
                assert!(!c.joint_moves);
                assert!(matches!(plan.phases[1], MixedSolverPhase::Exact(_)));
                if expected.is_none() {
                    assert!(
                        plan.limits
                            .termination
                            .unwrap()
                            .status(SearchProgress {
                                completed_steps: 2,
                                ..Default::default()
                            })
                            .unwrap()
                            .reached
                    );
                }
            },
        );
    }
}
#[test]
fn zero_request_is_not_missing_and_resolution_does_not_mutate_inputs() {
    let platform =
        TerminationResolver::new(Duration::from_secs(60), None, Some(7), Some(9)).unwrap();
    let q = request(None, Some(0), None);
    let before = (platform.clone(), q.clone());
    let r = platform.resolve(Some(&q));
    assert_eq!(r.profile().step_count_limit(), Some(0));
    assert_eq!(r.profile().move_count_limit(), Some(9));
    assert_eq!(r.profile().sliding_window(), None);
    assert_eq!(r.profile().minimum_improvement_ratio(), None);
    for mode in 0..6 {
        let factories = Cell::new(0);
        let out = r
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
        assert_eq!(out.reason, SolverTermination::Condition);
        assert_eq!(out.work.evaluations, 0);
        assert_eq!(factories.get(), 0);
    }
    assert_eq!((platform, q), before);
}
