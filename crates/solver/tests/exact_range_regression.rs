//! Original precision, bounds and integration checks; no upstream method credit.
use std::sync::Arc;
use timetuck::*;
#[test]
fn big_integer_progressions_match_small_independent_sequences() {
    for start in -5..=5 {
        for end in start..=10 {
            for step in 1..=7 {
                let r =
                    BigIntegerValueRange::with_step(start.into(), end.into(), step.into()).unwrap();
                let expected: Vec<_> = (start..end)
                    .filter(|v| (v - start) % step == 0)
                    .map(BigInt::from)
                    .collect();
                assert_eq!(r.len(), expected.len() as u128);
                assert_eq!(r.iter().collect::<Vec<_>>(), expected);
                for value in -7..=12 {
                    assert_eq!(r.contains(&value.into()), expected.contains(&value.into()));
                }
                assert_eq!(r.get(r.len()), None);
                assert_eq!(r.get(u128::MAX), None);
            }
        }
    }
}
#[test]
fn huge_offsets_and_steps_stay_exact_without_materializing_the_domain() {
    let offset = BigInt::from(10).pow(1000);
    let step = BigInt::from(10).pow(500);
    let r =
        BigIntegerValueRange::with_step(-&offset, -&offset + &step * 10u8, &step * 3u8).unwrap();
    assert_eq!(r.len(), 4);
    assert_eq!(r.get(3), Some(-&offset + &step * 9u8));
    assert!(r.contains(&(-&offset + &step * 6u8)));
    assert!(!r.contains(&(-&offset + &step * 7u8)));
    assert_eq!(r.iter().next_back(), r.get(3));
    for value in r.random_iter(43).take(100) {
        assert!(r.contains(&value));
    }
    assert!(variable_from_range(&r, 3).is_err());
    assert_eq!(variable_from_range(&r, 4).unwrap().domain().len(), 4);
}
#[test]
fn count_overflow_is_checked_and_invalid_steps_never_divide() {
    let max = BigInt::from(u128::MAX);
    let r = BigIntegerValueRange::new(BigInt::ZERO, max.clone()).unwrap();
    assert_eq!(r.len(), u128::MAX);
    assert_eq!(r.get(u128::MAX - 1), Some(&max - 1u8));
    assert_eq!(r.get(u128::MAX), None);
    assert_eq!(
        BigIntegerValueRange::new(BigInt::ZERO, &max + 1u8),
        Err(ExactRangeError::CountOverflow)
    );
    for step in [0, -1] {
        assert_eq!(
            BigIntegerValueRange::with_step(0.into(), 10.into(), step.into()),
            Err(ExactRangeError::NonPositiveStep)
        );
    }
    assert_eq!(
        BigIntegerValueRange::new(2.into(), 1.into()),
        Err(ExactRangeError::DescendingBounds)
    );
    let r = BigIntegerValueRange::new(max.clone(), max).unwrap();
    assert!(r.is_empty());
    assert_eq!(r.random_iter_with(|_| panic!("empty source")).next(), None);
}
#[test]
fn decimal_scale_is_explicit_even_at_extreme_and_negative_scales() {
    for scale in [i32::MIN, -3, 0, 2, i32::MAX] {
        let make = |n: i32| Decimal::new(n.into(), scale);
        let r = DecimalValueRange::new(make(-1), make(3)).unwrap();
        assert_eq!(r.scale(), scale);
        assert_eq!(r.len(), 4);
        assert_eq!(r.get(0), Some(make(-1)));
        assert_eq!(r.get(1), Some(make(0)));
        assert_eq!(r.get(3), Some(make(2)));
        assert!(r.contains(&make(1)));
        assert!(!r.contains(&make(3)));
    }
    let r = DecimalValueRange::new("1.0".parse().unwrap(), "2.0".parse().unwrap()).unwrap();
    assert!(r.contains(&"1.0".parse().unwrap()));
    assert!(!r.contains(&"1.00".parse().unwrap()));
    assert!(matches!(
        DecimalValueRange::new(
            Decimal::new(1.into(), i32::MIN),
            Decimal::new(2.into(), i32::MAX)
        ),
        Err(ExactRangeError::ScaleMismatch { .. })
    ));
    assert!(matches!(
        DecimalValueRange::with_step(
            "1.0".parse().unwrap(),
            "2.0".parse().unwrap(),
            "0.10".parse().unwrap()
        ),
        Err(ExactRangeError::ScaleMismatch { .. })
    ));
    assert_eq!(
        DecimalValueRange::with_step(
            "1.0".parse().unwrap(),
            "2.0".parse().unwrap(),
            "0.0".parse().unwrap()
        ),
        Err(ExactRangeError::NonPositiveStep)
    );
}
#[test]
fn decimal_nondividing_steps_match_coefficient_arithmetic_and_ordering() {
    for scale in [-2, 0, 3] {
        for step in 1..8 {
            let r = DecimalValueRange::with_step(
                Decimal::new((-7).into(), scale),
                Decimal::new(11.into(), scale),
                Decimal::new(step.into(), scale),
            )
            .unwrap();
            let expected: Vec<_> = (-7..11)
                .filter(|v| (v + 7) % step == 0)
                .map(|v| Decimal::new(v.into(), scale))
                .collect();
            assert_eq!(r.iter().collect::<Vec<_>>(), expected);
            for (i, value) in expected.iter().enumerate() {
                assert_eq!(r.get(i as u128), Some(value.clone()));
                assert!(r.contains(value));
            }
            assert_eq!(
                r.sorted_by(30, |a, b| b.numeric_cmp(a)).unwrap().values(),
                expected.into_iter().rev().collect::<Vec<_>>()
            );
        }
    }
}
#[test]
fn exact_ranges_compose_with_common_destinations_and_optional_members() {
    let offset = BigInt::from(10).pow(100);
    let a: SharedValueRange<'_, BigInt> = Arc::new(
        BigIntegerValueRange::with_step(offset.clone(), &offset + 10u8, 2.into()).unwrap(),
    );
    let b: SharedValueRange<'_, BigInt> =
        Arc::new(BigIntegerValueRange::with_step(&offset + 1u8, &offset + 10u8, 3.into()).unwrap());
    let common = CommonValueRanges::new([a, b]).unwrap();
    assert_eq!(common.pick_exact(None, 1, 10).unwrap(), Some(&offset + 4u8));
    let decimal = DecimalValueRange::new("0.01".parse().unwrap(), "0.04".parse().unwrap()).unwrap();
    let optional = OptionalValueRange::new(decimal).unwrap();
    assert_eq!(optional.len(), 4);
    assert_eq!(optional.get(0), Some(None));
    assert_eq!(optional.get(3), Some(Some("0.03".parse().unwrap())));
    let sum = CompositeValueRange::new(vec![
        Box::new(optional) as Box<dyn ValueRange<Value = Option<Decimal>>>,
        Box::new(EnumeratedValueRange::new([Some("9.00".parse().unwrap())])),
    ])
    .unwrap();
    assert_eq!(sum.len(), 5);
    assert_eq!(sum.get(4), Some(Some("9.00".parse().unwrap())));
}
#[test]
fn decimal_domains_and_scores_solve_without_binary_rounding() {
    let r = DecimalValueRange::new("0.01".parse().unwrap(), "0.04".parse().unwrap()).unwrap();
    let p = FiniteProblem::new(vec![
        variable_from_range(&r, 3).unwrap(),
        variable_from_range(&r, 3).unwrap(),
    ])
    .unwrap();
    let result = exhaustive_search(
        &p,
        |a| {
            SimpleDecimalScore::new(
                a.values()[0]
                    .as_ref()
                    .unwrap()
                    .add(a.values()[1].as_ref().unwrap()),
            )
        },
        100,
    );
    assert_eq!(result.evaluations, 9);
    let best = result.best.unwrap();
    assert_eq!(
        best.assignment.values(),
        &[Some("0.03".parse().unwrap()), Some("0.03".parse().unwrap())]
    );
    assert_eq!(best.score, SimpleDecimalScore::new("0.06".parse().unwrap()));
}
