// Adapted from the eight pinned Community *ScoreDefinitionTest classes.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn decimals(values: &[&str]) -> Vec<Decimal> {
    values.iter().map(|v| v.parse().unwrap()).collect()
}
macro_rules! bounds {
    ($score:ty, $values:expr) => {
        #[test]
        fn optimistic_only_up() {
            let score = <$score>::from_levels($values).unwrap();
            let trend = InitializingScoreTrend::parse("ONLY_UP", <$score>::level_count()).unwrap();
            assert_eq!(
                score
                    .finite_optimistic_bound(&trend)
                    .unwrap()
                    .ordinary_levels()
                    .collect::<Vec<_>>(),
                vec![i64::MAX; <$score>::level_count()]
            );
        }
        #[test]
        fn optimistic_only_down() {
            let score = <$score>::from_levels($values).unwrap();
            let trend =
                InitializingScoreTrend::parse("ONLY_DOWN", <$score>::level_count()).unwrap();
            assert_eq!(
                score
                    .finite_optimistic_bound(&trend)
                    .unwrap()
                    .ordinary_levels()
                    .collect::<Vec<_>>(),
                $values
            );
        }
        #[test]
        fn pessimistic_only_up() {
            let score = <$score>::from_levels($values).unwrap();
            let trend = InitializingScoreTrend::parse("ONLY_UP", <$score>::level_count()).unwrap();
            assert_eq!(
                score
                    .finite_pessimistic_bound(&trend)
                    .unwrap()
                    .ordinary_levels()
                    .collect::<Vec<_>>(),
                $values
            );
        }
        #[test]
        fn pessimistic_only_down() {
            let score = <$score>::from_levels($values).unwrap();
            let trend =
                InitializingScoreTrend::parse("ONLY_DOWN", <$score>::level_count()).unwrap();
            assert_eq!(
                score
                    .finite_pessimistic_bound(&trend)
                    .unwrap()
                    .ordinary_levels()
                    .collect::<Vec<_>>(),
                vec![i64::MIN; <$score>::level_count()]
            );
        }
    };
}
mod simple {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(<SimpleScore>::zero_score(), <SimpleScore>::ZERO);
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <SimpleScore>::one_softest_score().unwrap(),
            SimpleScore::new(1)
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(SimpleScore::level_count(), 1);
    }
    #[test]
    fn labels() {
        assert_eq!(SimpleScore::level_labels(), ["score"]);
    }
    bounds!(SimpleScore, vec![-1]);
    #[test]
    fn divide() {
        let dividend = <SimpleScore>::from_levels(vec![10]).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<SimpleScore>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<SimpleScore>::one_softest_score().unwrap())
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor = <SimpleScore>::from_levels(vec![10]).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <SimpleScore>::from_levels(vec![1]).unwrap()
        );
    }
}
mod hardsoft {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(<HardSoftScore>::zero_score(), <HardSoftScore>::ZERO);
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <HardSoftScore>::one_softest_score().unwrap(),
            HardSoftScore::new(0, 1)
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(HardSoftScore::level_count(), 2);
    }
    #[test]
    fn labels() {
        assert_eq!(HardSoftScore::level_labels(), ["hard score", "soft score"]);
    }
    #[test]
    fn feasible_count() {
        assert_eq!(HardSoftScore::feasible_level_count(), 1);
    }
    bounds!(HardSoftScore, vec![-1, -2]);
    #[test]
    fn divide() {
        let dividend = <HardSoftScore>::from_levels(vec![0, 10]).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<HardSoftScore>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<HardSoftScore>::one_softest_score().unwrap())
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor = <HardSoftScore>::from_levels(vec![10, 10]).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <HardSoftScore>::from_levels(vec![0, 1]).unwrap()
        );
    }
}
mod hardmediumsoft {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(
            <HardMediumSoftScore>::zero_score(),
            <HardMediumSoftScore>::ZERO
        );
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <HardMediumSoftScore>::one_softest_score().unwrap(),
            HardMediumSoftScore::new(0, 0, 1)
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(HardMediumSoftScore::level_count(), 3);
    }
    #[test]
    fn labels() {
        assert_eq!(
            HardMediumSoftScore::level_labels(),
            ["hard score", "medium score", "soft score"]
        );
    }
    #[test]
    fn feasible_count() {
        assert_eq!(HardMediumSoftScore::feasible_level_count(), 1);
    }
    bounds!(HardMediumSoftScore, vec![-1, -2, -3]);
    #[test]
    fn divide() {
        let dividend = <HardMediumSoftScore>::from_levels(vec![0, 1, 10]).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<HardMediumSoftScore>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<HardMediumSoftScore>::one_softest_score().unwrap())
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor = <HardMediumSoftScore>::from_levels(vec![10, 10, 10]).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <HardMediumSoftScore>::from_levels(vec![0, 0, 1]).unwrap()
        );
    }
}
mod bendable {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(
            <BendableScore<1, 2>>::zero_score(),
            <BendableScore<1, 2>>::ZERO
        );
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <BendableScore<1, 2>>::one_softest_score().unwrap(),
            BendableScore::new([0], [0, 1])
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(<BendableScore<1, 1>>::level_count(), 2);
        assert_eq!(<BendableScore<3, 4>>::level_count(), 7);
        assert_eq!(<BendableScore<4, 3>>::level_count(), 7);
        assert_eq!(<BendableScore<0, 5>>::level_count(), 5);
        assert_eq!(<BendableScore<5, 0>>::level_count(), 5);
    }
    #[test]
    fn labels() {
        assert_eq!(
            <BendableScore<1, 1>>::level_labels(),
            ["hard 0 score", "soft 0 score"]
        );
        assert_eq!(
            <BendableScore<3, 4>>::level_labels(),
            [
                "hard 0 score",
                "hard 1 score",
                "hard 2 score",
                "soft 0 score",
                "soft 1 score",
                "soft 2 score",
                "soft 3 score"
            ]
        );
        assert_eq!(
            <BendableScore<4, 3>>::level_labels(),
            [
                "hard 0 score",
                "hard 1 score",
                "hard 2 score",
                "hard 3 score",
                "soft 0 score",
                "soft 1 score",
                "soft 2 score"
            ]
        );
        assert_eq!(
            <BendableScore<0, 5>>::level_labels(),
            [
                "soft 0 score",
                "soft 1 score",
                "soft 2 score",
                "soft 3 score",
                "soft 4 score"
            ]
        );
        assert_eq!(
            <BendableScore<5, 0>>::level_labels(),
            [
                "hard 0 score",
                "hard 1 score",
                "hard 2 score",
                "hard 3 score",
                "hard 4 score"
            ]
        );
    }
    #[test]
    fn feasible_count() {
        assert_eq!(<BendableScore<1, 1>>::feasible_level_count(), 1);
        assert_eq!(<BendableScore<3, 4>>::feasible_level_count(), 3);
        assert_eq!(<BendableScore<4, 3>>::feasible_level_count(), 4);
        assert_eq!(<BendableScore<0, 5>>::feasible_level_count(), 0);
        assert_eq!(<BendableScore<5, 0>>::feasible_level_count(), 5);
    }
    #[test]
    fn invalid_shape() {
        assert!(matches!(
            <BendableScore<2, 3>>::from_levels(vec![1, 2, 3]),
            Err(ScoreDefinitionError::LevelCount {
                expected: 5,
                actual: 3
            })
        ));
    }
    #[test]
    fn create() {
        let values: Vec<_> = (0..5).map(|i| i64::from(i32::MAX) + i).collect();
        let score = BendableScore::<3, 2>::from_levels(values.clone()).unwrap();
        assert_eq!(score.hard_levels().len(), 3);
        assert_eq!(score.soft_levels().len(), 2);
        assert_eq!(score.hard_levels().as_slice(), &values[..3]);
        assert_eq!(score.soft_levels().as_slice(), &values[3..]);
    }
    bounds!(BendableScore<2,3>, vec![-1,-2,-3,-4,-5]);
    #[test]
    fn divide() {
        let dividend = <BendableScore<1, 1>>::from_levels(vec![0, 10]).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<BendableScore<1, 1>>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<BendableScore<1, 1>>::one_softest_score().unwrap())
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor = <BendableScore<1, 1>>::one_softest_score().unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <BendableScore<1, 1>>::from_levels(vec![0, 10]).unwrap()
        );
    }
}
mod simple_decimal {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(
            <SimpleDecimalScore>::zero_score(),
            <SimpleDecimalScore>::ZERO
        );
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <SimpleDecimalScore>::one_softest_score().unwrap(),
            SimpleDecimalScore::new(Decimal::from(1))
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(SimpleDecimalScore::level_count(), 1);
    }
    #[test]
    fn labels() {
        assert_eq!(SimpleDecimalScore::level_labels(), ["score"]);
    }
    #[test]
    fn divide() {
        let dividend = <SimpleDecimalScore>::from_levels(decimals(&["10"])).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<SimpleDecimalScore>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<SimpleDecimalScore>::one_softest_score().unwrap())
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor = <SimpleDecimalScore>::from_levels(decimals(&["10"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <SimpleDecimalScore>::from_levels(decimals(&["1"])).unwrap()
        );
    }
    #[test]
    fn negative_scale() {
        let dividend = <SimpleDecimalScore>::from_levels(decimals(&["1E+1"])).unwrap();
        let divisor = <SimpleDecimalScore>::from_levels(decimals(&["2.00"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&divisor).unwrap(),
            <SimpleDecimalScore>::from_levels(decimals(&["5"])).unwrap()
        );
    }
}
mod hardsoft_decimal {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(
            <HardSoftDecimalScore>::zero_score(),
            <HardSoftDecimalScore>::ZERO
        );
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <HardSoftDecimalScore>::one_softest_score().unwrap(),
            HardSoftDecimalScore::new(Decimal::ZERO, Decimal::from(1))
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(HardSoftDecimalScore::level_count(), 2);
    }
    #[test]
    fn labels() {
        assert_eq!(
            HardSoftDecimalScore::level_labels(),
            ["hard score", "soft score"]
        );
    }
    #[test]
    fn feasible_count() {
        assert_eq!(HardSoftDecimalScore::feasible_level_count(), 1);
    }
    #[test]
    fn divide() {
        let dividend = <HardSoftDecimalScore>::from_levels(decimals(&["0", "10"])).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<HardSoftDecimalScore>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<HardSoftDecimalScore>::one_softest_score().unwrap())
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor = <HardSoftDecimalScore>::from_levels(decimals(&["10", "10"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <HardSoftDecimalScore>::from_levels(decimals(&["0", "1"])).unwrap()
        );
    }
    #[test]
    fn negative_scale() {
        let dividend = <HardSoftDecimalScore>::from_levels(decimals(&["1E+1", "10"])).unwrap();
        let divisor = <HardSoftDecimalScore>::from_levels(decimals(&["2.00", "2"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&divisor).unwrap(),
            <HardSoftDecimalScore>::from_levels(decimals(&["5", "5"])).unwrap()
        );
    }
}
mod hardmediumsoft_decimal {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(
            <HardMediumSoftDecimalScore>::zero_score(),
            <HardMediumSoftDecimalScore>::ZERO
        );
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <HardMediumSoftDecimalScore>::one_softest_score().unwrap(),
            HardMediumSoftDecimalScore::new(Decimal::ZERO, Decimal::ZERO, Decimal::from(1))
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(HardMediumSoftDecimalScore::level_count(), 3);
    }
    #[test]
    fn labels() {
        assert_eq!(
            HardMediumSoftDecimalScore::level_labels(),
            ["hard score", "medium score", "soft score"]
        );
    }
    #[test]
    fn feasible_count() {
        assert_eq!(HardMediumSoftDecimalScore::feasible_level_count(), 1);
    }
    #[test]
    fn divide() {
        let dividend =
            <HardMediumSoftDecimalScore>::from_levels(decimals(&["0", "1", "10"])).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<HardMediumSoftDecimalScore>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(
                    &<HardMediumSoftDecimalScore>::one_softest_score().unwrap()
                )
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor =
            <HardMediumSoftDecimalScore>::from_levels(decimals(&["10", "10", "10"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <HardMediumSoftDecimalScore>::from_levels(decimals(&["0", "0", "1"])).unwrap()
        );
    }
    #[test]
    fn negative_scale() {
        let dividend =
            <HardMediumSoftDecimalScore>::from_levels(decimals(&["1E+1", "10", "10"])).unwrap();
        let divisor =
            <HardMediumSoftDecimalScore>::from_levels(decimals(&["2.00", "2", "2"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&divisor).unwrap(),
            <HardMediumSoftDecimalScore>::from_levels(decimals(&["5", "5", "5"])).unwrap()
        );
    }
}
mod bendable_decimal {
    use super::*;
    #[test]
    fn zero() {
        assert_eq!(
            <BendableDecimalScore<1, 2>>::zero_score(),
            <BendableDecimalScore<1, 2>>::ZERO
        );
    }
    #[test]
    fn one_softest() {
        assert_eq!(
            <BendableDecimalScore<1, 2>>::one_softest_score().unwrap(),
            BendableDecimalScore::new([Decimal::ZERO], [Decimal::ZERO, Decimal::from(1)])
        );
    }
    #[test]
    fn level_count() {
        assert_eq!(<BendableDecimalScore<1, 1>>::level_count(), 2);
        assert_eq!(<BendableDecimalScore<3, 4>>::level_count(), 7);
        assert_eq!(<BendableDecimalScore<4, 3>>::level_count(), 7);
        assert_eq!(<BendableDecimalScore<0, 5>>::level_count(), 5);
        assert_eq!(<BendableDecimalScore<5, 0>>::level_count(), 5);
    }
    #[test]
    fn labels() {
        assert_eq!(
            <BendableDecimalScore<1, 1>>::level_labels(),
            ["hard 0 score", "soft 0 score"]
        );
        assert_eq!(
            <BendableDecimalScore<3, 4>>::level_labels(),
            [
                "hard 0 score",
                "hard 1 score",
                "hard 2 score",
                "soft 0 score",
                "soft 1 score",
                "soft 2 score",
                "soft 3 score"
            ]
        );
        assert_eq!(
            <BendableDecimalScore<4, 3>>::level_labels(),
            [
                "hard 0 score",
                "hard 1 score",
                "hard 2 score",
                "hard 3 score",
                "soft 0 score",
                "soft 1 score",
                "soft 2 score"
            ]
        );
        assert_eq!(
            <BendableDecimalScore<0, 5>>::level_labels(),
            [
                "soft 0 score",
                "soft 1 score",
                "soft 2 score",
                "soft 3 score",
                "soft 4 score"
            ]
        );
        assert_eq!(
            <BendableDecimalScore<5, 0>>::level_labels(),
            [
                "hard 0 score",
                "hard 1 score",
                "hard 2 score",
                "hard 3 score",
                "hard 4 score"
            ]
        );
    }
    #[test]
    fn feasible_count() {
        assert_eq!(<BendableDecimalScore<1, 1>>::feasible_level_count(), 1);
        assert_eq!(<BendableDecimalScore<3, 4>>::feasible_level_count(), 3);
        assert_eq!(<BendableDecimalScore<4, 3>>::feasible_level_count(), 4);
        assert_eq!(<BendableDecimalScore<0, 5>>::feasible_level_count(), 0);
        assert_eq!(<BendableDecimalScore<5, 0>>::feasible_level_count(), 5);
    }
    #[test]
    fn invalid_shape() {
        assert!(matches!(
            <BendableDecimalScore<2, 3>>::from_levels(decimals(&["1", "2", "3"])),
            Err(ScoreDefinitionError::LevelCount {
                expected: 5,
                actual: 3
            })
        ));
    }
    #[test]
    fn create() {
        fn check<const H: usize, const S: usize>() {
            let values: Vec<_> = (0..H + S).map(|i| Decimal::from(i as i64)).collect();
            let score = BendableDecimalScore::<H, S>::from_levels(values.clone()).unwrap();
            assert_eq!(score.hard_levels().len(), H);
            assert_eq!(score.soft_levels().len(), S);
            assert_eq!(score.hard_levels().as_slice(), &values[..H]);
            assert_eq!(score.soft_levels().as_slice(), &values[H..]);
        }
        check::<1, 1>();
        check::<1, 2>();
        check::<1, 3>();
        check::<1, 4>();
        check::<2, 1>();
        check::<2, 2>();
        check::<2, 3>();
        check::<2, 4>();
        check::<3, 1>();
        check::<3, 2>();
        check::<3, 3>();
        check::<3, 4>();
        check::<4, 1>();
        check::<4, 2>();
        check::<4, 3>();
        check::<4, 4>();
    }
    #[test]
    fn divide() {
        let dividend = <BendableDecimalScore<1, 1>>::from_levels(decimals(&["0", "10"])).unwrap();
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(&<BendableDecimalScore<1, 1>>::zero_score())
                .unwrap(),
            dividend
        );
        assert_eq!(
            dividend
                .divide_by_sanitized_divisor(
                    &<BendableDecimalScore<1, 1>>::one_softest_score().unwrap()
                )
                .unwrap(),
            dividend
        );
        // The integer bendable source deliberately uses ONE again for its tenDivisor.
        let ten_divisor =
            <BendableDecimalScore<1, 1>>::from_levels(decimals(&["10", "10"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&ten_divisor).unwrap(),
            <BendableDecimalScore<1, 1>>::from_levels(decimals(&["0", "1"])).unwrap()
        );
    }
    #[test]
    fn negative_scale() {
        let dividend =
            <BendableDecimalScore<1, 1>>::from_levels(decimals(&["1E+1", "10"])).unwrap();
        let divisor = <BendableDecimalScore<1, 1>>::from_levels(decimals(&["2.00", "2"])).unwrap();
        assert_eq!(
            dividend.divide_by_sanitized_divisor(&divisor).unwrap(),
            <BendableDecimalScore<1, 1>>::from_levels(decimals(&["5", "5"])).unwrap()
        );
    }
}
