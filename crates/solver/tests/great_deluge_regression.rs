//! Original arithmetic and lifecycle boundaries; no upstream method credit.
use timetuck::*;
fn s(v: i64) -> SimpleScore {
    SimpleScore::new(v)
}
#[test]
fn water_is_stable_during_trials_and_aspiration_is_strict() {
    let mut a = GreatDeluge::new(Some(s(10)), GreatDelugeRise::Score(s(2)), s(0)).unwrap();
    for _ in 0..4 {
        assert!(!a.accepts(s(0), s(0)));
        assert!(a.accepts(s(0), s(1)));
        assert!(a.accepts(s(20), s(10)));
        assert!(!a.accepts(s(20), s(9)));
        assert!(!a.accepts(s(0), s(100).with_structural(-1)));
    }
    a.step_ended();
    assert!(!a.accepts(s(20), s(10)));
    assert!(a.accepts(s(20), s(12)));
}
#[test]
fn ratio_accumulates_fractional_rain_for_negative_positive_and_zero_levels() {
    let mut a = GreatDeluge::new(
        None,
        GreatDelugeRise::Ratio {
            numerator: 1,
            denominator: 3,
        },
        HardMediumSoftScore::new(-2, 2, 0),
    )
    .unwrap();
    for expected in [
        [-2, 2, 0],
        [-1, 3, 0],
        [0, 4, 0],
        [0, 4, 0],
        [1, 5, 0],
        [2, 6, 0],
    ] {
        a.step_ended();
        assert_eq!(a.water_levels(), &expected.map(Decimal::from));
    }
}
#[test]
fn thresholds_do_not_wrap_at_integer_boundaries() {
    let mut a = GreatDeluge::new(None, GreatDelugeRise::Score(s(i64::MAX)), s(i64::MAX)).unwrap();
    a.step_ended();
    assert_eq!(
        a.water_levels()[0].coefficient(),
        &(BigInt::from(i64::MAX) * 2)
    );
    assert!(!a.accepts(s(i64::MAX), s(i64::MAX)));
    assert!(a.accepts(s(i64::MIN), s(i64::MAX)));
    let mut b = GreatDeluge::new(
        None,
        GreatDelugeRise::Ratio {
            numerator: u64::MAX,
            denominator: 1,
        },
        s(i64::MIN),
    )
    .unwrap();
    b.step_ended();
    assert!(
        b.water_levels()[0]
            .numeric_cmp(&Decimal::from(i64::MAX))
            .is_gt()
    );
}
#[test]
fn decimal_precision_and_negative_scales_define_ratio_rounding() {
    let mut a = GreatDeluge::new(
        None,
        GreatDelugeRise::Ratio {
            numerator: 1,
            denominator: 10,
        },
        SimpleDecimalScore::new("-0.08".parse().unwrap()),
    )
    .unwrap();
    a.step_ended();
    assert_eq!(a.water_levels()[0], "-0.08".parse().unwrap());
    a.step_ended();
    assert_eq!(a.water_levels()[0], "-0.07".parse().unwrap());
    let mut b = GreatDeluge::new(
        None,
        GreatDelugeRise::Ratio {
            numerator: 1,
            denominator: 3,
        },
        SimpleDecimalScore::new(Decimal::new((-2).into(), -1)),
    )
    .unwrap();
    b.step_ended();
    assert_eq!(b.water_levels()[0], Decimal::from(-14));
    b.step_ended();
    assert_eq!(b.water_levels()[0], Decimal::from(-7));
    b.step_ended();
    assert_eq!(b.water_levels()[0], Decimal::ZERO);
    let mut c = GreatDeluge::new(
        None,
        GreatDelugeRise::Score(SimpleDecimalScore::new("0.001".parse().unwrap())),
        SimpleDecimalScore::new("-0.08".parse().unwrap()),
    )
    .unwrap();
    c.step_ended();
    assert_eq!(c.water_levels()[0], "-0.079".parse().unwrap());
    assert!(c.accepts(
        SimpleDecimalScore::new(1.into()),
        SimpleDecimalScore::new("-0.0790".parse().unwrap())
    ));
}
#[test]
fn empty_bendable_groups_and_owned_scores_are_supported() {
    for rise in [
        GreatDelugeRise::Score(BendableScore::<0, 0>::ZERO),
        GreatDelugeRise::Ratio {
            numerator: 1,
            denominator: 10,
        },
    ] {
        let mut a = GreatDeluge::new(None, rise, BendableScore::<0, 0>::ZERO).unwrap();
        a.step_ended();
        assert!(a.water_levels().is_empty());
        assert!(a.accepts(BendableScore::ZERO, BendableScore::ZERO));
    }
    type S = BendableDecimalScore<0, 2>;
    let initial = S::new([], [(-2).into(), 2.into()]);
    let mut a = GreatDeluge::new(
        None,
        GreatDelugeRise::Score(S::new([], [1.into(), 0.into()])),
        initial.clone(),
    )
    .unwrap();
    a.step_ended();
    assert!(!a.accepts(initial.clone(), initial));
}
#[test]
fn phase_sessions_clone_and_restart_independently_and_fallback_raises_water() {
    let config = Acceptance::GreatDeluge {
        initial_water_level: None,
        rise: GreatDelugeRise::Score(s(2)),
    };
    let mut a = config.start(s(0)).unwrap();
    let mut b = a.clone();
    a.step_ended(s(-10), &[]);
    assert!(!a.accepts(s(5), s(1), &[]));
    assert!(b.accepts(s(5), s(1), &[]));
    assert!(config.start(s(0)).unwrap().accepts(s(5), s(1), &[]));
    assert!(a.accepts(s(-10), s(-9), &[]));
}
#[test]
fn invalid_parameters_are_rejected_before_start_and_zero_rain_is_allowed() {
    for rise in [
        GreatDelugeRise::Score(s(-1)),
        GreatDelugeRise::Ratio {
            numerator: 0,
            denominator: 1,
        },
        GreatDelugeRise::Ratio {
            numerator: 1,
            denominator: 0,
        },
    ] {
        let c = Acceptance::GreatDeluge {
            initial_water_level: None,
            rise,
        };
        assert_eq!(
            c.validate(),
            Err(AcceptanceError::InvalidWaterLevelIncrement)
        );
        assert!(c.start(s(0)).is_err());
    }
    assert!(
        GreatDeluge::new(
            Some(s(0).with_structural(-1)),
            GreatDelugeRise::Score(s(1)),
            s(0)
        )
        .is_err()
    );
    assert!(
        GreatDeluge::new(None, GreatDelugeRise::Score(s(1).with_structural(-1)), s(0)).is_err()
    );
    assert!(
        GreatDeluge::new(None, GreatDelugeRise::Score(s(1)), s(0).with_structural(-1)).is_err()
    );
    let mut a = GreatDeluge::new(None, GreatDelugeRise::Score(s(0)), s(-2)).unwrap();
    a.step_ended();
    assert_eq!(a.water_levels(), &[Decimal::from(-2)]);
}

