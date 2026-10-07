use timetuck::*;
#[test]
fn integer_division_floors_exactly_and_rejects_overflow() {
    for a in -31..=31 {
        for b in -9..=9 {
            let divisor = if b == 0 { 1 } else { b };
            let got = SimpleScore::new(a)
                .with_structural(-3)
                .divide_by_sanitized_divisor(&SimpleScore::new(b).with_structural(7))
                .unwrap();
            assert_eq!(got.structural(), 0);
            let q = got.score();
            // q <= a/b < q+1, independently checked by cross multiplication.
            if divisor > 0 {
                assert!(q * divisor <= a && a < (q + 1) * divisor);
            } else {
                assert!(q * divisor >= a && a > (q + 1) * divisor);
            }
        }
    }
    assert_eq!(
        SimpleScore::new(i64::MAX)
            .divide_by_sanitized_divisor(&SimpleScore::new(1))
            .unwrap()
            .score(),
        i64::MAX
    );
    assert_eq!(
        HardSoftScore::new(0, i64::MIN).divide_by_sanitized_divisor(&HardSoftScore::new(0, -1)),
        Err(ScoreDefinitionError::IntegerOverflow { level: 1 })
    );
    assert_eq!(
        SimpleScore::new(i64::MIN)
            .divide_by_sanitized_divisor(&SimpleScore::ZERO)
            .unwrap()
            .score(),
        i64::MIN
    );
}
#[test]
fn decimal_division_preserves_precision_and_normalizes_zero_divisors() {
    let a = HardMediumSoftDecimalScore::from_levels(
        ["-7.00", "1E+1", "9007199254740993.00"]
            .map(|v| v.parse().unwrap())
            .to_vec(),
    )
    .unwrap()
    .with_structural(-9);
    let b = HardMediumSoftDecimalScore::from_levels(
        ["2.0", "0.000", "3"].map(|v| v.parse().unwrap()).to_vec(),
    )
    .unwrap();
    let out = a.divide_by_sanitized_divisor(&b).unwrap();
    assert_eq!(out.structural(), 0);
    let values: Vec<_> = out.ordinary_levels().collect();
    for (value, want) in values.iter().zip(["-3.50", "10", "3002399751580331.00"]) {
        assert_eq!(*value, want.parse::<Decimal>().unwrap());
    }
    assert_eq!(a.structural(), -9);
    assert_eq!(a.medium().scale(), -1);
    let empty = BendableDecimalScore::<0, 0>::ZERO;
    assert_eq!(empty.divide_by_sanitized_divisor(&empty).unwrap(), empty);
}
#[test]
fn shape_validation_and_empty_or_hard_only_units() {
    assert_eq!(
        HardSoftScore::from_levels(vec![1]),
        Err(ScoreDefinitionError::LevelCount {
            expected: 2,
            actual: 1
        })
    );
    assert_eq!(
        SimpleDecimalScore::from_levels(vec![]),
        Err(ScoreDefinitionError::LevelCount {
            expected: 1,
            actual: 0
        })
    );
    assert_eq!(
        BendableScore::<0, 0>::one_softest_score(),
        Err(ScoreDefinitionError::NoLevels)
    );
    assert_eq!(
        BendableDecimalScore::<0, 0>::one_softest_score(),
        Err(ScoreDefinitionError::NoLevels)
    );
    assert_eq!(
        BendableScore::<2, 0>::one_softest_score().unwrap(),
        BendableScore::new([0, 1], [])
    );
    assert_eq!(
        BendableScore::<0, 0>::from_levels(vec![]).unwrap(),
        BendableScore::ZERO
    );
    assert_eq!(BendableScore::<0, 0>::level_labels(), Vec::<String>::new());
}
#[test]
fn mixed_trend_bounds_cover_completions_without_changing_infinite_bounds() {
    use InitializingScoreTrendLevel::{Any, OnlyDown, OnlyUp};
    for first in [Any, OnlyDown, OnlyUp] {
        for second in [Any, OnlyDown, OnlyUp] {
            let trend = InitializingScoreTrend::new(vec![first, second]);
            let partial = HardSoftScore::new(-2, 3).with_structural(-1);
            let lower = partial.finite_pessimistic_bound(&trend).unwrap();
            let upper = partial.finite_optimistic_bound(&trend).unwrap();
            assert_eq!(lower.structural(), 0);
            assert_eq!(upper.structural(), 0);
            for a in -4..=0 {
                for b in 1..=5 {
                    let allowed = |t, x, p| match t {
                        Any => true,
                        OnlyDown => x <= p,
                        OnlyUp => x >= p,
                    };
                    if allowed(first, a, -2) && allowed(second, b, 3) {
                        let completed = HardSoftScore::new(a, b);
                        assert!(lower <= completed && completed <= upper);
                    }
                }
            }
        }
    }
    let any = InitializingScoreTrend::parse("ANY", 2).unwrap();
    let score = HardSoftScore::new(1, 2);
    let finite = score.finite_optimistic_bound(&any).unwrap();
    assert_eq!(finite, HardSoftScore::new(i64::MAX, i64::MAX));
    assert!(
        any.optimistic_bound(score)
            .unwrap()
            .compare_score(&finite)
            .is_gt()
    );
    assert!(matches!(
        score.finite_optimistic_bound(&InitializingScoreTrend::new(vec![Any])),
        Err(ScoreDefinitionError::Trend(ScoreTrendError::LevelCount {
            expected: 2,
            actual: 1
        }))
    ));
}
#[test]
fn generic_definition_builds_scores_for_an_actual_solve() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1i64, 2, 3]),
        Variable::new(vec![1, 2, 3]),
    ])
    .unwrap();
    let result = exhaustive_search(
        &p,
        |a| {
            let x = a.values()[0].unwrap();
            let y = a.values()[1].unwrap();
            HardSoftScore::from_levels(vec![if x == y { -1 } else { 0 }, x + y]).unwrap()
        },
        20,
    );
    assert!(result.is_optimal());
    assert_eq!(result.evaluations, 9);
    assert_eq!(result.best.unwrap().score, HardSoftScore::new(0, 5));
}
