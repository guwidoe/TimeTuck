//! Original cache invalidation and uncached-reference checks; no upstream credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    ops::{Add, Sub},
    time::Duration,
};
use timetuck::*;
thread_local! {
    static CONVERSIONS: Cell<usize> = const { Cell::new(0) };
    static PANIC_NEXT: Cell<bool> = const { Cell::new(false) };
    static PANIC_DROP_ZERO: Cell<bool> = const { Cell::new(false) };
}
#[derive(Clone)]
struct Level(i64);
impl ScoreLevel for Level {
    fn exact_decimal(&self) -> Option<Decimal> {
        CONVERSIONS.set(CONVERSIONS.get() + 1);
        assert!(!PANIC_NEXT.replace(false), "arithmetic callback");
        Some(Decimal::from(self.0))
    }
    fn zero() -> Self {
        Self(0)
    }
    fn numeric_cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
    fn progress_fraction(&self, target: &Self, current: &Self) -> f64 {
        self.0.progress_fraction(&target.0, &current.0)
    }
    fn cooling_loss(&self, candidate: &Self, temperature: &Self, gradient: f64) -> f64 {
        self.0.cooling_loss(&candidate.0, &temperature.0, gradient)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Counted(SimpleScore);
impl Drop for Counted {
    fn drop(&mut self) {
        if self.0 == SimpleScore::ZERO {
            assert!(!PANIC_DROP_ZERO.replace(false), "score destructor");
        }
    }
}
impl Score for Counted {
    type Level = Level;
    const HARD_LEVEL_COUNT: usize = 0;
    fn ordinary_levels(&self) -> impl Iterator<Item = Level> {
        self.0.ordinary_levels().map(Level)
    }
    fn structural(&self) -> i64 {
        self.0.structural()
    }
    fn hard_cmp(&self, other: &Self) -> Ordering {
        self.0.hard_cmp(&other.0)
    }
    fn is_feasible(&self) -> bool {
        self.0.is_feasible()
    }
}
impl Add for Counted {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}
impl Sub for Counted {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}
impl AdditiveScore for Counted {
    const ZERO: Self = Self(SimpleScore::ZERO);
    fn with_structural(self, structural: i64) -> Self {
        Self(self.0.with_structural(structural))
    }
}
fn score(value: i64) -> Counted {
    Counted(SimpleScore::new(value))
}
fn ns(value: u64) -> Duration {
    Duration::from_nanos(value)
}
fn rate(numerator: u64, denominator: u64) -> DiminishedReturns<Counted> {
    DiminishedReturns::new(DiminishedReturnsConfig::new(ns(100), numerator, denominator).unwrap())
        .unwrap()
}

#[test]
fn unchanged_polls_reuse_arithmetic_but_clock_eviction_and_score_changes_invalidate_it() {
    let mut rate = rate(1, 2);
    rate.start(ns(1000), score(0));
    assert!(!rate.is_terminated(ns(1100), &score(100))); // Grace establishes reference.
    CONVERSIONS.set(0);
    assert!(!rate.is_terminated(ns(1100), &score(100)));
    assert_eq!(CONVERSIONS.get(), 2);
    for now in 1100..1200 {
        assert!(!rate.is_terminated(ns(now), &score(100)));
        assert!(!rate.is_terminated(ns(now - 30), &score(100))); // Backward samples clamp.
    }
    assert_eq!(CONVERSIONS.get(), 2);
    rate.step(ns(1199), &score(140));
    assert!(!rate.is_terminated(ns(1199), &score(140)));
    assert_eq!(CONVERSIONS.get(), 4);
    // Replacing a same-timestamp sample also invalidates the comparison.
    rate.step(ns(1199), &score(150));
    assert!(!rate.is_terminated(ns(1199), &score(150)));
    assert_eq!(CONVERSIONS.get(), 6);
    // Predecessor changes from score0 to score100 despite an unchanged best.
    assert!(!rate.is_terminated(ns(1200), &score(150))); // Equality continues.
    assert_eq!(CONVERSIONS.get(), 8);
    for now in 1200..1299 {
        assert!(!rate.is_terminated(ns(now), &score(150)));
    }
    assert_eq!(CONVERSIONS.get(), 8);
    assert!(rate.is_terminated(ns(1299), &score(150)));
    assert_eq!(CONVERSIONS.get(), 10);
    assert!(rate.is_terminated(ns(2000), &score(999)));
    assert_eq!(CONVERSIONS.get(), 10); // Expiry remains latched.
}

#[test]
fn grace_reset_phase_rearming_and_panicking_arithmetic_cannot_reuse_stale_results() {
    let mut rate = rate(2, 1);
    rate.start(ns(0), score(0));
    assert!(!rate.is_terminated(ns(99), &score(1)));
    assert!(!rate.is_terminated(ns(100), &score(100))); // Grace transition returns false.
    assert!(rate.is_terminated(ns(100), &score(100))); // Rate above100% terminates next poll.
    rate.phase_started(score(5));
    assert!(!rate.is_terminated(ns(10000), &score(5)));
    rate.step_started(ns(10000));
    assert!(!rate.is_terminated(ns(10099), &score(5)));
    assert!(rate.is_terminated(ns(10100), &score(5)));

    let mut rate = self::rate(1, 2);
    rate.start(ns(0), score(0).with_structural(-1));
    assert!(!rate.is_terminated(ns(100), &score(100).with_structural(-1)));
    assert!(!rate.is_terminated(ns(101), &score(100).with_structural(-1)));
    assert!(!rate.is_terminated(ns(102), &score(0))); // Structural improvement resets.
    assert_eq!(rate.grace_start(), Some(ns(102)));
    assert!(!rate.is_terminated(ns(202), &score(100)));
    PANIC_NEXT.set(true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || rate.is_terminated(ns(202), &score(100))
        ))
        .is_err()
    );
    CONVERSIONS.set(0);
    assert!(!rate.is_terminated(ns(202), &score(100)));
    assert_eq!(CONVERSIONS.get(), 2); // Failed calculation never marks the cache clean.
    assert!(!rate.is_terminated(ns(203), &score(100)));
    assert_eq!(CONVERSIONS.get(), 2);
    rate.start(ns(500), score(20));
    assert!(rate.is_terminated(ns(600), &score(20))); // Explicit restart also clears it.
}

