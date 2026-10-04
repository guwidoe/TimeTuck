//! Original exact-arithmetic, history and independent-reference checks.
use std::time::Duration;
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn policy<S: AdditiveScore>(window: u64, n: u64, d: u64) -> DiminishedReturns<S> {
    DiminishedReturns::new(DiminishedReturnsConfig::new(ms(window), n, d).unwrap()).unwrap()
}
#[test]
fn invalid_configuration_and_empty_scores_fail_before_use() {
    assert_eq!(
        DiminishedReturnsConfig::new(Duration::ZERO, 1, 1),
        Err(DiminishedReturnsError::ZeroWindow)
    );
    for (n, d) in [(0, 1), (1, 0), (0, 0)] {
        assert_eq!(
            DiminishedReturnsConfig::new(ms(1), n, d),
            Err(DiminishedReturnsError::ZeroRatio)
        );
    }
    assert!(matches!(
        DiminishedReturns::<BendableScore<0, 0>>::new(
            DiminishedReturnsConfig::new(ms(1), 1, 1).unwrap()
        ),
        Err(DiminishedReturnsError::NoScoreLevels)
    ));
}
#[test]
fn integer_extremes_decimal_precision_and_duration_extremes_remain_exact() {
    let mut t = policy(10, 1, 2);
    t.start(ms(0), SimpleScore::new(i64::MIN));
    assert!(!t.is_terminated(ms(10), &SimpleScore::new(0))); // gain 2^63, no wrapping
    t.step(ms(15), &SimpleScore::new(1_i64 << 62));
    assert!(!t.is_terminated(ms(20), &SimpleScore::new(1_i64 << 62))); // exact equality
    let mut t = policy(10, 1, 2);
    t.start(ms(0), SimpleScore::new(i64::MIN));
    assert!(!t.is_terminated(ms(10), &SimpleScore::new(0)));
    t.step(ms(15), &SimpleScore::new((1_i64 << 62) - 1));
    assert!(t.is_terminated(ms(20), &SimpleScore::new((1_i64 << 62) - 1)));
    let s = |v: &str| v.parse::<SimpleDecimalScore>().unwrap();
    let mut t = policy(10, 1, 2);
    t.start(ms(0), s("9007199254740992.000000000000000000"));
    assert!(!t.is_terminated(ms(10), &s("9007199254740992.000000000000000002")));
    t.step(ms(15), &s("9007199254740992.000000000000000003"));
    assert!(!t.is_terminated(ms(20), &s("9007199254740992.000000000000000003")));
    let mut t = policy(10, 1, 2);
    t.start(ms(0), s("9007199254740992.000000000000000000"));
    assert!(!t.is_terminated(ms(10), &s("9007199254740992.000000000000000002")));
    assert!(t.is_terminated(ms(20), &s("9007199254740992.0000000000000000029")));
    let mut t = DiminishedReturns::new(
        DiminishedReturnsConfig::new(Duration::MAX, u64::MAX, u64::MAX).unwrap(),
    )
    .unwrap();
    t.start(Duration::ZERO, SimpleScore::ZERO);
    assert!(!t.is_terminated(Duration::MAX, &SimpleScore::new(i64::MAX)));
    assert!(!t.is_terminated(Duration::MAX, &SimpleScore::new(i64::MAX)));
}
#[test]
fn history_keeps_only_changes_and_a_window_predecessor() {
    let mut t = policy(10, 1, 2);
    t.start(ms(0), SimpleScore::ZERO);
    for n in 1..=1000 {
        let s = SimpleScore::new(n);
        t.step(ms(n as u64), &s);
        assert!(!t.is_terminated(ms(n as u64), &s));
        assert!(t.retained_samples() <= 11);
    }
    for n in 1001..=2000 {
        t.step(ms(n), &SimpleScore::new(1000));
    }
    assert_eq!(t.retained_samples(), 1);
    assert!(t.is_terminated(ms(2000), &SimpleScore::new(1000)));
    assert!(t.is_terminated(ms(2001), &SimpleScore::new(999999))); // no revival
    t.phase_started(SimpleScore::new(7));
    assert!(!t.grace_started());
    assert!(!t.is_terminated(ms(5000), &SimpleScore::new(7)));
    t.step_started(ms(5000));
    assert_eq!(t.grace_start(), Some(ms(5000)));
    assert!(!t.is_terminated(ms(5009), &SimpleScore::new(7)));
    assert!(t.is_terminated(ms(5010), &SimpleScore::new(7)));
}
#[test]
fn higher_priority_levels_reset_grace_and_backwards_times_never_rewind_it() {
    let mut t = policy(10, 1, 1);
    t.start(ms(20), HardMediumSoftScore::new(0, 0, 0));
    t.step(ms(25), &HardMediumSoftScore::new(0, 1, -100));
    assert_eq!(t.grace_start(), Some(ms(25)));
    assert!(!t.is_terminated(ms(1), &HardMediumSoftScore::new(0, 1, -100)));
    assert!(!t.is_terminated(ms(34), &HardMediumSoftScore::new(0, 1, -100)));
    assert!(t.is_terminated(ms(35), &HardMediumSoftScore::new(0, 1, -100)));
    let mut t = policy(10, 1, 1);
    t.start(ms(0), BendableScore::<2, 0>::new([0, 0], []));
    t.step(ms(9), &BendableScore::new([0, 1], []));
    assert_eq!(t.grace_start(), Some(ms(9))); // Every hard level resets, even last.
}

