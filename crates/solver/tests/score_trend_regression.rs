//! Original independent references for trends and symbolic lexicographic bounds.
use InitializingScoreTrendLevel::{Any, OnlyDown, OnlyUp};
use std::cmp::Ordering;
use timetuck::*;

#[test]
fn parsing_repetition_width_empty_scores_and_invalid_tokens_are_explicit() {
    for (token, level) in [("ANY", Any), ("ONLY_UP", OnlyUp), ("ONLY_DOWN", OnlyDown)] {
        for width in 0..=5 {
            assert_eq!(
                InitializingScoreTrend::parse(token, width)
                    .unwrap()
                    .levels(),
                vec![level; width]
            );
        }
    }
    for text in ["", "only_down", " ANY", "ANY ", "ANY//ONLY_UP", "ANY/"] {
        assert!(matches!(
            InitializingScoreTrend::parse(text, 2),
            Err(ScoreTrendError::UnknownLevel { .. })
        ));
    }
    assert_eq!(
        InitializingScoreTrend::parse("ANY/ONLY_UP", 3),
        Err(ScoreTrendError::LevelCount {
            expected: 3,
            actual: 2
        })
    );
    let t = InitializingScoreTrend::new(vec![OnlyDown]);
    assert_eq!(
        t.optimistic_bound(HardSoftScore::ZERO).unwrap_err(),
        ScoreTrendError::LevelCount {
            expected: 2,
            actual: 1
        }
    );
    let t = InitializingScoreTrend::new(vec![]);
    assert!(t.is_only_up() && t.is_only_down());
    assert_eq!(
        t.optimistic_bound(BendableScore::<0, 0>::ZERO)
            .unwrap()
            .compare_score(&BendableScore::ZERO),
        Ordering::Equal
    );
}

