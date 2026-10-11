//! Original clock representation and duration-boundary regressions.
use std::time::Duration;
use timetuck::*;
#[test]
fn duration_clock_checkpoints_wrap_grow_and_preserve_maximum_duration() {
    let ns = Duration::from_nanos;
    let mut ring = TimedScoreRing::<i32, Duration>::new(3).unwrap();
    for (t, s) in [(Duration::ZERO, 0), (ns(1), 1), (ns(2), 2)] {
        ring.push(t, s).unwrap();
    }
    assert_eq!(ring.retain_floor(ns(1)).unwrap(), (ns(1), &1));
    ring.push(Duration::from_secs(1), 3).unwrap();
    assert_eq!(ring.state().read, 1);
    assert_eq!(ring.state().write, 1);
    assert_eq!(ring.state().times, [Duration::from_secs(1), ns(1), ns(2)]);
    let mut restored = TimedScoreRing::from_state(ring.state()).unwrap();
    restored.push(Duration::MAX, 4).unwrap();
    assert_eq!(
        restored.state().times,
        [
            ns(1),
            ns(2),
            Duration::from_secs(1),
            Duration::MAX,
            Duration::ZERO,
            Duration::ZERO
        ]
    );
    assert_eq!(restored.state().read, 0);
    assert_eq!(restored.state().write, 4);
    assert_eq!(
        restored.retain_floor(Duration::MAX).unwrap(),
        (Duration::MAX, &4)
    );
    assert_eq!(restored.len(), 1);
    let before = restored.state();
    assert_eq!(
        restored.push(Duration::ZERO, 5),
        Err(TimedScoreRingError::BackwardsTime)
    );
    assert_eq!(restored.state(), before);
    assert_eq!(ring.len(), 3);
}
#[test]
fn generic_clock_default_is_padding_and_never_an_empty_sentinel() {
    #[derive(Clone, Copy, Default, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Tick(i64);
    let mut ring = TimedScoreRing::<_, Tick>::new(1).unwrap();
    ring.push(Tick(-5), 1).unwrap();
    ring.push(Tick::default(), 2).unwrap();
    ring.push(Tick(5), 3).unwrap();
    assert_eq!(ring.retain_floor(Tick::default()).unwrap(), (Tick(0), &2));
    ring.replace_latest_score(4).unwrap();
    let mut restored = TimedScoreRing::from_state(ring.state()).unwrap();
    assert_eq!(restored.peek().unwrap(), &2);
    assert_eq!(restored.retain_floor(Tick(5)).unwrap(), (Tick(5), &4));
    restored.clear();
    assert!(restored.state().times.iter().all(|t| *t == Tick(0)));
    assert!(restored.is_empty());
}
