//! Original Bendable decimal regressions; no upstream method credit.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use timetuck::*;
fn d(v: &str) -> Decimal {
    v.parse().unwrap()
}
fn hash(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
type B = BendableDecimalScore<2, 2>;
fn b(text: &str) -> B {
    text.parse().unwrap()
}
#[test]
fn scales_survive_zero_results_negation_and_group_formatting() {
    let a = b("-2structural/[1E+2/0.00]hard/[-1.20/0E+2]soft");
    assert_eq!((-a.clone()).to_string(), "[-1E+2/0.00]hard/[1.20/0E+2]soft");
    assert_eq!(a.abs().to_string(), "[1E+2/0.00]hard/[1.20/0E+2]soft");
    let zero = a.clone() - a.clone();
    assert_eq!(zero.to_string(), "[0E+2/0.00]hard/[0.00/0E+2]soft");
    assert_eq!(zero.to_short_string(), "0");
    assert_eq!((-zero.clone()).to_string(), zero.to_string());
    assert_eq!(
        zero.multiply(&d("0")).unwrap().to_string(),
        "[0/0.00]hard/[0.00/0]soft"
    );
    assert_eq!(
        a.to_short_string(),
        "-2structural/[1E+2/0.00]hard/[-1.20/0E+2]soft"
    );
    assert_eq!(
        b("[0.00/0E+2]hard/[0.0/1.20]soft").to_short_string(),
        "[0.0/1.20]soft"
    );
}
#[test]
fn empty_dimensions_clear_structural_state_without_dividing() {
    let empty = BendableDecimalScore::<0, 0>::ZERO.with_structural(-2);
    assert_eq!(empty.divide(&Decimal::ZERO).unwrap().structural(), 0);
    assert_eq!((-empty.clone()).to_string(), "[]hard/[]soft");
    assert!(!empty.is_feasible());
    assert!(empty.abs().is_feasible());
    assert_eq!(
        B::ZERO.divide(&Decimal::ZERO),
        Err(DecimalArithmeticError::DivisionByZero)
    );
    assert!(B::of_hard(2, d("1")).is_none());
    assert!(B::of_soft(usize::MAX, d("1")).is_none());
    assert!(empty.level(0).is_none());
    assert!(B::ZERO.level(usize::MAX).is_none());
    assert!(
        "[0]hard/[]soft"
            .parse::<BendableDecimalScore<0, 0>>()
            .is_err()
    );
    assert!(
        "[]hard/[0]soft"
            .parse::<BendableDecimalScore<0, 0>>()
            .is_err()
    );
    assert!("[0/0/0]hard/[0]soft".parse::<B>().is_err());
}
#[test]
fn numeric_equality_and_hash_ignore_scales_at_extreme_boundaries() {
    let a = b("[10E+2147483648/0E-2147483647]hard/[1.20/0E+2147483648]soft");
    let other = b("[100E+2147483647/0]hard/[1.200/0.00]soft");
    assert_eq!(a, other);
    assert_eq!(hash(&a), hash(&other));
    assert_ne!(a.hard_levels(), other.hard_levels());
    assert_ne!(a.clone().with_structural(-1), other);
    assert_eq!(a.level(2), Some(&d("1.20")));
}
#[test]
fn every_hard_level_controls_feasibility_and_history_reset() {
    let start = b("[0/-0.02]hard/[0/0]soft");
    let improved = b("[0/-0.01]hard/[-100/0]soft");
    let mut late = LateAcceptance::new(3, false, start.clone()).unwrap();
    late.step_ended(improved.clone());
    assert!(late.history().iter().all(|s| s == &improved));
    assert!(!improved.is_feasible());
    assert!(!b("[1/-0.01]hard/[100/0]soft").is_feasible());
    let progress = FeasibilityProgress::new(start, vec![0.5]).unwrap();
    assert!(!progress.is_reached(improved));
    assert!(progress.is_reached(b("[0/0]hard/[-100/-100]soft")));
    let temperature = b("[1/0.02]hard/[1/1]soft");
    let annealing = SimulatedAnnealing::new(temperature, 10, 77).unwrap();
    let probability = annealing
        .probability_at(B::ZERO, b("[0/-0.01]hard/[0/0]soft"), 0.0)
        .unwrap();
    assert!((probability - (-0.5_f64).exp()).abs() < 1e-14);
}
#[test]
fn exact_search_prioritizes_fractional_hard_levels_over_soft_rewards() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2])]).unwrap();
    let scores = [
        b("[0/0.0001]hard/[99999999999999999999/0]soft"),
        b("[0/0.0003]hard/[-99999999999999999999/0]soft"),
        b("[0/0.0002]hard/[0/0]soft"),
    ];
    let result = exhaustive_search(&problem, |a| scores[a.values()[0].unwrap()].clone(), 3);
    assert!(result.is_optimal());
    let best = result.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(1)]);
    assert_eq!(best.score, scores[1]);
}
