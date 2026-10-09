// Adapted from Timefold Community BestSolutionRecallerTest (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn check(before: (usize, i64), candidate: (usize, i64), improved: bool) {
    let mut tracker = BestSolutionTracker::new("original", SimpleScore::new(before.1), before.0);
    assert_eq!(
        tracker.consider(SimpleScore::new(candidate.1), candidate.0, || "candidate"),
        improved
    );
    let expected = if improved { candidate } else { before };
    assert_eq!(
        tracker.best().assignment,
        if improved { "candidate" } else { "original" }
    );
    assert_eq!(tracker.best().score, SimpleScore::new(expected.1));
    assert_eq!(tracker.best().required_unassigned, expected.0);
}
#[test]
fn unimproved_uninitialized_during_step() {
    check((1, -300), (2, 0), false);
}
#[test]
fn unimproved_initialized_during_step() {
    check((0, 0), (0, -1), false);
}
#[test]
fn improved_uninitialized_during_step() {
    check((2, 0), (1, 0), true);
}
#[test]
fn improved_initialized_during_step() {
    check((0, -1), (0, 0), true);
}
#[test]
fn unimproved_uninitialized_during_move() {
    check((0, -10), (1, -1), false);
}
#[test]
fn unimproved_initialized_during_move() {
    check((0, 0), (0, -1), false);
}
#[test]
fn improved_uninitialized_during_move() {
    check((1, 0), (0, -2), true);
}
#[test]
fn improved_initialized_during_move() {
    check((0, -2), (0, -1), true);
}
