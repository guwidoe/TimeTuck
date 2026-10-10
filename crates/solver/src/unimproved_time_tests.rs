//! Original timer lifecycle and actual phase-budget integration checks.
use super::*;
use crate::{LocalSearchTermination, time_limit::TimeBudget};
use std::cell::Cell;
fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
#[test]
fn phase_budget_does_not_arm_stagnation_during_initialization() {
    let now = Cell::new(ms(0));
    let mut budget = TimeBudget::<_, crate::SimpleScore>::new(None, Some(ms(1000)), || now.get());
    now.set(ms(10000));
    assert_eq!(budget.stop(), None);
    budget.record_improvement(); // Initialization events must not arm the timer.
    assert_eq!(budget.stop(), None);
    budget.start_search();
    now.set(ms(10500));
    assert_eq!(budget.stop(), None);
    budget.record_improvement();
    now.set(ms(11499));
    assert_eq!(budget.stop(), None);
    now.set(ms(11500));
    assert_eq!(
        budget.stop(),
        Some(LocalSearchTermination::UnimprovedTimeLimit)
    );
    budget.record_improvement();
    now.set(ms(11501));
    assert_eq!(
        budget.stop(),
        Some(LocalSearchTermination::UnimprovedTimeLimit)
    );
}
#[test]
fn elapsed_deadline_still_counts_initialization_and_precedes_stagnation() {
    let now = Cell::new(ms(0));
    let mut budget =
        TimeBudget::<_, crate::SimpleScore>::new(Some(ms(10)), Some(ms(1)), || now.get());
    now.set(ms(10));
    assert_eq!(budget.stop(), Some(LocalSearchTermination::TimeLimit));
    let mut budget =
        TimeBudget::<_, crate::SimpleScore>::new(Some(ms(10)), Some(ms(10)), || now.get());
    now.set(ms(0));
    budget.start_search();
    now.set(ms(10));
    assert_eq!(budget.stop(), Some(LocalSearchTermination::TimeLimit));
}
#[test]
fn inactive_zero_backward_samples_expiry_and_explicit_restart() {
    let mut zero = UnimprovedTimeLimit::new(Duration::ZERO);
    assert!(!zero.is_reached(Duration::MAX));
    assert_eq!(zero.progress(Duration::MAX), 0.0);
    zero.reset();
    zero.start(ms(0));
    assert!(zero.is_reached(ms(0)));
    let mut timer = UnimprovedTimeLimit::new(ms(10));
    timer.start(ms(100));
    assert_eq!(timer.progress(ms(105)), 0.5);
    assert_eq!(timer.progress(ms(102)), 0.5);
    timer.record_improvement(ms(103)); // Clamp to latest observation, 105.
    assert!(!timer.is_reached(ms(114)));
    assert!(timer.is_reached(ms(115)));
    timer.record_improvement(ms(116));
    assert!(timer.is_reached(ms(116)));
    assert_eq!(timer.progress(ms(116)), 1.0);
    timer.start(ms(116));
    assert!(!timer.is_reached(ms(125)));
    assert!(timer.is_reached(ms(126)));
    timer.reset();
    assert!(!timer.is_started());
    timer.start(Duration::MAX - ms(5));
    assert!(!timer.is_reached(Duration::MAX));
    assert_eq!(timer.progress(Duration::MAX), 0.5);
}