#[test]
fn repeated_poll_sequences_match_full_history_uncached_reference() {
    // The reference stores every observation, rescans history and recomputes the
    // integer inequality on every poll; it has no eviction or decision cache.
    for ratio in [(1_u64, 2_u64), (1, 1), (2, 1), (7, 100)] {
        for window in [1_u64, 17, 100] {
            for seed in 0..32_u64 {
                let config = DiminishedReturnsConfig::new(ns(window), ratio.0, ratio.1).unwrap();
                let mut actual = DiminishedReturns::new(config).unwrap();
                let mut now = 0_u64;
                let mut high_water = 0;
                let mut best = 0_i64;
                let mut history = vec![(0, 0_i64)];
                let mut reference = None;
                let mut expired = false;
                actual.start(ns(0), SimpleScore::ZERO);
                let mut random = seed + 1;
                for iteration in 0..1000 {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    now += (random >> 32) % 3;
                    let sampled = if iteration % 11 == 0 {
                        now.saturating_sub(17)
                    } else {
                        now
                    };
                    if random % 13 == 0 {
                        best += ((random >> 16) % 20) as i64;
                    }
                    if !expired {
                        high_water = high_water.max(sampled);
                        history.push((high_water, best));
                        if high_water >= window {
                            if let Some((gain, period)) = reference {
                                let predecessor = history
                                    .iter()
                                    .rev()
                                    .find(|(t, _)| *t <= high_water - window)
                                    .unwrap()
                                    .1;
                                expired = i128::from(best - predecessor)
                                    * i128::from(period)
                                    * i128::from(ratio.1)
                                    < i128::from(gain) * i128::from(window) * i128::from(ratio.0);
                            } else {
                                expired = best <= 0;
                                reference = Some((best, high_water));
                            }
                        }
                    }
                    assert_eq!(
                        actual.is_terminated(ns(sampled), &SimpleScore::new(best)),
                        expired,
                        "seed{seed} window{window} ratio{ratio:?} iteration{iteration}"
                    );
                }
            }
        }
    }
}

#[test]
fn panicking_evicted_score_does_not_leave_the_old_comparison_cached() {
    let mut rate = rate(1, 2);
    rate.start(ns(0), score(0));
    assert!(!rate.is_terminated(ns(100), &score(100)));
    assert!(!rate.is_terminated(ns(199), &score(140)));
    // Eviction changes predecessor0 to predecessor100 before its destructor
    // panics. A resumed poll must compare gain40, not reuse the old gain140.
    PANIC_DROP_ZERO.set(true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rate.is_terminated(ns(200), &score(140))
        }))
        .is_err()
    );
    CONVERSIONS.set(0);
    assert!(rate.is_terminated(ns(200), &score(140)));
    assert_eq!(CONVERSIONS.get(), 2);
}
