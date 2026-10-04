// Adapted from pinned Timefold Community DiminishedReturnsTerminationTest.
// Apache-2.0; complete fixtures/assertions retained. See compat/provenance.json.
use std::time::Duration;
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn limit<S: AdditiveScore>() -> DiminishedReturns<S> {
    DiminishedReturns::new(DiminishedReturnsConfig::new(ms(10), 1, 1).unwrap()).unwrap()
}
#[test]
fn no_improvement_in_grace_terminates() {
    let mut t = limit();
    t.start(ms(0), SimpleScore::ZERO);
    assert!(t.is_terminated(ms(10), &SimpleScore::ZERO));
}
#[test]
fn terminates_when_score_does_not_improve() {
    let mut t = limit();
    t.start(ms(0), SimpleScore::ZERO);
    assert!(!t.is_terminated(ms(10), &SimpleScore::new(1)));
    let score = SimpleScore::new(2);
    t.step(ms(11), &score);
    for n in [11, 12, 15, 19, 20] {
        assert!(!t.is_terminated(ms(n), &score));
    }
    assert!(t.is_terminated(ms(21), &score));
    t.start(ms(0), SimpleScore::new(1));
    t.step(ms(1), &SimpleScore::new(1));
    assert!(t.is_terminated(ms(10), &SimpleScore::new(1)));
    assert!(t.is_terminated(ms(11), &SimpleScore::new(1)));
}
#[test]
fn terminates_when_improvement_does_not_meet_criteria() {
    let mut t = limit();
    t.start(ms(0), SimpleScore::ZERO);
    let score = SimpleScore::new(2);
    assert!(!t.is_terminated(ms(10), &score));
    let score = SimpleScore::new(4);
    t.step(ms(11), &score);
    for n in [11, 12, 15, 19, 20] {
        assert!(!t.is_terminated(ms(n), &score));
    }
    let score = SimpleScore::new(6);
    t.step(ms(21), &score);
    for n in [21, 30] {
        assert!(!t.is_terminated(ms(n), &score));
    }
    let score = SimpleScore::new(7);
    t.step(ms(21), &score);
    assert!(t.is_terminated(ms(31), &score));
}
fn resets<S: AdditiveScore>(before: impl Fn(i64) -> S, after: impl Fn(i64) -> S) {
    let mut t = limit();
    t.start(ms(0), before(0));
    assert!(!t.is_terminated(ms(10), &before(1)));
    let score = before(2);
    t.step(ms(11), &score);
    for n in [11, 12, 15, 19, 20] {
        assert!(!t.is_terminated(ms(n), &score));
    }
    let score = after(2);
    t.step(ms(21), &score);
    for n in [21, 30] {
        assert!(!t.is_terminated(ms(n), &score));
    }
    let score = after(4);
    t.step(ms(31), &score);
    for n in [31, 40] {
        assert!(!t.is_terminated(ms(n), &score));
    }
    let score = after(5);
    t.step(ms(31), &score);
    assert!(t.is_terminated(ms(41), &score));
}
#[test]
fn improvement_in_init_score_resets_grace() {
    resets(
        |v| SimpleScore::new(v).with_structural(-1),
        SimpleScore::new,
    );
}
#[test]
fn improvement_in_hard_score_resets_grace() {
    resets(|v| HardSoftScore::new(-1, v), |v| HardSoftScore::new(0, v));
}
fn during_grace<S: AdditiveScore>(initial: S, make: impl Fn(i64) -> S) {
    let mut t = limit();
    t.start(ms(0), initial);
    for (now, value) in [(5, -1), (10, 0), (15, 1)] {
        let score = make(value);
        t.step(ms(now), &score);
        assert!(!t.is_terminated(ms(now), &score));
    }
    for now in [16, 19] {
        assert!(!t.is_terminated(ms(now), &make(1)));
    }
    assert!(t.is_terminated(ms(20), &make(1)));
}
#[test]
fn improvement_in_hard_score_during_grace() {
    during_grace(HardSoftScore::new(-1, 0), |v| HardSoftScore::new(0, v));
}
#[test]
fn improvement_in_init_score_during_grace() {
    during_grace(SimpleScore::ZERO.with_structural(-1), SimpleScore::new);
}
#[test]
fn grace_period_and_long_initialization_period() {
    let mut t =
        DiminishedReturns::new(DiminishedReturnsConfig::new(ms(1000), 1, 1).unwrap()).unwrap();
    let score = SimpleScore::ZERO.with_structural(-1);
    t.phase_started(score);
    assert!(!t.grace_started());
    assert!(!t.is_terminated(ms(50_000), &score));
    // Explicit clock replaces mocked Java phase/step scope and wall-clock origin.
    t.step_started(ms(50_000));
    assert!(t.grace_started());
    let start = t.grace_start().unwrap();
    assert!(!t.is_terminated(start, &score));
    assert!(t.is_terminated(start + ms(2000), &score));
    t.step(start + ms(2000), &score);
    let current = t.grace_start();
    t.step_started(start + ms(3000));
    assert_eq!(t.grace_start(), current);
    t.step(start + ms(3000), &score);
    assert!(t.is_terminated(start + ms(11_000), &score));
}
