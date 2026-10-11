//! Original phase-profile integration across all native model/backend paths.
#![allow(dead_code)]
include!("solver_support/fixture.rs");
use std::time::Duration;
fn profile(steps: Option<i64>, moves: Option<i64>) -> TerminationProfile {
    TerminationProfile::new(None, None, steps, moves, None, None).unwrap()
}
#[test]
fn phase_steps_reset_while_shared_budget_spans_all_six_paths() {
    let construction = profile(Some(1), None)
        .bind_construction(ConstructionConfig::default())
        .unwrap();
    let local = profile(Some(2), None).bind_local_search(LocalSearchConfig {
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::FirstAccepted,
        ..Default::default()
    });
    for mode in 0..6 {
        let shared = SearchTermination::Condition(SearchLimit::Steps(3));
        let r = run_scored(
            (mode, false, MixedConstructionOrder::BasicThenLists),
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&shared),
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::Construction(construction),
                    SolverPhase::Construction(construction),
                    SolverPhase::LocalSearch(local.clone()),
                    SolverPhase::LocalSearch(local.clone()),
                ],
            },
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        assert!(r.complete, "mode {mode}");
        assert_eq!(r.work.completed_steps, 3, "mode {mode}");
        assert_eq!(r.phases[2].work.completed_steps, 2);
        assert_eq!(r.phases[3].work.completed_steps, 1);
        assert_eq!(r.reason, SolverTermination::Condition);
    }
}
#[test]
fn explicit_zero_moves_stops_construction_in_all_six_paths() {
    let config = profile(None, Some(0))
        .bind_construction(ConstructionConfig::default())
        .unwrap();
    for mode in 0..6 {
        let r = run_scored(
            (mode, false, MixedConstructionOrder::BasicThenLists),
            SolverConfig {
                construction: config,
                ..Default::default()
            },
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        assert!(!r.complete);
        assert_eq!(r.work.move_evaluations, 0);
        assert_eq!(r.work.construction_steps, 0);
    }
}
#[test]
fn absent_fields_preserve_controls_and_explicit_settings_replace_them() {
    let token = CancellationToken::new();
    let construction = ConstructionConfig {
        cancellation: Some(&token),
        max_evaluations: 97,
        max_time: Some(Duration::from_secs(2)),
        max_steps: Some(13),
        max_move_evaluations: Some(19),
        interruption: ConstructionInterruption::DiscardIncompleteStep,
        ..Default::default()
    };
    assert_eq!(
        TerminationProfile::default()
            .bind_construction(construction)
            .unwrap(),
        construction
    );
    let replaced = profile(Some(0), Some(0))
        .bind_construction(construction)
        .unwrap();
    assert_eq!(replaced.max_steps, Some(0));
    assert_eq!(replaced.max_move_evaluations, Some(0));
    assert_eq!(replaced.max_evaluations, 97);
    assert_eq!(replaced.max_time, construction.max_time);
    assert_eq!(replaced.interruption, construction.interruption);
    assert!(std::ptr::eq(replaced.cancellation.unwrap(), &token));
    let local = LocalSearchConfig::<SimpleScore> {
        cancellation: Some(&token),
        max_evaluations: 97,
        max_steps: 13,
        max_unimproved_steps: Some(7),
        max_unimproved_time: Some(Duration::from_secs(2)),
        ..Default::default()
    };
    assert_eq!(
        TerminationProfile::default().bind_local_search(local.clone()),
        local
    );
    let p = TerminationProfile::new(
        Some(Duration::ZERO),
        Some(Duration::from_millis(1)),
        None,
        None,
        Some(Duration::from_secs(3)),
        Some((1, 2)),
    )
    .unwrap();
    let bound = p.bind_local_search(local);
    assert_eq!(bound.max_time, Some(Duration::ZERO));
    assert_eq!(bound.max_unimproved_time, Some(Duration::from_millis(1)));
    assert_eq!(bound.diminished_returns, p.diminished_returns());
    assert_eq!(bound.max_evaluations, 97);
    assert_eq!(bound.max_steps, 13);
    assert_eq!(bound.max_unimproved_steps, Some(7));
}
