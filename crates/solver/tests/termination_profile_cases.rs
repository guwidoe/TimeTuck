// Adapted from pinned Timefold Community service SolverTerminationConfigTest.
// SPDX-License-Identifier: Apache-2.0
use std::{borrow::Cow, time::Duration};
use timetuck::{TerminationProfile as P, TerminationProfileError};
fn sec(n: u64) -> Option<Duration> {
    Some(Duration::from_secs(n))
}
fn error(p: Result<P, TerminationProfileError>, parts: &[&str]) {
    let message = p.unwrap_err().to_string();
    for part in parts {
        assert!(message.contains(part), "{message}: {part}");
    }
}
#[test]
fn rejects_zero_ratio() {
    error(
        P::new(sec(60), None, None, None, None, Some((0, 1))),
        &["minimumImprovementRatio", "must be strictly positive"],
    );
}
#[test]
fn rejects_negative_steps() {
    error(
        P::new(sec(60), None, Some(-1), None, None, None),
        &["stepCountLimit", "cannot be negative"],
    );
}
#[test]
fn rejects_negative_moves() {
    error(
        P::new(sec(60), None, None, Some(-1), None, None),
        &["moveCountLimit", "cannot be negative"],
    );
}
#[test]
fn accepts_zero_counts() {
    let p = P::new(sec(60), None, Some(0), Some(0), None, None).unwrap();
    assert_eq!(p.step_count_limit(), Some(0));
    assert_eq!(p.move_count_limit(), Some(0));
}
#[test]
fn rejects_stagnation_with_steps() {
    error(
        P::new(sec(60), sec(30), Some(1000), None, None, None),
        &["unimprovedSpentLimit", "stepCountLimit"],
    );
}
#[test]
fn rejects_stagnation_with_moves() {
    error(
        P::new(sec(60), sec(30), None, Some(100000), None, None),
        &["unimprovedSpentLimit", "moveCountLimit"],
    );
}
#[test]
fn rejects_negative_ratio() {
    error(
        P::new(sec(60), None, None, None, None, Some((-1, 100))),
        &["must be strictly positive"],
    );
}
#[test]
fn accepts_missing_ratio() {
    let p = P::new(sec(60), None, None, None, None, None).unwrap();
    assert_eq!(p.minimum_improvement_ratio(), None);
    assert_eq!(p.sliding_window(), None);
}
#[test]
fn accepts_positive_ratio() {
    let p = P::new(sec(60), None, None, None, sec(300), Some((1, 100))).unwrap();
    assert_eq!(p.sliding_window(), sec(300));
    assert_eq!(p.minimum_improvement_ratio(), Some((1, 100)));
}
#[test]
fn legacy_five_fields_default_moves() {
    let p = P::new(sec(60), None, Some(1000), None, sec(300), Some((1, 100))).unwrap();
    assert_eq!(p.spent_limit(), sec(60));
    assert_eq!(p.step_count_limit(), Some(1000));
    assert_eq!(p.move_count_limit(), None);
    assert_eq!(p.sliding_window(), sec(300));
    assert_eq!(p.minimum_improvement_ratio(), Some((1, 100)));
}
#[test]
fn three_fields_default_tuning() {
    let p = P::new(sec(60), None, Some(100), None, None, None).unwrap();
    assert_eq!(p.move_count_limit(), None);
    assert_eq!(p.sliding_window(), None);
    assert_eq!(p.minimum_improvement_ratio(), None);
}
#[test]
fn four_fields_default_tuning() {
    let p = P::new(sec(60), None, None, Some(100000), None, None).unwrap();
    assert_eq!(p.move_count_limit(), Some(100000));
    assert_eq!(p.sliding_window(), None);
    assert_eq!(p.minimum_improvement_ratio(), None);
}
#[test]
fn accepts_move_limit() {
    let p = P::new(sec(60), None, None, Some(100000), None, None).unwrap();
    assert_eq!(p.move_count_limit(), Some(100000));
}
#[test]
fn counts_combine() {
    let p = P::new(sec(60), None, Some(1000), Some(100000), None, None).unwrap();
    assert_eq!(p.step_count_limit(), Some(1000));
    assert_eq!(p.move_count_limit(), Some(100000));
}
#[test]
fn fallback_fills_tuning() {
    let p = P::new(sec(60), None, None, None, None, None).unwrap();
    let b = P::new(None, None, None, None, sec(120), Some((1, 1000))).unwrap();
    let m = p.with_fallback(Some(&b)).unwrap();
    assert_eq!(m.sliding_window(), sec(120));
    assert_eq!(m.minimum_improvement_ratio(), Some((1, 1000)));
}
#[test]
fn primary_keeps_tuning() {
    let p = P::new(sec(60), None, None, None, sec(600), Some((1, 2))).unwrap();
    let b = P::new(None, None, None, None, sec(120), Some((1, 1000))).unwrap();
    let m = p.with_fallback(Some(&b)).unwrap();
    assert_eq!(m.sliding_window(), sec(600));
    assert_eq!(m.minimum_improvement_ratio(), Some((1, 2)));
}
#[test]
fn fallback_fills_moves() {
    let p = P::new(sec(60), None, None, None, None, None).unwrap();
    let b = P::new(None, None, None, Some(100000), None, None).unwrap();
    assert_eq!(
        p.with_fallback(Some(&b)).unwrap().move_count_limit(),
        Some(100000)
    );
}
#[test]
fn primary_keeps_moves() {
    let p = P::new(sec(60), None, None, Some(500000), None, None).unwrap();
    let b = P::new(None, None, None, Some(100000), None, None).unwrap();
    assert_eq!(
        p.with_fallback(Some(&b)).unwrap().move_count_limit(),
        Some(500000)
    );
}
#[test]
fn merge_rejects_moves_with_stagnation() {
    let p = P::new(sec(60), None, None, Some(100000), None, None).unwrap();
    let b = P::new(None, sec(30), None, None, None, None).unwrap();
    error(
        p.with_fallback(Some(&b)).map(Cow::into_owned),
        &["unimprovedSpentLimit", "moveCountLimit"],
    );
}
#[test]
fn merge_rejects_steps_with_stagnation() {
    let p = P::new(sec(60), None, Some(1000), None, None, None).unwrap();
    let b = P::new(None, sec(30), None, None, None, None).unwrap();
    error(
        p.with_fallback(Some(&b)).map(Cow::into_owned),
        &["unimprovedSpentLimit", "stepCountLimit"],
    );
}
#[test]
fn absent_fallback_keeps_identity() {
    let p = P::new(sec(60), None, None, Some(100000), None, None).unwrap();
    let m = p.with_fallback(None).unwrap();
    assert!(matches!(m, Cow::Borrowed(_)));
    assert!(std::ptr::eq(m.as_ref(), &p));
}
