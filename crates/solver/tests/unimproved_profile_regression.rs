//! Original end-to-end profile stagnation counts and resolution regressions.
#![allow(dead_code)]
include!("solver_support/fixture.rs");
use std::time::Duration;
fn profile(n: Option<i64>) -> TerminationProfile {
    TerminationProfile::default()
        .with_unimproved_step_limit(n)
        .unwrap()
}
fn sequence() -> SolverSequence<'static> {
    let local = profile(Some(2)).bind_local_search(LocalSearchConfig {
        max_steps: 100,
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::FirstAccepted,
        ..Default::default()
    });
    SolverSequence {
        phases: vec![SolverPhase::LocalSearch(local); 2],
        ..Default::default()
    }
}
#[test]
fn local_stagnation_resets_and_shared_stagnation_spans_all_six_paths() {
    for mode in 0..6 {
        for (shared, expected, reason) in [
            (None, 4, SolverTermination::PlanCompleted),
            (Some(3), 3, SolverTermination::UnimprovedStepLimit),
            (Some(0), 0, SolverTermination::UnimprovedStepLimit),
        ] {
            let out = profile(shared)
                .with_sequence(sequence(), |plan| {
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
                out.work.completed_steps, expected,
                "mode {mode}, shared {shared:?}"
            );
            assert_eq!(out.reason, reason);
            if shared.is_none() {
                assert!(out.phases.iter().all(|p| p.work.completed_steps == 2));
            }
        }
    }
}
#[test]
fn zero_missing_clear_fallback_and_policy_conflicts() {
    assert_eq!(profile(None).unimproved_step_count_limit(), None);
    assert_eq!(profile(Some(0)).unimproved_step_count_limit(), Some(0));
    assert_eq!(
        profile(Some(3)).with_unimproved_step_limit(None).unwrap(),
        profile(None)
    );
    assert_eq!(
        profile(None)
            .with_fallback(Some(&profile(Some(7))))
            .unwrap()
            .unimproved_step_count_limit(),
        Some(7)
    );
    assert_eq!(
        profile(Some(0))
            .with_fallback(Some(&profile(Some(7))))
            .unwrap()
            .unimproved_step_count_limit(),
        Some(0)
    );
    assert!(matches!(
        TerminationProfile::default().with_unimproved_step_limit(Some(-1)),
        Err(TerminationProfileError::NegativeCount {
            field: "unimprovedStepCountLimit",
            value: -1
        })
    ));
    assert_eq!(
        profile(Some(i64::MAX)).unimproved_step_count_limit(),
        Some(i64::MAX as u64)
    );
    let timed =
        TerminationProfile::new(None, Some(Duration::ZERO), None, None, None, None).unwrap();
    assert!(matches!(
        timed.clone().with_unimproved_step_limit(Some(1)),
        Err(TerminationProfileError::ConflictingLimits {
            count: "unimprovedStepCountLimit"
        })
    ));
    assert!(profile(Some(1)).with_fallback(Some(&timed)).is_err());
    assert!(timed.with_fallback(Some(&profile(Some(1)))).is_err());
    assert_eq!(timed.unimproved_step_count_limit(), None);
    let local = LocalSearchConfig::<SimpleScore> {
        max_unimproved_steps: Some(11),
        ..Default::default()
    };
    assert_eq!(
        profile(None).bind_local_search(local).max_unimproved_steps,
        Some(11)
    );
    assert_eq!(
        profile(Some(0))
            .bind_local_search(local)
            .max_unimproved_steps,
        Some(0)
    );
}
#[test]
fn platform_counts_inherit_and_request_time_suppresses_them() {
    let platform = TerminationResolver::new(Duration::from_secs(60), None, Some(9), Some(90))
        .unwrap()
        .with_unimproved_step_limit(Some(4))
        .unwrap();
    assert_eq!(
        platform
            .resolve(None)
            .profile()
            .unimproved_step_count_limit(),
        Some(4)
    );
    let zero = platform.resolve(Some(&profile(Some(0))));
    assert_eq!(zero.profile().unimproved_step_count_limit(), Some(0));
    assert_eq!(zero.profile().step_count_limit(), Some(9));
    assert_eq!(zero.profile().move_count_limit(), Some(90));
    assert!(!zero.uses_diminished_returns());
    let time = TerminationProfile::new(None, Some(Duration::ZERO), None, None, None, None).unwrap();
    let resolved = platform.resolve(Some(&time));
    assert_eq!(resolved.profile().unimproved_step_count_limit(), None);
    assert_eq!(resolved.profile().step_count_limit(), None);
    assert_eq!(resolved.profile().move_count_limit(), None);
    assert_eq!(platform.platform().unimproved_step_count_limit(), Some(4));
    assert!(
        platform
            .clone()
            .with_unimproved_step_limit(None)
            .unwrap()
            .platform()
            .unimproved_step_count_limit()
            .is_none()
    );
}
#[test]
fn request_stagnation_count_overrides_platform_time_and_binds_actual_solves() {
    let platform =
        TerminationResolver::new(Duration::from_secs(60), Some(Duration::ZERO), None, None)
            .unwrap();
    assert!(
        platform
            .clone()
            .with_unimproved_step_limit(Some(1))
            .is_err()
    );
    let resolved = platform.resolve(Some(&profile(Some(3))));
    assert_eq!(resolved.profile().unimproved_spent_limit(), None);
    assert_eq!(resolved.profile().unimproved_step_count_limit(), Some(3));
    assert!(!resolved.uses_diminished_returns());
    for mode in 0..6 {
        let mut plan = sequence();
        plan.limits.max_unimproved_steps = Some(0);
        plan.limits.max_unimproved_time = Some(Duration::ZERO);
        let out = resolved
            .with_sequence(plan, |plan| {
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
        assert_eq!(out.work.completed_steps, 3);
        assert_eq!(out.reason, SolverTermination::UnimprovedStepLimit);
    }
}
