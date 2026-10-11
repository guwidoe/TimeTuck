//! Original normalization, inheritance and runtime budget regressions.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn units(milliseconds: u64) -> TimeLimitSpec {
    TimeLimitSpec::Units(TimeLimitUnits {
        milliseconds: Some(milliseconds),
        ..Default::default()
    })
}
#[test]
fn empty_units_clear_settings_while_explicit_zero_suppresses_fallback() {
    let parent = TerminationProfile::default()
        .with_spent_time(Some(units(10)))
        .unwrap();
    for empty in [None, Some(TimeLimitSpec::Units(Default::default()))] {
        let child = parent.clone().with_spent_time(empty).unwrap();
        assert!(!child.is_configured());
        assert_eq!(child.spent_time_spec(), None);
        assert_eq!(
            child.with_fallback(Some(&parent)).unwrap().spent_limit(),
            parent.spent_limit()
        );
    }
    let child = TerminationProfile::default()
        .with_spent_time(Some(units(0)))
        .unwrap();
    assert!(child.is_configured());
    assert_eq!(
        child.with_fallback(Some(&parent)).unwrap().spent_limit(),
        Some(Duration::ZERO)
    );
}
#[test]
fn groups_merge_independently_preserving_inputs_and_nanosecond_precision() {
    let parent = TerminationProfile::default()
        .with_spent_time(Some(units(10)))
        .unwrap()
        .with_unimproved_time(Some(units(3)))
        .unwrap();
    let child = TerminationProfile::default()
        .with_spent_time(Some(TimeLimitSpec::Duration(Duration::from_nanos(7))))
        .unwrap();
    let before = (parent.clone(), child.clone());
    let merged = child.with_fallback(Some(&parent)).unwrap();
    assert_eq!(merged.spent_limit(), Some(Duration::from_nanos(7)));
    assert_eq!(merged.unimproved_time_spec(), Some(units(3)));
    assert_eq!((parent.clone(), child.clone()), before);
    let cleared = merged.into_owned().with_unimproved_time(None).unwrap();
    assert_eq!(cleared.unimproved_time_spec(), None);
    assert_eq!(cleared.unimproved_spent_limit(), None);
}
#[test]
fn overflow_and_existing_count_conflicts_are_rejected_before_binding() {
    let too_large = TimeLimitSpec::Units(TimeLimitUnits {
        days: Some(u64::MAX),
        ..Default::default()
    });
    assert_eq!(
        too_large.resolve(),
        Err(TerminationProfileError::TimeOverflow)
    );
    assert_eq!(
        TerminationProfile::default().with_spent_time(Some(too_large)),
        Err(TerminationProfileError::TimeOverflow)
    );
    let sum_overflow = TimeLimitSpec::Units(TimeLimitUnits {
        seconds: Some(u64::MAX),
        milliseconds: Some(1000),
        ..Default::default()
    });
    assert_eq!(
        sum_overflow.resolve(),
        Err(TerminationProfileError::TimeOverflow)
    );
    assert_eq!(
        TimeLimitSpec::Duration(Duration::MAX).resolve(),
        Ok(Some(Duration::MAX))
    );
    for (steps, moves) in [(Some(0), None), (None, Some(0))] {
        let p = TerminationProfile::new(None, None, steps, moves, None, None).unwrap();
        assert!(matches!(
            p.with_unimproved_time(Some(units(1))),
            Err(TerminationProfileError::ConflictingLimits { .. })
        ));
    }
    let child = TerminationProfile::new(None, None, Some(1), None, None, None).unwrap();
    let parent = TerminationProfile::default()
        .with_unimproved_time(Some(units(5)))
        .unwrap();
    let original = (child.clone(), parent.clone());
    assert!(child.with_fallback(Some(&parent)).is_err());
    assert_eq!((child, parent), original);
}
#[test]
fn configured_time_budgets_match_direct_limits_in_real_clocked_solves() {
    for unimproved in [false, true] {
        for representation in [units(5), TimeLimitSpec::Duration(Duration::from_millis(5))] {
            let parent = if unimproved {
                TerminationProfile::default().with_unimproved_time(Some(representation))
            } else {
                TerminationProfile::default().with_spent_time(Some(representation))
            }
            .unwrap();
            let child = TerminationProfile::default();
            let inherited = child.with_fallback(Some(&parent)).unwrap();
            let actual = inherited.with_limits(SolverLimits::default(), clocked_solve);
            let reference = clocked_solve(SolverLimits {
                max_time: (!unimproved).then_some(Duration::from_millis(5)),
                max_unimproved_time: unimproved.then_some(Duration::from_millis(5)),
                ..Default::default()
            });
            assert_eq!(actual.assignment, reference.assignment);
            assert_eq!(actual.score, reference.score);
            assert_eq!(actual.work, reference.work);
            assert_eq!(actual.phases, reference.phases);
            assert_eq!(actual.termination, reference.termination);
            assert!(actual.work.evaluations <= 2);
            assert!(matches!(
                actual.termination,
                SolverTermination::TimeLimit | SolverTermination::UnimprovedTimeLimit
            ));
        }
    }
}

fn clocked_solve(limits: SolverLimits<'_>) -> SolverResult<Assignment<i32>, SimpleScore> {
    let ticks = Arc::new(AtomicU64::new(100));
    let read = ticks.clone();
    let clock = SolverClock::new(move || Duration::from_millis(read.load(Ordering::SeqCst)));
    let problem =
        FiniteProblem::new(vec![Variable::new(vec![0_i32, 1]).with_initial(0); 3]).unwrap();
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
