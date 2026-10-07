use std::cell::Cell;
use timetuck::*;

fn d(value: &str) -> Decimal {
    value.parse().unwrap()
}

#[test]
fn snapshots_retain_raw_impacts_without_recomputing_and_survive_rollback() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let c = track_constraint(&mut b, "mixed", s.port(), SimpleScore::new(3), |v| {
        calls.set(calls.get() + 1);
        *v
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, -4)).unwrap();
    tx.apply(&s, TupleChange::put(1, 4)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 2);
    let before = constraint_assertion(&g, &c, |v, score| (*v, *score)).unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(before.score(), &SimpleScore::ZERO);
    assert_eq!(before.penalty_impact(), &d("4"));
    assert_eq!(before.reward_impact(), &d("4"));
    assert_eq!(before.impact(), d("8"));
    assert!(before.check(ConstraintExpectation::NoImpact, None).is_err());
    assert_eq!(
        before.justifications(),
        &[(-4, SimpleScore::new(-12)), (4, SimpleScore::new(12))]
    );
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(0, -9)).unwrap();
        // Dropping an uncommitted transaction restores retained factors and scores.
    }
    let after = constraint_assertion(&g, &c, |v, score| (*v, *score)).unwrap();
    assert_eq!(after.justifications(), before.justifications());
    assert_eq!(after.penalty_impact(), before.penalty_impact());
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, -2)).unwrap();
    tx.commit().unwrap();
    let count = calls.get();
    let changed = constraint_assertion(&g, &c, |v, _| *v).unwrap();
    assert_eq!(calls.get(), count);
    assert_eq!(changed.score(), &SimpleScore::new(6));
    assert_eq!(before.score(), &SimpleScore::ZERO);
    assert_eq!(before.penalty_impact(), &d("4"));
}

#[test]
fn fractional_impacts_ignore_scale_and_preserve_zero_and_duplicate_matches() {
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let s = b.source::<usize, Decimal>();
    let c = track_decimal_constraint(
        &mut b,
        "fraction",
        s.port(),
        "2.5".parse().unwrap(),
        Clone::clone,
    )
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (id, value) in ["-0.1", "-0.1", "0.00"].into_iter().enumerate() {
        tx.apply(&s, TupleChange::put(id, d(value))).unwrap();
    }
    tx.commit().unwrap();
    let a = constraint_assertion(&g, &c, |v, _| v.clone()).unwrap();
    assert_eq!(a.penalty_count(), 2);
    assert_eq!(a.reward_count(), 0);
    assert_eq!(a.score(), &"-0.50".parse::<SimpleDecimalScore>().unwrap());
    a.check(
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly(d("0.2000"))),
        None,
    )
    .unwrap();
    a.check_justifications(&[d("-0.1")], JustificationMatch::Contains, None)
        .unwrap();
    assert!(
        a.check_justifications(
            &[d("-0.1"), d("-0.1"), d("-0.1")],
            JustificationMatch::Contains,
            None
        )
        .is_err()
    );
    assert!(
        a.check_justifications(&[d("-0.1"), d("0.00")], JustificationMatch::Exact, None)
            .is_err()
    );
    a.check_justifications(
        &[d("0.00"), d("-0.1"), d("-0.1")],
        JustificationMatch::Exact,
        None,
    )
    .unwrap();
}

#[test]
fn minimum_integer_impact_has_an_exact_positive_magnitude() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let c = track_constraint(&mut b, "minimum", s.port(), SimpleScore::new(1), |v| *v).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, i64::MIN)).unwrap();
    tx.commit().unwrap();
    let a = constraint_assertion(&g, &c, |_, _| ()).unwrap();
    assert_eq!(a.penalty_impact(), &d("9223372036854775808"));
    assert_eq!(a.score(), &SimpleScore::new(i64::MIN));
}

#[test]
fn disabled_constraints_prune_callbacks_and_foreign_handles_fail_before_justifying() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let c = track_constraint(&mut b, "off", s.port(), SimpleScore::new(1), |_| {
        panic!("disabled factor")
    })
    .unwrap();
    let weights = [("off".into(), SimpleScore::ZERO)].into_iter().collect();
    let mut g = b.build_with_weights(&weights).unwrap();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    let a = constraint_assertion(&g, &c, |_, _| panic!("disabled justification")).unwrap();
    a.check(ConstraintExpectation::NoImpact, None).unwrap();
    assert_eq!(a.score(), &SimpleScore::ZERO);
    let foreign = StreamGraphBuilder::<SimpleScore>::new().build();
    assert!(constraint_assertion(&foreign, &c, |_, _| panic!("foreign justification")).is_err());
}