#[test]
fn generated_sparse_histories_match_untrimmed_integer_rate_reference() {
    struct Reference {
        start: u64,
        last: u64,
        base: (i64, i64),
        gain: Option<(i128, u64)>,
        all: Vec<(u64, (i64, i64))>,
        done: bool,
    }
    impl Reference {
        fn new(now: u64, s: (i64, i64)) -> Self {
            Self {
                start: now,
                last: now,
                base: s,
                gain: None,
                all: vec![(now, s)],
                done: false,
            }
        }
        fn observe(&mut self, now: u64, s: (i64, i64)) {
            if self.done {
                return;
            }
            let now = now.max(self.last);
            if s.0 > self.base.0 {
                *self = Self::new(now, s);
            } else {
                self.last = now;
                self.all.push((now, s));
            }
        }
        fn poll(&mut self, now: u64, s: (i64, i64), window: u64, n: u64, d: u64) -> bool {
            if self.done {
                return true;
            }
            self.observe(now, s);
            let age = self.last - self.start;
            if age < window {
                return false;
            }
            if self.gain.is_none() {
                let gain = i128::from(s.1) - i128::from(self.base.1);
                self.done = gain <= 0;
                self.gain = Some((gain, age));
                return self.done;
            }
            let old = self
                .all
                .iter()
                .rev()
                .find(|(time, _)| *time <= self.last - window)
                .unwrap()
                .1;
            let delta = i128::from(s.1) - i128::from(old.1);
            let (gain, period) = self.gain.unwrap();
            self.done = delta * i128::from(period) * i128::from(d)
                < gain * i128::from(window) * i128::from(n);
            self.done
        }
    }
    let mut rng = 0x987654321_u64;
    let mut random = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for (n, d) in [(1, 1), (1, 2), (2, 1), (7, 10)] {
        for window in [1, 3, 10, 37] {
            for _ in 0..32 {
                let mut actual = policy(window, n, d);
                let mut reference = Reference::new(0, (0, 0));
                actual.start(ms(0), HardSoftScore::ZERO);
                let mut now = 0;
                let mut best = (0, 0);
                for _ in 0..100 {
                    now += random() % 6;
                    if random() % 13 == 0 {
                        best.0 += 1;
                        best.1 = -((random() % 10) as i64);
                    } else {
                        best.1 += (random() % 4) as i64;
                    }
                    let s = HardSoftScore::new(best.0, best.1);
                    if random() % 3 == 0 {
                        reference.observe(now, best);
                        actual.step(ms(now), &s);
                    } else {
                        assert_eq!(
                            actual.is_terminated(ms(now), &s),
                            reference.poll(now, best, window, n, d)
                        );
                    }
                }
            }
        }
    }
}
