//! Original widened-arithmetic, clock and threshold regressions.
use std::time::Duration;
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
#[test]
fn accumulated_changes_reset_only_at_the_threshold_and_never_after_expiry() {
    let mut p = SignificantImprovement::new(
        SignificantImprovementConfig::new(ms(10), SimpleScore::new(7)).unwrap(),
    );
    assert!(!p.is_terminated(ms(999), &SimpleScore::new(100)));
    assert_eq!(p.progress(ms(999)), 0.0);
    p.start(ms(1000), SimpleScore::new(0));
    assert!(!p.is_terminated(ms(1005), &SimpleScore::new(4)));
    assert_eq!(p.progress(ms(1005)), 0.5);
    assert!(!p.is_terminated(ms(1010), &SimpleScore::new(7)));
    assert_eq!(p.progress(ms(1010)), 0.0);
    assert!(!p.is_terminated(ms(1019), &SimpleScore::new(13)));
    assert!((p.progress(ms(1019)) - 0.9).abs() < 1e-15);
    assert!(p.is_terminated(ms(1021), &SimpleScore::new(100)));
    assert!(p.is_terminated(ms(1), &SimpleScore::new(1000)));
    assert_eq!(p.progress(ms(1)), 1.0);
    p.start(ms(0), SimpleScore::new(50));
    assert!(!p.is_terminated(ms(0), &SimpleScore::new(50)));
}
#[test]
fn exact_differences_and_ignored_suffixes_handle_extreme_deterioration() {
    let finite = SignificantImprovementConfig::new(ms(10), HardSoftScore::new(1, 0)).unwrap();
    let prefix = finite.ignoring_trailing_levels(1).unwrap();
    let from = HardSoftScore::new(i64::MIN, i64::MAX);
    let to = HardSoftScore::new(i64::MIN + 1, i64::MIN);
    assert!(!finite.is_met(&from, &to));
    assert!(prefix.is_met(&from, &to));
    let large = SignificantImprovementConfig::new(ms(1), SimpleScore::new(i64::MAX)).unwrap();
    assert!(large.is_met(&SimpleScore::new(i64::MIN), &SimpleScore::new(0)));
    assert!(!large.is_met(&SimpleScore::new(1), &SimpleScore::new(i64::MAX)));
    let hard = SignificantImprovementConfig::new(ms(1), HardSoftScore::new(0, 100)).unwrap();
    assert!(hard.is_met(&HardSoftScore::ZERO, &HardSoftScore::new(1, -900)));
    assert!(!finite.is_met(&HardSoftScore::ZERO, &HardSoftScore::new(1, -100)));
    assert!(prefix.is_met(&HardSoftScore::ZERO, &HardSoftScore::new(1, -100)));
}
#[test]
fn decimal_thresholds_do_not_round_small_changes_or_wrap_large_ones() {
    let s = |v: &str| v.parse::<SimpleDecimalScore>().unwrap();
    let c = SignificantImprovementConfig::new(ms(1), s("0.000000000000000002")).unwrap();
    let old = s("9007199254740992.000000000000000000");
    assert!(!c.is_met(&old, &s("9007199254740992.0000000000000000019")));
    assert!(c.is_met(&old, &s("9007199254740992.000000000000000002")));
    let mut p = SignificantImprovement::new(c);
    p.start(ms(0), old);
    assert!(!p.is_terminated(ms(1), &s("9007199254740992.000000000000000002")));
    assert_eq!(p.progress(ms(1)), 0.0);
    assert!(p.is_terminated(ms(3), &s("9007199254740992.000000000000000100")));
}
#[test]
fn zero_negative_minima_zero_windows_and_duration_extremes_are_explicit() {
    for minimum in [0, -1, i64::MIN] {
        let c =
            SignificantImprovementConfig::new(Duration::ZERO, SimpleScore::new(minimum)).unwrap();
        assert!(!c.is_met(&SimpleScore::new(3), &SimpleScore::new(3)));
        assert!(!c.is_met(&SimpleScore::new(3), &SimpleScore::new(2)));
        assert!(c.is_met(&SimpleScore::new(3), &SimpleScore::new(4)));
        let mut p = SignificantImprovement::new(c);
        p.start(ms(5), SimpleScore::new(3));
        assert!(!p.is_terminated(ms(5), &SimpleScore::new(4)));
        assert_eq!(p.progress(ms(5)), 1.0);
        assert!(p.is_terminated(ms(6), &SimpleScore::new(5)));
    }
    let mut p = SignificantImprovement::new(
        SignificantImprovementConfig::new(Duration::MAX, SimpleScore::new(1)).unwrap(),
    );
    p.start(Duration::ZERO, SimpleScore::ZERO);
    assert!(!p.is_terminated(Duration::MAX, &SimpleScore::ZERO));
    assert_eq!(p.progress(Duration::MAX), 1.0);
    assert!(!p.is_terminated(Duration::ZERO, &SimpleScore::new(1)));
    assert_eq!(p.progress(Duration::MAX), 0.0);
}
#[test]
fn validates_prefix_width_and_structural_threshold_and_resets_on_initialization() {
    assert!(matches!(
        SignificantImprovementConfig::new(ms(1), BendableScore::<0, 0>::ZERO),
        Err(SignificantImprovementError::NoScoreLevels)
    ));
    for structural in [-1, 1] {
        assert!(matches!(
            SignificantImprovementConfig::new(ms(1), SimpleScore::ZERO.with_structural(structural)),
            Err(SignificantImprovementError::FlawedThreshold)
        ));
    }
    let c = SignificantImprovementConfig::new(ms(1), HardSoftScore::new(1, 0)).unwrap();
    assert_eq!(c.compared_levels(), 2);
    assert_eq!(c.window(), ms(1));
    assert_eq!(c.minimum(), &HardSoftScore::new(1, 0));
    assert!(matches!(
        c.ignoring_trailing_levels(2),
        Err(SignificantImprovementError::InvalidIgnoredLevels)
    ));
    assert!(matches!(
        c.ignoring_trailing_levels(usize::MAX),
        Err(SignificantImprovementError::InvalidIgnoredLevels)
    ));
    assert_eq!(
        c.ignoring_trailing_levels(1)
            .unwrap()
            .ignoring_trailing_levels(0)
            .unwrap()
            .compared_levels(),
        2
    );
    assert!(c.is_met(
        &HardSoftScore::new(0, 100).with_structural(-1),
        &HardSoftScore::new(-100, -100)
    ));
}
#[test]
fn generated_histories_match_widened_deadline_reference() {
    let mut seed = 971_u64;
    let mut random = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for window in [0, 1, 3, 20] {
        for minimum in [-5, 0, 1, 7, 100] {
            for _ in 0..32 {
                let mut p = SignificantImprovement::new(
                    SignificantImprovementConfig::new(ms(window), SimpleScore::new(minimum))
                        .unwrap(),
                );
                let mut anchor = 0_i128;
                let mut anchor_time = 0_u64;
                let mut now = 0;
                let mut best = 0_i64;
                let mut done = false;
                p.start(ms(0), SimpleScore::ZERO);
                for _ in 0..100 {
                    now += random() % 4;
                    best += (random() % 5) as i64;
                    if now - anchor_time > window {
                        done = true;
                    }
                    if !done
                        && i128::from(best) > anchor
                        && i128::from(best) - anchor >= i128::from(minimum)
                    {
                        anchor = i128::from(best);
                        anchor_time = now;
                    }
                    assert_eq!(p.is_terminated(ms(now), &SimpleScore::new(best)), done);
                    let gradient = if window == 0 {
                        1.0
                    } else {
                        ((now - anchor_time) as f64 / window as f64).min(1.0)
                    };
                    assert!((p.progress(ms(now)) - gradient).abs() < 1e-15);
                }
            }
        }
    }
}

#[test]
fn unsupported_level_arithmetic_is_rejected_when_building_configuration() {
    #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Level(i64);
    impl ScoreLevel for Level {
        fn zero() -> Self {
            Self(0)
        }
        fn numeric_cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.cmp(other)
        }
        fn progress_fraction(&self, _: &Self, _: &Self) -> f64 {
            0.0
        }
        fn cooling_loss(&self, _: &Self, _: &Self, _: f64) -> f64 {
            0.0
        }
    }
    #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct CustomScore(Level);
    impl Score for CustomScore {
        type Level = Level;
        const HARD_LEVEL_COUNT: usize = 0;
        fn ordinary_levels(&self) -> impl Iterator<Item = Level> {
            std::iter::once(self.0.clone())
        }
        fn structural(&self) -> i64 {
            0
        }
        fn hard_cmp(&self, _: &Self) -> std::cmp::Ordering {
            std::cmp::Ordering::Equal
        }
        fn is_feasible(&self) -> bool {
            true
        }
    }
    assert!(matches!(
        SignificantImprovementConfig::new(ms(1), CustomScore(Level(1))),
        Err(SignificantImprovementError::UnsupportedArithmetic)
    ));
}
