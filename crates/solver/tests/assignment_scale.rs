//! Original count/scale agreement and numeric boundary tests.
use timetuck::*;
fn compare(p: &impl ProblemStatistics) {
    let count = p.problem_size_statistics().assignment_space;
    let expected = ProblemScale::from_count(&count.count).log10();
    let actual = p.assignment_scale();
    assert_eq!(actual.upper_bound, !count.exact);
    if expected.is_finite() {
        assert!(
            (actual.scale.log10() - expected).abs() <= 1e-10,
            "{} != {expected}",
            actual.scale.log10()
        );
    } else {
        assert_eq!(actual.scale.log10(), expected);
    }
}
#[test]
fn exact_counts_and_logs_agree_for_basic_list_and_mixed_models() {
    for optional in [false, true] {
        for pinned in 0..3 {
            for restricted in [false, true] {
                for dependencies in [false, true] {
                    let mut entity = ListEntity::new(vec![0]);
                    if pinned == 1 {
                        entity = entity.with_pinned_prefix(1);
                    }
                    if pinned == 2 {
                        entity = entity.with_pinned(true);
                    }
                    let mut other = ListEntity::new(vec![]);
                    if restricted {
                        other = other.with_value_range(vec![1, 2]);
                    }
                    let mut list = ListProblem::new(vec![0, 1, 2], vec![entity, other]).unwrap();
                    if optional {
                        list = list.allowing_unassigned();
                    }
                    if dependencies {
                        list = list
                            .with_dependencies(vec![vec![], vec![0], vec![1]])
                            .unwrap();
                    }
                    let mut variable = Variable::new(vec![0, 1]);
                    if optional {
                        variable = variable.allowing_unassigned();
                    }
                    let basic = FiniteProblem::new(vec![
                        variable,
                        Variable::new(vec![1])
                            .with_initial(1)
                            .with_pinned(pinned > 0),
                    ])
                    .unwrap();
                    compare(&basic);
                    compare(&list);
                    compare(&MixedProblem::new(basic, list));
                }
            }
        }
    }
    for n in 0..=20 {
        for owners in 0..=5 {
            for optional in [false, true] {
                let mut p = ListProblem::new(
                    (0..n).collect::<Vec<_>>(),
                    vec![ListEntity::new(vec![]); owners],
                )
                .unwrap();
                if optional {
                    p = p.allowing_unassigned();
                }
                compare(&p);
            }
        }
    }
    for optional in [false, true] {
        for pinned in [false, true] {
            let mut v = Variable::<usize>::new(vec![]).with_pinned(pinned);
            if optional {
                v = v.allowing_unassigned();
            }
            compare(&FiniteProblem::new(vec![v]).unwrap());
        }
    }
    compare(&FiniteProblem::<usize>::new(vec![]).unwrap());
}
#[test]
fn very_large_scales_stay_finite_without_materializing_the_count() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 100_000]).unwrap();
    assert!(
        (p.assignment_scale().scale.log10() - 100_000.0 * std::f64::consts::LOG10_2).abs() < 1e-9
    );
    let p = ListProblem::new(
        (0..1000).collect::<Vec<_>>(),
        vec![ListEntity::new(vec![]); 10],
    )
    .unwrap();
    compare(&p);
    compare(&p.allowing_unassigned());
}
#[test]
fn checked_combinatorics_reject_invalid_inputs_and_saturate_logs_explicitly() {
    assert_eq!(
        binomial_coefficient(66, 33).unwrap(),
        7_219_428_434_016_265_740
    );
    assert!(matches!(
        binomial_coefficient(67, 33),
        Err(CombinatoricsError::BinomialOverflow { .. })
    ));
    for (n, k) in [(-1, 0), (1, -1), (1, 2)] {
        assert!(matches!(
            binomial_coefficient(n, k),
            Err(CombinatoricsError::InvalidBinomial { .. })
        ));
    }
    for base in [0, 1] {
        assert!(log_in_base(base, 2).is_err());
        assert!(scaled_log(0, base, 0).is_err());
        assert!(list_arrangements_scaled_log(1, base, 0, 0).is_err());
    }
    assert_eq!(log_in_base(2, 0).unwrap(), f64::NEG_INFINITY);
    assert_eq!(scaled_log(100, 2, 0).unwrap(), i64::MIN);
    assert_eq!(scaled_log(0, 2, 0).unwrap(), 0);
    assert_eq!(scaled_log(u64::MAX, 2, 2).unwrap(), i64::MAX);
    assert_eq!(
        list_arrangements_scaled_log(100, 2, 1, 0).unwrap(),
        i64::MIN
    );
    assert_eq!(list_arrangements_scaled_log(0, 2, 1, 0).unwrap(), 0);
    assert_eq!(list_arrangements_scaled_log(100, 2, 0, 0).unwrap(), 0);
    for n in 0..=60 {
        for k in 0..=n {
            assert_eq!(binomial_coefficient(n, k), binomial_coefficient(n, n - k));
            if k > 0 && k < n {
                assert_eq!(
                    binomial_coefficient(n, k).unwrap(),
                    binomial_coefficient(n - 1, k - 1).unwrap()
                        + binomial_coefficient(n - 1, k).unwrap()
                );
            }
        }
    }
}
