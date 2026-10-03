//! Original threshold/counter regressions; no upstream method credit.
use timetuck::*;
fn s(v: i64) -> SimpleScore {
    SimpleScore::new(v)
}
const MODES: [StepCountingMode; 5] = [
    StepCountingMode::Step,
    StepCountingMode::EqualOrImprovingStep,
    StepCountingMode::ImprovingStep,
    StepCountingMode::SelectedMove,
    StepCountingMode::AcceptedMove,
];
#[test]
fn trial_modes_count_repeated_checks_but_replace_thresholds_only_at_step_end() {
    for mode in [
        StepCountingMode::SelectedMove,
        StepCountingMode::AcceptedMove,
    ] {
        let mut a = StepCountingHillClimbing::new(2, mode, s(0)).unwrap();
        assert!(!a.accepts(s(0), s(-1)));
        assert_eq!(a.count(), u64::from(mode == StepCountingMode::SelectedMove));
        assert!(a.accepts(s(0), s(2)));
        assert!(a.accepts(s(0), s(3)));
        for _ in 0..10 {
            assert!(a.accepts(s(0), s(1)));
        }
        assert_eq!(a.count(), 2);
        assert_eq!(a.threshold(), s(0));
        a.step_ended(s(3));
        assert_eq!(a.count(), 0);
        assert_eq!(a.threshold(), s(3));
        // Excess trials from the prior scan do not carry across the boundary.
        assert!(a.accepts(s(3), s(5)));
        a.step_ended(s(5));
        assert_eq!(a.threshold(), s(3));
        assert_eq!(a.count(), 1);
    }
}
#[test]
fn acceptance_keeps_the_nonworsening_path_after_rejected_fallback() {
    for mode in MODES {
        let mut a = StepCountingHillClimbing::new(10, mode, s(10)).unwrap();
        assert!(!a.accepts(s(10), s(3)));
        a.step_ended(s(3));
        assert_eq!(a.threshold(), s(10));
        assert!(a.accepts(s(3), s(3)));
        assert!(a.accepts(s(3), s(4)));
        assert!(!a.accepts(s(3), s(2)));
    }
}
#[test]
fn qualifying_steps_compare_to_previous_step_instead_of_global_best() {
    for (mode, expected) in [
        (StepCountingMode::Step, 5),
        (StepCountingMode::EqualOrImprovingStep, 5),
        (StepCountingMode::ImprovingStep, 0),
    ] {
        let mut a = StepCountingHillClimbing::new(3, mode, s(0)).unwrap();
        a.step_ended(s(10));
        a.step_ended(s(4));
        a.step_ended(s(5));
        if mode == StepCountingMode::Step {
            assert_eq!(a.threshold(), s(5));
        } else {
            assert_eq!(a.threshold(), s(0));
        }
        a.step_ended(s(5));
        assert_eq!(a.threshold(), s(expected));
        a.step_ended(s(6));
        if mode == StepCountingMode::ImprovingStep {
            assert_eq!(a.threshold(), s(6));
        }
    }
}
#[test]
fn maximum_period_does_not_allocate_and_owned_scores_use_lexicographic_order() {
    let initial: HardSoftDecimalScore = "-1hard/100.25soft".parse().unwrap();
    let higher: HardSoftDecimalScore = "0hard/-100.50soft".parse().unwrap();
    let lower: HardSoftDecimalScore = "-2hard/999.50soft".parse().unwrap();
    for mode in MODES {
        let mut a = StepCountingHillClimbing::new(u64::MAX, mode, initial.clone()).unwrap();
        assert!(a.accepts(initial.clone(), higher.clone()));
        a.step_ended(higher.clone());
        assert!(a.accepts(higher.clone(), initial.clone()));
        assert!(!a.accepts(higher.clone(), lower.clone()));
        assert_eq!(a.threshold(), initial);
        let mut b = StepCountingHillClimbing::new(1, mode, initial.clone()).unwrap();
        assert!(b.accepts(initial.clone(), higher.clone()));
        b.step_ended(higher.clone());
        assert_eq!(b.threshold(), higher);
    }
}
#[test]
fn cloned_and_fresh_acceptance_sessions_have_independent_counters() {
    for mode in MODES {
        let config = Acceptance::StepCountingHillClimbing { period: 1, mode };
        let mut a = config.start(s(0)).unwrap();
        let mut b = a.clone();
        assert!(a.accepts(s(0), s(10), &[]));
        a.step_ended(s(10), &[]);
        assert!(!a.accepts(s(10), s(3), &[]));
        assert!(b.accepts(s(10), s(3), &[]));
        assert!(config.start(s(0)).unwrap().accepts(s(10), s(3), &[]));
    }
}
#[test]
fn zero_period_rejected_by_every_configuration_entry_point() {
    for mode in MODES {
        assert!(matches!(
            StepCountingHillClimbing::new(0, mode, s(0)),
            Err(AcceptanceError::ZeroStepCountingPeriod)
        ));
        let c = Acceptance::StepCountingHillClimbing { period: 0, mode };
        assert_eq!(c.validate(), Err(AcceptanceError::ZeroStepCountingPeriod));
        assert!(matches!(
            c.start(s(0)),
            Err(AcceptanceError::ZeroStepCountingPeriod)
        ));
    }
}
