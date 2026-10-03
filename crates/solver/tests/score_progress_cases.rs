// Adapted from pinned Timefold Community BestScore[Feasible]TerminationTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

fn target_lifecycle() {
    let p =
        ScoreTargetProgress::new(SimpleScore::new(-1100), SimpleScore::new(-1000), vec![]).unwrap();
    for (best, reached, gradient) in [
        (-1100, false, 0.0),
        (-1100, false, 0.0),
        (-1040, false, 0.6),
        (-1040, false, 0.6),
        (-1000, true, 1.0),
        (-900, true, 1.0),
    ] {
        assert_eq!(p.is_reached(SimpleScore::new(best)), reached);
        assert_eq!(p.progress(SimpleScore::new(best)), gradient);
    }
}
#[test]
fn best_score_solve_termination() {
    target_lifecycle()
}
#[test]
fn best_score_phase_termination() {
    target_lifecycle()
}
#[test]
fn calculate_time_gradient_simple_score() {
    for (start, target, current, expected, tolerance) in [
        (0, 10, 0, 0.0, 0.0),
        (0, 10, 6, 0.6, 0.0),
        (0, 10, 10, 1.0, 0.0),
        (0, 10, 11, 1.0, 0.0),
        (-10, 30, 0, 0.25, 0.0),
        (10, 40, 20, 0.33333, 0.00001),
    ] {
        let p = ScoreTargetProgress::new(SimpleScore::new(start), SimpleScore::new(target), vec![])
            .unwrap();
        assert!((p.progress(SimpleScore::new(current)) - expected).abs() <= tolerance);
    }
}
// These same literal source assertions occur in the HardSoft and Bendable HS methods.
fn two_levels<S: Score>(score: impl Fn(i64, i64) -> S) {
    for (start, current, expected) in [
        ((-20, -400), (-14, -340), 0.6),
        ((-20, -400), (-10, -400), 0.75),
        ((-20, -400), (-20, -300), 0.25),
        ((-20, -400), (-10, -900), 0.75),
        ((-20, -400), (-90, -300), 0.0),
        ((-10, -300), (-10, -300), 1.0),
        ((-20, -400), (-20, -400), 0.0),
        ((-20, -400), (-10, -300), 1.0),
        ((-10, -400), (-10, -340), 0.75 + (0.6 * 0.25)),
        ((-10, -400), (-20, -340), 0.0),
        ((-10, -400), (0, -340), 1.0),
        ((-20, -300), (-14, -300), (0.6 * 0.75) + 0.25),
        ((-20, -300), (-14, -400), 0.6 * 0.75),
        ((-20, -300), (-14, 0), (0.6 * 0.75) + 0.25),
    ] {
        let p = ScoreTargetProgress::new(score(start.0, start.1), score(-10, -300), vec![0.75])
            .unwrap();
        assert_eq!(p.progress(score(current.0, current.1)), expected);
    }
}
#[test]
fn calculate_time_gradient_hard_soft_score() {
    two_levels(HardSoftScore::new)
}
#[test]
fn calculate_time_gradient_bendable_score_hs() {
    two_levels(|h, s| BendableScore::new([h], [s]))
}
#[test]
fn calculate_time_gradient_bendable_score_hhsss() {
    let p = ScoreTargetProgress::new(
        BendableScore::new([-10, -100], [-50, -60, -70]),
        BendableScore::new([0, 0], [0, 0, -10]),
        vec![0.75; 4],
    )
    .unwrap();
    assert_eq!(
        p.progress(BendableScore::new([-4, -40], [-50, -60, -70])),
        0.6 * 0.75 + 0.6 * 0.25 * 0.75
    );
}
fn feasible_lifecycle() {
    let p = FeasibilityProgress::new(HardSoftScore::new(-100, -100), vec![]).unwrap();
    for (hard, reached, gradient) in [
        (-100, false, 0.0),
        (-80, false, 0.2),
        (-60, false, 0.4),
        (-40, false, 0.6),
        (-20, false, 0.8),
        (0, true, 1.0),
    ] {
        let best = HardSoftScore::new(hard, -100);
        assert_eq!(p.is_reached(best), reached);
        assert_eq!(p.progress(best), gradient);
    }
}
#[test]
fn best_score_feasible_solve_termination() {
    feasible_lifecycle()
}
#[test]
fn best_score_feasible_phase_termination() {
    feasible_lifecycle()
}
#[test]
fn calculate_feasibility_time_gradient_bendable_score_hhsss() {
    let p = FeasibilityProgress::new(BendableScore::new([-10, -100], [-50, -60, -70]), vec![0.75])
        .unwrap();
    assert_eq!(
        p.progress(BendableScore::new([-4, -40], [-50, -60, -70])),
        0.6
    );
}

// These six decimal observations occur in both upstream BigDecimal methods.
// Literal scales, weights and zero/tolerant offsets are preserved.
fn decimal_target_cases<S: Score>(make: impl Fn(&str) -> S, splits: Vec<f64>) {
    for (start, target, current, expected, tolerance) in [
        ("0.00", "10.00", "0.00", 0.0, 0.0),
        ("0.00", "10.00", "6.00", 0.6, 0.0),
        ("0.00", "10.00", "10.00", 1.0, 0.0),
        ("0.00", "10.00", "11.00", 1.0, 0.0),
        ("-10.00", "30.00", "0.00", 0.25, 0.0),
        ("10.00", "40.00", "20.00", 0.33333, 0.00001),
    ] {
        let p = ScoreTargetProgress::new(make(start), make(target), splits.clone()).unwrap();
        assert!((p.progress(make(current)) - expected).abs() <= tolerance);
    }
}
#[test]
fn calculate_time_gradient_simple_big_decimal_score() {
    decimal_target_cases(|s| SimpleDecimalScore::new(s.parse().unwrap()), vec![]);
}
#[test]
fn calculate_time_gradient_hard_soft_big_decimal_score() {
    decimal_target_cases(
        |s| HardSoftDecimalScore::new(s.parse().unwrap(), s.parse().unwrap()),
        vec![0.75],
    );
}
