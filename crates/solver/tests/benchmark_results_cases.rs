// Adapted from pinned Community benchmark ranking/result tests, Apache-2.0.
// Full provenance and helper-fixture attribution: compat/provenance.json.
use std::cmp::Ordering;
use timetuck::*;
fn simple(n: i64) -> BenchmarkScore {
    BenchmarkScore::from_score(&SimpleScore::new(n)).unwrap()
}
fn hs(h: i64, s: i64) -> BenchmarkScore {
    BenchmarkScore::from_score(&HardSoftScore::new(h, s)).unwrap()
}
fn summary(scores: &[i64], partial: &[usize]) -> BenchmarkSummary {
    BenchmarkSummary::new(
        scores
            .iter()
            .enumerate()
            .map(|(i, &n)| BenchmarkOutcome::Success {
                score: simple(n),
                initialized: !partial.contains(&i),
            }),
    )
}
fn order(a: &BenchmarkSummary, b: &BenchmarkSummary, mode: BenchmarkRanking) {
    assert_eq!(a.compare(b, mode), Ordering::Less);
    assert_eq!(b.compare(a, mode), Ordering::Greater);
    assert_eq!(a.compare(a, mode), Ordering::Equal);
    assert_eq!(b.compare(b, mode), Ordering::Equal);
}
#[test]
fn total_normal() {
    order(
        &summary(&[-1000, -400, -30], &[]),
        &summary(&[-1000, -200, -50], &[]),
        BenchmarkRanking::TotalScore,
    );
}
#[test]
fn total_is_equal() {
    order(
        &summary(&[-1005, -200, -30], &[]),
        &summary(&[-1000, -200, -35], &[]),
        BenchmarkRanking::TotalScore,
    );
}
fn different_definitions(mode: BenchmarkRanking) {
    let a = summary(&[-1000, -400, -30], &[]);
    let b = BenchmarkSummary::new([hs(0, -1000), hs(0, -200), hs(-7, -50)].map(|score| {
        BenchmarkOutcome::Success {
            score,
            initialized: true,
        }
    }));
    order(&a, &b, mode);
}
#[test]
fn total_different_score_definitions() {
    different_definitions(BenchmarkRanking::TotalScore);
}
#[test]
fn total_uninitialized_single_benchmarks() {
    let scores = [-1000, -400, -30];
    let mode = BenchmarkRanking::TotalScore;
    assert_eq!(
        summary(&scores, &[]).compare(&summary(&scores, &[]), mode),
        Ordering::Equal
    );
    let a = summary(&scores, &[0]);
    order(&a, &summary(&scores, &[1]), mode);
    order(&summary(&scores, &[0, 1]), &a, mode);
    order(&a, &summary(&scores, &[1]), mode);
}
#[test]
fn worst_normal() {
    order(
        &summary(&[-100, -2001, -30], &[]),
        &summary(&[-900, -2000, -30], &[]),
        BenchmarkRanking::WorstScore,
    );
}
#[test]
fn worst_is_equal() {
    order(
        &summary(&[-101, -2000, -30], &[]),
        &summary(&[-100, -2000, -40], &[]),
        BenchmarkRanking::WorstScore,
    );
}
#[test]
fn worst_different_score_definitions() {
    different_definitions(BenchmarkRanking::WorstScore);
}
#[test]
fn worst_uninitialized_single_benchmarks() {
    let mode = BenchmarkRanking::WorstScore;
    assert_eq!(
        summary(&[-1000, -400, -30], &[]).compare(&summary(&[-1000, -400, -30], &[]), mode),
        Ordering::Equal
    );
    let a = summary(&[-1000, -400, -30], &[0]);
    order(&summary(&[-1000, 400, -30], &[0, 1]), &a, mode);
    order(&a, &summary(&[-1000, 400, -30], &[1]), mode);
}
#[test]
fn resilient_compare_to() {
    for scores in [
        vec![simple(-20), simple(-1)],
        vec![hs(-20, -300), hs(-1, -4000)],
        vec![
            simple(-4000),
            hs(-300, -300),
            hs(-20, -4000),
            simple(-20),
            hs(-20, 4000),
            simple(-1),
        ],
    ] {
        for (i, a) in scores.iter().enumerate() {
            for (j, b) in scores.iter().enumerate() {
                assert_eq!(a.cmp(b), i.cmp(&j));
            }
        }
    }
}
fn percentage(a: (i64, i64), b: (i64, i64)) -> ScoreDifferencePercentage {
    ScoreDifferencePercentage::between(&HardSoftScore::new(a.0, a.1), &HardSoftScore::new(b.0, b.1))
        .unwrap()
}
fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() <= 0.00001, "{a} != {e}");
    }
}
#[test]
fn percentage_different_dimensions() {
    assert!(matches!(
        ScoreDifferencePercentage::between(
            &BendableScore::new([1, 2, 3], [4, 5]),
            &BendableScore::new([1, 2], [4, 5])
        ),
        Err(BenchmarkResultError::DifferentDimensions { left: 5, right: 4 })
    ));
}
#[test]
fn calculate_score_difference_percentage() {
    for (a, b, e) in [
        (-100, -100, 0.0),
        (100, 100, 0.0),
        (-100, -10, 0.9),
        (100, 10, -0.9),
        (-100, -1, 0.99),
        (100, 1, -0.99),
    ] {
        close(
            ScoreDifferencePercentage::between(&SimpleScore::new(a), &SimpleScore::new(b))
                .unwrap()
                .levels(),
            &[e],
        );
    }
    for (a, b, e) in [
        ((-100, -1), (-100, -1), [0.0, 0.0]),
        ((-100, -100), (-1, -10), [0.99, 0.9]),
        ((100, 100), (1, 10), [-0.99, -0.9]),
        ((100, -100), (-100, 200), [-2.0, 3.0]),
    ] {
        close(percentage(a, b).levels(), &e);
    }
}
#[test]
fn percentage_add() {
    let a = percentage((-100, -1), (-200, -10));
    close(
        a.add(&percentage((-100, -1), (-200, -10)))
            .unwrap()
            .levels(),
        &[-2.0, -18.0],
    );
}
#[test]
fn percentage_subtract() {
    let a = percentage((-100, -1), (-200, -10));
    close(
        a.subtract(&percentage((-100, -1), (-200, -10)))
            .unwrap()
            .levels(),
        &[0.0, 0.0],
    );
}
#[test]
#[allow(clippy::approx_constant)] // Exact factor in the upstream specification.
fn percentage_multiply() {
    let a = percentage((-100, -1), (-200, -10));
    close(a.multiply(3.14).levels(), &[-3.14, -28.26]);
    close(a.multiply(-1.0).levels(), &[1.0, 9.0]);
}
#[test]
fn percentage_divide_source() {
    // Upstream method named divide actually invokes multiply in both assertions.
    let a = percentage((-100, -1), (-200, -10));
    close(a.multiply(0.5).levels(), &[-0.5, -4.5]);
    close(a.multiply(-1.0).levels(), &[1.0, 9.0]);
}
#[test]
fn percentage_add_wrong_dimension() {
    let a = percentage((-100, -1), (-200, -10));
    let b = ScoreDifferencePercentage::between(&SimpleScore::new(-100), &SimpleScore::new(-200))
        .unwrap();
    assert!(matches!(
        a.add(&b),
        Err(BenchmarkResultError::DifferentDimensions { left: 2, right: 1 })
    ));
}
#[test]
fn percentage_subtract_wrong_dimension() {
    let a = percentage((-100, -1), (-200, -10));
    let b = ScoreDifferencePercentage::between(&SimpleScore::new(-100), &SimpleScore::new(-200))
        .unwrap();
    assert!(matches!(
        a.subtract(&b),
        Err(BenchmarkResultError::DifferentDimensions { left: 2, right: 1 })
    ));
}
