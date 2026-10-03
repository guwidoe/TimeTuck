//! Original numeric/validation regressions; no upstream method credit.
use timetuck::*;

#[test]
fn integer_deltas_are_widened_before_float_conversion() {
    let p = ScoreTargetProgress::new(
        SimpleScore::new(i64::MAX - 10),
        SimpleScore::new(i64::MAX),
        vec![],
    )
    .unwrap();
    assert_eq!(p.progress(SimpleScore::new(i64::MAX - 4)), 0.6);
    let p = ScoreTargetProgress::new(
        SimpleScore::new(i64::MIN),
        SimpleScore::new(i64::MAX),
        vec![],
    )
    .unwrap();
    assert_eq!(p.progress(SimpleScore::ZERO), 0.5);
    assert_eq!(p.progress(SimpleScore::new(i64::MIN)), 0.0);
    assert_eq!(p.progress(SimpleScore::new(i64::MAX)), 1.0);
    let p = FeasibilityProgress::new(HardSoftScore::new(i64::MIN, 0), vec![]).unwrap();
    assert_eq!(p.progress(HardSoftScore::new(i64::MIN / 2, 0)), 0.5);
}
#[test]
fn validation_rejects_bad_weights_and_flawed_reference_scores() {
    let zero = HardSoftScore::ZERO;
    assert!(matches!(
        ScoreTargetProgress::new(zero, zero, vec![]),
        Err(ScoreProgressError::WeightCount {
            expected: 1,
            actual: 0
        })
    ));
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.1] {
        assert!(matches!(
            ScoreTargetProgress::new(zero, zero, vec![invalid]),
            Err(ScoreProgressError::InvalidWeight { index: 0 })
        ));
        assert!(matches!(
            FeasibilityProgress::new(BendableScore::new([-1, -1], [0]), vec![invalid]),
            Err(ScoreProgressError::InvalidWeight { index: 0 })
        ));
    }
    assert!(matches!(
        ScoreTargetProgress::new(zero.with_structural(-1), zero, vec![0.75]),
        Err(ScoreProgressError::FlawedStart)
    ));
    assert!(matches!(
        ScoreTargetProgress::new(zero, zero.with_structural(-1), vec![0.75]),
        Err(ScoreProgressError::FlawedTarget)
    ));
    assert!(matches!(
        FeasibilityProgress::new(zero.with_structural(-1), vec![]),
        Err(ScoreProgressError::FlawedStart)
    ));
    assert!(matches!(
        FeasibilityProgress::new(zero, vec![0.75]),
        Err(ScoreProgressError::WeightCount {
            expected: 0,
            actual: 1
        })
    ));
}
#[test]
fn structural_flaws_cannot_be_hidden_by_large_ordinary_scores() {
    let p = ScoreTargetProgress::new(SimpleScore::new(-10), SimpleScore::ZERO, vec![]).unwrap();
    let flawed = SimpleScore::new(i64::MAX).with_structural(-1);
    assert!(!p.is_reached(flawed));
    assert_eq!(p.progress(flawed), 0.0);
    let f = FeasibilityProgress::new(HardSoftScore::new(-10, -10), vec![]).unwrap();
    let flawed = HardSoftScore::new(i64::MAX, i64::MAX).with_structural(-1);
    assert!(!f.is_reached(flawed));
    assert_eq!(f.progress(flawed), 0.0);
}
#[test]
fn feasibility_ignores_soft_scores_and_positive_hard_levels_do_not_compensate() {
    let p = FeasibilityProgress::new(BendableScore::new([-10, -20], [-100]), vec![0.75]).unwrap();
    for soft in [i64::MIN, 0, i64::MAX] {
        let best = BendableScore::new([10, -10], [soft]);
        assert!(!p.is_reached(best));
        assert_eq!(p.progress(best), 0.875);
        assert_eq!(p.progress(BendableScore::new([0, 0], [soft])), 1.0);
    }
    let p = FeasibilityProgress::new(SimpleScore::new(-100), vec![]).unwrap();
    assert!(p.is_reached(SimpleScore::new(i64::MIN)));
    assert_eq!(p.progress(SimpleScore::new(i64::MIN)), 1.0);
    let p = FeasibilityProgress::new(BendableScore::<0, 0>::ZERO, vec![]).unwrap();
    assert_eq!(p.progress(BendableScore::ZERO), 1.0);
}
#[test]
fn saturated_estimates_are_not_exact_termination_decisions() {
    let p = ScoreTargetProgress::new(HardSoftScore::new(-10, -10), HardSoftScore::ZERO, vec![0.0])
        .unwrap();
    assert_eq!(p.progress(HardSoftScore::new(-1, 0)), 1.0);
    assert!(!p.is_reached(HardSoftScore::new(-1, 0)));
    let p = FeasibilityProgress::new(BendableScore::new([-10, -10], [0]), vec![1.0]).unwrap();
    assert_eq!(p.progress(BendableScore::new([0, -1], [0])), 1.0);
    assert!(!p.is_reached(BendableScore::new([0, -1], [0])));
}
#[test]
fn reversed_and_flat_levels_are_bounded_and_regression_is_explicit() {
    let p = ScoreTargetProgress::new(
        HardSoftScore::new(-10, 10),
        HardSoftScore::new(0, -10),
        vec![0.75],
    )
    .unwrap();
    assert_eq!(p.progress(HardSoftScore::new(-5, 0)), 0.625);
    assert_eq!(p.progress(HardSoftScore::new(-5, -20)), 0.375);
    assert_eq!(p.progress(HardSoftScore::new(-11, 100)), 0.0);
    assert_eq!(p.progress(HardSoftScore::new(1, i64::MIN)), 1.0);
    let p = ScoreTargetProgress::new(SimpleScore::new(10), SimpleScore::ZERO, vec![]).unwrap();
    assert_eq!(p.progress(SimpleScore::new(5)), 1.0);
    assert_eq!(p.progress(SimpleScore::new(-1)), 0.0);
}

#[test]
fn score_progress_can_drive_explicit_annealing_without_changing_its_step_clock() {
    let p = ScoreTargetProgress::new(SimpleScore::new(-100), SimpleScore::ZERO, vec![]).unwrap();
    let a = SimulatedAnnealing::new(SimpleScore::new(20), 10, 1).unwrap();
    let current = SimpleScore::new(-40);
    let candidate = SimpleScore::new(-44);
    assert_eq!(
        a.probability_at(current, candidate, p.progress(current))
            .unwrap(),
        (-0.5_f64).exp()
    );
    assert_eq!(
        a.probability_at(current, candidate, p.progress(SimpleScore::ZERO))
            .unwrap(),
        0.0
    );
    assert_eq!(a.gradient(), 0.0);
    let p = FeasibilityProgress::new(HardMediumSoftScore::new(-10, -100, -100), vec![]).unwrap();
    assert_eq!(
        p.progress(HardMediumSoftScore::new(-5, i64::MIN, i64::MIN)),
        0.5
    );
    assert!(p.is_reached(HardMediumSoftScore::new(0, i64::MIN, i64::MIN)));
}
