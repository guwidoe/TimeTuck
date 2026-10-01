//! Original boundary/regression cases; not counted as adapted upstream methods.
use timetuck::{HardMediumSoftScore, HardSoftScore, SimpleScore};

#[test]
fn integer_overflow_is_explicit_in_every_profile() {
    assert_eq!(
        SimpleScore::new(i64::MAX) + SimpleScore::new(1),
        SimpleScore::new(i64::MIN)
    );
    assert_eq!(
        SimpleScore::new(i64::MIN) - SimpleScore::new(1),
        SimpleScore::new(i64::MAX)
    );
    assert_eq!(-SimpleScore::new(i64::MIN), SimpleScore::new(i64::MIN));
    assert_eq!(SimpleScore::new(i64::MIN).abs(), SimpleScore::new(i64::MIN));
    assert_eq!(
        SimpleScore::new(i64::MAX).checked_add(SimpleScore::new(1)),
        None
    );
    assert_eq!(
        SimpleScore::new(i64::MIN).checked_sub(SimpleScore::new(1)),
        None
    );
    assert_eq!(HardSoftScore::new(0, i64::MAX).checked_mul(2), None);
    assert_eq!(
        HardSoftScore::new(3, -5).checked_mul(7),
        Some(HardSoftScore::new(21, -35))
    );
}

#[test]
fn structural_arithmetic_clears_flaws_but_construction_preserves_them() {
    let score = HardSoftScore::new(7, -9).with_structural(-3);
    assert!(!score.is_feasible());
    assert_eq!((score + score).structural(), 0);
    assert_eq!((score - score).structural(), 0);
    assert_eq!((-score).structural(), 0);
    assert_eq!(score.abs().structural(), 0);
    assert_eq!(score.multiply(1.2).structural(), 0);
    assert_eq!(score.divide(2.0).structural(), 0);
    assert_eq!(score.power(2.0).structural(), 0);
    assert_eq!(score.with_structural(5).structural(), 5);
    assert!(score.with_structural(5).is_feasible());
    assert_eq!(
        score.with_structural(i64::MIN).checked_add(score),
        Some(HardSoftScore::new(14, -18))
    );
}

#[test]
fn ieee_float_edges_have_defined_conversion() {
    let score = SimpleScore::new(-5);
    assert_eq!(score.multiply(f64::NAN), SimpleScore::ZERO);
    assert_eq!(score.multiply(f64::INFINITY), SimpleScore::new(i64::MIN));
    assert_eq!(score.divide(0.0), SimpleScore::new(i64::MIN));
    assert_eq!(score.divide(-0.0), SimpleScore::new(i64::MAX));
    assert_eq!(SimpleScore::ZERO.divide(0.0), SimpleScore::ZERO);
    assert_eq!(score.power(0.5), SimpleScore::ZERO);
}

#[test]
fn parsing_rejects_incomplete_or_malformed_scores() {
    for text in [
        "",
        " 1",
        "1 ",
        "1/",
        "1//",
        "--1",
        "9223372036854775808",
        "1init/2",
    ] {
        assert!(text.parse::<SimpleScore>().is_err(), "{text:?}");
    }
    for text in ["0", "1hard", "1soft/2hard", "1hard/2soft/", "1hard//2soft"] {
        assert!(text.parse::<HardSoftScore>().is_err(), "{text:?}");
    }
    assert_eq!("+1".parse::<SimpleScore>().unwrap(), SimpleScore::new(1));
    assert_eq!(
        "*structural/0".parse::<SimpleScore>().unwrap().structural(),
        i64::MIN
    );
}

#[test]
fn formatting_round_trips_boundaries_and_level_priority() {
    let values = [i64::MIN, i64::MAX, -1, 0, 1];
    for structural in [i64::MIN, -1, 0] {
        for hard in values {
            for medium in values {
                for soft in values {
                    let score =
                        HardMediumSoftScore::new(hard, medium, soft).with_structural(structural);
                    assert_eq!(score.to_string().parse(), Ok(score));
                    assert_eq!(score.is_feasible(), structural == 0 && hard >= 0);
                }
            }
        }
    }
    assert!(HardSoftScore::new(0, i64::MIN) > HardSoftScore::new(-1, i64::MAX));
    assert!(SimpleScore::new(i64::MIN) > SimpleScore::new(i64::MAX).with_structural(-1));
}
