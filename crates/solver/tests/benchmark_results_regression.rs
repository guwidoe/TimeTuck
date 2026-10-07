// Original tests: no upstream case credit.
use timetuck::*;
fn success(n: i64, complete: bool) -> BenchmarkOutcome {
    BenchmarkOutcome::success(&SimpleScore::new(n), complete).unwrap()
}
#[test]
fn exact_totals_do_not_overflow_and_rank_failures_before_numeric_quality() {
    let a = BenchmarkSummary::new([success(i64::MAX, true), success(i64::MAX, true)]);
    let expected = Decimal::from(BigInt::from(i64::MAX) * 2);
    assert_eq!(a.total_score().unwrap().levels(), &[expected]);
    let failed = BenchmarkSummary::new([success(i64::MAX, true), BenchmarkOutcome::Failure]);
    let partial = BenchmarkSummary::new([success(i64::MAX, false), success(i64::MAX, true)]);
    let complete = BenchmarkSummary::new([success(i64::MIN, true), success(i64::MIN, true)]);
    let empty = BenchmarkSummary::new([]);
    for ranking in [BenchmarkRanking::TotalScore, BenchmarkRanking::WorstScore] {
        assert!(complete.compare(&partial, ranking).is_gt());
        assert!(partial.compare(&failed, ranking).is_gt());
        assert!(failed.compare(&empty, ranking).is_gt());
    }
    assert_eq!(
        (
            failed.run_count(),
            failed.failure_count(),
            failed.incomplete_count()
        ),
        (2, 1, 0)
    );
    assert_eq!(partial.incomplete_count(), 1);
    let flawed = BenchmarkSummary::new([BenchmarkOutcome::success(
        &SimpleScore::new(99).with_structural(-1),
        true,
    )
    .unwrap()]);
    assert_eq!(flawed.incomplete_count(), 1);
    assert_eq!(flawed.total_score().unwrap().structural(), 0);
    assert!(empty.total_score().is_none());
}
#[test]
fn decimal_ranking_ignores_scale_and_padding_and_is_transitive() {
    let a = BenchmarkScore::from_score(&"1.00".parse::<SimpleDecimalScore>().unwrap()).unwrap();
    let b = BenchmarkScore::from_score(&SimpleScore::new(1)).unwrap();
    let c = BenchmarkScore::from_score(&HardSoftScore::new(1, 0)).unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(a, c);
    let low = BenchmarkScore::from_score(&HardSoftScore::new(1, -1)).unwrap();
    let high = BenchmarkScore::from_score(&HardSoftScore::new(1, 1)).unwrap();
    assert!(low < a && a < high && low < high);
    let a = BenchmarkSummary::new([
        BenchmarkOutcome::success(&"0.10".parse::<SimpleDecimalScore>().unwrap(), true).unwrap(),
        BenchmarkOutcome::success(&"0.20".parse::<SimpleDecimalScore>().unwrap(), true).unwrap(),
    ]);
    assert_eq!(
        a.total_score().unwrap().levels()[0].numeric_cmp(&"0.3".parse().unwrap()),
        std::cmp::Ordering::Equal
    );
}
#[test]
fn percentages_preserve_tiny_changes_extreme_scales_zero_and_ieee_arithmetic() {
    let pct = |a: i64, b: i64| {
        ScoreDifferencePercentage::between(&SimpleScore::new(a), &SimpleScore::new(b)).unwrap()
    };
    assert_eq!(pct(0, 0).levels(), &[0.0]);
    assert_eq!(pct(0, 1).levels(), &[f64::INFINITY]);
    assert_eq!(pct(0, -1).levels(), &[f64::NEG_INFINITY]);
    let tiny = pct(i64::MAX - 1, i64::MAX).levels()[0];
    assert!(tiny > 0.0 && tiny < 1e-18);
    for scale in [i32::MIN, i32::MAX, -1000, 1000] {
        let a = SimpleDecimalScore::new(Decimal::new(10.into(), scale));
        let b = SimpleDecimalScore::new(Decimal::new(12.into(), scale));
        assert!(
            (ScoreDifferencePercentage::between(&a, &b).unwrap().levels()[0] - 0.2).abs() < 1e-15
        );
    }
    assert_eq!(pct(-100, -200).divide(2.0).levels(), &[-0.5]);
    assert_eq!(pct(-100, -200).divide(0.0).levels(), &[f64::NEG_INFINITY]);
    assert!(pct(0, 1).subtract(&pct(0, 1)).unwrap().levels()[0].is_nan());
    assert_eq!(
        ScoreDifferencePercentage::between(
            &SimpleScore::new(3).with_structural(-1),
            &SimpleScore::new(3)
        )
        .unwrap()
        .levels(),
        &[0.0]
    );
}
#[test]
fn actual_solver_results_can_be_ranked_across_search_budgets() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![-10_i64, -5, -1])]).unwrap();
    let solve = |budget| {
        exhaustive_search(
            &problem,
            |a| SimpleScore::new(a.values()[0].unwrap()),
            budget,
        )
        .best
        .unwrap()
        .score
    };
    let limited = solve(1);
    let full = solve(3);
    let a = BenchmarkSummary::new([BenchmarkOutcome::success(&limited, true).unwrap()]);
    let b = BenchmarkSummary::new([BenchmarkOutcome::success(&full, true).unwrap()]);
    assert!(b.compare(&a, BenchmarkRanking::TotalScore).is_gt());
    assert!(b.compare(&a, BenchmarkRanking::WorstScore).is_gt());
    assert!(
        (ScoreDifferencePercentage::between(&limited, &full)
            .unwrap()
            .levels()[0]
            - 0.9)
            .abs()
            < 1e-15
    );
}
