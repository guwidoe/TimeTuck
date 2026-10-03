// Adapted from pinned Timefold Community TimeMillisSpentTerminationTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::time::Duration;
use timetuck::ElapsedTimeLimit;
fn assertions() {
    let limit = ElapsedTimeLimit::new(Duration::from_millis(1000));
    for (millis, reached, gradient) in [
        (0, false, 0.0),
        (100, false, 0.1),
        (500, false, 0.5),
        (700, false, 0.7),
        (1000, true, 1.0),
        (1200, true, 1.0),
    ] {
        let elapsed = Duration::from_millis(millis);
        assert_eq!(limit.is_reached(elapsed), reached);
        assert_eq!(limit.progress(elapsed), gradient);
    }
}
#[test]
fn solve_termination() {
    assertions()
}
#[test]
fn phase_termination() {
    assertions()
}
