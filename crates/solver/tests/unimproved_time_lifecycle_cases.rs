// Adapted from pinned Community UnimprovedTimeMillisSpentTerminationTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::time::Duration;
use timetuck::*;
fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
#[test]
fn for_negative_unimproved_time_millis_exception_is_thrown() {
    assert!(
        UnimprovedTimeLimit::from_millis(-1)
            .unwrap_err()
            .to_string()
            .contains("cannot be negative")
    );
}
fn active_observations(timer: &mut UnimprovedTimeLimit) {
    timer.record_improvement(ms(500));
    assert!(!timer.is_reached(ms(1000)));
    assert_eq!(timer.progress(ms(1000)), 0.5);
    timer.record_improvement(ms(1000));
    assert!(timer.is_reached(ms(2000)));
    assert_eq!(timer.progress(ms(2000)), 1.0);
}
#[test]
fn solver_termination() {
    let mut timer = UnimprovedTimeLimit::new(ms(1000));
    timer.start(ms(0));
    active_observations(&mut timer);
}
#[test]
fn phase_termination() {
    let mut timer = UnimprovedTimeLimit::new(ms(1000));
    // Native start is the first improvement-search step, after phase setup.
    timer.start(ms(0));
    active_observations(&mut timer);
}
#[test]
fn solver_termination_with_construction_heuristic() {
    let mut timer = UnimprovedTimeLimit::new(ms(1000));
    for now in [1000, 2000] {
        assert!(!timer.is_reached(ms(now)));
        assert_eq!(timer.progress(ms(now)), 0.0);
    }
    // Construction ends; start the improvement interval at phase entry.
    timer.start(ms(2000));
    timer.record_improvement(ms(2500));
    assert!(!timer.is_reached(ms(3000)));
    assert_eq!(timer.progress(ms(3000)), 0.5);
    timer.record_improvement(ms(3000));
    assert!(timer.is_reached(ms(4000)));
    assert_eq!(timer.progress(ms(4000)), 1.0);
    timer.reset();
}
#[test]
fn phase_termination_with_construction_heuristic() {
    let mut timer = UnimprovedTimeLimit::new(ms(1000));
    timer.record_improvement(ms(500));
    assert!(!timer.is_reached(ms(1000)));
    assert_eq!(timer.progress(ms(1000)), 0.0);
    timer.record_improvement(ms(1000));
    assert!(!timer.is_reached(ms(2000)));
    assert_eq!(timer.progress(ms(2000)), 0.0);
}
#[test]
fn phase_termination_and_long_initialization_period() {
    let mut timer = UnimprovedTimeLimit::new(ms(1000));
    assert!(!timer.is_reached(ms(10000)));
    timer.start(ms(10000));
    timer.record_improvement(ms(10500));
    assert!(!timer.is_reached(ms(10500)));
    assert!(timer.is_reached(ms(11600)));
}
