//! Original level arithmetic checks; no upstream coverage credit.
#[path = "support/decimal_score.rs"]
mod support;
use support::DecimalScore;
use timetuck::*;
fn decimal(c: i64, scale: i32) -> Decimal {
    Decimal::new(c.into(), scale)
}
fn close(a: f64, b: f64) {
    assert!(
        a == b || (a.is_finite() && b.is_finite() && (a - b).abs() <= b.abs() * 5e-15),
        "actual {a:e}, expected {b:e}"
    );
}
#[test]
fn decimal_estimates_match_independent_fixed_point_rational_arithmetic() {
    // Convert each operand to a shared integer unit independently of Decimal's
    // alignment/normalization. Include sign changes, scale ties and reversals.
    let values = [-101i64, -7, -1, 0, 1, 7, 101];
    for a in values {
        for b in values {
            for c in values {
                for sa in -2..=2 {
                    for sb in -2..=2 {
                        for sc in -2..=2 {
                            let x = i128::from(a) * 10i128.pow((2 - sa) as u32);
                            let y = i128::from(b) * 10i128.pow((2 - sb) as u32);
                            let z = i128::from(c) * 10i128.pow((2 - sc) as u32);
                            let expected = if z >= y {
                                1.0
                            } else if z <= x || y <= x {
                                0.0
                            } else {
                                (z - x) as f64 / (y - x) as f64
                            };
                            close(
                                decimal(a, sa).progress_fraction(&decimal(b, sb), &decimal(c, sc)),
                                expected,
                            );
                            if c >= 0 {
                                for gradient in [0.0, 0.375, 1.0 - f64::EPSILON, 1.0] {
                                    let expected = if x <= y {
                                        0.0
                                    } else if z == 0 || gradient == 1.0 {
                                        f64::INFINITY
                                    } else {
                                        (x - y) as f64 / (z as f64 * (1.0 - gradient))
                                    };
                                    close(
                                        decimal(a, sa).cooling_loss(
                                            &decimal(b, sb),
                                            &decimal(c, sc),
                                            gradient,
                                        ),
                                        expected,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn decimal_deltas_survive_large_common_offsets_and_scale_cancellation() {
    let base = BigInt::from(10).pow(1000);
    for scale in [-1000, 0, 1000] {
        let a = Decimal::new(base.clone(), scale);
        let b = Decimal::new(&base + 10, scale);
        let c = Decimal::new(&base + 6, scale);
        close(a.progress_fraction(&b, &c), 0.6);
        close(b.cooling_loss(&c, &decimal(2, scale), 0.5), 4.0);
        let minus = Decimal::new(-&base, scale);
        close(minus.progress_fraction(&a, &Decimal::ZERO), 0.5);
        // Distinct stored scales, same value; no spurious decline or sign.
        let alternate = Decimal::new((&base + 10) * 100, scale + 2);
        assert_eq!(b.cooling_loss(&alternate, &Decimal::ZERO, 1.0), 0.0);
        assert_eq!(b.progress_fraction(&alternate, &b), 1.0);
    }
}
#[test]
fn full_scale_range_estimates_do_not_expand_billions_of_digits() {
    let tiny = decimal(1, i32::MAX);
    let huge = decimal(1, i32::MIN);
    let minus_huge = decimal(-1, i32::MIN);
    assert_eq!(tiny.cooling_loss(&Decimal::ZERO, &huge, 0.0), 0.0);
    assert_eq!(huge.cooling_loss(&tiny, &tiny, 0.0), f64::INFINITY);
    close(minus_huge.progress_fraction(&huge, &tiny), 0.5);
    close(huge.cooling_loss(&tiny, &huge, 0.5), 2.0);
    close(tiny.cooling_loss(&minus_huge, &huge, 0.5), 2.0);
    for scale in [i32::MIN, i32::MAX] {
        let zero = decimal(0, scale);
        assert_eq!(zero.numeric_cmp(&Decimal::ZERO), std::cmp::Ordering::Equal);
        assert_eq!(huge.cooling_loss(&tiny, &zero, 0.0), f64::INFINITY);
    }
}
#[test]
fn ratios_keep_subnormals_and_finite_values_near_float_limits() {
    close(
        decimal(1, 0).cooling_loss(&Decimal::ZERO, &decimal(1, 308), 0.0),
        1e308,
    );
    close(
        decimal(1, 0).cooling_loss(&Decimal::ZERO, &decimal(9, 309), 0.0),
        large_finite(),
    );
    let subnormal = decimal(5, 324).cooling_loss(&Decimal::ZERO, &decimal(1, 0), 0.0);
    assert_eq!(subnormal, f64::from_bits(1));
    assert_eq!(
        decimal(1, 325).cooling_loss(&Decimal::ZERO, &decimal(1, 0), 0.0),
        0.0
    );
}
fn large_finite() -> f64 {
    1e308 / 0.9
}
#[test]
fn decimal_progress_and_annealing_keep_exact_predicates_separate() {
    for scale in [-1000, 3, 1000] {
        let make = |h, s| DecimalScore::scaled(h, s, scale);
        let progress = ScoreTargetProgress::new(make(-10, -20), make(0, 0), vec![0.75]).unwrap();
        close(progress.progress(make(-5, -10)), 0.5);
        assert!(!progress.is_reached(make(-1, 100)));
        let feasible = FeasibilityProgress::new(make(-10, -20), vec![]).unwrap();
        close(feasible.progress(make(-4, -100)), 0.6);
        assert!(!feasible.is_reached(make(-1, 100)));
        assert!(feasible.is_reached(make(0, -100)));
        let a = SimulatedAnnealing::new(make(2, 4), 10, 7).unwrap();
        close(
            a.probability_at(make(0, 0), make(-1, -2), 0.5).unwrap(),
            (-2.0f64).exp(),
        );
        assert_eq!(
            a.probability_at(make(0, 0), make(-1, -2), 1.0).unwrap(),
            0.0
        );
        assert_eq!(a.probability_at(make(0, 0), make(0, 1), 1.0).unwrap(), 1.0);
        assert!(SimulatedAnnealing::new(make(-1, 1), 10, 7).is_err());
        assert_eq!(progress.progress(make(100, 100).with_structural(-1)), 0.0);
        assert_eq!(
            a.probability_at(make(0, 0), make(0, 1).with_structural(-1), 0.0)
                .unwrap(),
            0.0
        );
    }
}