#[test]
fn derived_intervals_contain_every_independent_allowed_completion() {
    // Independent component-wise relation defines legal completions. Every one
    // must belong to the lexicographic interval, including hard infeasibility.
    for a in [OnlyDown, Any, OnlyUp] {
        for b in [OnlyDown, Any, OnlyUp] {
            let t = InitializingScoreTrend::new(vec![a, b]);
            for x in -2..=2 {
                for y in -2..=2 {
                    let partial = HardSoftScore::new(x, y).with_structural(-3);
                    let lo = t.pessimistic_bound(partial).unwrap();
                    let hi = t.optimistic_bound(partial).unwrap();
                    assert!(lo <= hi);
                    for u in -3..=3 {
                        for v in -3..=3 {
                            let legal = |trend, p, c| match trend {
                                Any => true,
                                OnlyDown => c <= p,
                                OnlyUp => c >= p,
                            };
                            if legal(a, x, u) && legal(b, y, v) {
                                let full = HardSoftScore::new(u, v);
                                assert!(!lo.compare_score(&full).is_gt());
                                assert!(!hi.compare_score(&full).is_lt());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn infinities_are_not_integer_extrema_and_preserve_decimal_differences() {
    let t = InitializingScoreTrend::parse("ONLY_DOWN/ANY", 2).unwrap();
    let hi = t
        .optimistic_bound(HardSoftScore::new(i64::MIN, 42))
        .unwrap();
    assert!(
        hi.compare_score(&HardSoftScore::new(i64::MIN, i64::MAX))
            .is_gt()
    );
    assert!(
        hi.compare_score(&HardSoftScore::new(i64::MIN + 1, i64::MIN))
            .is_lt()
    );
    let lo = InitializingScoreTrend::parse("ONLY_UP/ANY", 2)
        .unwrap()
        .pessimistic_bound(HardSoftScore::new(i64::MAX, 42))
        .unwrap();
    assert!(
        lo.compare_score(&HardSoftScore::new(i64::MAX, i64::MIN))
            .is_lt()
    );
    let small: HardSoftDecimalScore = "9007199254740992.000000000000000001hard/0soft"
        .parse()
        .unwrap();
    let big: HardSoftDecimalScore = "9007199254740992.000000000000000002hard/0soft"
        .parse()
        .unwrap();
    let hi = t.optimistic_bound(small.clone()).unwrap();
    assert!(hi.compare_score(&big).is_lt());
    assert!(hi.compare_score(&small).is_gt());
    let repr: HardSoftDecimalScore =
        "9007199254740992.0000000000000000010hard/9999999999999999999999999999999999999999999soft"
            .parse()
            .unwrap();
    assert_eq!(hi, t.optimistic_bound(repr).unwrap());
}

#[test]
fn bound_order_is_transitive_and_ignores_every_level_after_infinity() {
    let mut bounds = Vec::new();
    for text in [
        "ANY",
        "ONLY_DOWN",
        "ONLY_UP",
        "ONLY_DOWN/ANY",
        "ONLY_UP/ANY",
        "ANY/ONLY_DOWN",
    ] {
        let t = InitializingScoreTrend::parse(text, 2).unwrap();
        for a in [-1, 0, 1] {
            for b in [-1, 0, 1] {
                bounds.push(t.optimistic_bound(HardSoftScore::new(a, b)).unwrap());
                bounds.push(t.pessimistic_bound(HardSoftScore::new(a, b)).unwrap());
            }
        }
    }
    bounds.sort();
    for (i, a) in bounds.iter().enumerate() {
        assert_eq!(a.cmp(a), Ordering::Equal);
        for b in &bounds[i..] {
            assert!(!a.cmp(b).is_gt());
            assert_eq!(a.cmp(b), b.cmp(a).reverse());
        }
    }
    let a = InitializingScoreTrend::parse("ANY/ONLY_DOWN", 2)
        .unwrap()
        .optimistic_bound(HardSoftScore::new(-99, 99))
        .unwrap();
    let b = InitializingScoreTrend::parse("ANY/ONLY_UP", 2)
        .unwrap()
        .optimistic_bound(HardSoftScore::new(99, -99))
        .unwrap();
    assert_eq!(a, b);
}

#[test]
fn three_level_order_matches_independent_finite_score_cuts() {
    let mut cases = Vec::new();
    for a in [Any, OnlyUp, OnlyDown] {
        for b in [Any, OnlyUp, OnlyDown] {
            for c in [Any, OnlyUp, OnlyDown] {
                let levels = [a, b, c];
                let trend = InitializingScoreTrend::new(levels.to_vec());
                for values in [[-2, 4, 0], [0, 0, 0], [0, 1, -3], [2, -4, 1]] {
                    let s = HardMediumSoftScore::new(values[0], values[1], values[2])
                        .with_structural(-9);
                    for upper in [false, true] {
                        let reference: Vec<_> = levels
                            .iter()
                            .zip(values)
                            .map(|(t, v)| {
                                if *t == if upper { OnlyDown } else { OnlyUp } {
                                    (1, v)
                                } else if upper {
                                    (2, 0)
                                } else {
                                    (0, 0)
                                }
                            })
                            .collect();
                        let bound = if upper {
                            trend.optimistic_bound(s)
                        } else {
                            trend.pessimistic_bound(s)
                        }
                        .unwrap();
                        assert_eq!(bound.partial_score().structural(), 0);
                        assert!(bound.compare_score(&s).is_gt()); // Structural state is compared first.
                        assert!(bound.compare_score(&s.with_structural(1)).is_lt());
                        cases.push((bound, reference));
                    }
                }
            }
        }
    }
    for (a, expected_a) in &cases {
        for (b, expected_b) in &cases {
            // Bounds partition finite scores. Once both sides reach the same
            // infinity, subsequent levels do not distinguish these partitions.
            let expected = expected_a
                .iter()
                .zip(expected_b)
                .find_map(|(a, b)| {
                    if a != b {
                        Some(a.cmp(b))
                    } else if a.0 != 1 {
                        Some(Ordering::Equal)
                    } else {
                        None
                    }
                })
                .unwrap_or(Ordering::Equal);
            assert_eq!(a.cmp(b), expected, "{a:?} / {b:?}");
        }
    }
}