#[test]
fn custom_levels_without_exact_arithmetic_keep_other_search_policies() {
    use std::cmp::Ordering;
    #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Custom(i64);
    impl ScoreLevel for Custom {
        fn zero() -> Self {
            Self(0)
        }
        fn numeric_cmp(&self, b: &Self) -> Ordering {
            self.cmp(b)
        }
        fn progress_fraction(&self, b: &Self, c: &Self) -> f64 {
            self.0.progress_fraction(&b.0, &c.0)
        }
        fn cooling_loss(&self, b: &Self, t: &Self, g: f64) -> f64 {
            self.0.cooling_loss(&b.0, &t.0, g)
        }
    }
    impl Score for Custom {
        type Level = Self;
        const HARD_LEVEL_COUNT: usize = 0;
        fn ordinary_levels(&self) -> impl Iterator<Item = Self> {
            std::iter::once(self.clone())
        }
        fn structural(&self) -> i64 {
            0
        }
        fn hard_cmp(&self, _: &Self) -> Ordering {
            Ordering::Equal
        }
        fn is_feasible(&self) -> bool {
            true
        }
    }
    let c = Acceptance::GreatDeluge {
        initial_water_level: None,
        rise: GreatDelugeRise::Score(Custom(1)),
    };
    assert_eq!(
        c.validate(),
        Err(AcceptanceError::UnsupportedScoreArithmetic)
    );
    assert!(c.start(Custom(0)).is_err());
    assert!(
        Acceptance::HillClimbing
            .start(Custom(0))
            .unwrap()
            .accepts(Custom(0), Custom(1), &[])
    );
}

