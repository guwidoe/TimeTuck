//! Original controls for the entire finite IEEE754 envelope and exact thresholds.
use std::time::Duration;
use timetuck::*;

#[test]
fn every_finite_exponent_and_subnormal_boundary_bind_without_rounding() {
    let window = Duration::from_secs(1);
    for exponent in 0u64..2047 {
        for fraction in [0, 1, (1u64 << 51) + 7, (1u64 << 52) - 1] {
            let bits = (exponent << 52) | fraction;
            if bits == 0 {
                continue;
            }
            let value = f64::from_bits(bits);
            let config = DiminishedReturnsConfig::from_f64(window, value).unwrap();
            let copied = config;
            assert_eq!(copied, config);
            let (n, d) = config.ratio();
            // Independent unreduced fraction from IEEE's implicit leading bit.
            let significand = if exponent == 0 {
                fraction
            } else {
                fraction + (1u64 << 52)
            };
            let power = if exponent == 0 {
                -1074
            } else {
                exponent as i32 - 1075
            };
            let (expected_n, expected_d) = if power >= 0 {
                (BigInt::from(significand) << power as usize, BigInt::from(1))
            } else {
                (
                    BigInt::from(significand),
                    BigInt::from(1) << (-power) as usize,
                )
            };
            assert_eq!(&n * expected_d, expected_n * &d, "bits {bits:016x}");
            let mut plan = MixedSolverSequence::<SimpleScore>::default();
            let receipt = RuntimeSolverProperties {
                termination: Some(RuntimeTerminationProperties {
                    diminished_returns: Some(RuntimeDiminishedReturns {
                        sliding_window_duration: Some(window),
                        minimum_improvement_ratio: Some(value),
                        ..Default::default()
                    }),
                }),
            }
            .bind_mixed(&mut plan)
            .unwrap();
            assert_eq!(receipt.resolved, Some(config));
            assert_eq!(receipt.applied_local_phases, 1);
        }
    }
    assert_eq!(
        DiminishedReturnsConfig::from_f64(window, 123.0).unwrap(),
        DiminishedReturnsConfig::new(window, 123, 1).unwrap()
    );
    let default = DiminishedReturnsConfig::from_f64(window, 0.0001).unwrap();
    assert_eq!(
        default.ratio(),
        (BigInt::from(7378697629483821u64), BigInt::from(1) << 66)
    );
    for invalid in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            DiminishedReturnsConfig::from_f64(window, invalid),
            Err(DiminishedReturnsError::RatioMustBePositiveFinite)
        );
    }
    assert_eq!(
        DiminishedReturnsConfig::from_f64(Duration::ZERO, 0.0001),
        Err(DiminishedReturnsError::ZeroWindow)
    );
}

#[test]
fn equality_and_adjacent_integer_rates_remain_strict_at_both_extremes() {
    let score = |coefficient: BigInt| SimpleDecimalScore::new(Decimal::new(coefficient, 0));
    for ratio in [
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        0.0001,
        0.1,
        0.5,
        123.0,
        2f64.powi(64),
        1e300,
        f64::MAX,
    ] {
        let config = DiminishedReturnsConfig::from_f64(Duration::from_secs(10), ratio).unwrap();
        let (n, d) = config.ratio();
        for (offset, stop) in [(-1, true), (0, false), (1, false)] {
            let mut termination = DiminishedReturns::new(config).unwrap();
            termination.start(Duration::ZERO, SimpleDecimalScore::ZERO);
            let reference = score(d.clone());
            assert!(!termination.is_terminated(Duration::from_secs(10), &reference));
            let recent = score(&d + &n + offset);
            assert_eq!(
                termination.is_terminated(Duration::from_secs(20), &recent),
                stop,
                "ratio {ratio}, offset {offset}"
            );
        }
    }
}
