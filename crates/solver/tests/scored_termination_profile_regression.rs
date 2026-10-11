//! Original score-profile composition, binding and runtime regressions.
#![allow(dead_code)]
include!("solver_support/fixture.rs");
use std::{
    borrow::Cow,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn stagnation(n: u64) -> TerminationProfile {
    TerminationProfile::default()
        .with_unimproved_time(Some(TimeLimitSpec::Duration(ms(n))))
        .unwrap()
}
#[test]
fn construction_validates_thresholds_and_feasibility_without_rejecting_disabled_stops() {
    let empty = TerminationProfile::default();
    for requested in [None, Some(false)] {
        let p =
            ScoredTerminationProfile::<SimpleScore>::new(empty.clone(), None, requested).unwrap();
        assert_eq!(p.feasibility(), requested);
    }
    let hard = ScoredTerminationProfile::<HardSoftScore>::new(empty, None, Some(true)).unwrap();
    assert_eq!(hard.feasibility(), Some(true));
    let zero = ScoredTerminationProfile::new(stagnation(0), Some(SimpleScore::ZERO), None).unwrap();
    assert_eq!(
        zero.significant_improvement().unwrap().window(),
        Duration::ZERO
    );
    let error = ScoredTerminationProfile::new(
        stagnation(5),
        Some(SimpleScore::new(2).with_structural(-1)),
        None,
    )
    .unwrap_err();
    assert_eq!(
        error,
        ScoredTerminationProfileError::InvalidThreshold(
            SignificantImprovementError::FlawedThreshold
        )
    );
    assert!(error.source().is_some());
}
#[test]
fn overlay_selects_one_stagnation_variant_and_preserves_or_clears_explicit_controls() {
    let inherited = SignificantImprovementConfig::new(ms(9), SimpleScore::new(2)).unwrap();
    let old_condition = SearchTermination::Condition(SearchLimit::MoveEvaluations(100));
    let existing = SolverLimits {
        significant_improvement: Some(&inherited),
        max_unimproved_time: Some(ms(8)),
        max_time: Some(ms(80)),
        max_evaluations: 77,
        stop_when_feasible: true,
        termination: Some(&old_condition),
        ..Default::default()
    };
    let empty =
        ScoredTerminationProfile::<SimpleScore>::new(Default::default(), None, None).unwrap();
    empty.with_limits(existing, |limits| assert_eq!(limits, existing));
    let plain =
        ScoredTerminationProfile::<SimpleScore>::new(stagnation(5), None, Some(false)).unwrap();
    plain.with_limits(existing, |limits| {
        assert_eq!(limits.max_unimproved_time, Some(ms(5)));
        assert_eq!(limits.significant_improvement, None);
        assert!(!limits.stop_when_feasible);
        assert_eq!(limits.max_time, Some(ms(80)));
        assert_eq!(limits.max_evaluations, 77);
        assert_eq!(limits.termination, Some(&old_condition));
    });
    let significant =
        ScoredTerminationProfile::new(stagnation(5), Some(SimpleScore::new(1)), None).unwrap();
    significant.with_limits(existing, |limits| {
        assert_eq!(limits.max_unimproved_time, None);
        assert_eq!(
            limits.significant_improvement,
            significant.significant_improvement()
        );
        assert!(limits.stop_when_feasible);
        assert_eq!(limits.max_evaluations, 77);
    });
    significant.with_sequence(SolverSequence::default(), |sequence| {
        assert_eq!(
            sequence.limits.significant_improvement,
            significant.significant_improvement()
        );
        assert_eq!(sequence.limits.max_unimproved_time, None);
    });
    significant.with_mixed_sequence(MixedSolverSequence::<SimpleScore>::default(), |sequence| {
        assert_eq!(
            sequence.limits.significant_improvement,
            significant.significant_improvement()
        );
        assert_eq!(sequence.limits.max_unimproved_time, None);
    });
}
#[test]
fn prepared_steps_stop_real_solves_across_all_six_models_and_backends() {
    let profile = TerminationProfile::new(None, None, Some(3), None, None, None).unwrap();
    let p = ScoredTerminationProfile::<SimpleScore>::new(profile, None, Some(false)).unwrap();
    for mode in 0..6 {
        let actual = p
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
        assert_eq!(actual.reason, SolverTermination::Condition);
        assert_eq!(actual.work.completed_steps, 3);
        assert_eq!(actual.phases.len(), 2);
    }
}
fn clocked(limits: SolverLimits<'_>) -> SolverResult<Assignment<i32>, SimpleScore> {
    let ticks = Arc::new(AtomicU64::new(100));
    let read = ticks.clone();
    let clock = SolverClock::new(move || ms(read.load(Ordering::SeqCst)));
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 3]).unwrap();
    let mut network = ConstraintNetwork::new(3);
    network
        .add_constraint("clock", vec![0, 1, 2], |_| {
            ticks.fetch_add(7, Ordering::SeqCst);
            SimpleScore::ZERO
        })
        .unwrap();
    solve_with_extensions(
        &problem,
        &network,
        SolverSequence {
            limits,
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 100,
                ..Default::default()
            })],
        },
        SolverExtensions {
            clock: Some(&clock),
            ..Default::default()
        },
    )
    .unwrap()
}
#[test]
fn prepared_stagnation_matches_manually_bound_runtime_controls_with_injected_clock() {
    for threshold in [None, Some(SimpleScore::new(1))] {
        let profile = ScoredTerminationProfile::new(stagnation(5), threshold, None).unwrap();
        let actual = profile.with_limits(SolverLimits::default(), clocked);
        let significant =
            threshold.map(|minimum| SignificantImprovementConfig::new(ms(5), minimum).unwrap());
        let reference = clocked(SolverLimits {
            max_unimproved_time: threshold.is_none().then_some(ms(5)),
            significant_improvement: significant.as_ref(),
            ..Default::default()
        });
        assert_eq!(actual.assignment, reference.assignment);
        assert_eq!(actual.score, reference.score);
        assert_eq!(actual.work, reference.work);
        assert_eq!(actual.phases, reference.phases);
        assert_eq!(actual.termination, reference.termination);
        assert_eq!(
            actual.termination,
            if threshold.is_some() {
                SolverTermination::SignificantImprovementTimeLimit
            } else {
                SolverTermination::UnimprovedTimeLimit
            }
        );
        assert!(actual.work.evaluations < 8);
    }
}
#[test]
fn composition_handles_empty_nested_and_repeated_conditions_without_flattening() {
    for style in [TerminationComposition::Any, TerminationComposition::All] {
        assert!(SearchTermination::compose(&[], style).is_none());
    }
    let children = [
        SearchTermination::Condition(SearchLimit::Steps(2)),
        SearchTermination::Condition(SearchLimit::MoveEvaluations(1)),
    ];
    let inner = SearchTermination::All(&children);
    let one = [inner];
    let borrowed = SearchTermination::compose(&one, TerminationComposition::Any).unwrap();
    assert!(matches!(borrowed, Cow::Borrowed(_)));
    assert!(std::ptr::eq(borrowed.as_ref(), &one[0]));
    let repeated = [inner, inner];
    let all = SearchTermination::compose(&repeated, TerminationComposition::All).unwrap();
    let progress = SearchProgress {
        completed_steps: 2,
        move_evaluations: MoveEvaluationCounts::default(),
        ..Default::default()
    };
    assert!(!all.status(progress).unwrap().reached);
    let any = SearchTermination::compose(&children, Default::default()).unwrap();
    assert!(any.status(progress).unwrap().reached);
}