// Original regressions for the explicit binary64 policy; no method credit.
#[test]
fn binary64_ratio_retains_tenth_step_boundary_and_exact_control() {
    let mut exact = GreatDeluge::new(
        None,
        GreatDelugeRise::Ratio {
            numerator: 1,
            denominator: 10,
        },
        HardSoftScore::new(-1, -9),
    )
    .unwrap();
    let mut floating = GreatDeluge::new(
        None,
        GreatDelugeRise::Binary64Ratio {
            numerator: 1,
            denominator: 10,
        },
        HardSoftScore::new(-1, -9),
    )
    .unwrap();
    for _ in 0..10 {
        exact.step_ended();
        floating.step_ended();
    }
    assert_eq!(exact.water_levels(), &[Decimal::ZERO, Decimal::ZERO]);
    assert_eq!(
        floating.water_levels(),
        &[Decimal::from(-1), Decimal::from(-1)]
    );
    assert!(floating.accepts(HardSoftScore::new(0, 1), HardSoftScore::new(0, -9)));
    assert!(!exact.accepts(HardSoftScore::new(0, 1), HardSoftScore::new(0, -9)));
    let mut fork = floating.clone();
    floating.step_ended();
    assert_eq!(floating.water_levels(), &[Decimal::ZERO, Decimal::ZERO]);
    assert_eq!(fork.water_levels(), &[Decimal::from(-1), Decimal::from(-1)]);
    fork.step_ended();
    assert_eq!(floating.water_levels(), fork.water_levels());
}

#[test]
fn binary64_ratio_preserves_aspiration_trial_stability_and_zero_components() {
    let mut a = GreatDeluge::new(
        Some(HardMediumSoftScore::new(-2, 2, 0)),
        GreatDelugeRise::Binary64Ratio {
            numerator: 1,
            denominator: 3,
        },
        HardMediumSoftScore::ZERO,
    )
    .unwrap();
    for _ in 0..3 {
        assert!(a.accepts(
            HardMediumSoftScore::new(-3, 0, 0),
            HardMediumSoftScore::new(-2, -100, 0)
        ));
        assert!(!a.accepts(
            HardMediumSoftScore::new(10, 0, 0),
            HardMediumSoftScore::new(-3, 0, 0)
        ));
        assert!(!a.accepts(
            HardMediumSoftScore::ZERO,
            HardMediumSoftScore::new(100, 0, 0).with_structural(-1)
        ));
        assert_eq!(a.water_levels(), &[-2, 2, 0].map(Decimal::from));
    }
    a.step_ended();
    assert_eq!(a.water_levels(), &[-2, 2, 0].map(Decimal::from));
    a.step_ended();
    assert_eq!(a.water_levels(), &[-1, 3, 0].map(Decimal::from));
    a.step_ended();
    assert_eq!(a.water_levels(), &[0, 4, 0].map(Decimal::from));
}

#[test]
fn binary64_ratio_decodes_wide_thresholds_without_integer_saturation() {
    let mut a = GreatDeluge::new(
        None,
        GreatDelugeRise::Binary64Ratio {
            numerator: u64::MAX,
            denominator: 1,
        },
        s(i64::MIN),
    )
    .unwrap();
    a.step_ended();
    assert_eq!(
        a.water_levels()[0].coefficient(),
        &(BigInt::from(1u8) << 127usize)
    );
    assert!(!a.accepts(s(i64::MAX), s(i64::MAX)));
    assert!(a.accepts(s(i64::MIN), s(i64::MAX)));
    let mut lower = GreatDeluge::new(
        None,
        GreatDelugeRise::Binary64Ratio {
            numerator: 1,
            denominator: u64::MAX,
        },
        s(-i64::MAX),
    )
    .unwrap();
    lower.step_ended();
    assert_eq!(lower.water_levels(), &[Decimal::from(i64::MIN)]);
}

#[test]
fn binary64_ratio_rejects_nonunit_or_wide_water_and_zero_ratios() {
    for (num, den) in [(0, 1), (1, 0)] {
        assert_eq!(
            GreatDeluge::new(
                None,
                GreatDelugeRise::Binary64Ratio {
                    numerator: num,
                    denominator: den
                },
                s(0)
            )
            .unwrap_err(),
            AcceptanceError::InvalidWaterLevelIncrement
        );
    }
    for value in [
        Decimal::new(1.into(), 1),
        Decimal::new(1.into(), -1),
        Decimal::new(BigInt::from(i64::MAX) + 1u8, 0),
    ] {
        let result = GreatDeluge::new(
            None,
            GreatDelugeRise::Binary64Ratio {
                numerator: 1,
                denominator: 10,
            },
            SimpleDecimalScore::new(value),
        );
        assert_eq!(
            result.unwrap_err(),
            AcceptanceError::UnsupportedScoreArithmetic
        );
    }
    let result = GreatDeluge::new(
        Some(s(-1).with_structural(-1)),
        GreatDelugeRise::Binary64Ratio {
            numerator: 0,
            denominator: 0,
        },
        s(0),
    );
    assert_eq!(result.unwrap_err(), AcceptanceError::InvalidWaterLevel);
    let empty = GreatDeluge::new(
        None,
        GreatDelugeRise::Binary64Ratio {
            numerator: 1,
            denominator: 10,
        },
        BendableScore::<0, 0>::ZERO,
    )
    .unwrap();
    assert!(empty.water_levels().is_empty());
}
